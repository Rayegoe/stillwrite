//! Persistent Pi RPC host.
//!
//! StillWrite owns the child-process boundary and the small amount of state
//! needed to route one active Agent Work. Pi owns the model loop and its
//! session log; this module deliberately does not copy Pi's full protocol.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{agent_work, atomic_write, workspace_root, AppState};

const MAX_RPC_RECORD_SIZE: usize = 2 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 64 * 1024;
const MAX_VERSION_OUTPUT: usize = 32 * 1024;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const ABORT_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(3);

const SYSTEM_PROMPT: &str = include_str!("../resources/pi/SYSTEM.md");
const STILLWRITE_TOOLS: &str = include_str!("../resources/pi/stillwrite-tools.ts");

type EventSink = Arc<dyn Fn(Value) + Send + Sync + 'static>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Launcher {
    executable: PathBuf,
    label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LaunchConfig {
    launcher: Launcher,
    provider: Option<OsString>,
    model: Option<OsString>,
    thinking: Option<OsString>,
    agent_dir: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct RuntimeResources {
    system_prompt: PathBuf,
    extension: PathBuf,
}

#[derive(Clone)]
struct Runtime {
    workspace_root: PathBuf,
    config: LaunchConfig,
    process: ChildProcess,
}

#[derive(Default)]
struct ProcessStore {
    runtime: Mutex<Option<Runtime>>,
}

impl Drop for ProcessStore {
    fn drop(&mut self) {
        if let Ok(runtime) = self.runtime.get_mut() {
            if let Some(runtime) = runtime.take() {
                runtime.process.shutdown();
            }
        }
    }
}

/// Tauri-managed persistent process state. Clones share the same store so
/// blocking commands can safely move a handle into Tauri's worker pool.
#[derive(Clone, Default)]
pub struct PiProcessState {
    store: Arc<ProcessStore>,
}

#[derive(Clone, Debug)]
struct ActiveRun {
    run_id: String,
    session_ref: Option<String>,
    settling: bool,
}

#[derive(Clone)]
struct ChildProcess {
    inner: Arc<ChildProcessInner>,
}

struct ChildProcessInner {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<String, mpsc::Sender<Result<RpcResponse, String>>>>,
    next_request_id: AtomicU64,
    dead: AtomicBool,
    stderr: Mutex<String>,
    session_root: PathBuf,
    active_run: Mutex<Option<ActiveRun>>,
    sink: EventSink,
}

#[derive(Clone, Debug)]
struct RpcResponse {
    id: Option<String>,
    command: String,
    success: bool,
    data: Option<Value>,
    error: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartInput {
    pub run_id: String,
    pub prompt: String,
    pub title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartResponse {
    pub accepted: bool,
    pub run_id: String,
    pub request_id: String,
    pub pi_session_ref: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAbortResponse {
    pub accepted: bool,
    pub run_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProbeResponse {
    pub available: bool,
    pub launcher: String,
    pub version: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

impl ChildProcess {
    fn is_dead(&self) -> bool {
        self.inner.dead.load(Ordering::Acquire)
    }

    fn request(&self, mut command: Value, timeout: Duration) -> Result<RpcResponse, String> {
        if self.is_dead() {
            return Err("Pi 进程不可用".into());
        }
        let object = command
            .as_object_mut()
            .ok_or_else(|| "Pi RPC 命令必须是 JSON 对象".to_string())?;
        let id = format!(
            "sw-{}",
            self.inner.next_request_id.fetch_add(1, Ordering::Relaxed) + 1
        );
        object.insert("id".into(), Value::String(id.clone()));
        let mut bytes = serde_json::to_vec(&command)
            .map_err(|error| format!("编码 Pi RPC 命令失败: {error}"))?;
        bytes.push(b'\n');

        let (sender, receiver) = mpsc::channel();
        self.inner
            .pending
            .lock()
            .map_err(|_| "Pi RPC 请求状态锁定失败".to_string())?
            .insert(id.clone(), sender);

        let write_result = (|| -> Result<(), String> {
            let mut stdin = self
                .inner
                .stdin
                .lock()
                .map_err(|_| "Pi RPC stdin 状态锁定失败".to_string())?;
            let stdin = stdin
                .as_mut()
                .ok_or_else(|| "Pi RPC stdin 已关闭".to_string())?;
            stdin
                .write_all(&bytes)
                .map_err(|error| format!("写入 Pi RPC 命令失败: {error}"))?;
            stdin
                .flush()
                .map_err(|error| format!("刷新 Pi RPC 命令失败: {error}"))
        })();
        if let Err(error) = write_result {
            self.remove_pending(&id);
            self.mark_dead(error.clone());
            return Err(error);
        }

        match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.remove_pending(&id);
                let error = format!("等待 Pi RPC 响应超时: {id}");
                self.mark_dead(error.clone());
                Err(error)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.remove_pending(&id);
                let error = "Pi RPC 响应通道已断开".to_string();
                self.mark_dead(error.clone());
                Err(error)
            }
        }
    }

    fn remove_pending(&self, id: &str) {
        if let Ok(mut pending) = self.inner.pending.lock() {
            pending.remove(id);
        }
    }

    fn begin_run(&self, run: ActiveRun) -> Result<(), String> {
        let mut active = self
            .inner
            .active_run
            .lock()
            .map_err(|_| "Agent 运行状态锁定失败".to_string())?;
        if active.is_some() {
            return Err("已有 Agent 工作正在运行".into());
        }
        *active = Some(run);
        Ok(())
    }

    fn active_run(&self) -> Option<ActiveRun> {
        self.inner
            .active_run
            .lock()
            .ok()
            .and_then(|run| run.clone())
    }

    fn begin_settling(&self, run_id: &str) -> Option<ActiveRun> {
        let mut active = self.inner.active_run.lock().ok()?;
        let run = active.as_mut()?;
        if run.run_id != run_id || run.settling {
            return None;
        }
        run.settling = true;
        Some(run.clone())
    }

    fn take_run(&self, run_id: &str) -> Option<ActiveRun> {
        let mut active = self.inner.active_run.lock().ok()?;
        if active.as_ref().map(|run| run.run_id.as_str()) != Some(run_id) {
            return None;
        }
        active.take()
    }

    fn clear_run(&self) {
        if let Ok(mut active) = self.inner.active_run.lock() {
            active.take();
        }
    }

    fn emit_run_event(
        &self,
        run_id: &str,
        event_type: &str,
        fields: impl IntoIterator<Item = (String, Value)>,
    ) {
        let mut event = Map::new();
        event.insert("type".into(), Value::String(event_type.into()));
        event.insert("runId".into(), Value::String(run_id.into()));
        for (key, value) in fields {
            event.insert(key, value);
        }
        (self.inner.sink)(Value::Object(event));
    }

    fn dispatch_line(&self, value: Value) -> Result<(), String> {
        let object = value
            .as_object()
            .ok_or_else(|| "Pi RPC 记录必须是 JSON 对象".to_string())?;
        if object.get("type").and_then(Value::as_str) == Some("response") {
            let response = parse_rpc_response(&value)?;
            let id = response
                .id
                .as_deref()
                .ok_or_else(|| "Pi RPC 响应缺少 request id".to_string())?;
            let sender = self
                .inner
                .pending
                .lock()
                .map_err(|_| "Pi RPC 请求状态锁定失败".to_string())?
                .remove(id)
                .ok_or_else(|| format!("Pi RPC 响应 id 无法关联: {id}"))?;
            let _ = sender.send(Ok(response));
            return Ok(());
        }

        let event_type = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| "Pi RPC 事件缺少 type".to_string())?;
        let Some(run) = self.active_run() else {
            return Ok(());
        };
        match event_type {
            "agent_start" => self.emit_run_event(&run.run_id, "agent_start", []),
            "message_update" => {
                let assistant_event = object.get("assistantMessageEvent");
                let delta = assistant_event
                    .and_then(|event| {
                        event
                            .get("type")
                            .and_then(Value::as_str)
                            .zip(event.get("delta"))
                    })
                    .and_then(|(kind, delta)| (kind == "text_delta").then(|| delta.as_str()))
                    .flatten()
                    .or_else(|| object.get("delta").and_then(Value::as_str));
                if let Some(delta) = delta {
                    self.emit_run_event(
                        &run.run_id,
                        "message_update",
                        [("delta".into(), Value::String(delta.into()))],
                    );
                }
            }
            "tool_execution_start" => {
                let tool_name = object
                    .get("toolName")
                    .and_then(Value::as_str)
                    .unwrap_or("workspace");
                self.emit_run_event(
                    &run.run_id,
                    "tool_execution_start",
                    [("toolName".into(), Value::String(tool_name.into()))],
                );
            }
            "tool_execution_end" => self.emit_run_event(&run.run_id, "tool_execution_end", []),
            "compaction_start" => self.emit_run_event(&run.run_id, "compaction_start", []),
            "compaction_end" => self.emit_run_event(&run.run_id, "compaction_end", []),
            "extension_error" | "error" => {
                let message = object
                    .get("error")
                    .map(short_value)
                    .or_else(|| object.get("message").map(short_value))
                    .unwrap_or_else(|| "StillWrite Pi extension failed".into());
                if self.take_run(&run.run_id).is_some() {
                    self.emit_run_event(
                        &run.run_id,
                        "error",
                        [("message".into(), Value::String(message))],
                    );
                }
            }
            "agent_settled" => {
                let Some(run) = self.begin_settling(&run.run_id) else {
                    return Ok(());
                };
                let process = self.clone();
                thread::spawn(move || process.resolve_settled_run(run));
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_settled_run(&self, run: ActiveRun) {
        let response = self.request(
            json!({ "type": "get_last_assistant_text" }),
            COMMAND_TIMEOUT,
        );
        let result = match response {
            Ok(response) if response.success => {
                let text = response
                    .data
                    .as_ref()
                    .and_then(|data| data.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                Ok(text.to_string())
            }
            Ok(response) => Err(rpc_failure("读取 Pi 最终文本失败", &response)),
            Err(error) => Err(error),
        };

        if self.take_run(&run.run_id).is_none() {
            // A user may have aborted while the final-text request was in flight.
            return;
        }
        match result {
            Ok(text) => self.emit_run_event(
                &run.run_id,
                "agent_settled",
                [
                    ("text".into(), Value::String(text)),
                    (
                        "piSessionRef".into(),
                        run.session_ref.map(Value::String).unwrap_or(Value::Null),
                    ),
                ],
            ),
            Err(error) => self.emit_run_event(
                &run.run_id,
                "error",
                [("message".into(), Value::String(short_error(&error)))],
            ),
        }
    }

    fn mark_dead(&self, reason: String) {
        if self.inner.dead.swap(true, Ordering::AcqRel) {
            return;
        }
        let error = short_error(&reason);
        if let Ok(mut pending) = self.inner.pending.lock() {
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(error.clone()));
            }
        }
        if let Some(run) = self.active_run().and_then(|run| self.take_run(&run.run_id)) {
            self.emit_run_event(
                &run.run_id,
                "error",
                [("message".into(), Value::String(error))],
            );
        }
        self.kill_child();
    }

    fn shutdown(&self) {
        self.inner.dead.store(true, Ordering::Release);
        if let Ok(mut pending) = self.inner.pending.lock() {
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err("Pi 进程已停止".into()));
            }
        }
        self.clear_run();
        if let Ok(mut stdin) = self.inner.stdin.lock() {
            stdin.take();
        }
        self.kill_child();
    }

    fn kill_child(&self) {
        if let Ok(mut child) = self.inner.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn stderr(&self) -> String {
        self.inner
            .stderr
            .lock()
            .map(|stderr| stderr.clone())
            .unwrap_or_default()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.process.shutdown();
    }
}

impl PiProcessState {
    fn from_store(store: Arc<ProcessStore>) -> Self {
        Self { store }
    }

    fn ensure_for_workspace(
        &self,
        app: &AppHandle,
        root: &Path,
        config: LaunchConfig,
    ) -> Result<ChildProcess, String> {
        let root = root.to_path_buf();
        let mut guard = self
            .store
            .runtime
            .lock()
            .map_err(|_| "Pi 进程状态锁定失败".to_string())?;
        if let Some(runtime) = guard.as_ref() {
            if runtime.workspace_root == root
                && runtime.config == config
                && !runtime.process.is_dead()
            {
                return Ok(runtime.process.clone());
            }
        }
        guard.take();

        let resources = materialize_runtime_resources(app)?;
        let session_root = session_root(app, &root)?;
        let process = spawn_process(
            &config,
            &root,
            &session_root,
            &resources,
            tauri_event_sink(app),
        )?;
        let handshake = process.request(json!({ "type": "get_state" }), STARTUP_TIMEOUT);
        match handshake {
            Ok(response) if response.success => {}
            Ok(response) => {
                let error = rpc_failure("Pi 启动握手失败", &response);
                process.shutdown();
                return Err(error);
            }
            Err(error) => {
                process.shutdown();
                return Err(format!("Pi 启动握手失败: {error}"));
            }
        }
        *guard = Some(Runtime {
            workspace_root: root,
            config,
            process: process.clone(),
        });
        Ok(process)
    }

    fn current_process(&self) -> Option<ChildProcess> {
        self.store
            .runtime
            .lock()
            .ok()
            .and_then(|runtime| runtime.as_ref().map(|runtime| runtime.process.clone()))
    }

    pub fn shutdown_for_workspace(&self, next_root: &Path) {
        let old = self.store.runtime.lock().ok().and_then(|mut runtime| {
            if runtime
                .as_ref()
                .map(|current| current.workspace_root != next_root)
                .unwrap_or(false)
            {
                runtime.take()
            } else {
                None
            }
        });
        if let Some(runtime) = old {
            let process = runtime.process.clone();
            if process.active_run().is_some() {
                let _ = process.request(json!({ "type": "clear_queue" }), ABORT_TIMEOUT);
                let _ = process.request(json!({ "type": "abort" }), ABORT_TIMEOUT);
            }
            process.shutdown();
        }
    }
}

fn tauri_event_sink(app: &AppHandle) -> EventSink {
    let app = app.clone();
    Arc::new(move |event| {
        if let Err(error) = app.emit("agent-event", event) {
            eprintln!("发送 Agent 事件失败: {error}");
        }
    })
}

fn emit_direct_event(
    process: &ChildProcess,
    run_id: &str,
    event_type: &str,
    message: Option<String>,
) {
    let fields = message
        .map(|message| vec![("message".into(), Value::String(message))])
        .unwrap_or_default();
    process.emit_run_event(run_id, event_type, fields);
}

fn wait_for_idle(process: &ChildProcess) -> Result<(), String> {
    let deadline = Instant::now() + IDLE_TIMEOUT;
    loop {
        let response = process.request(json!({ "type": "get_state" }), ABORT_TIMEOUT)?;
        if !response.success {
            return Err(rpc_failure("读取 Pi 停止状态失败", &response));
        }
        let busy = response
            .data
            .as_ref()
            .map(|data| {
                data.get("isStreaming").and_then(Value::as_bool) == Some(true)
                    || data.get("isCompacting").and_then(Value::as_bool) == Some(true)
            })
            .unwrap_or(false);
        if !busy {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("Pi 在停止后仍未进入 idle 状态".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn configured_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn configured_value(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn executable_on_path(name: &str, path_value: Option<&OsString>) -> Option<PathBuf> {
    path_value.and_then(|value| {
        env::split_paths(value)
            .map(|directory| directory.join(name))
            .find(|candidate| is_executable(candidate))
    })
}

fn canonical_executable(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    if !is_executable(&path) {
        return Err(format!("{label} 不存在或不可执行: {}", path.display()));
    }
    path.canonicalize()
        .map_err(|error| format!("无法解析 {label}: {error}"))
}

fn discover_launcher(
    explicit_executable: Option<PathBuf>,
    path_value: Option<OsString>,
) -> Result<Launcher, String> {
    if let Some(path) = explicit_executable {
        let executable = canonical_executable(path, "STILLWRITE_PI_EXECUTABLE")?;
        return Ok(Launcher {
            label: executable.display().to_string(),
            executable,
        });
    }
    if let Some(path) = executable_on_path("pi", path_value.as_ref()) {
        let executable = canonical_executable(path, "PATH 中的 pi")?;
        return Ok(Launcher {
            label: executable.display().to_string(),
            executable,
        });
    }
    Err("未找到 Pi：请安装 @mariozechner/pi-coding-agent 或配置 STILLWRITE_PI_EXECUTABLE".into())
}

fn launcher_from_env() -> Result<Launcher, String> {
    discover_launcher(
        configured_path("STILLWRITE_PI_EXECUTABLE"),
        env::var_os("PATH"),
    )
}

fn launch_config_from_env() -> Result<LaunchConfig, String> {
    let agent_dir = configured_path("STILLWRITE_PI_AGENT_DIR");
    Ok(LaunchConfig {
        launcher: launcher_from_env()?,
        provider: configured_value("STILLWRITE_PI_PROVIDER"),
        model: configured_value("STILLWRITE_PI_MODEL"),
        thinking: configured_value("STILLWRITE_PI_THINKING"),
        agent_dir,
    })
}

fn materialize_runtime_resources(app: &AppHandle) -> Result<RuntimeResources, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位 Pi 运行时目录: {error}"))?
        .join("agent")
        .join("pi")
        .join("runtime");
    fs::create_dir_all(&root).map_err(|error| format!("创建 Pi 运行时目录失败: {error}"))?;
    let system_prompt = root.join("SYSTEM.md");
    let extension = root.join("stillwrite-tools.ts");
    if fs::read_to_string(&system_prompt).ok().as_deref() != Some(SYSTEM_PROMPT) {
        atomic_write(&system_prompt, SYSTEM_PROMPT)?;
    }
    if fs::read_to_string(&extension).ok().as_deref() != Some(STILLWRITE_TOOLS) {
        atomic_write(&extension, STILLWRITE_TOOLS)?;
    }
    Ok(RuntimeResources {
        system_prompt,
        extension,
    })
}

fn session_root(app: &AppHandle, workspace: &Path) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位 Pi session 目录: {error}"))?
        .join("agent")
        .join("pi")
        .join("workspaces")
        .join(agent_work::workspace_key(workspace))
        .join("sessions");
    fs::create_dir_all(&root).map_err(|error| format!("创建 Pi session 目录失败: {error}"))?;
    root.canonicalize()
        .map_err(|error| format!("解析 Pi session 目录失败: {error}"))
}

fn spawn_process(
    config: &LaunchConfig,
    workspace: &Path,
    sessions: &Path,
    resources: &RuntimeResources,
    sink: EventSink,
) -> Result<ChildProcess, String> {
    let mut command = Command::new(&config.launcher.executable);
    command
        .current_dir(workspace)
        .args([
            OsString::from("--mode"),
            OsString::from("rpc"),
            OsString::from("--session-dir"),
            sessions.as_os_str().to_os_string(),
            OsString::from("--no-builtin-tools"),
            OsString::from("--no-context-files"),
            OsString::from("--no-skills"),
            OsString::from("--no-prompt-templates"),
            OsString::from("--no-extensions"),
            OsString::from("--extension"),
            resources.extension.as_os_str().to_os_string(),
            OsString::from("--tools"),
            OsString::from("workspace_list,workspace_read,workspace_search"),
            OsString::from("--system-prompt"),
            resources.system_prompt.as_os_str().to_os_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PI_SKIP_VERSION_CHECK", "1")
        .env("STILLWRITE_PI_WORKSPACE_ROOT", workspace);
    if let Some(provider) = &config.provider {
        command.arg("--provider").arg(provider);
    }
    if let Some(model) = &config.model {
        command.arg("--model").arg(model);
    }
    if let Some(thinking) = &config.thinking {
        command.arg("--thinking").arg(thinking);
    }
    if let Some(agent_dir) = &config.agent_dir {
        command.env("PI_CODING_AGENT_DIR", agent_dir);
    } else {
        command.env_remove("PI_CODING_AGENT_DIR");
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 Pi 失败: {error}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法打开 Pi stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Pi stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取 Pi stderr".to_string())?;
    let process = ChildProcess {
        inner: Arc::new(ChildProcessInner {
            child: Mutex::new(child),
            stdin: Mutex::new(Some(stdin)),
            pending: Mutex::new(HashMap::new()),
            next_request_id: AtomicU64::new(0),
            dead: AtomicBool::new(false),
            stderr: Mutex::new(String::new()),
            session_root: sessions.to_path_buf(),
            active_run: Mutex::new(None),
            sink,
        }),
    };

    let stdout_process = process.clone();
    thread::spawn(move || read_stdout(stdout_process, stdout));
    let stderr_process = process.clone();
    thread::spawn(move || read_stderr(stderr_process, stderr));
    let wait_process = process.clone();
    thread::spawn(move || wait_for_exit(wait_process));
    Ok(process)
}

fn read_stdout(process: ChildProcess, stdout: impl Read + Send + 'static) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_jsonl_record(&mut reader) {
            Ok(Some(record)) => match serde_json::from_slice::<Value>(&record) {
                Ok(value) => {
                    if let Err(error) = process.dispatch_line(value) {
                        process.mark_dead(format!("Pi RPC 协议无效: {error}"));
                        break;
                    }
                }
                Err(error) => {
                    process.mark_dead(format!("Pi RPC JSON 无效: {error}"));
                    break;
                }
            },
            Ok(None) => {
                if !process.is_dead() {
                    let diagnostic = process.stderr();
                    let reason = if diagnostic.trim().is_empty() {
                        "Pi stdout 已关闭".to_string()
                    } else {
                        format!("Pi 进程已退出: {diagnostic}")
                    };
                    process.mark_dead(reason);
                }
                break;
            }
            Err(error) => {
                process.mark_dead(format!("读取 Pi RPC stdout 失败: {error}"));
                break;
            }
        }
    }
}

fn read_stderr(process: ChildProcess, mut stderr: impl Read) {
    let mut buffer = [0_u8; 4096];
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                if let Ok(mut output) = process.inner.stderr.lock() {
                    let mut remaining = MAX_STDERR_BYTES.saturating_sub(output.len());
                    if remaining > 0 {
                        let chunk = String::from_utf8_lossy(&buffer[..count]);
                        for character in chunk.chars() {
                            let width = character.len_utf8();
                            if width > remaining {
                                break;
                            }
                            output.push(character);
                            remaining -= width;
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}

fn wait_for_exit(process: ChildProcess) {
    loop {
        let check = {
            match process.inner.child.lock() {
                Ok(mut child) => child
                    .try_wait()
                    .map_err(|error| format!("检查 Pi 进程状态失败: {error}")),
                Err(_) => Err("Pi child 状态锁定失败".to_string()),
            }
        };
        let status = match check {
            Ok(status) => status,
            Err(error) => {
                process.mark_dead(error);
                break;
            }
        };
        if let Some(status) = status {
            if !process.is_dead() {
                let diagnostic = process.stderr();
                let reason = if diagnostic.trim().is_empty() {
                    format!("Pi 进程意外退出: {status}")
                } else {
                    format!("Pi 进程意外退出: {diagnostic}")
                };
                process.mark_dead(reason);
            }
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn read_jsonl_record(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut record = Vec::new();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            if record.is_empty() {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete RPC record",
            ));
        }
        if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            if record.len() + newline > MAX_RPC_RECORD_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "RPC record too large",
                ));
            }
            record.extend_from_slice(&buffer[..newline]);
            reader.consume(newline + 1);
            break;
        }
        if record.len() + buffer.len() > MAX_RPC_RECORD_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "RPC record too large",
            ));
        }
        let count = buffer.len();
        record.extend_from_slice(buffer);
        reader.consume(count);
    }
    if record.last() == Some(&b'\r') {
        record.pop();
    }
    if record.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "empty RPC record",
        ));
    }
    Ok(Some(record))
}

fn parse_rpc_response(value: &Value) -> Result<RpcResponse, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Pi RPC 响应不是对象".to_string())?;
    let id = object.get("id").and_then(Value::as_str).map(str::to_string);
    let command = object
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "Pi RPC 响应缺少 command".to_string())?
        .to_string();
    let success = object
        .get("success")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Pi RPC 响应缺少 success".to_string())?;
    Ok(RpcResponse {
        id,
        command,
        success,
        data: object.get("data").cloned(),
        error: object.get("error").cloned(),
    })
}

