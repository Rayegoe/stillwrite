//! Persistent Pi RPC host.
//!
//! StillWrite owns the child-process boundary and the small amount of state
//! needed to route one active Agent Work. Pi owns the model loop and its
//! session log; this module deliberately does not copy Pi's full protocol.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::{HashMap, HashSet},
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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state_store::ActorKind;
use crate::work::{self, WorkStatus};
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
                    let mut fields = vec![("message".into(), Value::String(message))];
                    let tail = stderr_tail(self);
                    if !tail.is_empty() {
                        fields.push(("stderr".into(), Value::String(tail)));
                    }
                    self.emit_run_event(&run.run_id, "error", fields);
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
        let result = match result {
            Ok(text) => {
                // 首个 assistant response 已产生，session 文件此刻应已落盘；
                // 重新解析拿到权威引用，失败则退回启动时记录的逻辑引用。
                let verified = self
                    .request(json!({ "type": "get_state" }), COMMAND_TIMEOUT)
                    .ok()
                    .filter(|response| response.success)
                    .map(|response| {
                        session_ref_from_state(
                            &self.inner.session_root,
                            &response.data.unwrap_or(Value::Null),
                        )
                    })
                    .and_then(|parsed| parsed.ok())
                    .flatten();
                let session_ref = verified.or(run.session_ref.clone());
                Ok((text, session_ref))
            }
            Err(error) => Err(error),
        };
        match result {
            Ok((text, session_ref)) => self.emit_run_event(
                &run.run_id,
                "agent_settled",
                [
                    ("text".into(), Value::String(text)),
                    (
                        "piSessionRef".into(),
                        session_ref.map(Value::String).unwrap_or(Value::Null),
                    ),
                ],
            ),
            Err(error) => {
                let mut fields: Vec<(String, Value)> =
                    vec![("message".into(), Value::String(short_error(&error)))];
                let tail = stderr_tail(self);
                if !tail.is_empty() {
                    fields.push(("stderr".into(), Value::String(tail)));
                }
                self.emit_run_event(&run.run_id, "error", fields);
            }
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

    /// 当前进行中的 run id（若有）。Work 取消命令用它判断是否需要中止 Pi。
    pub fn active_run_id(&self) -> Option<String> {
        self.current_process()?.active_run().map(|run| run.run_id)
    }

    /// 关闭属于其它 Workspace 的 Pi 进程。若进程上有进行中的 run，
    /// 返回其 run id（调用方负责把对应 Work 转为 cancelled）。
    pub fn shutdown_for_workspace(&self, next_root: &Path) -> Option<String> {
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
        let aborted_run = old
            .as_ref()
            .and_then(|runtime| runtime.process.active_run().map(|run| run.run_id));
        if let Some(runtime) = old {
            let process = runtime.process.clone();
            if process.active_run().is_some() {
                let _ = process.request(json!({ "type": "clear_queue" }), ABORT_TIMEOUT);
                let _ = process.request(json!({ "type": "abort" }), ABORT_TIMEOUT);
            }
            process.shutdown();
        }
        aborted_run
    }
}

fn tauri_event_sink(app: &AppHandle) -> EventSink {
    let app = app.clone();
    let streaming_runs: Arc<Mutex<HashSet<String>>> = Arc::default();
    Arc::new(move |event| {
        journal_pipeline_event(&app, &streaming_runs, &event);
        if let Err(error) = app.emit("agent-event", event) {
            eprintln!("发送 Agent 事件失败: {error}");
        }
    })
}

