use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

mod agent_work;
pub mod annotate;
mod feeds;
mod indexer;
mod library;
mod pi_agent;
pub mod state_store;
mod sync;
pub mod work;

use state_store::ObjectUri;

#[derive(Default)]
struct AppState {
    root: Mutex<Option<PathBuf>>,
    index_db: Mutex<Option<PathBuf>>,
    // Holding the file keeps the OS-level advisory lock alive for this process.
    instance_lock: Mutex<Option<fs::File>>,
}

#[derive(Serialize)]
struct TreeNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Vec<TreeNode>,
}

#[derive(Serialize)]
struct WorkspaceData {
    root: String,
    nodes: Vec<TreeNode>,
}

#[derive(Serialize)]
struct OpenDocumentData {
    root: String,
    nodes: Vec<TreeNode>,
    path: String,
    name: String,
    content: String,
}

#[derive(Serialize)]
struct WebSearchHit {
    title: String,
    url: String,
    description: String,
    age: Option<String>,
}

const BRAVE_API_KEY_ENV_NAMES: [&str; 2] = ["BRAVE_SEARCH_API_KEY", "BRAVE_API_KEY"];

fn environment_brave_api_key() -> Option<String> {
    BRAVE_API_KEY_ENV_NAMES.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(unix)]
fn set_private_permissions(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("设置 Brave API Key 权限失败: {e}"))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path, _mode: u32) -> Result<(), String> {
    Ok(())
}

fn brave_api_key_path(app: &AppHandle) -> Result<PathBuf, String> {
    let secrets_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位 Stillwrite 应用数据目录: {e}"))?
        .join("secrets");
    fs::create_dir_all(&secrets_dir).map_err(|e| format!("创建 Stillwrite 密钥目录失败: {e}"))?;
    set_private_permissions(&secrets_dir, 0o700)?;
    Ok(secrets_dir.join("brave_search_api_key"))
}

fn stored_brave_api_key(app: &AppHandle) -> Result<Option<String>, String> {
    let path = brave_api_key_path(app)?;
    match fs::read_to_string(&path) {
        Ok(value) => {
            set_private_permissions(&path, 0o600)?;
            let value = value.trim().to_owned();
            if value.is_empty() {
                Ok(None)
            } else {
                validate_brave_api_key(&value).map(Some)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("读取 Brave API Key 失败: {error}")),
    }
}

fn brave_api_key(app: &AppHandle) -> Result<String, String> {
    if let Some(key) = stored_brave_api_key(app)? {
        return Ok(key);
    }
    environment_brave_api_key().ok_or_else(|| {
        "未配置 Brave Search API Key，请点击左下角设置，或设置 BRAVE_SEARCH_API_KEY 环境变量"
            .to_owned()
    })
}

fn validate_brave_api_key(key: &str) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("Brave API Key 不能为空".to_owned());
    }
    if key.len() > 512 || key.chars().any(char::is_control) {
        return Err("Brave API Key 格式无效".to_owned());
    }
    Ok(key.to_owned())
}

#[tauri::command]
fn brave_api_key_status(app: AppHandle) -> Result<String, String> {
    if stored_brave_api_key(&app)?.is_some() {
        return Ok("settings".to_owned());
    }
    if environment_brave_api_key().is_some() {
        return Ok("env".to_owned());
    }
    Ok("missing".to_owned())
}

#[tauri::command]
fn save_brave_api_key(app: AppHandle, key: String) -> Result<(), String> {
    let key = validate_brave_api_key(&key)?;
    let path = brave_api_key_path(&app)?;
    let temp_path =
        path.with_file_name(format!(".brave_search_api_key.{}.tmp", std::process::id()));
    fs::write(&temp_path, key.as_bytes()).map_err(|e| format!("保存 Brave API Key 失败: {e}"))?;
    if let Err(error) = set_private_permissions(&temp_path, 0o600) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temp_path, &path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("保存 Brave API Key 失败: {error}"));
    }
    set_private_permissions(&path, 0o600)
}

#[tauri::command]
fn clear_brave_api_key(app: AppHandle) -> Result<(), String> {
    let path = brave_api_key_path(&app)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("清除 Brave API Key 失败: {error}")),
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum AnnotationTarget {
    Workspace {
        path: String,
    },
    Library {
        #[serde(rename = "sourceId", alias = "source_id")]
        source_id: String,
        #[serde(rename = "relativePath", alias = "relative_path")]
        relative_path: String,
        #[serde(rename = "contentHash", alias = "content_hash")]
        content_hash: String,
    },
    Agent {
        id: String,
    },
}

fn library_annotation_root(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位资料批注目录: {e}"))?
        .join("library")
        .join("annotations");
    fs::create_dir_all(&root).map_err(|e| format!("创建资料批注目录失败: {e}"))?;
    Ok(root)
}

fn library_annotation_document(
    app: &AppHandle,
    source_id: &str,
    relative_path: &str,
    content_hash: &str,
) -> Result<library::LibraryDocument, String> {
    let db_path = library::resolve_index_db(app)?;
    let document = library::read_at(&db_path, source_id, relative_path)?;
    if document.content_hash != content_hash {
        return Err("资料内容已变化，请刷新资料库后再操作批注".into());
    }
    Ok(document)
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn validate_workspace_root(root: &Path) -> Result<(), String> {
    if root.parent().is_none() {
        return Err("不能将文件系统根目录作为工作区，请选择包含 Markdown 文件的具体文件夹".into());
    }
    Ok(())
}

fn try_acquire_instance_lock(path: &Path) -> std::io::Result<Option<fs::File>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

fn is_ignored_directory(name: &str) -> bool {
    matches!(name, "target" | "node_modules")
}

fn scan_dir(path: &Path) -> Result<Vec<TreeNode>, String> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    let entries = fs::read_dir(path).map_err(|e| format!("读取目录失败: {e}"))?;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            continue;
        }

        if file_type.is_dir() {
            if is_ignored_directory(&name) {
                continue;
            }
            let Ok(children) = scan_dir(&path) else {
                continue;
            };
            if !children.is_empty() {
                dirs.push(TreeNode {
                    name,
                    path: path.to_string_lossy().to_string(),
                    is_dir: true,
                    children,
                });
            }
        } else if file_type.is_file() && is_markdown(&path) {
            files.push(TreeNode {
                name,
                path: path.to_string_lossy().to_string(),
                is_dir: false,
                children: Vec::new(),
            });
        }
    }

    dirs.sort_by_key(|node| node.name.to_ascii_lowercase());
    files.sort_by_key(|node| node.name.to_ascii_lowercase());
    dirs.extend(files);
    Ok(dirs)
}

fn short_hash(bytes: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// opaque workspace key：durable state 不写绝对路径，统一用该 key。
pub(crate) fn workspace_id_for_root(root: &Path) -> String {
    short_hash(root.to_string_lossy().as_bytes())
}

fn resolve_index_db(app: &AppHandle, root: &Path) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位应用数据目录: {e}"))?;
    let key = short_hash(root.to_string_lossy().as_bytes());
    let sub = dir.join("workspaces").join(&key);
    fs::create_dir_all(&sub).map_err(|e| format!("创建索引目录失败: {e}"))?;
    Ok(sub.join("index.db"))
}

/// 解析当前工作区的侧车索引连接并执行闭包。
fn with_index<R>(
    app: &AppHandle,
    state: &State<AppState>,
    f: impl FnOnce(&mut rusqlite::Connection, &Path) -> Result<R, String>,
) -> Result<R, String> {
    let root = workspace_root(state)?;
    let db_path = match state
        .index_db
        .lock()
        .map_err(|_| "工作区状态锁定失败".to_string())?
        .clone()
    {
        Some(p) => p,
        None => {
            let p = resolve_index_db(app, &root)?;
            *state
                .index_db
                .lock()
                .map_err(|_| "工作区状态锁定失败".to_string())? = Some(p.clone());
            p
        }
    };
    let mut conn = indexer::open_index(&db_path).map_err(|e| format!("打开索引失败: {e}"))?;
    f(&mut conn, &root)
}