fn rpc_failure(context: &str, response: &RpcResponse) -> String {
    let detail = response
        .error
        .as_ref()
        .map(short_value)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("command {} rejected", response.command));
    format!("{context}: {detail}")
}

fn short_value(value: &Value) -> String {
    let text = value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string());
    short_error(&text)
}

fn short_error(error: &str) -> String {
    let clean = error.split_whitespace().collect::<Vec<_>>().join(" ");
    clean.chars().take(512).collect()
}

fn probe_version(launcher: &Launcher, workspace: &Path) -> Result<String, String> {
    let output = Command::new(&launcher.executable)
        .current_dir(workspace)
        .arg("--version")
        .env("PI_SKIP_VERSION_CHECK", "1")
        .output()
        .map_err(|error| format!("启动 Pi 版本探测失败: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Pi 版本探测失败: {}", short_error(&stderr)));
    }
    let stdout = output.stdout;
    if stdout.len() > MAX_VERSION_OUTPUT {
        return Err("Pi 版本响应超过大小限制".into());
    }
    let version = String::from_utf8_lossy(&stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_string();
    if version.is_empty() {
        return Err("Pi 版本探测没有返回版本号".into());
    }
    Ok(version)
}

fn session_ref_from_state(session_root: &Path, state: &Value) -> Result<Option<String>, String> {
    let Some(raw) = state.get("sessionFile").and_then(Value::as_str) else {
        return Ok(None);
    };
    let path = PathBuf::from(raw);
    let candidate = if path.is_absolute() {
        path
    } else {
        session_root.join(path)
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("Pi session 路径不可用: {error}"))?;
    let root = session_root
        .canonicalize()
        .map_err(|error| format!("解析 Pi session 根目录失败: {error}"))?;
    if !canonical.starts_with(&root) {
        return Err("Pi session 路径不在 Workspace 专属 session 目录内".into());
    }
    let relative = canonical
        .strip_prefix(&root)
        .map_err(|_| "Pi session 路径无效".to_string())?;
    if relative.as_os_str().is_empty() {
        return Ok(None);
    }
    Ok(Some(relative.to_string_lossy().replace('\\', "/")))
}