/// 把 Pi 运行期事件映射为状态链，逐条追加进该 run 的持久化收据。
/// 收据写入是尽力而为的：失败只影响可观测性，不影响运行本身。
fn journal_pipeline_event(app: &AppHandle, streaming_runs: &Mutex<HashSet<String>>, event: &Value) {
    let Some(object) = event.as_object() else {
        return;
    };
    let Some(run_id) = object.get("runId").and_then(Value::as_str) else {
        return;
    };
    let event_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut fields = serde_json::Map::new();
    fields.insert("event".into(), Value::String(event_type.into()));
    match event_type {
        "agent_start" => {
            fields.insert("stage".into(), json!("MODEL_RUNNING"));
        }
        // 每个 delta 都写文件会把收据撑爆；只在首个文本增量落 STREAMING。
        "message_update" => {
            if !object.contains_key("delta") {
                return;
            }
            let first = streaming_runs
                .lock()
                .map(|mut seen| seen.insert(run_id.to_string()))
                .unwrap_or(false);
            if !first {
                return;
            }
            fields.insert("stage".into(), json!("STREAMING"));
        }
        "tool_execution_start" | "tool_execution_end" => {
            if let Some(tool_name) = object.get("toolName").and_then(Value::as_str) {
                fields.insert("toolName".into(), json!(tool_name));
            }
        }
        "compaction_start" | "compaction_end" => {}
        "agent_settled" => {
            fields.insert("stage".into(), json!("SETTLED"));
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                fields.insert("textLength".into(), json!(text.chars().count()));
            }
            if let Some(session_ref) = object.get("piSessionRef").and_then(Value::as_str) {
                fields.insert("piSessionRef".into(), json!(session_ref));
            }
        }
        "agent_stopped" => {
            fields.insert("stage".into(), json!("STOPPED"));
        }
        "error" => {
            fields.insert("stage".into(), json!("FAILED"));
            if let Some(message) = object.get("message") {
                fields.insert("error".into(), message.clone());
            }
            let reason = object
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Pi 运行失败");
            fail_run_work(app, run_id, reason);
        }
        _ => return,
    }
    append_run_receipt(app, run_id, Value::Object(fields));
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

const RECEIPT_STDERR_TAIL_CHARS: usize = 1024;
const RECEIPT_MAX_RUNS: usize = 50;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StartStage {
    StartingPi,
    PiReady,
    SessionAllocated,
    PromptSent,
}

impl StartStage {
    fn label(self) -> &'static str {
        match self {
            Self::StartingPi => "STARTING_PI",
            Self::PiReady => "PI_READY",
            Self::SessionAllocated => "SESSION_ALLOCATED",
            Self::PromptSent => "PROMPT_SENT",
        }
    }
}

/// 运行收据目录：`<AppData>/agent/runs/<run-id>.jsonl`。
fn runs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位 Agent 运行收据目录: {error}"))?
        .join("agent")
        .join("runs");
    fs::create_dir_all(&dir).map_err(|error| format!("创建 Agent 运行收据目录失败: {error}"))?;
    Ok(dir)
}