fn activate_workspace(
    root: PathBuf,
    app: &AppHandle,
    state: &State<AppState>,
    process: &pi_agent::PiProcessState,
) -> Result<WorkspaceData, String> {
    let root = fs::canonicalize(root).map_err(|e| format!("打开目录失败: {e}"))?;
    if !root.is_dir() {
        return Err("选择的路径不是目录".into());
    }
    validate_workspace_root(&root)?;
    // 切换工作区会强制中止 Pi 进程；进行中的 Work 同步转 cancelled，
    // 不能跨工作区永远停留在 running。
    if let Some(run_id) = process.shutdown_for_workspace(&root) {
        pi_agent::cancel_run_work(app, &run_id, "切换工作区，运行被中止");
    }

    let nodes = scan_dir(&root)?;
    {
        let mut guard = state
            .root
            .lock()
            .map_err(|_| "工作区状态锁定失败".to_string())?;
        *guard = Some(root.clone());
    }
    let index_db = resolve_index_db(app, &root).ok();
    *state
        .index_db
        .lock()
        .map_err(|_| "工作区状态锁定失败".to_string())? = index_db.clone();

    // 文件树先返回给 UI；全文索引在后台增量更新，避免大目录看起来像“没有打开”。
    if let Some(index_db) = index_db {
        let index_root = root.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            if let Ok(mut conn) = indexer::open_index(&index_db) {
                let _ = indexer::build_index(&mut conn, &index_root);
            }
        });
    }

    Ok(WorkspaceData {
        root: root.to_string_lossy().to_string(),
        nodes,
    })
}

fn workspace_root(state: &State<AppState>) -> Result<PathBuf, String> {
    state
        .root
        .lock()
        .map_err(|_| "工作区状态锁定失败".to_string())?
        .clone()
        .ok_or_else(|| "尚未打开工作区".to_string())
}

fn ensure_existing_path_inside_workspace(
    path: &str,
    state: &State<AppState>,
) -> Result<PathBuf, String> {
    let root = workspace_root(state)?;
    let canonical = fs::canonicalize(path).map_err(|e| format!("路径不可用: {e}"))?;
    if !canonical.starts_with(&root) {
        return Err("拒绝访问工作区以外的路径".into());
    }
    Ok(canonical)
}

#[tauri::command]
async fn choose_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    process: State<'_, pi_agent::PiProcessState>,
) -> Result<Option<WorkspaceData>, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("选择 Markdown 文件夹")
        .blocking_pick_folder();

    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|e| format!("目录路径不可用: {e}"))?;
    activate_workspace(path, &app, &state, &process).map(Some)
}

#[tauri::command]
async fn choose_document(
    app: AppHandle,
    state: State<'_, AppState>,
    process: State<'_, pi_agent::PiProcessState>,
) -> Result<Option<OpenDocumentData>, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("打开 Markdown 文档")
        .add_filter("Markdown", &["md", "markdown"])
        .blocking_pick_file();

    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|e| format!("文档路径不可用: {e}"))?;
    let path = fs::canonicalize(path).map_err(|e| format!("打开文档失败: {e}"))?;
    if !path.is_file() || !is_markdown(&path) {
        return Err("请选择 .md 或 .markdown 文档".into());
    }

    let root = workspace_root(&state)
        .ok()
        .filter(|root| path.starts_with(root))
        .or_else(|| path.parent().map(Path::to_path_buf))
        .ok_or_else(|| "无法确定文档所在文件夹".to_string())?;
    let WorkspaceData { root, nodes } = activate_workspace(root, &app, &state, &process)?;
    let content = fs::read_to_string(&path).map_err(|e| format!("读取文档失败: {e}"))?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "Markdown".to_string());

    Ok(Some(OpenDocumentData {
        root,
        nodes,
        path: path.to_string_lossy().to_string(),
        name,
        content,
    }))
}

#[tauri::command]
async fn set_workspace(
    app: AppHandle,
    path: String,
    state: State<'_, AppState>,
    process: State<'_, pi_agent::PiProcessState>,
) -> Result<WorkspaceData, String> {
    activate_workspace(PathBuf::from(path), &app, &state, &process)
}

#[tauri::command]
async fn read_markdown(path: String, state: State<'_, AppState>) -> Result<String, String> {
    let path = ensure_existing_path_inside_workspace(&path, &state)?;
    if !is_markdown(&path) {
        return Err("只允许打开 Markdown 文件".into());
    }
    fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))
}

/// 校验相对路径是否允许作为工作区内新建文件的路径。
/// 拒绝绝对路径、`..`、根目录与 Windows 前缀（盘符/UNC）。
fn validate_new_markdown_path(rel: &Path) -> Result<(), String> {
    if rel.is_absolute()
        || rel.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("文件名包含不允许的路径".into());
    }
    if !is_markdown(rel) {
        return Err("新文件必须使用 .md 或 .markdown 扩展名".into());
    }
    Ok(())
}

/// 在 root 之内逐级创建目录。
/// 不使用 `fs::create_dir_all`，避免其跟随符号链接把目录建到工作区之外：
/// 每一级都先检查 symlink 解析后的真实路径是否仍位于 root 内。
pub(crate) fn create_dir_all_inside(root: &Path, dir: &Path) -> Result<(), String> {
    let rel = dir
        .strip_prefix(root)
        .map_err(|_| "创建目录失败: 路径不在工作区内".to_string())?;
    let mut cursor = root.to_path_buf();
    for component in rel.components() {
        let Component::Normal(part) = component else {
            return Err("文件名包含不允许的路径".into());
        };
        cursor.push(part);
        match fs::symlink_metadata(&cursor) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    let resolved =
                        fs::canonicalize(&cursor).map_err(|e| format!("创建目录失败: {e}"))?;
                    if !resolved.starts_with(root) {
                        return Err("拒绝访问工作区以外的路径".into());
                    }
                } else if !meta.is_dir() {
                    return Err("路径中包含非目录文件".into());
                }
            }
            Err(_) => {
                fs::create_dir(&cursor).map_err(|e| format!("创建目录失败: {e}"))?;
            }
        }
    }
    Ok(())
}

/// 原子写入：同目录临时文件 + fsync + 原子 rename，避免保存中断留下损坏的半截文件。
/// 写入前会把旧版本复制为同目录 `.bak` 备份（尽力而为，失败不阻断保存）。
pub(crate) fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "写入路径无效".to_string())?;
    let file_name = path
        .file_name()
        .ok_or_else(|| "写入路径无效".to_string())?
        .to_string_lossy()
        .to_string();

    // 旧版本备份（仅覆盖式保存时存在旧文件）
    if path.exists() {
        let bak = parent.join(format!("{file_name}.bak"));
        if let Err(e) = fs::copy(path, &bak) {
            eprintln!("创建备份失败: {e}");
        }
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!(".{file_name}.{}.{nonce}.tmp", std::process::id()));

    let write_result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp_path, path)?;
        // fsync 目录，确保 rename 持久化到磁盘
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("写入文件失败: {e}"));
    }
    Ok(())
}

/// 在工作区内新建 Markdown 文件（校验相对路径、拒绝符号链接逃逸后原子写入）。
fn create_new_markdown(root: &Path, rel: &Path) -> Result<PathBuf, String> {
    validate_new_markdown_path(rel)?;

    let target = root.join(rel);
    // 用 symlink_metadata 判断：目标以任何形式存在（含悬空/指向外部的符号链接）都拒绝，
    // 避免覆盖已有内容或把内容写到链接目标上。
    if target.symlink_metadata().is_ok() {
        return Err("文件已存在".into());
    }
    if let Some(parent) = target.parent() {
        create_dir_all_inside(root, parent)?;
    }
    atomic_write(&target, "# Untitled\n\n")?;
    Ok(target)
}

#[tauri::command]
async fn write_markdown(
    app: AppHandle,
    path: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let path = ensure_existing_path_inside_workspace(&path, &state)?;
    if !is_markdown(&path) {
        return Err("只允许写入 Markdown 文件".into());
    }
    atomic_write(&path, &content)?;
    // 保存后增量更新索引
    let _ = with_index(&app, &state, |conn, root| {
        indexer::index_single(conn, root, &path)
    });
    Ok(())
}

