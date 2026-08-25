use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{Arc, Mutex},
    thread,
};
use tauri::State;

use crate::{workspace_root, AppState};

const PROFILE: &str = "ace-writing";
const MAX_PROCESS_OUTPUT: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
struct Launcher {
    executable: PathBuf,
    prefix_args: Vec<OsString>,
    label: String,
}

#[derive(Default)]
pub struct AgentProcessState {
    active_pid: Arc<Mutex<Option<u32>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProbeResponse {
    available: bool,
    launcher: String,
    profile: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnInput {
    prompt: String,
    session_id: String,
    message_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnResponse {
    status: String,
    text: String,
    channel_id: String,
    thread_id: String,
    conversation_id: String,
    run_id: Option<String>,
    receipt_ref: Option<String>,
}

#[derive(Serialize)]
struct TransportRequest<'a> {
    version: u8,
    prompt: &'a str,
    tenant_id: &'static str,
    user_id: &'static str,
    channel_id: &'a str,
    thread_id: &'a str,
    message_id: &'a str,
}

#[derive(Deserialize)]
struct TransportResponse {
    version: u8,
    status: String,
    text: String,
    surface: String,
    tenant_id: String,
    user_id: String,
    channel_id: String,
    thread_id: String,
    conversation_id: String,
    run_id: Option<String>,
    receipt_ref: Option<String>,
}

struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn configured_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
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
    project_root: Option<PathBuf>,
) -> Result<Launcher, String> {
    if let Some(path) = explicit_executable {
        let executable = canonical_executable(path, "STILLWRITE_ZUAEF_EXECUTABLE")?;
        return Ok(Launcher {
            label: executable.display().to_string(),
            executable,
            prefix_args: Vec::new(),
        });
    }

    if let Some(path) = executable_on_path("zuaef-agent", path_value.as_ref()) {
        let executable = canonical_executable(path, "PATH 中的 zuaef-agent")?;
        return Ok(Launcher {
            label: executable.display().to_string(),
            executable,
            prefix_args: Vec::new(),
        });
    }

    if let Some(root) = project_root {
        let root = root
            .canonicalize()
            .map_err(|error| format!("无法解析 STILLWRITE_ZUAEF_PROJECT_ROOT: {error}"))?;
        if !root.is_dir() {
            return Err(format!("zuaef project root 不是目录: {}", root.display()));
        }
        let uv = executable_on_path("uv", path_value.as_ref())
            .ok_or_else(|| "已配置 zuaef project root，但 PATH 中没有 uv".to_string())?;
        let executable = canonical_executable(uv, "PATH 中的 uv")?;
        return Ok(Launcher {
            label: format!("uv run --project {} zuaef-agent", root.display()),
            executable,
            prefix_args: vec![
                "run".into(),
                "--project".into(),
                root.into_os_string(),
                "zuaef-agent".into(),
            ],
        });
    }

    Err("未找到 zuaef-agent：请配置 STILLWRITE_ZUAEF_EXECUTABLE，加入 PATH，或配置 STILLWRITE_ZUAEF_PROJECT_ROOT".into())
}

fn launcher_from_env() -> Result<Launcher, String> {
    discover_launcher(
        configured_path("STILLWRITE_ZUAEF_EXECUTABLE"),
        env::var_os("PATH"),
        configured_path("STILLWRITE_ZUAEF_PROJECT_ROOT"),
    )
}

fn config_root_args() -> Result<Vec<OsString>, String> {
    let Some(path) = configured_path("STILLWRITE_ZUAEF_CONFIG_ROOT") else {
        return Ok(Vec::new());
    };
    let path = path
        .canonicalize()
        .map_err(|error| format!("无法解析 STILLWRITE_ZUAEF_CONFIG_ROOT: {error}"))?;
    if !path.is_dir() {
        return Err(format!("zuaef config root 不是目录: {}", path.display()));
    }
    Ok(vec!["--config-root".into(), path.into_os_string()])
}

fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取 Agent 进程输出失败: {error}"))?;
    if bytes.len() > limit {
        return Err(format!("Agent 进程输出超过 {limit} 字节限制"));
    }
    Ok(bytes)
}

fn run_process(
    launcher: &Launcher,
    args: &[OsString],
    stdin: Option<&[u8]>,
    active_pid: Option<Arc<Mutex<Option<u32>>>>,
) -> Result<ProcessOutput, String> {
    let mut command = Command::new(&launcher.executable);
    command
        .args(&launcher.prefix_args)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 zuaef-agent 失败: {error}"))?;
    let pid = child.id();
    if let Some(slot) = &active_pid {
        let mut active = slot
            .lock()
            .map_err(|_| "Agent 进程状态锁定失败".to_string())?;
        if active.is_some() {
            let _ = child.kill();
            return Err("已有 Agent 请求正在运行".into());
        }
        *active = Some(pid);
    }
    if let Some(bytes) = stdin {
        child
            .stdin
            .take()
            .ok_or_else(|| "无法打开 Agent 进程 stdin".to_string())?
            .write_all(bytes)
            .map_err(|error| format!("写入 Agent 请求失败: {error}"))?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Agent 进程 stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取 Agent 进程 stderr".to_string())?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout, MAX_PROCESS_OUTPUT));
    let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_PROCESS_OUTPUT));
    let status = child
        .wait()
        .map_err(|error| format!("等待 Agent 进程失败: {error}"));
    if let Some(slot) = &active_pid {
        if let Ok(mut active) = slot.lock() {
            if *active == Some(pid) {
                *active = None;
            }
        }
    }
    let status = status?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| "Agent stdout 读取线程异常".to_string())??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Agent stderr 读取线程异常".to_string())??;
    Ok(ProcessOutput {
        status,
        stdout,
        stderr,
    })
}