/// Work 证据探针：run 收据文件路径（只报告存在性，不读取内容）。
/// run id 非法时返回 None，避免用任意字符串拼路径。
pub(crate) fn receipt_path(app: &AppHandle, run_id: &str) -> Result<Option<PathBuf>, String> {
    if !valid_identifier(run_id) {
        return Ok(None);
    }
    Ok(Some(runs_dir(app)?.join(format!("{run_id}.jsonl"))))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// 尽力而为地追加一条运行收据；run_id 已由 valid_identifier 约束为安全文件名。
fn append_run_receipt(app: &AppHandle, run_id: &str, fields: Value) {
    let Ok(dir) = runs_dir(app) else {
        return;
    };
    let Some(extra) = fields.as_object() else {
        return;
    };
    let mut line = serde_json::Map::new();
    line.insert("ts".into(), json!(unix_ms()));
    line.insert("runId".into(), json!(run_id));
    for (key, value) in extra {
        line.insert(key.clone(), value.clone());
    }
    let Ok(mut record) = serde_json::to_string(&Value::Object(line)) else {
        return;
    };
    record.push('\n');
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{run_id}.jsonl")))
    {
        let _ = file.write_all(record.as_bytes());
    }
}

/// 供其它模块（如 Agent Work 保存）补记运行收据，例如 WORK_SAVED。
pub(crate) fn record_run_event(app: &AppHandle, run_id: &str, fields: Value) {
    if !valid_identifier(run_id) {
        return;
    }
    append_run_receipt(app, run_id, fields);
}

// ---------------------------------------------------------------------------
// Pi → Work bridge（M1）
//
// 一次 Pi 请求对应一条 durable Work（receipt_ref = run id）。状态映射：
// 请求创建=queued；Pi accepted=running；Artifact 保存（lib.rs）=needs_human；
// abort=cancelled；依赖缺失=blocked；致命错误=failed；
// 人明确接受=completed（Work 侧 completed 转换，UI 待 M2 接入）。
// Pi 返回最终文本不是任何 Work 状态变化——settled 后仍停在 running，
// 直到 Artifact 固化为 needs_human。
// ---------------------------------------------------------------------------

/// 按收据引用推进 Work 状态。桥接是尽力而为的：状态库不可用时请求本身
/// 不回滚（运行收据仍记录事实），只留下 stderr 痕迹。
fn transition_run_work(
    app: &AppHandle,
    run_id: &str,
    to: WorkStatus,
    actor: ActorKind,
    reason: Option<&str>,
) {
    if !valid_identifier(run_id) {
        return;
    }
    let result = (|| -> Result<(), String> {
        let mut conn = crate::open_durable_state(app)?;
        let Some(record) = work::find_work_by_receipt(&conn, run_id)? else {
            // 旧请求 / 无 Work 的运行路径：没有可推进的对象，不算错误。
            return Ok(());
        };
        work::transition_work(&mut conn, &record.id, to, actor, reason).map(|_| ())
    })();
    if let Err(error) = result {
        eprintln!("推进 Work 状态失败 (run {run_id} → {}): {error}", to.as_str());
    }
}

/// abort 语义的 Work 取消（用户停止 / 切换工作区中止）。
pub(crate) fn cancel_run_work(app: &AppHandle, run_id: &str, reason: &str) {
    transition_run_work(app, run_id, WorkStatus::Cancelled, ActorKind::Human, Some(reason));
}

/// 运行期致命错误（Pi 进程死亡 / 扩展报错）→ Work failed。
fn fail_run_work(app: &AppHandle, run_id: &str, reason: &str) {
    transition_run_work(app, run_id, WorkStatus::Failed, ActorKind::Agent, Some(reason));
}

/// Work 视图的取消入口：该 Work 的 run 正在 Pi 上运行时先走 abort 核心
/// （核心会把 Work 落为 cancelled），否则直接做状态转换。同状态重复取消
/// 由 transition_work 幂等吸收。
pub fn cancel_work(
    process: &PiProcessState,
    app: &AppHandle,
    work_id: &str,
) -> Result<work::WorkRecord, String> {
    let mut conn = crate::open_durable_state(app)?;
    let record = work::get_work(&conn, work_id)?.ok_or_else(|| format!("Work 不存在: {work_id}"))?;
    let running = record
        .receipt_ref
        .as_deref()
        .zip(process.active_run_id())
        .is_some_and(|(receipt, active)| receipt == active);
    if running {
        drop(conn);
        abort_active_run(process, app)?;
        conn = crate::open_durable_state(app)?;
    }
    let reason = if running {
        "用户停止 Agent"
    } else {
        "用户取消工作"
    };
    work::transition_work(
        &mut conn,
        work_id,
        WorkStatus::Cancelled,
        ActorKind::Human,
        Some(reason),
    )
}

/// 启动失败分类：缺 Pi 可执行文件 / 缺 provider 模型属于依赖缺失（补齐后可重试）
/// → blocked；其余（握手失败、session 被拒、prompt 被拒等）→ failed。
fn start_failure_is_missing_dependency(error: &str) -> bool {
    error.contains("未找到 Pi")
        || error.contains("不存在或不可执行")
        || error.contains("没有可用模型")
        || error.contains("provider/auth")
}

/// 最近一次 Pi 进程的 stderr 尾部，用于失败收据定位。
fn stderr_tail(process: &ChildProcess) -> String {
    let stderr = process.stderr();
    if stderr.trim().is_empty() {
        return String::new();
    }
    let tail = stderr
        .chars()
        .rev()
        .take(RECEIPT_STDERR_TAIL_CHARS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    tail.trim_start().to_string()
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

fn user_home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
}

fn executable_in_user_install(home: Option<&Path>) -> Option<PathBuf> {
    let home = home?;
    let mut candidates = vec![home.join(".local").join("bin").join("pi")];
    let pi_node_root = home.join(".local").join("share").join("pi-node");
    if let Ok(entries) = fs::read_dir(pi_node_root) {
        let mut versioned = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|file_type| file_type.is_dir())
                    .map(|_| entry.path().join("bin").join("pi"))
            })
            .collect::<Vec<_>>();
        versioned.sort_by(|left, right| right.cmp(left));
        candidates.extend(versioned);
    }
    candidates
        .into_iter()
        .find(|candidate| is_executable(candidate))
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
    discover_launcher_with_home(explicit_executable, path_value, user_home_dir().as_deref())
}