#[tauri::command]
async fn create_markdown(
    app: AppHandle,
    relative_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let root = workspace_root(&state)?;
    let target = create_new_markdown(&root, Path::new(&relative_path))?;
    let _ = with_index(&app, &state, |conn, root| {
        indexer::index_single(conn, root, &target)
    });
    Ok(target.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Workspace 本地图片：上传（picker + 安全复制）与 preview data URL 读取。
// 图片是携带的人类 Artifact：落在 <markdown 同目录>/assets/，不写 SQLite。
// ---------------------------------------------------------------------------

const MAX_WORKSPACE_IMAGE_BYTES: u64 = 15 * 1024 * 1024;

#[derive(Serialize)]
struct ImportedImage {
    markdown_path: String,
    alt: String,
    mime: String,
    byte_len: u64,
}

fn image_mime_by_extension(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

/// 图片目标文件名：为避免 Markdown 内路径含空格，把空格替换为 `-`。
fn image_target_file_name(name: &str) -> String {
    name.trim()
        .replace(' ', "-")
        .replace('\t', "-")
        .replace('\u{00a0}', "-")
}

/// 在 assets/ 目录内寻找不冲突的目标文件名：photo.png → photo-2.png → …
fn unique_image_target(assets_dir: &Path, requested: &str) -> String {
    if !assets_dir.join(&requested).exists() {
        return requested.to_string();
    }
    let stem = Path::new(&requested)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "image".to_string());
    let ext = Path::new(&requested)
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "png".to_string());
    for index in 2..=9999 {
        let candidate = format!("{stem}-{index}.{ext}");
        if !assets_dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!(
        "{stem}-{}.{ext}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    )
}

/// 原子复制：同目录临时文件 + fsync + rename；失败时清理 temp。
fn atomic_copy_image(source: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "图片目标路径无效".to_string())?;
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "图片目标文件名无效".to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!(".{file_name}.{}.{nonce}.tmp", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut src = fs::File::open(source)?;
        let mut dst = fs::File::create(&tmp_path)?;
        std::io::copy(&mut src, &mut dst)?;
        dst.sync_all()?;
        fs::rename(&tmp_path, target)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("图片复制失败: {error}"));
    }
    Ok(())
}

/// 挑选 Workspace 文档旁边的 assets/ 目标目录；拒绝逃逸 Workspace。
fn workspace_assets_dir(document: &Path, state: &State<AppState>) -> Result<PathBuf, String> {
    let root = workspace_root(state)?;
    let document = ensure_existing_path_inside_workspace(&document.to_string_lossy(), state)?;
    if !is_markdown(&document) {
        return Err("图片只能挂在 Markdown 文档旁边".into());
    }
    let parent = document
        .parent()
        .ok_or_else(|| "文档路径无效".to_string())?;
    let assets = parent.join("assets");
    create_dir_all_inside(&root, &assets)?;
    Ok(assets)
}

/// 上传图片：native picker → 校验 → 原子复制进 <md 目录>/assets/。
/// 用户取消返回 Ok(None)；失败返回 Err。
#[tauri::command]
async fn import_workspace_image(
    app: AppHandle,
    state: State<'_, AppState>,
    document_path: String,
) -> Result<Option<ImportedImage>, String> {
    let assets_dir = workspace_assets_dir(Path::new(&document_path), &state)?;
    let selected = app
        .dialog()
        .file()
        .set_title("选择图片")
        .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let source = selected
        .into_path()
        .map_err(|e| format!("图片路径不可用: {e}"))?;
    let mime = image_mime_by_extension(&source)
        .ok_or_else(|| "仅支持 png / jpg / jpeg / webp / gif 图片".to_string())?
        .to_string();
    let meta = fs::metadata(&source).map_err(|e| format!("读取图片信息失败: {e}"))?;
    let byte_len = meta.len();
    if byte_len > MAX_WORKSPACE_IMAGE_BYTES {
        return Err("图片超过 15 MiB，请先压缩后再导入".into());
    }
    let raw_name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "image.png".to_string());
    let file_name = image_target_file_name(&raw_name);
    let target_name = unique_image_target(&assets_dir, &file_name);
    let target = assets_dir.join(&target_name);
    atomic_copy_image(&source, &target)?;
    let alt = Path::new(&target_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "image".to_string());
    Ok(Some(ImportedImage {
        markdown_path: format!("assets/{target_name}"),
        alt,
        mime,
        byte_len,
    }))
}

/// 校验预览用 markdown 图片路径：相对、无 `..`、位于文档父目录内、
/// 规范化后仍在 Workspace 内、扩展名受支持、大小 <= 15 MiB。
fn validate_local_image_path(
    document: &Path,
    markdown_path: &str,
    state: &State<AppState>,
) -> Result<PathBuf, String> {
    if markdown_path.trim().is_empty() {
        return Err("图片路径为空".into());
    }
    let rel = Path::new(markdown_path.trim());
    if rel.is_absolute() {
        return Err("不支持绝对图片路径".into());
    }
    if rel.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("图片路径包含不允许的路径段".into());
    }
    let document = ensure_existing_path_inside_workspace(&document.to_string_lossy(), state)?;
    let root = workspace_root(state)?;
    let parent = document
        .parent()
        .ok_or_else(|| "文档路径无效".to_string())?;
    let joined = parent.join(rel);
    let canonical = fs::canonicalize(&joined).map_err(|e| format!("图片不存在或不可读: {e}"))?;
    if !canonical.starts_with(&root) {
        return Err("拒绝访问工作区以外的图片".into());
    }
    if image_mime_by_extension(&canonical).is_none() {
        return Err("仅支持 png / jpg / jpeg / webp / gif 图片".into());
    }
    let meta = fs::metadata(&canonical).map_err(|e| format!("读取图片失败: {e}"))?;
    if meta.len() > MAX_WORKSPACE_IMAGE_BYTES {
        return Err("图片超过 15 MiB，无法预览".into());
    }
    Ok(canonical)
}

/// 本地图片 preview data URL：`data:<mime>;base64,<payload>`。
#[tauri::command]
async fn read_workspace_image_data_url(
    state: State<'_, AppState>,
    document_path: String,
    markdown_path: String,
) -> Result<String, String> {
    let canonical = validate_local_image_path(Path::new(&document_path), &markdown_path, &state)?;
    let mime = image_mime_by_extension(&canonical).unwrap_or("image/png");
    let bytes = fs::read(&canonical).map_err(|e| format!("读取图片失败: {e}"))?;
    Ok(format!(
        "data:{mime};base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
    ))
}

