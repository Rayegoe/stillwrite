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
    path::{Component, Path, PathBuf},
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
    pub pi_session_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteAgentWorkInput {
    pub id: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentWorkMeta {
    id: String,
    title: String,
    prompt: String,
    #[serde(default)]
    origin_uri: Option<String>,
    #[serde(default)]
    origin_quote: Option<String>,
    created_at: u64,
    updated_at: u64,
    #[serde(default)]
    pi_session_ref: Option<String>,
}

/// The old sidecar is read only as a compatibility input.  It is projected
/// into `AgentWorkMeta` and is never serialized again.
#[derive(Clone, Debug, Deserialize)]
struct LegacyAgentWorkMeta {
    id: String,
    title: String,
    prompt: String,
    #[serde(default)]
    origin_uri: Option<String>,
    #[serde(default)]
    origin_quote: Option<String>,
    created_at: u64,
    updated_at: u64,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    receipt_ref: Option<String>,
}

impl LegacyAgentWorkMeta {
    fn into_current(self) -> AgentWorkMeta {
        let _ = (
            self.thread_id,
            self.conversation_id,
            self.run_id,
            self.receipt_ref,
        );
        AgentWorkMeta {
            id: self.id,
            title: self.title,
            prompt: self.prompt,
            origin_uri: self.origin_uri,
            origin_quote: self.origin_quote,
            created_at: self.created_at,
            updated_at: self.updated_at,
            pi_session_ref: None,
        }
    }
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
    pub pi_session_ref: Option<String>,
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
    pub pi_session_ref: Option<String>,
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
    let pi_session_ref = validate_session_ref(input.pi_session_ref.as_deref())?;
    let now = unix_timestamp();
    let meta = AgentWorkMeta {
        id: id.clone(),
        title,
        prompt: input.prompt,
        origin_uri: input.origin_uri,
        origin_quote: input.origin_quote,
        created_at: now,
        updated_at: now,
        pi_session_ref,
    };
    write_document(&root, &meta, &input.content)?;
    read_at(app, workspace_root, &id)
}

pub fn list(app: &AppHandle, workspace_root: &Path) -> Result<Vec<AgentWorkSummary>, String> {
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
    Ok(document_from_meta(workspace_root, meta, content))
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
        let body =
            fs::read_to_string(&path).map_err(|e| format!("读取 Agent 工作元数据失败: {e}"))?;
        return match serde_json::from_str::<AgentWorkMeta>(&body) {
            Ok(meta) => validate_meta(meta),
            Err(v2_error) => serde_json::from_str::<LegacyAgentWorkMeta>(&body)
                .map(LegacyAgentWorkMeta::into_current)
                .map_err(|legacy_error| {
                    format!("解析 Agent 工作元数据失败: {v2_error}; legacy: {legacy_error}")
                })
                .and_then(validate_meta),
        };
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
        pi_session_ref: None,
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
        pi_session_ref: meta.pi_session_ref,
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
        pi_session_ref: document.pi_session_ref.clone(),
    }
}

fn document_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.md"))
}

fn meta_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.json"))
}

fn validate_meta(meta: AgentWorkMeta) -> Result<AgentWorkMeta, String> {
    validate_session_ref(meta.pi_session_ref.as_deref())?;
    Ok(meta)
}

fn validate_session_ref(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let path = Path::new(value);
    if value.is_empty()
        || value.contains('\0')
        || value.contains('\\')
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("Pi session 引用必须是 session 目录内的相对路径".into());
    }
    Ok(Some(value.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "stillwrite-agent-work-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn v2_metadata_writes_only_product_fields() {
        let root = temp_dir("v2-fields");
        let meta = AgentWorkMeta {
            id: "work-1".into(),
            title: "A work".into(),
            prompt: "Rewrite this".into(),
            origin_uri: Some("workspace://a.md".into()),
            origin_quote: Some("A quote".into()),
            created_at: 1,
            updated_at: 2,
            pi_session_ref: Some("session.jsonl".into()),
        };
        write_document(&root, &meta, "# A work\n").unwrap();
        let body: Value =
            serde_json::from_str(&fs::read_to_string(meta_path(&root, "work-1")).unwrap()).unwrap();
        let keys = body
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            keys,
            [
                "created_at",
                "id",
                "origin_quote",
                "origin_uri",
                "pi_session_ref",
                "prompt",
                "title",
                "updated_at",
            ]
            .into_iter()
            .map(String::from)
            .collect()
        );
        assert_eq!(
            read_meta(&root, "work-1")
                .unwrap()
                .pi_session_ref
                .as_deref(),
            Some("session.jsonl")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_metadata_is_read_and_projects_to_v2_on_write() {
        let root = temp_dir("legacy-fields");
        fs::write(document_path(&root, "work-legacy"), "# Legacy\n").unwrap();
        fs::write(
            meta_path(&root, "work-legacy"),
            r#"{
              "id":"work-legacy",
              "title":"Legacy",
              "prompt":"old prompt",
              "origin_uri":null,
              "origin_quote":null,
              "created_at":10,
              "updated_at":11,
              "thread_id":"thread",
              "conversation_id":"conversation",
              "run_id":"run",
              "receipt_ref":"receipt"
            }"#,
        )
        .unwrap();
        let legacy = read_meta(&root, "work-legacy").unwrap();
        assert_eq!(legacy.id, "work-legacy");
        assert_eq!(legacy.pi_session_ref, None);
        write_document(&root, &legacy, "# Legacy edited\n").unwrap();
        let body = fs::read_to_string(meta_path(&root, "work-legacy")).unwrap();
        assert!(!body.contains("thread_id"));
        assert!(!body.contains("receipt_ref"));
        assert!(serde_json::from_str::<AgentWorkMeta>(&body).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn session_reference_must_be_relative() {
        assert!(validate_session_ref(Some("../outside.jsonl")).is_err());
        assert!(validate_session_ref(Some("/tmp/outside.jsonl")).is_err());
        assert!(validate_session_ref(Some("sessions\\outside.jsonl")).is_err());
        assert_eq!(
            validate_session_ref(Some("2026/session.jsonl")).unwrap(),
            Some("2026/session.jsonl".into())
        );
    }
}