fn discover_launcher_with_home(
    explicit_executable: Option<PathBuf>,
    path_value: Option<OsString>,
    home: Option<&Path>,
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
    if let Some(path) = executable_in_user_install(home) {
        let executable = canonical_executable(path, "用户目录中的 Pi")?;
        return Ok(Launcher {
            label: executable.display().to_string(),
            executable,
        });
    }
    Err("未找到 Pi：请安装 Pi，或配置 STILLWRITE_PI_EXECUTABLE".into())
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
    let system_prompt = fs::read_to_string(&resources.system_prompt)
        .map_err(|error| format!("读取 Pi system prompt 失败: {error}"))?;
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
            OsString::from(system_prompt),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PI_SKIP_VERSION_CHECK", "1")
        .env("STILLWRITE_PI_WORKSPACE_ROOT", workspace)
        .env("PATH", launcher_env_path(&config.launcher));
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

/// 计算 Pi 子进程 PATH 应包含的候选目录。
/// pi 脚本用 `#!/usr/bin/env node` 找解释器，而 GUI 启动的进程 PATH 不含
/// 任何用户级 Node；且可执行文件若是符号链接，canonicalize 之后落在
/// node_modules 深处（如 dist/bundle/），其父目录并没有 node。因此除
/// 可执行文件自身目录外，还要带上已知的用户安装位置：
/// `~/.local/share/pi-node/*/bin`（版本化自管安装、自带便携 node）与
/// `~/.local/bin`。顺序：自身目录 → local/bin → 新版 pi-node 在前 → 原有 PATH。
fn child_env_dirs(executable: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    fn record(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
        if !dirs.iter().any(|existing| existing == &dir) {
            dirs.push(dir);
        }
    }
    if let Some(parent) = executable.parent() {
        record(&mut dirs, parent.to_path_buf());
    }
    let Some(home) = home else {
        return dirs;
    };
    record(&mut dirs, home.join(".local").join("bin"));
    let mut version_bins: Vec<PathBuf> =
        fs::read_dir(home.join(".local").join("share").join("pi-node"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|entry| entry.path().join("bin"))
            .filter(|bin| bin.is_dir())
            .collect();
    version_bins.sort_by(|left, right| right.cmp(left));
    for bin in version_bins {
        record(&mut dirs, bin);
    }
    dirs
}

fn launcher_env_path(launcher: &Launcher) -> OsString {
    let base = env::var_os("PATH").unwrap_or_default();
    let mut paths = child_env_dirs(&launcher.executable, user_home_dir().as_deref());
    paths.extend(env::split_paths(&base));
    env::join_paths(paths).unwrap_or(base)
}

fn probe_version(launcher: &Launcher, workspace: &Path) -> Result<String, String> {
    let output = Command::new(&launcher.executable)
        .current_dir(workspace)
        .arg("--version")
        .env("PI_SKIP_VERSION_CHECK", "1")
        .env("PATH", launcher_env_path(launcher))
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
    let Some(raw) = state
        .get("sessionFile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
    else {
        return Ok(None);
    };
    let path = PathBuf::from(raw);
    let root = session_root
        .canonicalize()
        .map_err(|error| format!("解析 Pi session 根目录失败: {error}"))?;
    // Pi 把 session 文件延迟到首个 assistant response 才创建；prompt 发出前
    // 该路径只是预定值。存在则实体校验，不存在时只做词法校验，绝不在
    // 握手阶段因文件未落盘而阻断 prompt。
    let parent_escape = path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir));
    let candidate: PathBuf = if path.is_absolute() {
        path.clone()
    } else {
        root.join(&path)
    };
    let relative: String = if candidate.is_file() {
        let canonical = candidate
            .canonicalize()
            .map_err(|error| format!("Pi session 路径不可用: {error}"))?;
        if !canonical.starts_with(&root) {
            return Err("Pi session 路径不在 Workspace 专属 session 目录内".into());
        }
        canonical
            .strip_prefix(&root)
            .map_err(|_| "Pi session 路径无效".to_string())?
            .to_string_lossy()
            .replace('\\', "/")
    } else {
        if parent_escape {
            return Err("Pi session 路径不允许包含 ..".into());
        }
        if !candidate.starts_with(&root) {
            return Err("Pi session 路径不在 Workspace 专属 session 目录内".into());
        }
        candidate
            .strip_prefix(&root)
            .map_err(|_| "Pi session 路径无效".to_string())?
            .to_string_lossy()
            .replace('\\', "/")
    };
    if relative.is_empty() {
        return Ok(None);
    }
    Ok(Some(relative))
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
    // Work 桥接第一步：Pi 请求成立即落地 durable Work（queued，receipt_ref=run id）。
    // 创建失败则整个请求失败——没有 durable 记录的 Agent 请求不允许发生。
    {
        let mut conn = crate::open_durable_state(&app)?;
        work::create_work(
            &mut conn,
            work::NewWork {
                workspace_id: Some(crate::workspace_id_for_root(&root)),
                title: title.clone(),
                intent: prompt.clone(),
                receipt_ref: Some(run_id.clone()),
            },
        )?;
    }
    let process = PiProcessState::from_store(Arc::clone(&process.store));
    tauri::async_runtime::spawn_blocking(move || {
        // 状态链：STARTING_PI → PI_READY → SESSION_ALLOCATED → PROMPT_SENT，
        // 之后由事件侧接管（MODEL_RUNNING → STREAMING → SETTLED / FAILED）。
        let mut stage = StartStage::StartingPi;
        let mut child_slot: Option<ChildProcess> = None;
        let outcome: Result<AgentStartResponse, String> = (|| {
            let config = launch_config_from_env()?;
            let child = process.ensure_for_workspace(&app, &root, config)?;
            child_slot = Some(child.clone());
            stage = StartStage::PiReady;
            append_run_receipt(
                &app,
                &run_id,
                json!({ "event": "stage", "stage": "PI_READY" }),
            );
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
            // 此处只解析逻辑引用，不要求文件已落盘；实体校验推迟到首个
            // assistant response 之后的 resolve_settled_run。
            let session_ref = session_ref_from_state(&child.inner.session_root, &state_data)?;
            let (provider, model) = state_model_info(&state_data);
            stage = StartStage::SessionAllocated;
            append_run_receipt(
                &app,
                &run_id,
                json!({
                    "event": "stage",
                    "stage": "SESSION_ALLOCATED",
                    "piSessionRef": session_ref,
                    "provider": provider,
                    "model": model,
                    "title": title,
                }),
            );
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
            stage = StartStage::PromptSent;
            append_run_receipt(
                &app,
                &run_id,
                json!({
                    "event": "stage",
                    "stage": "PROMPT_SENT",
                    "requestId": prompt_response.id,
                    "title": title,
                }),
            );
            // Pi 接受请求 → Work running（桥接尽力而为，失败只留痕迹不回滚已发出的请求）
            transition_run_work(
                &app,
                &run_id,
                WorkStatus::Running,
                ActorKind::Agent,
                Some("pi accepted"),
            );
            Ok(AgentStartResponse {
                accepted: true,
                // 不能把 run_id move 进响应：外层失败路径还要用它写收据。
                run_id: run_id.clone(),
                request_id: prompt_response.id.unwrap_or_else(|| "unknown".into()),
                pi_session_ref: session_ref,
            })
        })();
        if let Err(error) = &outcome {
            let mut fields = serde_json::Map::new();
            fields.insert("event".into(), json!("failed"));
            fields.insert("stage".into(), json!(stage.label()));
            fields.insert("error".into(), json!(short_error(error)));
            if let Some(child) = child_slot.as_ref() {
                let tail = stderr_tail(child);
                if !tail.is_empty() {
                    fields.insert("stderr".into(), json!(tail));
                }
            }
            append_run_receipt(&app, &run_id, Value::Object(fields));
            // 启动失败 → Work blocked（依赖缺失，可恢复）或 failed（其余致命错误）
            let message = short_error(error);
            let status = if start_failure_is_missing_dependency(&message) {
                WorkStatus::Blocked
            } else {
                WorkStatus::Failed
            };
            transition_run_work(&app, &run_id, status, ActorKind::Agent, Some(&message));
            return Err(format!(
                "Agent 启动在 {} 阶段失败：{}",
                stage.label(),
                error
            ));
        }
        outcome
    })
    .await
    .map_err(|error| format!("Agent start 任务异常: {error}"))?
}

/// abort 的共享核心：清队列 → 中止 → 等待 idle；成功时把对应 Work 转为
/// cancelled。`agent_abort` 命令与 `work_cancel` 命令（Work 视图入口）共用。
/// 返回被中止的 run id（没有进行中的 run 时为 None）。
pub(crate) fn abort_active_run(
    process: &PiProcessState,
    app: &AppHandle,
) -> Result<Option<String>, String> {
    let Some(child) = process.current_process() else {
        return Ok(None);
    };
    let Some(run) = child.active_run() else {
        return Ok(None);
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
        // abort 成功 → Work cancelled（失败时收据侧仍记录 STOPPED 事实）
        cancel_run_work(app, &run.run_id, "用户停止 Agent");
    }
    Ok(Some(run.run_id))
}

#[tauri::command]
pub async fn agent_abort(
    app: AppHandle,
    process: State<'_, PiProcessState>,
) -> Result<AgentAbortResponse, String> {
    let process = PiProcessState::from_store(Arc::clone(&process.store));
    tauri::async_runtime::spawn_blocking(move || {
        let run_id = abort_active_run(&process, &app)?;
        Ok(AgentAbortResponse {
            accepted: run_id.is_some(),
            run_id,
        })
    })
    .await
    .map_err(|error| format!("Agent abort 任务异常: {error}"))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunReceiptSummary {
    pub run_id: String,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub title: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// 最后一次记录到的状态链阶段，如 SESSION_ALLOCATED、SETTLED、FAILED。
    pub stage: Option<String>,
    /// settled | failed | stopped | unknown（unknown 含进行中与异常中断）。
    pub outcome: String,
    pub error: Option<String>,
}

/// 读取持久化运行收据（`<AppData>/agent/runs/*.jsonl`），最近优先。
/// 目的：任何一步失败在重启应用之后仍然可查，而不是只活在 WebView 内存里。
#[tauri::command]
pub async fn agent_recent_runs(app: AppHandle) -> Result<Vec<AgentRunReceiptSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || list_run_receipts(&app))
        .await
        .map_err(|error| format!("读取 Agent 运行收据任务异常: {error}"))?
}