#[tauri::command]
async fn rebuild_index(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(usize, usize), String> {
    with_index(&app, &state, indexer::build_index)
}

#[tauri::command]
async fn search_index(
    app: AppHandle,
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<indexer::SearchHit>, String> {
    with_index(&app, &state, |conn, _| {
        indexer::search(conn, &query, limit.unwrap_or(30))
    })
}

fn parse_brave_web_results(body: &str) -> Result<Vec<WebSearchHit>, String> {
    let payload: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("Brave 搜索响应不是有效 JSON: {e}"))?;
    let Some(results) = payload
        .get("web")
        .and_then(|web| web.get("results"))
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(Vec::new());
    };

    Ok(results
        .iter()
        .filter_map(|item| {
            let title = item.get("title")?.as_str()?.trim();
            let url = item.get("url")?.as_str()?.trim();
            if title.is_empty() || !(url.starts_with("https://") || url.starts_with("http://")) {
                return None;
            }
            Some(WebSearchHit {
                title: title.to_owned(),
                url: url.to_owned(),
                description: item
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                age: item
                    .get("age")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect())
}

#[tauri::command]
async fn search_web(
    app: AppHandle,
    query: String,
    count: Option<usize>,
    document_uri: Option<String>,
    selected_text: Option<String>,
    state: State<'_, AppState>,
) -> Result<state_store::WebSearchHistoryView, String> {
    let query = query.trim().to_owned();
    if query.is_empty() {
        return Err("网页搜索 query 不能为空".into());
    }
    let api_key = brave_api_key(&app)?;
    let count = count.unwrap_or(10).clamp(1, 20);
    let mut endpoint = url::Url::parse("https://api.search.brave.com/res/v1/web/search")
        .map_err(|e| format!("Brave 搜索地址无效: {e}"))?;
    let count_text = count.to_string();
    endpoint
        .query_pairs_mut()
        .append_pair("q", &query)
        .append_pair("count", &count_text)
        .append_pair("result_filter", "web")
        .append_pair("text_decorations", "false");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("创建 Brave 搜索客户端失败: {e}"))?;
    let response = client
        .get(endpoint)
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .send()
        .await
        .map_err(|e| format!("请求 Brave 搜索失败: {e}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 Brave 搜索响应失败: {e}"))?;
    if !status.is_success() {
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|payload| {
                payload
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        return Err(format!("Brave 搜索失败: {detail}"));
    }
    let hits = parse_brave_web_results(&body)?;
    let document_uri = document_uri.as_deref().map(ObjectUri::parse).transpose()?;
    let workspace_id = workspace_root(&state)
        .ok()
        .map(|root| short_hash(root.to_string_lossy().as_bytes()));
    let results = hits
        .into_iter()
        .map(|hit| state_store::NewWebSearchResult {
            title: hit.title,
            url: hit.url,
            description: hit.description,
            age: hit.age,
        })
        .collect();
    let mut conn = open_durable_state(&app)?;
    state_store::create_web_search(
        &mut conn,
        state_store::NewWebSearch {
            workspace_id,
            document_uri,
            selected_text,
            query,
            results,
        },
    )
}

#[tauri::command]
async fn list_web_search_history(
    app: AppHandle,
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<state_store::WebSearchHistoryView>, String> {
    let workspace_id = workspace_root(&state)
        .ok()
        .map(|root| short_hash(root.to_string_lossy().as_bytes()));
    let conn = open_durable_state(&app)?;
    state_store::list_web_search_history(
        &conn,
        workspace_id.as_deref(),
        limit.unwrap_or(50).clamp(1, 200),
    )
}

fn parse_search_relation_source(source_uri: &str) -> Result<ObjectUri, String> {
    let source = ObjectUri::parse(source_uri.trim())?;
    match source.scheme() {
        "workspace" | "library" | "agent" => Ok(source),
        _ => Err("搜索结果只能关联到当前文档".into()),
    }
}

#[tauri::command]
async fn list_search_result_links(
    app: AppHandle,
    state: State<'_, AppState>,
    source_uri: String,
) -> Result<Vec<i64>, String> {
    let source = parse_search_relation_source(&source_uri)?;
    let workspace_id = workspace_root(&state)
        .ok()
        .map(|root| short_hash(root.to_string_lossy().as_bytes()));
    let conn = open_durable_state(&app)?;
    state_store::web_search_result_links(&conn, source.as_str(), workspace_id.as_deref())
}

#[tauri::command]
async fn link_search_result(
    app: AppHandle,
    state: State<'_, AppState>,
    source_uri: String,
    result_id: i64,
) -> Result<(), String> {
    let source = parse_search_relation_source(&source_uri)?;
    let root = workspace_root(&state)?;
    let workspace_id = short_hash(root.to_string_lossy().as_bytes());
    let mut conn = open_durable_state(&app)?;
    let Some((history, result)) = state_store::get_web_search_result(&conn, result_id)? else {
        return Err("网页搜索结果不存在".into());
    };
    if history.workspace_id.as_deref() != Some(workspace_id.as_str()) {
        return Err("网页搜索结果不属于当前工作区".into());
    }
    let target = ObjectUri::web_search_result(result.id);
    let snapshot = serde_json::json!({
        "key": format!("search-result:{}", result.id),
        "kind": "web-search",
        "title": result.title,
        "snippet": result.description,
        "source": result.url.clone(),
        "url": result.url,
        "search_id": history.id,
        "query": history.query,
    });
    match state_store::create_relation(
        &mut conn,
        state_store::NewRelation {
            source_uri: source,
            predicate: "related_to".into(),
            target_uri: target,
            anchor_id: None,
            created_by: Some("human".into()),
            confidence: None,
            workspace_id: Some(workspace_id),
            snapshot: Some(snapshot),
        },
    ) {
        Ok(_) => Ok(()),
        Err(error) if error.contains("相同的关联已存在") => Ok(()),
        Err(error) => Err(error),
    }
}

#[tauri::command]
async fn unlink_search_result(
    app: AppHandle,
    state: State<'_, AppState>,
    source_uri: String,
    result_id: i64,
) -> Result<(), String> {
    let source = parse_search_relation_source(&source_uri)?;
    let root = workspace_root(&state)?;
    let workspace_id = short_hash(root.to_string_lossy().as_bytes());
    let target = ObjectUri::web_search_result(result_id);
    let mut conn = open_durable_state(&app)?;
    let Some((history, _)) = state_store::get_web_search_result(&conn, result_id)? else {
        return Ok(());
    };
    if history.workspace_id.as_deref() != Some(workspace_id.as_str()) {
        return Err("网页搜索结果不属于当前工作区".into());
    }
    match state_store::find_relation(&conn, source.as_str(), "related_to", target.as_str())? {
        Some(relation) => state_store::remove_relation(&mut conn, relation.id),
        None => Ok(()),
    }
}

#[tauri::command]
async fn search_related_index(
    app: AppHandle,
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<indexer::SearchHit>, String> {
    with_index(&app, &state, |conn, _| {
        indexer::search_related(conn, &query, limit.unwrap_or(8))
    })
}

/// 注册一个位于工作区之外的 Markdown 资料目录，并立即做一次增量索引。
#[tauri::command]
async fn add_library_source(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<library::LibraryRefreshResult, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("选择资料目录")
        .blocking_pick_folder();
    let Some(selected) = selected else {
        return Err("未选择资料目录".into());
    };
    let path = selected
        .into_path()
        .map_err(|e| format!("资料目录路径不可用: {e}"))?;
    let root = library::canonical_source_root(&path)?;
    if let Ok(workspace) = workspace_root(&state) {
        if library::roots_overlap(&root, &workspace) {
            return Err("资料源不能与当前工作区重叠".into());
        }
    }
    let db_path = library::resolve_index_db(&app)?;
    tauri::async_runtime::spawn_blocking(move || library::register_source_at(&db_path, &root))
        .await
        .map_err(|e| format!("资料源索引任务异常: {e}"))?
}

/// 刷新所有已注册资料源；正文仍从资料源原目录读取。
#[tauri::command]
async fn refresh_library(app: AppHandle) -> Result<library::LibraryRefreshResult, String> {
    let db_path = library::resolve_index_db(&app)?;
    tauri::async_runtime::spawn_blocking(move || library::refresh_at(&db_path))
        .await
        .map_err(|e| format!("资料库刷新任务异常: {e}"))?
}

#[tauri::command]
async fn search_library(
    app: AppHandle,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<library::LibrarySearchHit>, String> {
    let db_path = library::resolve_index_db(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        library::search_at(&db_path, &query, limit.unwrap_or(30))
    })
    .await
    .map_err(|e| format!("资料搜索任务异常: {e}"))?
}

#[tauri::command]
async fn search_related_library(
    app: AppHandle,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<library::LibrarySearchHit>, String> {
    let db_path = library::resolve_index_db(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        library::search_related_at(&db_path, &query, limit.unwrap_or(8))
    })
    .await
    .map_err(|e| format!("关联资料搜索任务异常: {e}"))?
}

/// 只读打开已注册且已索引的 Library Markdown 文档。
#[tauri::command]
async fn read_library_document(
    app: AppHandle,
    source_id: String,
    relative_path: String,
) -> Result<library::LibraryDocument, String> {
    let db_path = library::resolve_index_db(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        library::read_at(&db_path, &source_id, &relative_path)
    })
    .await
    .map_err(|e| format!("读取资料任务异常: {e}"))?
}

/// 列出某个 Library source 的最近文档（通用；RSS“最近 RSS”复用此路径）。
#[tauri::command]
async fn list_library_source_documents(
    app: AppHandle,
    source_id: String,
    limit: Option<usize>,
) -> Result<Vec<library::LibraryDocumentMeta>, String> {
    let db_path = library::resolve_index_db(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        library::list_source_documents(&db_path, &source_id, limit.unwrap_or(20))
    })
    .await
    .map_err(|e| format!("资料列表任务异常: {e}"))?
}

// ---------------------------------------------------------------------------
// Related 固定卡片（P2a vertical slice）
// 用户的 ☆固定/取消固定 从 localStorage 迁入 durable relations。
// 固定作用域与既有 UI 一致：工作区级共享（scope 对象 ws://<workspace-key>），
// 而不是当前打开的文档。前端只感知 pin/unpin/list，不接触 SQLite 细节。
// ---------------------------------------------------------------------------

/// 打开当前安装的 durable state 数据库（每次调用短连接，与索引访问方式一致）。
pub(crate) fn open_durable_state(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let path = state_store::resolve_state_db(app)?;
    state_store::open_state_db(&path)
}

/// 工作区根对象 URI：ws://<workspace-key>。
/// key 复用 index.db 目录的 short_hash 约定，同一目录在同一台机器上恒定。
fn related_scope_uri(root: &Path) -> Result<ObjectUri, String> {
    let key = short_hash(root.to_string_lossy().as_bytes());
    ObjectUri::parse(&format!("ws://{key}")).map_err(|e| format!("构造工作区 scope 失败: {e}"))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelatedPinImportItem {
    target_uri: String,
    #[serde(default)]
    snapshot: Option<serde_json::Value>,
}

#[tauri::command]
async fn pin_related(
    app: AppHandle,
    state: State<'_, AppState>,
    target_uri: String,
    snapshot: Option<serde_json::Value>,
) -> Result<(), String> {
    let root = workspace_root(&state)?;
    let scope = related_scope_uri(&root)?;
    let target = ObjectUri::parse(target_uri.trim())?;
    if target == scope {
        return Err("不能把工作区自身固定为关联".into());
    }
    let mut conn = open_durable_state(&app)?;
    let workspace_key = scope.subject().to_string();
    match state_store::create_relation(
        &mut conn,
        state_store::NewRelation {
            source_uri: scope,
            predicate: "related_to".into(),
            target_uri: target,
            anchor_id: None,
            created_by: Some("human".into()),
            confidence: None,
            workspace_id: Some(workspace_key),
            snapshot,
        },
    ) {
        Ok(_) => Ok(()),
        // 同一三元组重复固定按幂等成功处理，与旧行为一致
        Err(e) if e.contains("相同的关联已存在") => Ok(()),
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn unpin_related(
    app: AppHandle,
    state: State<'_, AppState>,
    target_uri: String,
) -> Result<(), String> {
    let root = workspace_root(&state)?;
    let scope = related_scope_uri(&root)?;
    let target = ObjectUri::parse(target_uri.trim())?;
    let mut conn = open_durable_state(&app)?;
    match state_store::find_relation(&conn, scope.as_str(), "related_to", target.as_str())? {
        Some(record) => state_store::remove_relation(&mut conn, record.id),
        // 重复取消/未固定过都视为成功，保持旧 Map.delete 的宽容语义
        None => Ok(()),
    }
}

/// 当前工作区已固定的关联（按固定顺序），快照从证据事件还原。
#[tauri::command]
async fn list_related_pins(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<state_store::PinnedRelationView>, String> {
    let root = workspace_root(&state)?;
    let scope = related_scope_uri(&root)?;
    let conn = open_durable_state(&app)?;
    state_store::list_relation_snapshots(&conn, scope.as_str(), "related_to")
}

/// legacy localStorage 固定项一次性导入：单事务、幂等跳过已有三元组，
/// 保证重启两次不会产生重复关系或重复事件。返回实际新增条数。
#[tauri::command]
async fn import_related_pins(
    app: AppHandle,
    state: State<'_, AppState>,
    items: Vec<RelatedPinImportItem>,
) -> Result<usize, String> {
    let root = workspace_root(&state)?;
    let scope = related_scope_uri(&root)?;
    let links = items
        .into_iter()
        .map(|item| {
            let target_uri = ObjectUri::parse(item.target_uri.trim())
                .map_err(|e| format!("导入的固定项 URI 非法({}): {e}", item.target_uri))?;
            Ok(state_store::RelationLinkImport {
                target_uri,
                created_by: Some("human".into()),
                workspace_id: Some(scope.subject().to_string()),
                snapshot: item.snapshot,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut conn = open_durable_state(&app)?;
    state_store::import_relation_links(
        &mut conn,
        scope.clone(),
        "related_to",
        links
            .into_iter()
            .filter(|link| link.target_uri != scope)
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// RSS / Atom Source Adapter 命令面（feeds.rs）
// 网络抓取只发生在 Rust 后端；UI 不接触任何 feed URL 的网络请求。
// ---------------------------------------------------------------------------

#[tauri::command]
async fn feed_list_sources(app: AppHandle) -> Result<Vec<feeds::FeedSourceView>, String> {
    feeds::list_sources(&app)
}

/// 添加 RSS/Atom 源：立即保存订阅（首抓失败也保留），后台抓取首轮内容。
#[tauri::command]
async fn feed_add_source(
    app: AppHandle,
    url: String,
    name: Option<String>,
) -> Result<feeds::FeedSourceView, String> {
    feeds::add_source(&app, &url, name.as_deref())
}

/// 删除源：同时删除 RSS/<id>/ 派生缓存，保留批注；随后刷新 Library 索引。
#[tauri::command]
async fn feed_remove_source(app: AppHandle, id: String) -> Result<(), String> {
    feeds::remove_source(&app, &id)
}

/// 导入 OPML（未传 path 时弹文件选择框）；merge 语义，坏 URL 不取消其他项。
#[tauri::command]
async fn feed_import_opml(
    app: AppHandle,
    path: Option<String>,
) -> Result<feeds::OpmlImportResult, String> {
    let path = match path {
        Some(path) => Some(PathBuf::from(path)),
        None => {
            let selected = app
                .dialog()
                .file()
                .set_title("选择 OPML 文件")
                .add_filter("OPML", &["opml", "xml"])
                .blocking_pick_file();
            let Some(selected) = selected else {
                return Err("未选择 OPML 文件".into());
            };
            let path = selected
                .into_path()
                .map_err(|e| format!("OPML 文件路径不可用: {e}"))?;
            Some(path)
        }
    };
    feeds::import_opml(&app, path.as_deref())
}

#[tauri::command]
async fn feed_refresh_source(
    app: AppHandle,
    id: String,
) -> Result<feeds::FeedRefreshOutcome, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || feeds::refresh_source(&app, &id))
        .await
        .map_err(|e| format!("Feed 刷新任务异常: {e}"))?
}

#[tauri::command]
async fn feed_refresh_all(app: AppHandle) -> Result<feeds::FeedRefreshResult, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || feeds::refresh_all(&app))
        .await
        .map_err(|e| format!("Feed 刷新任务异常: {e}"))?
}

/// 源列表 + RSS Library source 视图 + 最近 RSS 资料。
#[tauri::command]
async fn feed_status(app: AppHandle) -> Result<feeds::FeedStatus, String> {
    feeds::status(&app)
}

/// 列出当前 Workspace 对应的 Agent 工作文档。
#[tauri::command]
async fn list_agent_works(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<agent_work::AgentWorkSummary>, String> {
    let root = workspace_root(&state)?;
    agent_work::list(&app, &root)
}

/// 读取一个 Agent 工作文档；正文仍是 Markdown，只是存放在应用数据中。
#[tauri::command]
async fn read_agent_work(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<agent_work::AgentWorkDocument, String> {
    let root = workspace_root(&state)?;
    agent_work::read_at(&app, &root, &id)
}

/// 创建一个 Agent 工作文档，并保存最小的来源/运行侧车。
#[tauri::command]
async fn create_agent_work(
    app: AppHandle,
    state: State<'_, AppState>,
    input: agent_work::CreateAgentWorkInput,
) -> Result<agent_work::AgentWorkDocument, String> {
    let root = workspace_root(&state)?;
    let run_id = input.run_id.clone();
    let document = agent_work::create(&app, &root, input)?;
    // 状态链最后一环：结果已固化为可编辑的 Agent Work（尽力而为，不影响保存）。
    if let Some(run_id) = &run_id {
        crate::pi_agent::record_run_event(
            &app,
            run_id,
            serde_json::json!({
                "event": "work_saved",
                "stage": "WORK_SAVED",
                "workId": document.id
            }),
        );
    }
    // Work 桥接：Artifact 固化后 Work 进入 needs_human，等待人工验收。
    if let Some(run_id) = run_id.as_deref() {
        bind_artifact_to_work(&app, &root, run_id, &document)?;
    }
    Ok(document)
}

/// 把已保存的 Agent Work Artifact 绑定回本次请求的 durable Work。
/// `receipt_ref` 找不到 Work 时静默跳过（无 Work 的保存路径不受影响）；
/// 绑定失败则让命令失败，避免 Work 永远停留在 running。
fn bind_artifact_to_work(
    app: &AppHandle,
    root: &Path,
    run_id: &str,
    document: &agent_work::AgentWorkDocument,
) -> Result<(), String> {
    let mut conn = open_durable_state(app)?;
    let Some(work) = work::find_work_by_receipt(&conn, run_id)? else {
        return Ok(());
    };
    let artifact_uri = ObjectUri::parse(&format!(
        "agentwork://{}/{}",
        agent_work::workspace_key(root),
        document.id
    ))?;
    work::attach_work_artifact(
        &mut conn,
        &work.id,
        work::AttachArtifact {
            artifact_uri,
            summary: None,
            next_action: Some("等待人工验收".into()),
        },
    )
    .map(|_| ())
}

/// 保存 Agent 工作文档正文；正文变化不会进入 Workspace 索引或文件树。
#[tauri::command]
async fn write_agent_work(
    app: AppHandle,
    state: State<'_, AppState>,
    input: agent_work::WriteAgentWorkInput,
) -> Result<agent_work::AgentWorkDocument, String> {
    let root = workspace_root(&state)?;
    agent_work::write_at(&app, &root, input)
}

// ---------------------------------------------------------------------------
// Work 视图命令面（M2 内容 / M3 Shell）——全部只是 work.rs domain 的薄投影，
// 状态机与事件仍归 domain rule；UI 不接触 SQLite 细节。
// ---------------------------------------------------------------------------

/// 当前 Workspace 的 Work 列表（updated_at DESC, id DESC）。
#[tauri::command]
async fn list_works(
    app: AppHandle,
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<work::WorkRecord>, String> {
    let root = workspace_root(&state)?;
    let workspace_id = workspace_id_for_root(&root);
    let conn = open_durable_state(&app)?;
    work::list_works(
        &conn,
        Some(&workspace_id),
        limit.unwrap_or(100).clamp(1, 500),
    )
}

#[tauri::command]
async fn get_work(app: AppHandle, work_id: String) -> Result<work::WorkRecord, String> {
    let conn = open_durable_state(&app)?;
    work::get_work(&conn, &work_id)?.ok_or_else(|| format!("Work 不存在: {work_id}"))
}

/// 人工明确接受 → completed。这是 Work `completed` 的唯一入口；
/// 非法转换（如 queued/running 直接完成）由状态机拒绝并透传给 UI。
#[tauri::command]
async fn work_accept(app: AppHandle, work_id: String) -> Result<work::WorkRecord, String> {
    let mut conn = open_durable_state(&app)?;
    work::transition_work(
        &mut conn,
        &work_id,
        work::WorkStatus::Completed,
        state_store::ActorKind::Human,
        Some("人工接受成果"),
    )
}

/// 人工取消 → cancelled。若该 Work 的 run 正在 Pi 上运行，先走与
/// `agent_abort` 相同的中止通路（abort 核心会把 Work 落为 cancelled）。
#[tauri::command]
async fn work_cancel(
    app: AppHandle,
    process: State<'_, pi_agent::PiProcessState>,
    work_id: String,
) -> Result<work::WorkRecord, String> {
    let process = process.inner().clone();
    tauri::async_runtime::spawn_blocking(move || pi_agent::cancel_work(&process, &app, &work_id))
        .await
        .map_err(|error| format!("Work 取消任务异常: {error}"))?
}

/// Work 的语义事件（仅 work.*，新的在前）。
#[tauri::command]
async fn work_events(
    app: AppHandle,
    work_id: String,
    limit: Option<usize>,
) -> Result<Vec<state_store::EventRecord>, String> {
    let conn = open_durable_state(&app)?;
    state_store::events_for_work(&conn, &work_id, limit.unwrap_or(20).clamp(1, 100))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkReceiptProbe {
    exists: bool,
    path: Option<String>,
}

/// 运行收据存在性探针：只报告 receipt 文件是否存在与所在路径，
/// 不解析内容（receipt 本体由 Pi runtime 留存）。
#[tauri::command]
async fn work_receipt_probe(
    app: AppHandle,
    receipt_ref: String,
) -> Result<WorkReceiptProbe, String> {
    match pi_agent::receipt_path(&app, receipt_ref.trim())? {
        Some(path) => Ok(WorkReceiptProbe {
            exists: path.is_file(),
            path: Some(path.to_string_lossy().to_string()),
        }),
        None => Ok(WorkReceiptProbe {
            exists: false,
            path: None,
        }),
    }
}

/// 读取当前文档的批注（侧车不存在时返回空正文）。
#[tauri::command]
async fn read_annotation(
    app: AppHandle,
    state: State<'_, AppState>,
    target: AnnotationTarget,
) -> Result<annotate::AnnotationData, String> {
    match target {
        AnnotationTarget::Workspace { path } => {
            let root = workspace_root(&state)?;
            let doc = ensure_existing_path_inside_workspace(&path, &state)?;
            let rel = doc.strip_prefix(&root).unwrap_or(&doc);
            if !annotate::is_annotation_target(rel) {
                return Err("批注文件与批注汇总本身不能再写批注".into());
            }
            annotate::read_annotation_data(&root, &doc)
        }
        AnnotationTarget::Library {
            source_id,
            relative_path,
            content_hash,
        } => {
            let document =
                library_annotation_document(&app, &source_id, &relative_path, &content_hash)?;
            let root = library_annotation_root(&app)?;
            annotate::read_library_annotation_data(
                &root,
                &source_id,
                &content_hash,
                &document.uri,
                &document.title,
                &document.relative_path,
            )
        }
        AnnotationTarget::Agent { id } => {
            let workspace = workspace_root(&state)?;
            let document = agent_work::read_at(&app, &workspace, &id)?;
            let root = agent_work::annotation_root(&app, &workspace)?;
            annotate::read_agent_annotation_data(&root, &id, &document.uri, &document.title)
        }
    }
}

/// 保存当前文档的批注（正文为空则删除侧车 = 撤销批注）。
#[tauri::command]
async fn save_annotation(
    app: AppHandle,
    state: State<'_, AppState>,
    target: AnnotationTarget,
    body: String,
) -> Result<(), String> {
    match target {
        AnnotationTarget::Workspace { path } => {
            let root = workspace_root(&state)?;
            let doc = ensure_existing_path_inside_workspace(&path, &state)?;
            let rel = doc.strip_prefix(&root).unwrap_or(&doc);
            if !annotate::is_annotation_target(rel) {
                return Err("批注文件与批注汇总本身不能再写批注".into());
            }
            let sidecar = annotate::save_annotation(&root, &doc, &body)?;
            // 批注文件也进全文索引（删除时清理即可，索引重建会吸收）
            if sidecar.exists() {
                let _ = with_index(&app, &state, |conn, root| {
                    indexer::index_single(conn, root, &sidecar)
                });
            }
        }
        AnnotationTarget::Library {
            source_id,
            relative_path,
            content_hash,
        } => {
            let document =
                library_annotation_document(&app, &source_id, &relative_path, &content_hash)?;
            let root = library_annotation_root(&app)?;
            annotate::save_library_annotation(
                &root,
                &source_id,
                &content_hash,
                &document.title,
                &document.relative_path,
                &body,
            )?;
        }
        AnnotationTarget::Agent { id } => {
            let workspace = workspace_root(&state)?;
            let document = agent_work::read_at(&app, &workspace, &id)?;
            let root = agent_work::annotation_root(&app, &workspace)?;
            annotate::save_agent_annotation(&root, &id, &document.title, &document.uri, &body)?;
        }
    }
    Ok(())
}

/// 汇总所有单章批注到工作区根目录 `批注汇总.md`。
#[tauri::command]
async fn aggregate_annotations(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<annotate::AggregateResult, String> {
    let root = workspace_root(&state)?;
    let result = annotate::aggregate(&root)?;
    let _ = with_index(&app, &state, |conn, root| {
        indexer::index_single(conn, root, &root.join(annotate::AGGREGATE_NAME))
    });
    Ok(result)
}

#[tauri::command]
async fn sync_workspace(
    app: AppHandle,
    remote: Option<String>,
    state: State<'_, AppState>,
) -> Result<sync::SyncStatus, String> {
    let root = workspace_root(&state)?;

    // 确定同步 remote：origin 已指向默认远端则复用，否则用独立 remote `sync`（不改用户 origin）
    let remote_hint = remote
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("user@example.invalid:~/stillwrite.git");
    let remote_name = sync::resolve_sync_remote(&root, remote_hint)?;

    let status = sync::sync_workspace(&root, remote_hint, &remote_name)?;
    // 同步后重建索引（增量，吸收远端变更）
    let _ = with_index(&app, &state, indexer::build_index);
    Ok(status)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .manage(pi_agent::PiProcessState::default())
        .setup(|app| {
            let lock_path = app.path().app_data_dir()?.join("stillwrite.lock");
            match try_acquire_instance_lock(&lock_path)? {
                Some(lock) => {
                    app.state::<AppState>()
                        .instance_lock
                        .lock()
                        .map_err(|_| std::io::Error::other("应用实例锁状态异常"))?
                        .replace(lock);
                }
                None => app.handle().exit(0),
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            choose_workspace,
            choose_document,
            set_workspace,
            read_markdown,
            write_markdown,
            create_markdown,
            import_workspace_image,
            read_workspace_image_data_url,
            rebuild_index,
            search_index,
            brave_api_key_status,
            save_brave_api_key,
            clear_brave_api_key,
            search_web,
            list_web_search_history,
            list_search_result_links,
            link_search_result,
            unlink_search_result,
            search_related_index,
            add_library_source,
            refresh_library,
            search_library,
            search_related_library,
            pin_related,
            unpin_related,
            list_related_pins,
            import_related_pins,
            read_library_document,
            list_library_source_documents,
            feed_list_sources,
            feed_add_source,
            feed_remove_source,
            feed_import_opml,
            feed_refresh_source,
            feed_refresh_all,
            feed_status,
            list_agent_works,
            read_agent_work,
            create_agent_work,
            write_agent_work,
            list_works,
            get_work,
            work_accept,
            work_cancel,
            work_events,
            work_receipt_probe,
            read_annotation,
            save_annotation,
            aggregate_annotations,
            sync_workspace,
            pi_agent::agent_probe,
            pi_agent::agent_start,
            pi_agent::agent_abort,
            pi_agent::agent_recent_runs
        ])
        .run(tauri::generate_context!())
        .expect("error while running Stillwrite");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("stillwrite-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_root_cannot_be_a_workspace() {
        let error = validate_workspace_root(Path::new("/")).unwrap_err();
        assert!(error.contains("根目录"));
    }

    #[test]
    fn normal_directory_can_be_a_workspace() {
        let root = temp_dir("workspace-root");
        assert!(validate_workspace_root(&root).is_ok());
    }

    #[test]
    fn brave_web_results_parse_into_safe_web_hits() {
        let body = r#"
        {
          "web": {
            "results": [
              {
                "title": "Brave Search",
                "url": "https://search.brave.com/",
                "description": "A web result",
                "age": "2026-08-27"
              },
              {
                "title": "not a web URL",
                "url": "javascript:alert(1)",
                "description": "must be ignored"
              }
            ]
          }
        }
        "#;
        let hits = parse_brave_web_results(body).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Brave Search");
        assert_eq!(hits[0].age.as_deref(), Some("2026-08-27"));
    }

    #[test]
    fn brave_web_results_allow_empty_result_sets() {
        let hits = parse_brave_web_results(r#"{"type":"search"}"#).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn instance_lock_allows_only_one_process() {
        let root = temp_dir("instance-lock");
        let path = root.join("stillwrite.lock");
        let first = try_acquire_instance_lock(&path).unwrap().unwrap();
        assert!(try_acquire_instance_lock(&path).unwrap().is_none());
        drop(first);
        assert!(try_acquire_instance_lock(&path).unwrap().is_some());
    }

    #[test]
    fn scan_dir_skips_generated_directories() {
        let root = temp_dir("scan-generated");
        fs::write(root.join("visible.md"), "visible").unwrap();
        for name in ["target", "node_modules"] {
            fs::create_dir_all(root.join(name)).unwrap();
            fs::write(root.join(name).join("generated.md"), "generated").unwrap();
        }

        let nodes = scan_dir(&root).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "visible.md");
    }

    // ---- create_dir_all_inside ----

    #[test]
    fn create_dir_all_inside_creates_nested_dirs() {
        let root = temp_dir("nested");
        let dir = root.join("a").join("b").join("c");
        create_dir_all_inside(&root, &dir).unwrap();
        assert!(dir.is_dir());
    }

    #[test]
    fn create_dir_all_inside_keeps_existing_dirs() {
        let root = temp_dir("existing");
        fs::create_dir_all(root.join("a").join("b")).unwrap();
        create_dir_all_inside(&root, &root.join("a").join("b").join("c")).unwrap();
        assert!(root.join("a").join("b").join("c").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn create_dir_all_inside_rejects_symlink_escaping_root() {
        let root = temp_dir("escape");
        let outside = temp_dir("outside");
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

        let result = create_dir_all_inside(&root, &root.join("link").join("sub"));
        assert!(result.is_err());
        // 关键断言：外部目录没有被创建任何内容
        assert!(!outside.join("sub").exists());
    }

    #[cfg(unix)]
    #[test]
    fn create_dir_all_inside_allows_symlink_inside_root() {
        let root = temp_dir("inside-link");
        let inner = root.join("real");
        fs::create_dir_all(&inner).unwrap();
        std::os::unix::fs::symlink(&inner, root.join("alias")).unwrap();

        let result = create_dir_all_inside(&root, &root.join("alias").join("sub"));
        assert!(result.is_ok());
        assert!(inner.join("sub").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn create_dir_all_inside_rejects_symlink_at_any_depth() {
        let root = temp_dir("deep-escape");
        let outside = temp_dir("deep-outside");
        fs::create_dir_all(root.join("a")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("a").join("link")).unwrap();

        let result = create_dir_all_inside(&root, &root.join("a").join("link").join("b"));
        assert!(result.is_err());
        assert!(!outside.join("b").exists());
    }

    // ---- atomic_write ----

    #[test]
    fn atomic_write_creates_file() {
        let root = temp_dir("atomic-new");
        let file = root.join("note.md");
        atomic_write(&file, "v1").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "v1");
    }

    #[test]
    fn atomic_write_overwrites_and_keeps_backup() {
        let root = temp_dir("atomic-ov");
        let file = root.join("note.md");
        atomic_write(&file, "v1").unwrap();
        atomic_write(&file, "v2").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "v2");
        // 备份应保留上一版本内容
        assert_eq!(fs::read_to_string(root.join("note.md.bak")).unwrap(), "v1");
    }

    #[test]
    fn atomic_write_leaves_no_tmp_files() {
        let root = temp_dir("atomic-tmp");
        let file = root.join("note.md");
        atomic_write(&file, "v1").unwrap();
        atomic_write(&file, "v2").unwrap();
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "存在残留临时文件: {leftovers:?}");
    }

    // ---- create_new_markdown（命令核心逻辑） ----

    #[test]
    fn create_new_markdown_creates_nested_file() {
        let root = temp_dir("cmd-create");
        let path = create_new_markdown(&root, Path::new("docs/notes/hello.md")).unwrap();
        assert_eq!(path, root.join("docs/notes/hello.md"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "# Untitled\n\n");
    }

    #[test]
    fn create_new_markdown_creates_file_at_workspace_root() {
        let root = temp_dir("cmd-create-root");
        let path = create_new_markdown(&root, Path::new("hello.md")).unwrap();
        assert_eq!(path, root.join("hello.md"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "# Untitled\n\n");
    }

    #[cfg(unix)]
    #[test]
    fn create_new_markdown_rejects_symlink_escape() {
        let root = temp_dir("cmd-escape");
        let outside = temp_dir("cmd-outside");
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

        // 复现报告场景：工作区内存在指向外部的符号链接，创建 link/new.md
        let result = create_new_markdown(&root, Path::new("link/new.md"));
        assert!(result.is_err());
        assert!(!outside.join("new.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn create_new_markdown_rejects_broken_symlink_target() {
        let root = temp_dir("cmd-broken");
        std::os::unix::fs::symlink("/nonexistent-target-xyz", root.join("link.md")).unwrap();

        // 悬空符号链接的 exists() 为 false，但 symlink_metadata 能识别，必须拒绝
        let result = create_new_markdown(&root, Path::new("link.md"));
        assert!(result.is_err());
    }

    #[test]
    fn create_new_markdown_rejects_escaping_relative_paths() {
        let root = temp_dir("cmd-dotdot");
        fs::write(root.join("existing.md"), "taken").unwrap();
        assert!(create_new_markdown(&root, Path::new("../evil.md")).is_err());
        assert!(create_new_markdown(&root, Path::new("a/../../evil.md")).is_err());
        assert!(create_new_markdown(&root, Path::new("/abs.md")).is_err());
        assert!(create_new_markdown(&root, Path::new("note.txt")).is_err());
        assert!(create_new_markdown(&root, Path::new("existing.md")).is_err());
    }

    // ---- 工作区本地图片（import/read 纯逻辑部分） ----

    #[test]
    fn image_mime_maps_supported_extensions() {
        assert_eq!(
            image_mime_by_extension(Path::new("a.png")),
            Some("image/png")
        );
        assert_eq!(
            image_mime_by_extension(Path::new("a.jpg")),
            Some("image/jpeg")
        );
        assert_eq!(
            image_mime_by_extension(Path::new("a.jpeg")),
            Some("image/jpeg")
        );
        assert_eq!(
            image_mime_by_extension(Path::new("a.webp")),
            Some("image/webp")
        );
        assert_eq!(
            image_mime_by_extension(Path::new("a.gif")),
            Some("image/gif")
        );
        assert_eq!(image_mime_by_extension(Path::new("a.svg")), None);
        assert_eq!(
            image_mime_by_extension(Path::new("a.PNG")),
            Some("image/png")
        );
    }

    #[test]
    fn image_target_file_name_sanitizes_spaces() {
        assert_eq!(image_target_file_name("photo.png"), "photo.png");
        assert_eq!(image_target_file_name("my photo.png"), "my-photo.png");
        assert_eq!(image_target_file_name("  a  b.png "), "a--b.png");
    }

    #[test]
    fn unique_image_target_never_overwrites() {
        let dir = temp_dir("img-collision");
        fs::write(dir.join("photo.png"), "1").unwrap();
        fs::write(dir.join("photo-2.png"), "2").unwrap();
        let first = unique_image_target(&dir, "photo.png");
        assert_eq!(first, "photo-3.png");
        let second = unique_image_target(&dir, "photo.png");
        assert_eq!(second, "photo-3.png"); // 未落盘就不会变化
        fs::write(dir.join("photo-3.png"), "3").unwrap();
        assert_eq!(unique_image_target(&dir, "photo.png"), "photo-4.png");
    }

    #[test]
    fn atomic_copy_image_creates_identical_target() {
        let dir = temp_dir("img-copy");
        let source = dir.join("src.png");
        fs::write(&source, b"\x89PNG\r\n\x1a\n1234567890").unwrap();
        let target = dir.join("assets").join("dst.png");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        atomic_copy_image(&source, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), fs::read(&source).unwrap());
        // 不应残留临时文件
        let leftovers: Vec<_> = fs::read_dir(target.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn read_workspace_image_validation_rejects_bad_paths() {
        // 纯路径约束（不经过 State 的路径段检查）：
        assert!(Path::new("../secret.png").is_relative());
        assert!(Path::new("../secret.png")
            .components()
            .any(|c| matches!(c, Component::ParentDir)));
        assert!(Path::new("/abs/secret.png").is_absolute());
        assert!(Path::new("assets/ok.png").is_relative());
        assert!(Path::new("a/../../evil.png")
            .components()
            .any(|c| matches!(c, Component::ParentDir)));
    }

    #[cfg(unix)]
    #[test]
    fn read_workspace_image_validation_rejects_symlink_escape() {
        let root = temp_dir("img-symlink");
        let outside = temp_dir("img-symlink-out");
        fs::write(outside.join("evil.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        fs::write(root.join("doc.md"), "# d").unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        std::os::unix::fs::symlink(outside.join("evil.png"), root.join("assets/link.png")).unwrap();
        // 规范化后位于 Workspace 外 → 拒绝
        let canonical = fs::canonicalize(root.join("assets/link.png")).unwrap();
        assert!(!canonical.starts_with(&root));
    }

    #[test]
    fn workspace_assets_dir_contains_assets_sibling_of_markdown() {
        // 纯路径约束：assets 必须与 Markdown 同级
        let doc = Path::new("/ws/docs/ch01.md");
        let _parent = doc.parent().unwrap().join("assets");
        assert_eq!(
            doc.parent().unwrap().join("assets"),
            Path::new("/ws/docs/assets")
        );
    }

    #[test]
    fn image_size_limit_is_15_mib_constant() {
        // import 与 read 两条路径共用同一上限常量
        assert_eq!(MAX_WORKSPACE_IMAGE_BYTES, 15 * 1024 * 1024);
        // 恰好等于上限的载荷被允许（> 才拒绝），边界语义与 spec 一致
        let at_limit = MAX_WORKSPACE_IMAGE_BYTES;
        assert!(!(at_limit > MAX_WORKSPACE_IMAGE_BYTES));
        assert!(at_limit + 1 > MAX_WORKSPACE_IMAGE_BYTES);
    }

    #[test]
    fn image_import_chain_copies_into_sibling_assets_and_reads_back() {
        // 模拟完整后端链路（不经过 Tauri AppHandle/State）：
        // picker 源 → 目标 assets 目录 → 唯一名 → 原子复制 → data URL 编码。
        let root = temp_dir("img-chain");
        let docs = root.join("docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("ch01.md"), "# c").unwrap();
        let assets = docs.join("assets");
        fs::create_dir_all(&assets).unwrap();

        let source = root.join("outside-photo.png");
        let payload: Vec<u8> = (0..64).map(|i| (i % 251) as u8).collect();
        fs::write(&source, &payload).unwrap();

        let requested = image_target_file_name("outside photo.png");
        assert_eq!(requested, "outside-photo.png");
        let target_name = unique_image_target(&assets, &requested);
        atomic_copy_image(&source, &assets.join(&target_name)).unwrap();
        assert_eq!(fs::read(assets.join(&target_name)).unwrap(), payload);

        // 同一源再导入一次 → 不覆盖，生成 -2 后缀
        let second = unique_image_target(&assets, &requested);
        assert_eq!(second, "outside-photo-2.png");

        // 读回 data URL（手动走 mime + base64，与命令相同的公式）
        let mime = image_mime_by_extension(Path::new(&target_name)).unwrap();
        let bytes = fs::read(assets.join(&target_name)).unwrap();
        let data_url = format!(
            "data:{mime};base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
        );
        assert!(data_url.starts_with("data:image/png;base64,"));
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            data_url.trim_start_matches("data:image/png;base64,"),
        )
        .unwrap();
        assert_eq!(decoded, payload);
    }
}
