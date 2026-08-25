//! Agent Work 文档：Agent 产生的可持久化 Markdown 工作成果。
//!
//! 正文保存在 StillWrite 应用数据目录，不进入 Workspace 文件树、Library 或 git。
//! UI 仍把它当成普通可读写 Markdown 文档；同一 Workspace 的 Agent Work 使用独立
//! 目录，避免不同项目之间互相混淆。

use crate::atomic_write;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentWorkInput {
    pub id: Option<String>,
    pub title: String,
    pub content: String,
    pub prompt: String,
    pub origin_uri: Option<String>,
    pub origin_quote: Option<String>,
    pub thread_id: Option<String>,
    pub conversation_id: Option<String>,
    pub run_id: Option<String>,
    pub receipt_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteAgentWorkInput {
    pub id: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AgentWorkMeta {
    id: String,
    title: String,
    prompt: String,
    origin_uri: Option<String>,
    origin_quote: Option<String>,
    created_at: u64,
    updated_at: u64,
    thread_id: Option<String>,
    conversation_id: Option<String>,
    run_id: Option<String>,
    receipt_ref: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWorkDocument {
    pub id: String,
    pub uri: String,
    pub title: String,
    pub content: String,
    pub content_hash: String,
    pub prompt: String,
    pub origin_uri: Option<String>,
    pub origin_quote: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub status: String,
    pub thread_id: Option<String>,
    pub conversation_id: Option<String>,
    pub run_id: Option<String>,
    pub receipt_ref: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWorkSummary {
    pub id: String,
    pub uri: String,
    pub title: String,
    pub prompt: String,
    pub origin_uri: Option<String>,
    pub origin_quote: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub status: String,
    pub thread_id: Option<String>,
}

pub fn workspace_key(workspace_root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(workspace_root.to_string_lossy().as_bytes());
    hasher
        .finalize()
        .iter()
        .take(12)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn works_root(app: &AppHandle, workspace_root: &Path) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位 Agent 工作目录: {e}"))?
        .join("agent")
        .join("works")
        .join(workspace_key(workspace_root));
    fs::create_dir_all(&root).map_err(|e| format!("创建 Agent 工作目录失败: {e}"))?;
    Ok(root)
}

pub fn annotation_root(app: &AppHandle, workspace_root: &Path) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位 Agent 批注目录: {e}"))?
        .join("agent")
        .join("annotations")
        .join(workspace_key(workspace_root));
    fs::create_dir_all(&root).map_err(|e| format!("创建 Agent 批注目录失败: {e}"))?;
    Ok(root)
}

pub fn create(
    app: &AppHandle,
    workspace_root: &Path,
    input: CreateAgentWorkInput,
) -> Result<AgentWorkDocument, String> {
    let root = works_root(app, workspace_root)?;
    let id = input.id.unwrap_or_else(new_id);
    validate_id(&id)?;
    let title = if input.title.trim().is_empty() {
        title_from_content(&input.content)
    } else {
        input.title.trim().to_string()
    };
    let now = unix_timestamp();
    let meta = AgentWorkMeta {
        id: id.clone(),
        title,
        prompt: input.prompt,
        origin_uri: input.origin_uri,
        origin_quote: input.origin_quote,
        created_at: now,
        updated_at: now,
        thread_id: input.thread_id,
        conversation_id: input.conversation_id,
        run_id: input.run_id,
        receipt_ref: input.receipt_ref,
    };
    write_document(&root, &meta, &input.content)?;
    read_at(app, workspace_root, &id)
}

pub fn list(
    app: &AppHandle,
    workspace_root: &Path,
) -> Result<Vec<AgentWorkSummary>, String> {
    let root = works_root(app, workspace_root)?;
    let mut works = Vec::new();
    for entry in fs::read_dir(&root)
        .map_err(|e| format!("读取 Agent 工作失败: {e}"))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Ok(document) = read_at(app, workspace_root, id) {
            works.push(summary(&document));
        }
    }
    works.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(works)
}

pub fn read_at(
    app: &AppHandle,
    workspace_root: &Path,
    id: &str,
) -> Result<AgentWorkDocument, String> {
    validate_id(id)?;
    let root = works_root(app, workspace_root)?;
    let meta = read_meta(&root, id)?;
    let content = fs::read_to_string(document_path(&root, id))
        .map_err(|e| format!("读取 Agent 工作失败: {e}"))?;
    Ok(document_from_meta(
        workspace_root,
        meta,
        content,
    ))
}

pub fn write_at(
    app: &AppHandle,
    workspace_root: &Path,
    input: WriteAgentWorkInput,
) -> Result<AgentWorkDocument, String> {
    validate_id(&input.id)?;
    let root = works_root(app, workspace_root)?;
    let mut meta = read_meta(&root, &input.id)?;
    meta.updated_at = unix_timestamp();
    write_document(&root, &meta, &input.content)?;
    read_at(app, workspace_root, &input.id)
}

fn write_document(root: &Path, meta: &AgentWorkMeta, content: &str) -> Result<(), String> {
    atomic_write(&document_path(root, &meta.id), content)?;
    let meta_body = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("编码 Agent 工作元数据失败: {e}"))?;
    atomic_write(&meta_path(root, &meta.id), &meta_body)
}

fn read_meta(root: &Path, id: &str) -> Result<AgentWorkMeta, String> {
    let path = meta_path(root, id);
    if path.is_file() {
        let body = fs::read_to_string(&path).map_err(|e| format!("读取 Agent 工作元数据失败: {e}"))?;
        return serde_json::from_str(&body).map_err(|e| format!("解析 Agent 工作元数据失败: {e}"));
    }
    let content = fs::read_to_string(document_path(root, id))
        .map_err(|e| format!("读取 Agent 工作失败: {e}"))?;
    let now = unix_timestamp();
    Ok(AgentWorkMeta {
        id: id.to_string(),
        title: title_from_content(&content),
        prompt: String::new(),
        origin_uri: None,
        origin_quote: None,
        created_at: now,
        updated_at: now,
        thread_id: None,
        conversation_id: None,
        run_id: None,
        receipt_ref: None,
    })
}

fn document_from_meta(
    workspace_root: &Path,
    meta: AgentWorkMeta,
    content: String,
) -> AgentWorkDocument {
    let key = workspace_key(workspace_root);
    AgentWorkDocument {
        uri: format!("agent://{key}/{}", meta.id),
        id: meta.id,
        title: meta.title,
        content_hash: content_hash(&content),
        content,
        prompt: meta.prompt,
        origin_uri: meta.origin_uri,
        origin_quote: meta.origin_quote,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
        status: "completed".into(),
        thread_id: meta.thread_id,
        conversation_id: meta.conversation_id,
        run_id: meta.run_id,
        receipt_ref: meta.receipt_ref,
    }
}

fn summary(document: &AgentWorkDocument) -> AgentWorkSummary {
    AgentWorkSummary {
        id: document.id.clone(),
        uri: document.uri.clone(),
        title: document.title.clone(),
        prompt: document.prompt.clone(),
        origin_uri: document.origin_uri.clone(),
        origin_quote: document.origin_quote.clone(),
        created_at: document.created_at,
        updated_at: document.updated_at,
        status: document.status.clone(),
        thread_id: document.thread_id.clone(),
    }
}

fn document_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.md"))
}

fn meta_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.json"))
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("Agent 工作标识无效".into());
    }
    Ok(())
}

fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("work-{:x}-{:x}", std::process::id(), nanos)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn title_from_content(content: &str) -> String {
    let first = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Agent 工作");
    let title = first.trim_start_matches('#').trim();
    let mut value = title.to_string();
    if value.chars().count() > 80 {
        value = value.chars().take(80).collect();
        value.push('…');
    }
    value
}