fn list_run_receipts(app: &AppHandle) -> Result<Vec<AgentRunReceiptSummary>, String> {
    let dir = runs_dir(app)?;
    let mut files: Vec<(PathBuf, SystemTime)> = fs::read_dir(&dir)
        .map_err(|error| format!("读取 Agent 运行收据目录失败: {error}"))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect();
    files.sort_by(|left, right| right.1.cmp(&left.1));
    files.truncate(RECEIPT_MAX_RUNS);
    Ok(files
        .into_iter()
        .filter_map(|(path, _)| summarize_receipt(&path).ok().flatten())
        .collect())
}

fn truncate_text(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn summarize_receipt(path: &Path) -> Result<Option<AgentRunReceiptSummary>, String> {
    let Some(run_id) = path.file_stem().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    if !valid_identifier(run_id) {
        return Ok(None);
    }
    let body = fs::read_to_string(path).map_err(|error| format!("读取运行收据失败: {error}"))?;
    let mut summary = AgentRunReceiptSummary {
        run_id: run_id.to_string(),
        started_at: None,
        ended_at: None,
        title: None,
        provider: None,
        model: None,
        stage: None,
        outcome: "unknown".into(),
        error: None,
    };
    for line in body.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let ts = record.get("ts").and_then(Value::as_u64);
        if summary.started_at.is_none() {
            summary.started_at = ts;
        }
        if ts.is_some() {
            summary.ended_at = ts;
        }
        for key in ["title", "provider", "model"] {
            if let Some(value) = record.get(key).and_then(Value::as_str) {
                if !value.is_empty() {
                    summary_stage_set(&mut summary, key, value);
                }
            }
        }
        if let Some(stage) = record.get("stage").and_then(Value::as_str) {
            summary.stage = Some(stage.to_string());
        }
        match record.get("event").and_then(Value::as_str) {
            Some("failed") => {
                summary.outcome = "failed".into();
                summary.error = record
                    .get("error")
                    .and_then(Value::as_str)
                    .map(|error| truncate_text(error, 400));
            }
            Some("agent_settled") => summary.outcome = "settled".into(),
            Some("agent_stopped") => summary.outcome = "stopped".into(),
            _ => {}
        }
    }
    Ok(Some(summary))
}