fn state_model_info(state: &Value) -> (Option<String>, Option<String>) {
    let model = state.get("model");
    let provider = model
        .and_then(|model| model.get("provider"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let model_id = model
        .and_then(|model| model.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    (provider, model_id)
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[tauri::command]
pub async fn agent_probe(
    app: AppHandle,
    state: State<'_, AppState>,
    process: State<'_, PiProcessState>,
) -> Result<AgentProbeResponse, String> {
    let root = workspace_root(&state)?;
    let process = PiProcessState::from_store(Arc::clone(&process.store));
    tauri::async_runtime::spawn_blocking(move || {
        let config = launch_config_from_env()?;
        let launcher = config.launcher.label.clone();
        let version = probe_version(&config.launcher, &root)?;
        let child = process.ensure_for_workspace(&app, &root, config)?;
        let response = child.request(json!({ "type": "get_state" }), COMMAND_TIMEOUT)?;
        if !response.success {
            return Err(rpc_failure("读取 Pi 当前状态失败", &response));
        }
        let data = response.data.unwrap_or(Value::Null);
        let (provider, model) = state_model_info(&data);
        if provider.is_none() || model.is_none() {
            return Err("Pi 已启动但没有可用模型；请在 Pi 外部完成 provider/auth 配置".into());
        }
        Ok(AgentProbeResponse {
            available: true,
            launcher,
            version,
            provider,
            model,
        })
    })
    .await
    .map_err(|error| format!("Agent probe 任务异常: {error}"))?
}

#[tauri::command]
pub async fn agent_start(
    app: AppHandle,
    state: State<'_, AppState>,
    process: State<'_, PiProcessState>,
    input: AgentStartInput,
) -> Result<AgentStartResponse, String> {
    let prompt = input.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("Agent 指令不能为空".into());
    }
    if !valid_identifier(&input.run_id) {
        return Err("Agent run id 格式无效".into());
    }
    let title = if input.title.trim().is_empty() {
        "Agent 工作".to_string()
    } else {
        input.title.trim().chars().take(120).collect()
    };
    let run_id = input.run_id;
    let root = workspace_root(&state)?;
    let process = PiProcessState::from_store(Arc::clone(&process.store));
    tauri::async_runtime::spawn_blocking(move || {
        let config = launch_config_from_env()?;
        let child = process.ensure_for_workspace(&app, &root, config)?;
        if child.active_run().is_some() {
            return Err("已有 Agent 工作正在运行".into());
        }
        let new_session = child.request(json!({ "type": "new_session" }), COMMAND_TIMEOUT)?;
        if !new_session.success {
            return Err(rpc_failure("创建 Pi session 失败", &new_session));
        }
        if new_session
            .data
            .as_ref()
            .and_then(|data| data.get("cancelled"))
            .and_then(Value::as_bool)
            == Some(true)
        {
            return Err("Pi 取消了新 session".into());
        }

        let named = child.request(
            json!({ "type": "set_session_name", "name": title }),
            COMMAND_TIMEOUT,
        )?;
        if !named.success {
            return Err(rpc_failure("设置 Pi session 名称失败", &named));
        }
        let state_response = child.request(json!({ "type": "get_state" }), COMMAND_TIMEOUT)?;
        if !state_response.success {
            return Err(rpc_failure("读取 Pi session 状态失败", &state_response));
        }
        let state_data = state_response.data.unwrap_or(Value::Null);
        let session_ref = session_ref_from_state(&child.inner.session_root, &state_data)?;
        child.begin_run(ActiveRun {
            run_id: run_id.clone(),
            session_ref: session_ref.clone(),
            settling: false,
        })?;
        let prompt_response = child.request(
            json!({ "type": "prompt", "message": prompt }),
            COMMAND_TIMEOUT,
        );
        let prompt_response = match prompt_response {
            Ok(response) => response,
            Err(error) => {
                child.clear_run();
                return Err(error);
            }
        };
        if !prompt_response.success {
            child.clear_run();
            return Err(rpc_failure("Pi 拒绝 Agent 请求", &prompt_response));
        }
        Ok(AgentStartResponse {
            accepted: true,
            run_id,
            request_id: prompt_response.id.unwrap_or_else(|| "unknown".into()),
            pi_session_ref: session_ref,
        })
    })
    .await
    .map_err(|error| format!("Agent start 任务异常: {error}"))?
}

#[tauri::command]
pub async fn agent_abort(process: State<'_, PiProcessState>) -> Result<AgentAbortResponse, String> {
    let process = PiProcessState::from_store(Arc::clone(&process.store));
    tauri::async_runtime::spawn_blocking(move || {
        let Some(child) = process.current_process() else {
            return Ok(AgentAbortResponse {
                accepted: false,
                run_id: None,
            });
        };
        let Some(run) = child.active_run() else {
            return Ok(AgentAbortResponse {
                accepted: false,
                run_id: None,
            });
        };
        let control = (|| -> Result<(), String> {
            let clear = child.request(json!({ "type": "clear_queue" }), ABORT_TIMEOUT)?;
            if !clear.success {
                return Err(rpc_failure("清空 Pi 队列失败", &clear));
            }
            let abort = child.request(json!({ "type": "abort" }), ABORT_TIMEOUT)?;
            if !abort.success {
                return Err(rpc_failure("停止 Pi Agent 失败", &abort));
            }
            wait_for_idle(&child)?;
            Ok(())
        })();
        if let Err(error) = control {
            let message = format!("停止 Agent 失败，已终止 Pi 进程: {error}");
            child.clear_run();
            child.shutdown();
            emit_direct_event(&child, &run.run_id, "error", Some(short_error(&message)));
            return Err(message);
        }
        if child.take_run(&run.run_id).is_some() {
            emit_direct_event(&child, &run.run_id, "agent_stopped", None);
        }
        Ok(AgentAbortResponse {
            accepted: true,
            run_id: Some(run.run_id),
        })
    })
    .await
    .map_err(|error| format!("Agent abort 任务异常: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Cursor,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("stillwrite-pi-{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[cfg(unix)]
    fn make_executable(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    fn fake_process(root: &Path, body: &str) -> (ChildProcess, Arc<Mutex<Vec<Value>>>) {
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).unwrap();
        let fake = root.join("pi");
        make_executable(&fake, body);
        let config = LaunchConfig {
            launcher: Launcher {
                executable: fake,
                label: "fake".into(),
            },
            provider: None,
            model: None,
            thinking: None,
            agent_dir: None,
        };
        let resources = RuntimeResources {
            system_prompt: root.join("SYSTEM.md"),
            extension: root.join("tools.ts"),
        };
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let events_sink = Arc::clone(&events);
        let sink: EventSink = Arc::new(move |event| events_sink.lock().unwrap().push(event));
        (
            spawn_process(&config, root, &sessions, &resources, sink).unwrap(),
            events,
        )
    }

    #[test]
    #[cfg(unix)]
    fn launcher_prefers_explicit_pi_executable() {
        let root = temp_dir("explicit-launcher");
        let explicit = root.join("configured-pi");
        let path_pi = root.join("pi");
        make_executable(&explicit, "#!/bin/sh\n");
        make_executable(&path_pi, "#!/bin/sh\n");

        let launcher = discover_launcher(
            Some(explicit.clone()),
            Some(env::join_paths([root.as_path()]).unwrap()),
        )
        .unwrap();
        assert_eq!(launcher.executable, explicit.canonicalize().unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn launcher_falls_back_to_pi_on_path() {
        let root = temp_dir("path-launcher");
        let path_pi = root.join("pi");
        make_executable(&path_pi, "#!/bin/sh\n");
        let launcher =
            discover_launcher(None, Some(env::join_paths([root.as_path()]).unwrap())).unwrap();
        assert_eq!(launcher.executable, path_pi.canonicalize().unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn non_executable_explicit_pi_is_rejected() {
        let root = temp_dir("non-executable");
        let path = root.join("pi");
        fs::write(&path, "#!/bin/sh\n").unwrap();
        let error = discover_launcher(Some(path), None).unwrap_err();
        assert!(error.contains("STILLWRITE_PI_EXECUTABLE"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_pi_has_installation_guidance() {
        let error = discover_launcher(None, Some(OsString::from(""))).unwrap_err();
        assert!(error.contains("安装") || error.contains("STILLWRITE_PI_EXECUTABLE"));
    }

    #[test]
    fn jsonl_reader_accepts_crlf_and_rejects_empty_records() {
        let mut reader = BufReader::new(Cursor::new(b"{\"ok\":true}\r\n".to_vec()));
        assert_eq!(
            read_jsonl_record(&mut reader).unwrap().unwrap(),
            b"{\"ok\":true}"
        );
        let mut empty = BufReader::new(Cursor::new(b"\n".to_vec()));
        assert!(read_jsonl_record(&mut empty).is_err());
        let mut incomplete = BufReader::new(Cursor::new(b"{\"ok\":true}".to_vec()));
        assert!(read_jsonl_record(&mut incomplete).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn malformed_stdout_marks_process_dead_and_fails_pending_request() {
        let root = temp_dir("malformed");
        let (process, _) = fake_process(&root, "#!/bin/sh\nread line\nprintf 'not-json\\n'\n");
        let error = process
            .request(json!({ "type": "get_state" }), COMMAND_TIMEOUT)
            .unwrap_err();
        assert!(error.contains("JSON") || error.contains("协议"));
        let deadline = Instant::now() + Duration::from_secs(2);
        while !process.is_dead() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(process.is_dead());
        process.shutdown();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn abort_keeps_persistent_process_available_for_the_next_session() {
        let root = temp_dir("abort-alive");
        let (process, _) = fake_process(
            &root,
            r##"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"type":"get_state"'*) printf '{"id":"%s","type":"response","command":"get_state","success":true,"data":{"model":{"provider":"fake","id":"fake-model"}}}\n' "$id" ;;
    *'"type":"new_session"'*) printf '{"id":"%s","type":"response","command":"new_session","success":true,"data":{"cancelled":false}}\n' "$id" ;;
    *'"type":"set_session_name"'*) printf '{"id":"%s","type":"response","command":"set_session_name","success":true}\n' "$id" ;;
    *'"type":"prompt"'*) printf '{"id":"%s","type":"response","command":"prompt","success":true}\n' "$id" ;;
    *'"type":"clear_queue"'*) printf '{"id":"%s","type":"response","command":"clear_queue","success":true}\n' "$id" ;;
    *'"type":"abort"'*) printf '{"id":"%s","type":"response","command":"abort","success":true}\n' "$id" ;;
  esac
done
"##,
        );
        assert!(
            process
                .request(json!({ "type": "get_state" }), COMMAND_TIMEOUT)
                .unwrap()
                .success
        );
        process
            .begin_run(ActiveRun {
                run_id: "run-abort".into(),
                session_ref: None,
                settling: false,
            })
            .unwrap();
        assert!(
            process
                .request(
                    json!({ "type": "prompt", "message": "go" }),
                    COMMAND_TIMEOUT
                )
                .unwrap()
                .success
        );
        assert!(
            process
                .request(json!({ "type": "clear_queue" }), ABORT_TIMEOUT)
                .unwrap()
                .success
        );
        assert!(
            process
                .request(json!({ "type": "abort" }), ABORT_TIMEOUT)
                .unwrap()
                .success
        );
        assert!(process.take_run("run-abort").is_some());
        assert!(!process.is_dead());
        assert!(
            process
                .request(json!({ "type": "new_session" }), COMMAND_TIMEOUT)
                .unwrap()
                .success
        );
        process.shutdown();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn process_exit_emits_error_for_active_run_without_replay() {
        let root = temp_dir("crash");
        let (process, events) = fake_process(
            &root,
            r##"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"type":"get_state"'*) printf '{"id":"%s","type":"response","command":"get_state","success":true,"data":{"model":{"provider":"fake","id":"fake-model"}}}\n' "$id" ;;
    *'"type":"prompt"'*) printf '{"id":"%s","type":"response","command":"prompt","success":true}\n' "$id"; exit 0 ;;
  esac
done
"##,
        );
        assert!(
            process
                .request(json!({ "type": "get_state" }), COMMAND_TIMEOUT)
                .unwrap()
                .success
        );
        process
            .begin_run(ActiveRun {
                run_id: "run-crash".into(),
                session_ref: None,
                settling: false,
            })
            .unwrap();
        assert!(
            process
                .request(
                    json!({ "type": "prompt", "message": "go" }),
                    COMMAND_TIMEOUT
                )
                .unwrap()
                .success
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let has_error = events
                .lock()
                .unwrap()
                .iter()
                .any(|event| event.get("type").and_then(Value::as_str) == Some("error"));
            if has_error || Instant::now() >= deadline {
                assert!(has_error, "crashed fake Pi did not emit an error");
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(process.is_dead());
        assert!(process.active_run().is_none());
        process.shutdown();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn session_reference_cannot_escape_session_root() {
        let root = temp_dir("session-ref");
        let state = json!({ "sessionFile": "../outside.jsonl" });
        assert!(session_ref_from_state(&root, &state).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn fake_pi_streams_and_settles_with_authoritative_text() {
        let root = temp_dir("fake-stream");
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).unwrap();
        let fake = root.join("pi");
        make_executable(
            &fake,
            r##"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"type":"get_state"'*) printf '{"id":"%s","type":"response","command":"get_state","success":true,"data":{"model":{"provider":"fake","id":"fake-model"},"sessionFile":"/tmp/fake-session.jsonl","sessionId":"fake-session"}}\n' "$id" ;;
    *'"type":"new_session"'*) printf '{"id":"%s","type":"response","command":"new_session","success":true,"data":{"cancelled":false}}\n' "$id" ;;
    *'"type":"set_session_name"'*) printf '{"id":"%s","type":"response","command":"set_session_name","success":true}\n' "$id" ;;
    *'"type":"prompt"'*) printf '{"id":"%s","type":"response","command":"prompt","success":true}\n{"type":"agent_start"}\n{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"hello "}}\n{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"world"}}\n{"type":"agent_settled"}\n' "$id" ;;
    *'"type":"get_last_assistant_text"'*) printf '{"id":"%s","type":"response","command":"get_last_assistant_text","success":true,"data":{"text":"hello world"}}\n' "$id" ;;
    *'"type":"clear_queue"'*) printf '{"id":"%s","type":"response","command":"clear_queue","success":true}\n' "$id" ;;
    *'"type":"abort"'*) printf '{"id":"%s","type":"response","command":"abort","success":true}\n' "$id" ;;
  esac
done
"##,
        );
        let config = LaunchConfig {
            launcher: Launcher {
                executable: fake,
                label: "fake".into(),
            },
            provider: None,
            model: None,
            thinking: None,
            agent_dir: None,
        };
        let resources = RuntimeResources {
            system_prompt: root.join("SYSTEM.md"),
            extension: root.join("tools.ts"),
        };
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let events_sink = Arc::clone(&events);
        let sink: EventSink = Arc::new(move |event| events_sink.lock().unwrap().push(event));
        let process = spawn_process(&config, &root, &sessions, &resources, sink).unwrap();
        let handshake = process
            .request(json!({ "type": "get_state" }), COMMAND_TIMEOUT)
            .unwrap();
        assert!(handshake.success);
        process
            .begin_run(ActiveRun {
                run_id: "run-1".into(),
                session_ref: Some("fake.jsonl".into()),
                settling: false,
            })
            .unwrap();
        let accepted = process
            .request(
                json!({ "type": "prompt", "message": "go" }),
                COMMAND_TIMEOUT,
            )
            .unwrap();
        assert!(accepted.success);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if events
                .lock()
                .unwrap()
                .iter()
                .any(|event| event.get("type").and_then(Value::as_str) == Some("agent_settled"))
            {
                break;
            }
            assert!(Instant::now() < deadline, "fake Pi did not settle");
            thread::sleep(Duration::from_millis(10));
        }
        let events = events.lock().unwrap();
        let settled = events
            .iter()
            .find(|event| event.get("type").and_then(Value::as_str) == Some("agent_settled"))
            .unwrap();
        assert_eq!(
            settled.get("text").and_then(Value::as_str),
            Some("hello world")
        );
        assert!(!process.is_dead());
        process.shutdown();
        fs::remove_dir_all(root).unwrap();
    }
}
