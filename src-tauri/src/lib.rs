use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

mod indexer;
mod sync;

#[derive(Default)]
struct AppState {
    root: Mutex<Option<PathBuf>>,
    index_db: Mutex<Option<PathBuf>>,
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

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn scan_dir(path: &Path) -> Result<Vec<TreeNode>, String> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    let entries = fs::read_dir(path).map_err(|e| format!("读取目录失败: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let file_type = entry.file_type().map_err(|e| format!("读取类型失败: {e}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            continue;
        }

        if file_type.is_dir() {
            let children = scan_dir(&path)?;
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
    let sub = dir.join("workspaces").join(format!("{key}"));
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
) -> Result<WorkspaceData, String> {
    let root = fs::canonicalize(root).map_err(|e| format!("打开目录失败: {e}"))?;
    if !root.is_dir() {
        return Err("选择的路径不是目录".into());
    }

    let nodes = scan_dir(&root)?;
    {
        let mut guard = state
            .root
            .lock()
            .map_err(|_| "工作区状态锁定失败".to_string())?;
        *guard = Some(root.clone());
    }

    // 建立侧车索引（增量）
    let _ = with_index(app, state, |conn, root| indexer::build_index(conn, root));

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

fn ensure_existing_path_inside_workspace(path: &str, state: &State<AppState>) -> Result<PathBuf, String> {
    let root = workspace_root(state)?;
    let canonical = fs::canonicalize(path).map_err(|e| format!("路径不可用: {e}"))?;
    if !canonical.starts_with(&root) {
        return Err("拒绝访问工作区以外的路径".into());
    }
    Ok(canonical)
}

#[tauri::command]
async fn choose_workspace(app: AppHandle, state: State<'_, AppState>) -> Result<Option<WorkspaceData>, String> {
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
    activate_workspace(path, &app, &state).map(Some)
}

#[tauri::command]
fn set_workspace(app: AppHandle, path: String, state: State<AppState>) -> Result<WorkspaceData, String> {
    activate_workspace(PathBuf::from(path), &app, &state)
}

#[tauri::command]
fn read_markdown(path: String, state: State<AppState>) -> Result<String, String> {
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
fn create_dir_all_inside(root: &Path, dir: &Path) -> Result<(), String> {
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
                    let resolved = fs::canonicalize(&cursor)
                        .map_err(|e| format!("创建目录失败: {e}"))?;
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
fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
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
async fn write_markdown(app: AppHandle, path: String, content: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = ensure_existing_path_inside_workspace(&path, &state)?;
    if !is_markdown(&path) {
        return Err("只允许写入 Markdown 文件".into());
    }
    atomic_write(&path, &content)?;
    // 保存后增量更新索引
    let _ = with_index(&app, &state, |conn, root| indexer::index_single(conn, root, &path));
    Ok(())
}

#[tauri::command]
fn create_markdown(app: AppHandle, relative_path: String, state: State<AppState>) -> Result<String, String> {
    let root = workspace_root(&state)?;
    let target = create_new_markdown(&root, Path::new(&relative_path))?;
    let _ = with_index(&app, &state, |conn, root| indexer::index_single(conn, root, &target));
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
fn rebuild_index(app: AppHandle, state: State<AppState>) -> Result<(usize, usize), String> {
    with_index(&app, &state, |conn, root| indexer::build_index(conn, root))
}

#[tauri::command]
fn search_index(
    app: AppHandle,
    query: String,
    limit: Option<usize>,
    state: State<AppState>,
) -> Result<Vec<indexer::SearchHit>, String> {
    with_index(&app, &state, |conn, _| {
        indexer::search(conn, &query, limit.unwrap_or(30))
    })
}

#[tauri::command]
async fn sync_workspace(
    app: AppHandle,
    remote: Option<String>,
    state: State<'_, AppState>,
) -> Result<sync::SyncStatus, String> {
    let root = workspace_root(&state)?;

    // 若尚未配置 origin 且前端提供了远程地址，自动补齐
    let remote_hint = remote
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("radxa@192.168.100.106:~/stillwrite.git");
    let has_origin = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_origin && remote.is_some() {
        let _ = std::process::Command::new("git")
            .args(["remote", "add", "origin", remote_hint])
            .current_dir(&root)
            .output();
    }

    let status = sync::sync_workspace(&root, remote_hint)?;
    // 同步后重建索引（增量，吸收远端变更）
    let _ = with_index(&app, &state, |conn, root| indexer::build_index(conn, root));
    Ok(status)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            choose_workspace,
            set_workspace,
            read_markdown,
            write_markdown,
            create_markdown,
            rebuild_index,
            search_index,
            sync_workspace
        ])
        .run(tauri::generate_context!())
        .expect("error while running Stillwrite");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("stillwrite-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
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