fn workspace_fingerprint(root: &Path) -> String {
    let digest = Sha256::digest(root.as_os_str().as_encoded_bytes());
    format!("ws-{digest:x}")
}

fn valid_transport_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn process_error(output: &ProcessOutput) -> String {
    let diagnostic = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if diagnostic.is_empty() {
        format!("zuaef-agent 退出状态: {}", output.status)
    } else {
        diagnostic
    }
}

#[tauri::command]
pub async fn agent_probe() -> Result<AgentProbeResponse, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let launcher = launcher_from_env()?;
        let mut args: Vec<OsString> = vec!["profile".into(), "check".into(), PROFILE.into()];
        args.extend(config_root_args()?);
        let output = run_process(&launcher, &args, None, None)?;
        if !output.status.success() {
            return Err(process_error(&output));
        }
        Ok(AgentProbeResponse {
            available: true,
            launcher: launcher.label,
            profile: PROFILE.into(),
        })
    })
    .await
    .map_err(|error| format!("Agent probe 任务异常: {error}"))?
}

#[tauri::command]
pub async fn agent_turn(
    state: State<'_, AppState>,
    process: State<'_, AgentProcessState>,
    input: AgentTurnInput,
) -> Result<AgentTurnResponse, String> {
    let prompt = input.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("Agent 指令不能为空".into());
    }
    if !valid_transport_id(&input.session_id) || !valid_transport_id(&input.message_id) {
        return Err("Agent session/message id 格式无效".into());
    }
    let root = workspace_root(&state)?;
    let active_pid = Arc::clone(&process.active_pid);
    tauri::async_runtime::spawn_blocking(move || {
        let launcher = launcher_from_env()?;
        let channel_id = workspace_fingerprint(&root);
        let request = TransportRequest {
            version: 1,
            prompt: &prompt,
            tenant_id: "local",
            user_id: "supervisor",
            channel_id: &channel_id,
            thread_id: &input.session_id,
            message_id: &input.message_id,
        };
        let request_bytes = serde_json::to_vec(&request)
            .map_err(|error| format!("编码 Agent 请求失败: {error}"))?;
        let mut args: Vec<OsString> = vec![
            "gateway".into(),
            "turn".into(),
            "--surface".into(),
            "stillwrite".into(),
            "--profile".into(),
            PROFILE.into(),
        ];
        args.extend(config_root_args()?);
        let output = run_process(&launcher, &args, Some(&request_bytes), Some(active_pid))?;
        if !output.status.success() {
            return Err(process_error(&output));
        }
        let stdout = std::str::from_utf8(&output.stdout)
            .map_err(|error| format!("Agent 协议响应不是 UTF-8: {error}"))?
            .trim();
        if stdout.lines().count() != 1 {
            return Err("Agent stdout 必须恰好包含一个协议响应".into());
        }
        let response: TransportResponse = serde_json::from_str(stdout)
            .map_err(|error| format!("解析 Agent 协议响应失败: {error}"))?;
        if response.version != 1
            || response.surface != "stillwrite"
            || response.tenant_id != "local"
            || response.user_id != "supervisor"
            || response.channel_id != channel_id
            || response.thread_id != input.session_id
        {
            return Err("Agent 协议响应的 routing identity 不匹配".into());
        }
        Ok(AgentTurnResponse {
            status: response.status,
            text: response.text,
            channel_id: response.channel_id,
            thread_id: response.thread_id,
            conversation_id: response.conversation_id,
            run_id: response.run_id,
            receipt_ref: response.receipt_ref,
        })
    })
    .await
    .map_err(|error| format!("Agent turn 任务异常: {error}"))?
}

#[tauri::command]
pub fn agent_cancel(process: State<'_, AgentProcessState>) -> Result<bool, String> {
    let pid = *process
        .active_pid
        .lock()
        .map_err(|_| "Agent 进程状态锁定失败".to_string())?;
    let Some(pid) = pid else {
        return Ok(false);
    };
    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        if result == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(false);
        }
        Err(format!("停止 Agent 本地进程失败: {error}"))
    }
    #[cfg(not(unix))]
    {
        Err("当前平台暂不支持停止 Agent 本地进程".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("stillwrite-{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn launcher_prefers_explicit_executable() {
        let root = temp_dir("explicit-launcher");
        let explicit = root.join("configured-agent");
        let path_agent = root.join("zuaef-agent");
        make_executable(&explicit);
        make_executable(&path_agent);

        let launcher = discover_launcher(
            Some(explicit.clone()),
            Some(env::join_paths([root.as_path()]).unwrap()),
            None,
        )
        .unwrap();

        assert_eq!(launcher.executable, explicit.canonicalize().unwrap());
        assert!(launcher.prefix_args.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn launcher_uses_uv_only_after_path_agent_is_absent() {
        let bin = temp_dir("uv-launcher-bin");
        let project = temp_dir("uv-launcher-project");
        let uv = bin.join("uv");
        make_executable(&uv);

        let launcher = discover_launcher(
            None,
            Some(env::join_paths([bin.as_path()]).unwrap()),
            Some(project.clone()),
        )
        .unwrap();

        assert_eq!(launcher.executable, uv.canonicalize().unwrap());
        assert_eq!(launcher.prefix_args[0], "run");
        assert_eq!(launcher.prefix_args[1], "--project");
        assert_eq!(launcher.prefix_args[2], project.canonicalize().unwrap());
        assert_eq!(launcher.prefix_args[3], "zuaef-agent");
        fs::remove_dir_all(bin).unwrap();
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn transport_ids_are_narrow() {
        assert!(valid_transport_id("gui-1234"));
        assert!(!valid_transport_id("../../escape"));
        assert!(!valid_transport_id("contains space"));
    }
}