fn summary_stage_set(summary: &mut AgentRunReceiptSummary, key: &str, value: &str) {
    match key {
        "title" => summary.title = Some(value.to_string()),
        "provider" => summary.provider = Some(value.to_string()),
        "model" => summary.model = Some(value.to_string()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Cursor,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn start_failure_classification_splits_dependency_from_fatal() {
        // 依赖缺失 → blocked（补齐 Pi / 模型配置后可重试）
        for message in [
            "未找到 Pi：请安装 Pi，或配置 STILLWRITE_PI_EXECUTABLE",
            "PATH 中的 pi 不存在或不可执行: /usr/bin/pi",
            "Pi 已启动但没有可用模型；请在 Pi 外部完成 provider/auth 配置",
        ] {
            assert!(
                start_failure_is_missing_dependency(message),
                "应识别为依赖缺失: {message}"
            );
        }
        // 其余启动失败 → failed
        for message in [
            "Pi 启动握手失败: timeout",
            "Pi 拒绝 Agent 请求: quota exceeded",
            "创建 Pi session 失败",
        ] {
            assert!(
                !start_failure_is_missing_dependency(message),
                "应识别为致命错误: {message}"
            );
        }
    }

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
        fs::write(&resources.system_prompt, SYSTEM_PROMPT).unwrap();
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
        let error = discover_launcher_with_home(None, Some(OsString::from("")), None).unwrap_err();
        assert!(error.contains("安装") || error.contains("STILLWRITE_PI_EXECUTABLE"));
    }

    #[test]
    #[cfg(unix)]
    fn launcher_falls_back_to_user_pi_node_install() {
        let root = temp_dir("user-pi-install");
        let home = root.join("home");
        let path_pi = home
            .join(".local")
            .join("share")
            .join("pi-node")
            .join("node-v22.23.1-linux-x64")
            .join("bin")
            .join("pi");
        fs::create_dir_all(path_pi.parent().unwrap()).unwrap();
        make_executable(&path_pi, "#!/bin/sh\n");
        let launcher =
            discover_launcher_with_home(None, Some(OsString::from("/no-such-path")), Some(&home))
                .unwrap();
        assert_eq!(launcher.executable, path_pi.canonicalize().unwrap());
        fs::remove_dir_all(root).unwrap();
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
    fn session_reference_accepts_logical_path_before_materialization() {
        let root = temp_dir("session-ref-logical");
        // Pi 把 session 文件延迟到首个 assistant response 才创建；
        // prompt 之前只存在预定的逻辑路径，必须被接受。
        let state = json!({ "sessionFile": "2026-08/session.jsonl" });
        assert_eq!(
            session_ref_from_state(&root, &state).unwrap(),
            Some("2026-08/session.jsonl".into())
        );
        // 已落盘的文件（含绝对路径）仍要做实体校验并归一化。
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested").join("real.jsonl"), "{}").unwrap();
        let state = json!({ "sessionFile": root.join("nested").join("real.jsonl") });
        assert_eq!(
            session_ref_from_state(&root, &state).unwrap(),
            Some("nested/real.jsonl".into())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn session_reference_rejects_missing_absolute_path_outside_root() {
        let root = temp_dir("session-ref-absent");
        let state = json!({ "sessionFile": "/tmp/no-such-sw-session-file.jsonl" });
        let error = session_ref_from_state(&root, &state).unwrap_err();
        assert!(error.contains("不在"), "unexpected error: {error}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn run_receipt_summary_extracts_stage_and_failure() {
        let dir = temp_dir("receipt-summary");
        let path = dir.join("run-failed-1.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"ts\":100,\"runId\":\"run-failed-1\",\"event\":\"stage\",\"stage\":\"PI_READY\"}\n",
                "{\"ts\":101,\"runId\":\"run-failed-1\",\"event\":\"stage\",\"stage\":\"SESSION_ALLOCATED\",\"provider\":\"openai\",\"model\":\"gpt\",\"title\":\"问答\"}\n",
                "{\"ts\":102,\"runId\":\"run-failed-1\",\"event\":\"failed\",\"stage\":\"PROMPT_SENT\",\"error\":\"等待 Pi RPC 响应超时\"}\n"
            ),
        )
        .unwrap();
        let summary = summarize_receipt(&path).unwrap().unwrap();
        assert_eq!(summary.run_id, "run-failed-1");
        assert_eq!(summary.outcome, "failed");
        assert_eq!(summary.stage.as_deref(), Some("PROMPT_SENT"));
        assert_eq!(summary.title.as_deref(), Some("问答"));
        assert_eq!(summary.provider.as_deref(), Some("openai"));
        assert_eq!(summary.model.as_deref(), Some("gpt"));
        assert_eq!(summary.started_at, Some(100));
        assert_eq!(summary.ended_at, Some(102));
        assert_eq!(summary.error.as_deref(), Some("等待 Pi RPC 响应超时"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn run_receipt_summary_reports_settled_runs() {
        let dir = temp_dir("receipt-settled");
        let path = dir.join("run-ok-1.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"ts\":10,\"runId\":\"run-ok-1\",\"event\":\"stage\",\"stage\":\"PROMPT_SENT\",\"title\":\"总结\"}\n",
                "{\"ts\":11,\"runId\":\"run-ok-1\",\"event\":\"agent_settled\",\"stage\":\"SETTLED\",\"textLength\":42}\n",
                "{\"ts\":12,\"runId\":\"run-ok-1\",\"event\":\"work_saved\",\"stage\":\"WORK_SAVED\"}\n"
            ),
        )
        .unwrap();
        let summary = summarize_receipt(&path).unwrap().unwrap();
        assert_eq!(summary.outcome, "settled");
        assert_eq!(summary.stage.as_deref(), Some("WORK_SAVED"));
        assert_eq!(summary.error, None);
        fs::remove_dir_all(dir).unwrap();
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
        fs::write(&resources.system_prompt, SYSTEM_PROMPT).unwrap();
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

    #[test]
    #[cfg(unix)]
    fn launcher_dir_is_prepended_to_child_path() {
        let root = temp_dir("env-path");
        let executable = root.join("bin").join("pi");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        let launcher = Launcher {
            executable,
            label: "fake".into(),
        };
        let path_value = launcher_env_path(&launcher);
        let mut parts = env::split_paths(&path_value);
        assert_eq!(parts.next().as_deref(), Some(root.join("bin").as_path()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn child_env_dirs_include_user_node_installs() {
        // 符号链接安装形态下，canonicalize 后的可执行文件在 node_modules
        // 深处；其所在目录必须仍在候选里，但 pi-node 的版本化 bin 目录
        // （带便携 node）与 ~/.local/bin 也必须被纳入，且新版本优先。
        let home = temp_dir("env-home");
        let node_bin = home.join(".local/share/pi-node/node-v22-fake-linux-x64/bin");
        let older_bin = home.join(".local/share/pi-node/node-v20-fake-linux-x64/bin");
        fs::create_dir_all(&node_bin).unwrap();
        fs::create_dir_all(&older_bin).unwrap();
        let exec_root = temp_dir("env-exec-dir");
        let executable = exec_root.join("deep").join("cli.js");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();

        let dirs = child_env_dirs(&executable, Some(&home));
        assert_eq!(dirs[0], executable.parent().unwrap());
        assert!(dirs.contains(&home.join(".local/bin")));
        let newest = dirs.iter().position(|dir| dir == &node_bin).unwrap();
        let older = dirs.iter().position(|dir| dir == &older_bin).unwrap();
        assert!(newest < older);

        let minimal = child_env_dirs(&executable, None);
        assert_eq!(minimal, vec![executable.parent().unwrap()]);

        fs::remove_dir_all(home).unwrap();
        fs::remove_dir_all(exec_root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn env_shebang_resolves_sibling_interpreter_via_child_path() {
        // 复刻 pi-node 用户目录安装形态：脚本与解释器同目录，
        // 脚本用 #!/usr/bin/env 找解释器。修复前 spawn 直接失败。
        let root = temp_dir("env-shebang");
        let bin = root.join("pi-bin");
        fs::create_dir_all(&bin).unwrap();
        make_executable(&bin.join("swfake-node"), "#!/bin/sh\nexec /bin/sh \"$@\"\n");
        make_executable(
            &bin.join("pi"),
            r#"#!/usr/bin/env swfake-node
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"type":"get_state"'*) printf '{"id":"%s","type":"response","command":"get_state","success":true,"data":{}}\n' "$id" ;;
  esac
done
"#,
        );
        let config = LaunchConfig {
            launcher: Launcher {
                executable: bin.join("pi"),
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
        fs::write(&resources.system_prompt, SYSTEM_PROMPT).unwrap();
        fs::create_dir_all(root.join("sessions")).unwrap();
        let events = Arc::new(Mutex::new(Vec::<Value>::new()));
        let events_sink = Arc::clone(&events);
        let sink: EventSink = Arc::new(move |event| events_sink.lock().unwrap().push(event));
        let process =
            spawn_process(&config, &root, &root.join("sessions"), &resources, sink).unwrap();
        let handshake = process
            .request(json!({ "type": "get_state" }), COMMAND_TIMEOUT)
            .expect("handshake with env-shebang interpreter should succeed");
        assert!(handshake.success);
        process.shutdown();
        fs::remove_dir_all(root).unwrap();
    }
}
