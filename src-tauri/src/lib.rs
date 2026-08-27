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
mod sync;

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
    process.shutdown_for_workspace(&root);

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
    if let Some(run_id) = run_id {
        crate::pi_agent::record_run_event(
            &app,
            &run_id,
            serde_json::json!({
                "event": "work_saved",
                "stage": "WORK_SAVED",
                "workId": document.id
            }),
        );
    }
    Ok(document)
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
        .invoke_handler(tauri::generate_handler![
            choose_workspace,
            choose_document,
            set_workspace,
            read_markdown,
            write_markdown,
            create_markdown,
            rebuild_index,
            search_index,
            search_related_index,
            add_library_source,
            refresh_library,
            search_library,
            search_related_library,
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
}
