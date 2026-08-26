//! StillWrite RSS / Atom Source Adapter。
//!
//! 职责严格限制为：feed source 配置、HTTP 抓取、RSS/Atom/JSON Feed 解析、
//! item 身份、HTML 片段 → Markdown、物化本地 Markdown、OPML 导入。
//!
//! 它不负责：Library 搜索 / 批注 / Agent / Related / Workspace / EPUB。
//!
//! 数据流：
//! ```text
//! Feed URL → HTTP（ETag/Last-Modified 条件请求）→ feed-rs 解析
//!   → 规范化 item → Markdown 物化 → <AppData>/library/RSS/<feed-id>/<date>__<title>__<id>.md
//!   → 现有 library::refresh_at → Library 索引/搜索/批注/关联/Agent
//! ```
//!
//! 持久配置（用户意图）与派生状态（etag/时间）分离：
//! - `rss-sources.json`   —— 用户订阅，删除即丢失订阅。
//! - `rss-fetch-state.json` —— 可删除的派生缓存，删除后只导致下一次完整抓取。

use crate::library;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

/// RSS 物化目录固定内部名，作为 Library 的一个普通 source 注册。
pub const RSS_LIBRARY_NAME: &str = "RSS";
const CONFIG_VERSION: u32 = 1;

const USER_AGENT: &str = concat!(
    "StillWrite/",
    env!("CARGO_PKG_VERSION"),
    " (+RSS Source Adapter)"
);
const ACCEPT: &str =
    "application/rss+xml, application/atom+xml, application/xml, text/xml, */*;q=0.5";

/// Feed 响应体大小上限（5 MiB，与 spec/验源脚本一致）。
const MAX_FEED_BYTES: u64 = 5 * 1024 * 1024;
/// 单次请求的整体超时（connect + read）。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
/// 有界并发：最多同时抓取几个源。
const WORKER_COUNT: usize = 4;
/// 单次 refresh 每个源最多物化的条目数（防病态大 feed 拖垮磁盘）。
const MAX_ITEMS_PER_FEED: usize = 200;

// ---------------------------------------------------------------------------
// 数据模型
// ---------------------------------------------------------------------------

/// 用户订阅配置（rss-sources.json）。只保存用户意图。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FeedSource {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct FeedSourcesConfig {
    version: u32,
    sources: Vec<FeedSource>,
}

/// 可删除的派生抓取状态（rss-fetch-state.json）。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
struct FeedFetchState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_fetch_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct FetchStateFile {
    version: u32,
    sources: BTreeMap<String, FeedFetchState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_global_refresh_at: Option<i64>,
}

/// FeedSource + 派生状态，供 UI 展示。
#[derive(Serialize, Clone, Debug)]
pub struct FeedSourceView {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_fetch_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct OpmlImportResult {
    pub added: usize,
    pub duplicates: usize,
    pub invalid: usize,
    pub warnings: Vec<String>,
    pub sources: Vec<FeedSourceView>,
}

#[derive(Serialize, Clone, Debug)]
pub struct FeedRefreshOutcome {
    pub source_id: String,
    pub name: String,
    /// "ok" | "unchanged" | "error"
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// feed 里实际出现的条目数
    pub items: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct FeedRefreshResult {
    pub sources: Vec<FeedRefreshOutcome>,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub failed: usize,
    pub materialized: usize,
}

/// 最近 RSS 资料（来自现有 library_documents，非 RSS 专属表）。
#[derive(Serialize, Clone, Debug)]
pub struct RecentRssItem {
    pub uri: String,
    pub source_id: String,
    pub relative_path: String,
    pub content_hash: String,
    pub title: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_name: Option<String>,
    /// 物化日期（来自文件名 <YYYY-MM-DD>__…）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct FeedStatus {
    pub sources: Vec<FeedSourceView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_library_source: Option<library::LibrarySource>,
    pub recent: Vec<RecentRssItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_refresh_at: Option<i64>,
}

// ---------------------------------------------------------------------------
// 纯逻辑：URL / ID / 文件名
// ---------------------------------------------------------------------------

/// 规范化 feed URL：trim、仅 http/https、去掉 fragment。
pub fn canonicalize_feed_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Feed 地址不能为空".into());
    }
    let mut parsed = url::Url::parse(trimmed).map_err(|e| format!("Feed 地址无效：{e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("只允许 http/https 的 Feed 地址".into());
    }
    if parsed.host_str().map(str::is_empty).unwrap_or(true) {
        return Err("Feed 地址缺少主机名".into());
    }
    parsed.set_fragment(None);
    Ok(parsed.to_string())
}

/// 由 canonical feed URL 派生稳定 source id（不依赖 source name / 自增 ID）。
pub fn source_id(url: &str) -> String {
    short_sha256(url, 12)
}

fn short_sha256(input: &str, hex_digits: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let bytes = hasher.finalize();
    let full: String = bytes
        .iter()
        .take(hex_digits.div_ceil(2))
        .map(|b| format!("{b:02x}"))
        .collect();
    full.chars().take(hex_digits).collect()
}

/// feed-rs 在“无 guid 且无链接”时会补一个随机 UUID，无法跨抓取稳定；
/// 其“链接+标题哈希”形态为 32 位小写十六进制。识别这两类合成 id，
/// 让真实 guid 优先，其余回落到链接 / (标题+时间)。
fn looks_synthetic_id(id: &str) -> bool {
    let trimmed = id.trim();
    if trimmed.len() == 32 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if trimmed.len() == 36 {
        let parts: Vec<&str> = trimmed.split('-').collect();
        if parts.len() == 5
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return true;
        }
    }
    false
}

fn canonical_link(entry: &feed_rs::model::Entry) -> Option<String> {
    entry
        .links
        .iter()
        .find(|l| l.rel.as_deref() == Some("alternate"))
        .or_else(|| {
            entry
                .links
                .iter()
                .find(|l| l.rel.as_deref() != Some("enclosure") && !l.href.trim().is_empty())
        })
        .map(|l| l.href.trim())
        .filter(|href| !href.is_empty() && href.starts_with("http"))
        .map(str::to_string)
}

/// item 身份（按 spec 优先级）：guid/id → canonical link → (标题 + 发布时间)。
fn item_identity(entry: &feed_rs::model::Entry) -> String {
    let guid = entry.id.trim();
    if !guid.is_empty() && !looks_synthetic_id(guid) {
        return guid.to_string();
    }
    if let Some(link) = canonical_link(entry) {
        return link;
    }
    let title = entry.title.as_ref().map(|t| t.content.trim()).unwrap_or("");
    let when = entry
        .published
        .or(entry.updated)
        .map(|t| t.timestamp().to_string())
        .unwrap_or_default();
    format!("{title}\u{1f}|{when}")
}

/// item_id：身份字符串 SHA-256 截取稳定短 hash。
pub fn item_id(identity: &str) -> String {
    short_sha256(identity, 8)
}

/// 文件名清洗：去掉 `/ \ : * ? " < > |` 与控制字符、压空白、去首尾点、
/// 截断长度。产出物只能是单个文件名段，不可能含 `..`、绝对路径或分隔符。
pub fn sanitize_filename_part(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches([' ', '.']);
    if trimmed.is_empty() {
        return "untitled".to_string();
    }
    let mut out = trimmed.to_string();
    if out.chars().count() > 80 {
        out = out.chars().take(80).collect();
    }
    out
}

/// 物化文件名：`<YYYY-MM-DD>__<title>__<item-id>.md`。
/// 目录 = `<rss_root>/<source-id>/`，绝不使用远端路径。
fn materialized_relative_path(
    source: &FeedSource,
    entry: &feed_rs::model::Entry,
    fetched_at: DateTime<Local>,
) -> String {
    let date = entry
        .published
        .or(entry.updated)
        .map(|t| t.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| fetched_at.format("%Y-%m-%d").to_string());
    let title = entry
        .title
        .as_ref()
        .map(|t| t.content.trim())
        .filter(|t| !t.is_empty())
        .unwrap_or("无标题条目");
    let id = item_id(&item_identity(entry));
    format!(
        "{}/{date}__{}__{id}.md",
        source.id,
        sanitize_filename_part(title)
    )
}

// ---------------------------------------------------------------------------
// 纯逻辑：HTML → Markdown
// ---------------------------------------------------------------------------

/// 移除可执行/嵌入内容块（script/style/noscript/iframe/object/embed/svg/template），
/// 之后交给成熟的 html2md 转换。这是标签级过滤器，不是 HTML 解析器。
fn strip_executable_html(input: &str) -> String {
    const BLOCKED: &[&str] = &[
        "script", "style", "noscript", "iframe", "object", "embed", "svg", "template",
    ];
    let lowered = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    let mut search_from = 0;
    while let Some(relative) = lowered[search_from..].find('<') {
        let open = search_from + relative;
        // 注释交给 html2md 忽略，这里只处理标签
        let rest = &lowered[open + 1..];
        let name_len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(rest.len());
        let name = &rest[..name_len];
        let blocked = BLOCKED.iter().find(|b| **b == name);
        let close_tag = name.is_empty() && rest.starts_with('/');
        match blocked {
            Some(tag) => {
                // 自闭合 `<script/>` 或 `<script …>`：跳过到标签结束
                let tag_end = lowered[open + 1..]
                    .find('>')
                    .map(|p| open + 1 + p + 1)
                    .unwrap_or(input.len());
                let self_closing = lowered[open..tag_end].ends_with("/>");
                if self_closing {
                    out.push_str(&input[cursor..open]);
                    cursor = tag_end;
                    search_from = tag_end;
                    continue;
                }
                // 找关闭标签 `</tag`，忽略大小写
                let needle = format!("</{tag}");
                let close = lowered[tag_end..].find(&needle).map(|p| tag_end + p);
                out.push_str(&input[cursor..open]);
                match close {
                    Some(close_start) => {
                        let close_end = lowered[close_start..]
                            .find('>')
                            .map(|p| close_start + p + 1)
                            .unwrap_or(input.len());
                        cursor = close_end;
                        search_from = close_end;
                    }
                    None => {
                        cursor = input.len();
                        search_from = input.len();
                    }
                }
            }
            _ if close_tag => {
                // 孤立关闭标签，保留原样
                search_from = open + 2;
            }
            _ => {
                out.push_str(&input[cursor..open]);
                cursor = open;
                search_from = open + 1;
            }
        }
    }
    out.push_str(&input[cursor..]);
    out
}

/// 弱嗅探：RSS description 常把 HTML 标成 text/plain，识别常见块级/行内标签。
fn looks_like_html(text: &str) -> bool {
    if !text.contains('<') {
        return false;
    }
    let lowered = text.to_ascii_lowercase();
    [
        "<p",
        "<div",
        "<br",
        "<a ",
        "<img",
        "<li",
        "<h1",
        "<h2",
        "<h3",
        "<h4",
        "<h5",
        "<h6",
        "<blockquote",
        "<pre",
        "<code",
        "<table",
        "<ul",
        "<ol",
        "<strong",
        "<em",
        "<b>",
        "<i>",
        "<span",
    ]
    .iter()
    .any(|tag| lowered.contains(tag))
}

/// 把 Entry 的正文候选转成 Markdown：
/// - content.body（HTML → markdown；纯文本原样）
/// - 其次 summary（RSS description 常为 HTML 但标成 text/plain，弱嗅探后转换）
/// - 都没有 → 提示打开原文，不伪装全文。
fn entry_body(entry: &feed_rs::model::Entry) -> String {
    if let Some(content) = &entry.content {
        if let Some(body) = content
            .body
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        {
            let essence = content.content_type.to_string().to_ascii_lowercase();
            if essence.starts_with("text/plain") && !looks_like_html(body) {
                return body.trim().to_string();
            }
            let converted = html_fragment_to_markdown(body);
            if !converted.is_empty() {
                return converted;
            }
        }
    }
    if let Some(summary) = &entry.summary {
        let body = summary.content.trim();
        if !body.is_empty() {
            let essence = summary.content_type.to_string().to_ascii_lowercase();
            if (essence.starts_with("text/plain") && !looks_like_html(body))
                || body.chars().all(|c| !c.is_control() && c != '<')
            {
                return body.to_string();
            }
            let converted = html_fragment_to_markdown(body);
            if !converted.is_empty() {
                return converted;
            }
        }
    }
    "(此 Feed 未提供正文或摘要，请打开原文)".to_string()
}

fn html_fragment_to_markdown(html: &str) -> String {
    let cleaned = strip_executable_html(html);
    let converted = html2md::parse_html(&cleaned);
    converted.trim().to_string()
}

/// enclosure / media 只保留为 Markdown 链接，不下载、不缓存、不播放。
fn entry_enclosures(entry: &feed_rs::model::Entry) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for link in &entry.links {
        if link.rel.as_deref() == Some("enclosure") {
            let href = link.href.trim();
            if !href.is_empty() {
                let label = link
                    .media_type
                    .as_deref()
                    .filter(|m| !m.is_empty())
                    .unwrap_or("attachment");
                if !out.iter().any(|(_, u)| u == href) {
                    out.push((label.to_string(), href.to_string()));
                }
            }
        }
    }
    for media in &entry.media {
        if let Some(content) = media.content.first() {
            if let Some(url) = &content.url {
                let href = url.as_str().trim();
                if !href.is_empty() {
                    let label = content
                        .content_type
                        .as_ref()
                        .map(|m| m.to_string())
                        .filter(|m| !m.is_empty())
                        .unwrap_or_else(|| "attachment".to_string());
                    if !out.iter().any(|(_, u)| u == href) {
                        out.push((label, href.to_string()));
                    }
                }
            }
        }
    }
    out
}

/// 标准物化模板（03_DATA_FILE_CONTRACT §4）。
fn render_item_markdown(source: &FeedSource, entry: &feed_rs::model::Entry) -> String {
    let title = entry
        .title
        .as_ref()
        .map(|t| t.content.trim())
        .filter(|t| !t.is_empty())
        .unwrap_or("无标题条目");
    let published_display = entry
        .published
        .or(entry.updated)
        .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "未知".to_string());
    let link = canonical_link(entry);
    let body = entry_body(entry);
    let enclosures = entry_enclosures(entry);

    let mut out = String::with_capacity(256 + body.len());
    out.push_str(&format!("# {title}\n\n"));
    out.push_str(&format!("> 来源：{}\n", source.name));
    out.push_str(&format!("> 发布：{published_display}\n"));
    match &link {
        Some(href) => out.push_str(&format!("> 原文：{href}\n")),
        None => out.push_str("> 原文：（无）\n"),
    }
    out.push_str(&format!("> Feed：{}\n\n", source.url));
    out.push_str(&body);
    out.push('\n');
    if !enclosures.is_empty() {
        out.push_str("\n## 附件\n\n");
        for (label, href) in enclosures {
            out.push_str(&format!("- [{label}]({href})\n"));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 配置 / 状态文件 IO（原子写；复用 lib.rs 的 atomic_write 风格）
// ---------------------------------------------------------------------------

fn atomic_write_json(path: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::atomic_write(path, std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
}

pub fn load_sources_file(path: &Path) -> Result<Vec<FeedSource>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|e| format!("读取 Feed 配置失败：{e}"))?;
    let config: FeedSourcesConfig =
        serde_json::from_str(&text).map_err(|e| format!("Feed 配置格式错误：{e}"))?;
    if config.version != CONFIG_VERSION {
        return Err(format!(
            "Feed 配置版本不兼容：{}（应用支持 v{CONFIG_VERSION}）",
            config.version
        ));
    }
    Ok(config.sources)
}

pub fn save_sources_file(path: &Path, sources: &[FeedSource]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 Feed 配置目录失败：{e}"))?;
    }
    let config = FeedSourcesConfig {
        version: CONFIG_VERSION,
        sources: sources.to_vec(),
    };
    let bytes =
        serde_json::to_vec_pretty(&config).map_err(|e| format!("序列化 Feed 配置失败：{e}"))?;
    atomic_write_json(path, &bytes)
}

fn load_state_file(path: &Path) -> Result<FetchStateFile, String> {
    if !path.exists() {
        return Ok(FetchStateFile::default());
    }
    let text = fs::read_to_string(path).map_err(|e| format!("读取 Feed 状态失败：{e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Feed 状态格式错误：{e}"))
}

fn save_state_file(path: &Path, state: &FetchStateFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 Feed 状态目录失败：{e}"))?;
    }
    let bytes =
        serde_json::to_vec_pretty(state).map_err(|e| format!("序列化 Feed 状态失败：{e}"))?;
    atomic_write_json(path, &bytes)
}

// ---------------------------------------------------------------------------
// 纯逻辑：OPML
// ---------------------------------------------------------------------------

/// OPML 1/2 通用：递归读取所有带 `xmlUrl` 的 `<outline>`；
/// 名称取 `title` 优先于 `text`。不保留 folder taxonomy。
pub fn parse_opml(xml: &str) -> Result<Vec<(String, String)>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("OPML 解析失败：{e}"))?;
    if doc.root_element().tag_name().name() != "opml" {
        return Err("文件不是 OPML 文档（缺少 <opml> 根节点）".into());
    }
    let mut out = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() != "outline" {
            continue;
        }
        let Some(url) = node.attribute("xmlUrl") else {
            continue;
        };
        let url = url.trim();
        if url.is_empty() {
            continue;
        }
        let name = node
            .attribute("title")
            .or_else(|| node.attribute("text"))
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or("未命名源");
        out.push((name.to_string(), url.to_string()));
    }
    if out.is_empty() {
        return Err("OPML 中没有找到带 xmlUrl 的 outline 节点".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// 纯逻辑：抓取 + 物化
// ---------------------------------------------------------------------------

enum FetchError {
    Unchanged,
    Http { status: u16, message: String },
    Network { message: String },
}

impl FetchError {
    fn message(&self) -> String {
        match self {
            FetchError::Unchanged => "无变化".to_string(),
            FetchError::Http { status, message } => {
                if message.is_empty() {
                    format!("HTTP {status}")
                } else {
                    format!("HTTP {status}：{message}")
                }
            }
            FetchError::Network { message } => message.clone(),
        }
    }
}

struct FetchedFeed {
    bytes: Vec<u8>,
    status: u16,
    etag: Option<String>,
    last_modified: Option<String>,
}

fn build_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .build()
        .map_err(|e| format!("初始化 HTTP 客户端失败：{e}"))
}

fn extract_header(response: &reqwest::blocking::Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 条件 GET。304 视为“成功且无变化”，不物化任何内容。
fn fetch_feed(
    client: &reqwest::blocking::Client,
    source: &FeedSource,
    state: &FeedFetchState,
) -> Result<FetchedFeed, FetchError> {
    let mut request = client
        .get(&source.url)
        .header(reqwest::header::ACCEPT, ACCEPT);
    if let Some(etag) = &state.etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = &state.last_modified {
        request = request.header(reqwest::header::IF_MODIFIED_SINCE, last_modified);
    }

    let response = request.send().map_err(|e| FetchError::Network {
        message: describe_reqwest_error(&e, &source.url),
    })?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Err(FetchError::Unchanged);
    }
    if !status.is_success() {
        let message = if status.is_redirection() {
            "重定向次数过多或重定向失败".to_string()
        } else {
            String::new()
        };
        return Err(FetchError::Http {
            status: status.as_u16(),
            message,
        });
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_FEED_BYTES)
    {
        return Err(FetchError::Network {
            message: format!("Feed 响应体超过 {} MiB 上限", MAX_FEED_BYTES / 1024 / 1024),
        });
    }
    let etag = extract_header(&response, "etag");
    let last_modified = extract_header(&response, "last-modified");
    let mut reader = response.take(MAX_FEED_BYTES + 1);
    let mut bytes = Vec::with_capacity(64 * 1024);
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| FetchError::Network {
            message: format!("读取 Feed 响应失败：{e}"),
        })?;
    if bytes.len() as u64 > MAX_FEED_BYTES {
        return Err(FetchError::Network {
            message: format!("Feed 响应体超过 {} MiB 上限", MAX_FEED_BYTES / 1024 / 1024),
        });
    }
    Ok(FetchedFeed {
        bytes,
        status: status.as_u16(),
        etag,
        last_modified,
    })
}

fn describe_reqwest_error(error: &reqwest::Error, url: &str) -> String {
    if error.is_timeout() {
        return format!("请求超时（{}s）：{url}", REQUEST_TIMEOUT.as_secs());
    }
    if error.is_connect() {
        return format!("连接失败：{url}");
    }
    if error.is_redirect() || error.to_string().to_ascii_lowercase().contains("redirect") {
        return format!("重定向失败（可能存在重定向环）：{url}");
    }
    format!("网络错误：{error}")
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 抓取并物化单个源。不触碰任何全局文件；返回 outcome + 新派生状态。
fn refresh_one(
    client: &reqwest::blocking::Client,
    rss_root: &Path,
    source: &FeedSource,
    state: &FeedFetchState,
    now: DateTime<Utc>,
) -> (FeedRefreshOutcome, FeedFetchState, Option<String>) {
    let mut outcome = FeedRefreshOutcome {
        source_id: source.id.clone(),
        name: source.name.clone(),
        status: "ok".to_string(),
        http_status: None,
        added: 0,
        updated: 0,
        unchanged: 0,
        items: 0,
        new_title: None,
        error: None,
    };
    let mut new_state = state.clone();
    new_state.last_fetch_at = Some(now.timestamp());

    let fetched = match fetch_feed(client, source, state) {
        Ok(fetched) => fetched,
        Err(FetchError::Unchanged) => {
            outcome.status = "unchanged".to_string();
            new_state.last_error = None;
            return (outcome, new_state, None);
        }
        Err(error) => {
            if let FetchError::Http { status, message } = error {
                outcome.status = "error".to_string();
                outcome.http_status = Some(status);
                outcome.error = Some(if message.is_empty() {
                    format!("HTTP {status}")
                } else {
                    format!("HTTP {status}：{message}")
                });
            } else {
                outcome.status = "error".to_string();
                outcome.error = Some(error.message());
            }
            new_state.last_error = outcome.error.clone();
            return (outcome, new_state, None);
        }
    };
    outcome.http_status = Some(fetched.status);
    new_state.etag = fetched.etag;
    new_state.last_modified = fetched.last_modified;
    new_state.last_error = None;

    let parsed = match feed_rs::parser::parse(fetched.bytes.as_slice()) {
        Ok(feed) => feed,
        Err(error) => {
            outcome.status = "error".to_string();
            outcome.error = Some(format!("Feed 解析失败：{error}"));
            new_state.last_error = outcome.error.clone();
            return (outcome, new_state, None);
        }
    };

    // Feed 标题可见时采纳为 source 名（仅当当前名字还是占位/主机名形态）。
    let adopted_title = parsed
        .title
        .as_ref()
        .map(|t| t.content.trim().to_string())
        .filter(|t| !t.is_empty())
        .filter(|t| {
            t != &source.name
                && (source.name.starts_with("未命名源")
                    || source.name.starts_with("http")
                    || source.name == url_host(&source.url).unwrap_or_default())
        });
    if let Some(title) = &adopted_title {
        outcome.name = title.clone();
        outcome.new_title = Some(title.clone());
    }

    outcome.items = parsed.entries.len();
    for entry in parsed.entries.iter().take(MAX_ITEMS_PER_FEED) {
        let has_content = entry.title.is_some()
            || entry.content.is_some()
            || entry.summary.is_some()
            || !entry.links.is_empty();
        if !has_content {
            continue;
        }
        let relative = materialized_relative_path(source, entry, now.with_timezone(&Local));
        let target = rss_root.join(&relative);
        let markdown = render_item_markdown(&materialized_source(source, &outcome.name), entry);
        match write_materialized(&target, &markdown) {
            WriteKind::Added => outcome.added += 1,
            WriteKind::Updated => outcome.updated += 1,
            WriteKind::Unchanged => outcome.unchanged += 1,
        }
    }
    (outcome, new_state, adopted_title)
}

/// 物化时用采纳后的 feed 标题（若有）作为“来源”署名。
fn materialized_source(source: &FeedSource, adopted_name: &str) -> FeedSource {
    let mut copy = source.clone();
    copy.name = adopted_name.to_string();
    copy
}

enum WriteKind {
    Added,
    Updated,
    Unchanged,
}

fn write_materialized(path: &Path, markdown: &str) -> WriteKind {
    let Some(parent) = path.parent() else {
        return WriteKind::Unchanged;
    };
    if fs::create_dir_all(parent).is_err() {
        return WriteKind::Unchanged;
    }
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == markdown {
            return WriteKind::Unchanged;
        }
        if fs::write(path, markdown).is_ok() {
            return WriteKind::Updated;
        }
        return WriteKind::Unchanged;
    }
    if fs::write(path, markdown).is_ok() {
        WriteKind::Added
    } else {
        WriteKind::Unchanged
    }
}

fn url_host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
}

/// 有界并发（≤4 线程波次）刷新一组源；单个源失败不影响其他源。
/// 使用调用线程的 scope，避免引入常驻线程池。
fn refresh_sources(
    client: &reqwest::blocking::Client,
    rss_root: &Path,
    sources: &[FeedSource],
    state_file: &mut FetchStateFile,
    only: Option<&str>,
) -> Vec<FeedRefreshOutcome> {
    let targets: Vec<&FeedSource> = sources
        .iter()
        .filter(|s| only.is_none() || only == Some(s.id.as_str()))
        .collect();
    if targets.is_empty() {
        return Vec::new();
    }

    let (tx, rx) = mpsc::channel::<(String, FeedRefreshOutcome, FeedFetchState, Option<String>)>();
    let now = Utc::now();
    thread::scope(|scope| {
        for wave in targets.chunks(WORKER_COUNT) {
            let mut handles = Vec::with_capacity(wave.len());
            for source in wave {
                let tx = tx.clone();
                let state = state_file
                    .sources
                    .get(&source.id)
                    .cloned()
                    .unwrap_or_default();
                handles.push(scope.spawn(move || {
                    let (outcome, new_state, adopted) =
                        refresh_one(client, rss_root, source, &state, now);
                    let _ = tx.send((source.id.clone(), outcome, new_state, adopted));
                }));
            }
            for handle in handles {
                let _ = handle.join();
            }
        }
        drop(tx);
    });

    let mut outcomes = Vec::with_capacity(targets.len());
    while let Ok((id, outcome, new_state, _adopted)) = rx.recv() {
        state_file.sources.insert(id.clone(), new_state);
        outcomes.push(outcome);
    }
    // 按配置顺序返回（线程完成顺序不定）
    let order: std::collections::HashMap<&str, usize> = targets
        .iter()
        .enumerate()
        .map(|(index, s)| (s.id.as_str(), index))
        .collect();
    outcomes.sort_by_key(|o| {
        order
            .get(o.source_id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    // 名字采纳结果通过 outcome.new_title 带回，由调用方落盘配置。
    outcomes
}

// ---------------------------------------------------------------------------
// App 层：路径解析 + Library 接线
// ---------------------------------------------------------------------------

fn library_data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位应用数据目录：{e}"))?
        .join("library");
    fs::create_dir_all(&dir).map_err(|e| format!("创建资料库数据目录失败：{e}"))?;
    Ok(dir)
}

pub fn sources_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(library_data_root(app)?.join("rss-sources.json"))
}

fn fetch_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(library_data_root(app)?.join("rss-fetch-state.json"))
}

/// `<AppData>/library/RSS/` — RSS 物化目录。
pub fn rss_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(library_data_root(app)?.join(RSS_LIBRARY_NAME))
}

/// 确保 RSS 物化目录存在并作为普通 Library source 注册（幂等）。
fn ensure_rss_library_source(app: &AppHandle, rss_root: &Path) -> Result<(), String> {
    fs::create_dir_all(rss_root).map_err(|e| format!("创建 RSS 物化目录失败：{e}"))?;
    let db_path = library::resolve_index_db(app)?;
    // 已注册则只刷新（增量，按 mtime/size 跳过未变文件），避免重复全量扫描。
    if library::find_source_by_root(&db_path, rss_root)?.is_none() {
        library::register_source_at(&db_path, rss_root)?;
    } else {
        library::refresh_at(&db_path)?;
    }
    Ok(())
}

/// 抓取后的 Library 索引刷新（物化文件 → library_documents + FTS + related FTS）。
fn refresh_library_index(app: &AppHandle) -> Result<library::LibraryRefreshResult, String> {
    let db_path = library::resolve_index_db(app)?;
    library::refresh_at(&db_path)
}

fn apply_adopted_titles(sources: &mut Vec<FeedSource>, outcomes: &[FeedRefreshOutcome]) -> bool {
    let mut changed = false;
    for outcome in outcomes {
        if let Some(new_title) = &outcome.new_title {
            if let Some(source) = sources.iter_mut().find(|s| s.id == outcome.source_id) {
                if source.name != *new_title {
                    source.name = new_title.clone();
                    changed = true;
                }
            }
        }
    }
    changed
}

fn view_of(source: &FeedSource, state: &FetchStateFile) -> FeedSourceView {
    let entry = state.sources.get(&source.id);
    FeedSourceView {
        id: source.id.clone(),
        name: source.name.clone(),
        url: source.url.clone(),
        last_fetch_at: entry.and_then(|e| e.last_fetch_at),
        last_error: entry.and_then(|e| e.last_error.clone()),
    }
}

pub fn list_sources(app: &AppHandle) -> Result<Vec<FeedSourceView>, String> {
    let config_path = sources_config_path(app)?;
    let sources = load_sources_file(&config_path)?;
    let state = load_state_file(&fetch_state_path(app)?)?;
    Ok(sources.iter().map(|s| view_of(s, &state)).collect())
}

/// 添加源：先持久化用户意图（哪怕首次抓取失败也保留），再后台抓取首轮内容。
pub fn add_source(
    app: &AppHandle,
    url: &str,
    name: Option<&str>,
) -> Result<FeedSourceView, String> {
    let canonical = canonicalize_feed_url(url)?;
    let config_path = sources_config_path(app)?;
    let mut sources = load_sources_file(&config_path)?;
    if sources.iter().any(|s| s.url == canonical) {
        return Err("该 Feed 已经存在".into());
    }
    let id = source_id(&canonical);
    let fallback_name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| url_host(&canonical).unwrap_or_else(|| "未命名源".to_string()));
    let source = FeedSource {
        id,
        name: fallback_name,
        url: canonical,
    };
    sources.push(source.clone());
    save_sources_file(&config_path, &sources)?;
    let state = load_state_file(&fetch_state_path(app)?)?;
    let view = view_of(&source, &state);

    // 首次抓取与标题采纳在后台执行；失败不撤销订阅。
    let app = app.clone();
    let source = source.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = refresh_source_impl(&app, &source.id) {
            eprintln!("Feed 首次抓取失败 [{}]: {error}", source.name);
        }
    });
    Ok(view)
}

/// 删除源：从配置删除、删除派生物化缓存目录、清理派生抓取状态，
/// 然后刷新 Library 索引。批注（属于用户劳动成果）保留。
pub fn remove_source(app: &AppHandle, id: &str) -> Result<(), String> {
    let config_path = sources_config_path(app)?;
    let mut sources = load_sources_file(&config_path)?;
    let Some(removed) = sources.iter().find(|s| s.id == id).cloned() else {
        return Err("Feed 源不存在".into());
    };
    sources.retain(|s| s.id != id);
    save_sources_file(&config_path, &sources)?;

    let state_path = fetch_state_path(app)?;
    let mut state = load_state_file(&state_path)?;
    state.sources.remove(id);
    save_state_file(&state_path, &state)?;

    let root = rss_root(app)?;
    let dir = root.join(&removed.id);
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|e| format!("删除 Feed 缓存目录失败：{e}"))?;
    }
    refresh_library_index(app)?;
    Ok(())
}

/// OPML 导入（merge 语义）：已存在 URL 跳过，坏 URL 不取消其他项。
pub fn import_opml(app: &AppHandle, path: Option<&Path>) -> Result<OpmlImportResult, String> {
    let Some(path) = path else {
        return Err("未选择 OPML 文件".into());
    };
    let xml = fs::read_to_string(path).map_err(|e| format!("读取 OPML 文件失败：{e}"))?;
    let entries = parse_opml(&xml).map_err(|e| format!("OPML 无法导入：{e}"))?;

    let config_path = sources_config_path(app)?;
    let mut sources = load_sources_file(&config_path)?;
    let mut existing: HashSet<String> = sources.iter().map(|s| s.url.clone()).collect();
    let mut added = 0usize;
    let mut duplicates = 0usize;
    let mut invalid = 0usize;
    let mut warnings = Vec::new();

    for (name, raw_url) in entries {
        let canonical = match canonicalize_feed_url(&raw_url) {
            Ok(url) => url,
            Err(error) => {
                invalid += 1;
                warnings.push(format!("{name}：{error}"));
                continue;
            }
        };
        if existing.contains(&canonical) {
            duplicates += 1;
            continue;
        }
        sources.push(FeedSource {
            id: source_id(&canonical),
            name: if name.trim().is_empty() {
                url_host(&canonical).unwrap_or_else(|| "未命名源".to_string())
            } else {
                name.trim().to_string()
            },
            url: canonical.clone(),
        });
        existing.insert(canonical);
        added += 1;
    }
    save_sources_file(&config_path, &sources)?;
    if added > 0 {
        let root = rss_root(app)?;
        if let Err(error) = ensure_rss_library_source(app, &root) {
            warnings.push(format!("注册 RSS 资料源失败：{error}"));
        }
    }
    let state = load_state_file(&fetch_state_path(app)?)?;
    let views = sources.iter().map(|s| view_of(s, &state)).collect();
    Ok(OpmlImportResult {
        added,
        duplicates,
        invalid,
        warnings,
        sources: views,
    })
}

/// 刷新单个源（app 层）。
pub fn refresh_source(app: &AppHandle, id: &str) -> Result<FeedRefreshOutcome, String> {
    refresh_source_impl(app, id)
}

fn refresh_source_impl(app: &AppHandle, id: &str) -> Result<FeedRefreshOutcome, String> {
    let config_path = sources_config_path(app)?;
    let mut sources = load_sources_file(&config_path)?;
    let state_path = fetch_state_path(app)?;
    let mut state = load_state_file(&state_path)?;

    let root = rss_root(app)?;
    ensure_rss_library_source(app, &root)?;

    let client = build_client()?;
    let outcomes = refresh_sources(&client, &root, &sources, &mut state, Some(id));
    let Some(outcome) = outcomes.into_iter().next() else {
        return Err("Feed 源不存在".into());
    };
    let _ = apply_adopted_titles(&mut sources, std::slice::from_ref(&outcome));
    save_sources_file(&config_path, &sources)?;
    state.last_global_refresh_at = Some(unix_timestamp());
    save_state_file(&state_path, &state)?;
    let _ = refresh_library_index(app);
    Ok(outcome)
}

/// 刷新全部源：4 路有界并发，失败源互不影响。
pub fn refresh_all(app: &AppHandle) -> Result<FeedRefreshResult, String> {
    let config_path = sources_config_path(app)?;
    let mut sources = load_sources_file(&config_path)?;
    let state_path = fetch_state_path(app)?;
    let mut state = load_state_file(&state_path)?;

    let root = rss_root(app)?;
    ensure_rss_library_source(app, &root)?;

    let client = build_client()?;
    let outcomes = refresh_sources(&client, &root, &sources, &mut state, None);
    apply_adopted_titles(&mut sources, &outcomes);
    save_sources_file(&config_path, &sources)?;
    state.last_global_refresh_at = Some(unix_timestamp());
    save_state_file(&state_path, &state)?;
    let _ = refresh_library_index(app);

    let mut result = FeedRefreshResult::default();
    for outcome in outcomes {
        result.materialized += outcome.added + outcome.updated;
        match outcome.status.as_str() {
            "error" => result.failed += 1,
            "unchanged" => result.unchanged += 1,
            _ => {}
        }
        result.added += outcome.added;
        result.updated += outcome.updated;
        result.sources.push(outcome);
    }
    Ok(result)
}

/// 汇总状态：源列表 + RSS Library source 视图 + 最近 RSS 资料。
pub fn status(app: &AppHandle) -> Result<FeedStatus, String> {
    let config_path = sources_config_path(app)?;
    let sources = load_sources_file(&config_path)?;
    let state = load_state_file(&fetch_state_path(app)?)?;
    let views: Vec<FeedSourceView> = sources.iter().map(|s| view_of(s, &state)).collect();

    let mut rss_library_source = None;
    let mut recent = Vec::new();
    if !sources.is_empty() {
        let root = rss_root(app)?;
        let db_path = library::resolve_index_db(app)?;
        // 幂等查询，不触发全量刷新（feed_status 会被频繁调用）
        if let Ok(Some(source)) = library::find_source_by_root(&db_path, &root) {
            rss_library_source = Some(source);
        } else if let Ok(result) = library::register_source_at(&db_path, &root) {
            rss_library_source = result
                .sources
                .into_iter()
                .find(|s| s.name == RSS_LIBRARY_NAME);
        }
        if let Some(library_source) = &rss_library_source {
            if let Ok(documents) = library::list_source_documents(&db_path, &library_source.id, 30)
            {
                recent = documents
                    .into_iter()
                    .filter_map(|document| {
                        let (feed_id, file) = document
                            .relative_path
                            .split_once('/')
                            .map(|(dir, file)| (Some(dir.to_string()), file.to_string()))
                            .unwrap_or((None, document.relative_path.clone()));
                        let date = file
                            .split("__")
                            .next()
                            .filter(|prefix| {
                                prefix.len() == 10
                                    && prefix.as_bytes().iter().enumerate().all(|(i, b)| match i {
                                        0..=3 | 5..=6 | 8..=9 => b.is_ascii_digit(),
                                        4 | 7 => *b == b'-',
                                        _ => false,
                                    })
                            })
                            .map(str::to_string);
                        let feed_name = feed_id
                            .as_ref()
                            .and_then(|id| sources.iter().find(|s| &s.id == id))
                            .map(|s| s.name.clone());
                        Some(RecentRssItem {
                            uri: document.uri.clone(),
                            source_id: document.source_id.clone(),
                            relative_path: document.relative_path.clone(),
                            content_hash: document.content_hash.clone(),
                            title: document.title.clone(),
                            snippet: document.snippet.clone(),
                            feed_id,
                            feed_name,
                            date,
                        })
                    })
                    .collect();
            }
        }
    }

    Ok(FeedStatus {
        sources: views,
        rss_library_source,
        recent,
        last_refresh_at: state.last_global_refresh_at,
    })
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc as test_mpsc;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "stillwrite-feeds-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const RSS_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>测试 Feed</title>
    <link>https://example.com/</link>
    <description>示例</description>
    <item>
      <title>第一篇：具身智能</title>
      <link>https://example.com/2026/08/26/embodied</link>
      <guid>urn:example:embodied-2026</guid>
      <pubDate>Tue, 26 Aug 2026 08:30:00 +0800</pubDate>
      <description><![CDATA[<p>制造现场正在变化。<b>机器人</b>开始接管。</p><script>alert('x')</script>]]></description>
      <enclosure url="https://example.com/podcast/ep1.mp3" type="audio/mpeg" length="1024"/>
    </item>
    <item>
      <title>第二篇：Agent 编排</title>
      <link>https://example.com/2026/08/25/agents</link>
      <guid>urn:example:agents-2026</guid>
      <pubDate>Mon, 25 Aug 2026 20:00:00 +0800</pubDate>
      <description>纯文本摘要，没有标签。</description>
    </item>
  </channel>
</rss>"#;

    const ATOM_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Atom 测试</title>
  <id>urn:example:atom-feed</id>
  <updated>2026-08-26T09:00:00Z</updated>
  <entry>
    <title>Atom 条目</title>
    <id>urn:example:atom-entry-1</id>
    <published>2026-08-26T07:00:00Z</published>
    <updated>2026-08-26T08:00:00Z</updated>
    <link rel="alternate" href="https://example.com/atom/1"/>
    <content type="html">&lt;p&gt;Atom 正文&lt;/p&gt;</content>
  </entry>
</feed>"#;

    const OPML_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>sources</title></head>
  <body>
    <outline text="组 A" title="组 A">
      <outline text="源一" title="源一" type="rss" xmlUrl="https://a.example.com/feed"/>
      <outline text="源二" title="源二" type="rss" xmlUrl="https://b.example.com/feed.xml"/>
    </outline>
    <outline text="无 URL" title="无 URL"/>
    <outline text="源一重复" title="源一重复" xmlUrl="https://a.example.com/feed#frag"/>
  </body>
</opml>"#;

    /// 极简 HTTP 服务：响应固定 body / 状态码，支持 ETag 条件请求。
    #[allow(dead_code)] // 字段仅供线程内部使用，测试只看 url()/hits()
    struct TestServer {
        addr: std::net::SocketAddr,
        body: Vec<u8>,
        status: u16,
        etag: Option<&'static str>,
        last_modified: Option<&'static str>,
        stop: test_mpsc::Sender<()>,
        request_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl TestServer {
        fn serve(
            body: &[u8],
            status: u16,
            etag: Option<&'static str>,
            last_modified: Option<&'static str>,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let body = body.to_vec();
            let body_for_thread = body.clone();
            let (stop_tx, stop_rx) = test_mpsc::channel::<()>();
            let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let count_for_thread = count.clone();
            thread::spawn(move || {
                listener.set_nonblocking(true).unwrap();
                loop {
                    if stop_rx.try_recv().is_ok() {
                        break;
                    }
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            count_for_thread.fetch_add(1, Ordering::Relaxed);
                            handle_connection(
                                &mut stream,
                                &body_for_thread,
                                status,
                                etag,
                                last_modified,
                            );
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });
            Self {
                addr,
                body,
                status,
                etag,
                last_modified,
                stop: stop_tx,
                request_count: count,
            }
        }

        fn url(&self) -> String {
            format!("http://{}/feed.xml", self.addr)
        }

        fn hits(&self) -> usize {
            self.request_count.load(Ordering::Relaxed)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            let _ = self.stop.send(());
        }
    }

    fn handle_connection(
        stream: &mut TcpStream,
        body: &[u8],
        status: u16,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) {
        use std::io::{BufRead, Write};
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }
        let mut headers = Vec::new();
        let mut line = String::new();
        while reader.read_line(&mut line).is_ok() && line != "\r\n" && !line.is_empty() {
            headers.push(line.trim_end().to_string());
            line.clear();
        }
        let has_if_none_match = headers
            .iter()
            .any(|h| h.to_ascii_lowercase().starts_with("if-none-match:"));
        let has_if_modified_since = headers
            .iter()
            .any(|h| h.to_ascii_lowercase().starts_with("if-modified-since:"));
        let mut response = String::from("HTTP/1.1 200 OK\r\n");
        if status == 304
            || (etag.is_some() && has_if_none_match && etag == self_etag(&headers).as_deref())
            || (last_modified.is_some() && has_if_modified_since)
        {
            response = format!("HTTP/1.1 304 Not Modified\r\n");
            if let Some(etag) = etag {
                response.push_str(&format!("ETag: {etag}\r\n"));
            }
            if let Some(lm) = last_modified {
                response.push_str(&format!("Last-Modified: {lm}\r\n"));
            }
            response.push_str("\r\n");
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        if status != 200 {
            response = format!("HTTP/1.1 {status} Error\r\nContent-Length: 0\r\n\r\n");
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        if let Some(etag) = etag {
            response.push_str(&format!("ETag: {etag}\r\n"));
        }
        if let Some(lm) = last_modified {
            response.push_str(&format!("Last-Modified: {lm}\r\n"));
        }
        response.push_str("Content-Type: application/rss+xml\r\n");
        response.push_str(&format!("Content-Length: {}\r\n", body.len()));
        response.push_str("Connection: close\r\n\r\n");
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(body);
    }

    fn self_etag(headers: &[String]) -> Option<String> {
        headers
            .iter()
            .find(|h| h.to_ascii_lowercase().starts_with("if-none-match:"))
            .map(|h| {
                h.split_once(':')
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default()
            })
            .filter(|v| !v.is_empty())
    }

    fn test_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .user_agent("stillwrite-test")
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap()
    }

    fn parse_fixture(xml: &str) -> feed_rs::model::Feed {
        feed_rs::parser::parse(xml.as_bytes()).expect("fixture 必须可解析")
    }

    // ---------- URL / ID ----------

    #[test]
    fn canonicalize_feed_url_rules() {
        assert_eq!(
            canonicalize_feed_url("  https://example.com/feed.xml#fragment  ").unwrap(),
            "https://example.com/feed.xml"
        );
        assert_eq!(
            canonicalize_feed_url("http://example.com/feed").unwrap(),
            "http://example.com/feed"
        );
        assert!(canonicalize_feed_url("file:///etc/passwd").is_err());
        assert!(canonicalize_feed_url("javascript:alert(1)").is_err());
        assert!(canonicalize_feed_url("ftp://example.com/feed").is_err());
        assert!(canonicalize_feed_url("").is_err());
        assert!(canonicalize_feed_url("not a url").is_err());
    }

    #[test]
    fn source_id_is_stable_and_name_independent() {
        let a = source_id("https://example.com/feed.xml");
        let b = source_id("https://example.com/feed.xml");
        let c = source_id("https://other.example.com/feed.xml");
        assert_eq!(a, b);
        assert_eq!(a.len(), 12);
        assert_ne!(a, c);
    }

    #[test]
    fn item_identity_prefers_guid_then_link_then_title_time() {
        let feed = parse_fixture(RSS_FIXTURE);
        let first = &feed.entries[0];
        assert_eq!(item_identity(first), "urn:example:embodied-2026");

        // guid 是 feed-rs 合成 32 hex 时回落到 canonical link
        let mut synth = first.clone();
        synth.id = "0123456789abcdef0123456789abcdef".to_string();
        assert_eq!(
            item_identity(&synth),
            "https://example.com/2026/08/26/embodied"
        );

        // 无 guid 无链接 → (标题 + 发布时间)
        let mut bare = first.clone();
        bare.id = "00000000-0000-0000-0000-000000000000".to_string();
        bare.links.clear();
        let identity = item_identity(&bare);
        assert!(identity.contains("具身智能"));
        // 同一输入两次结果稳定
        assert_eq!(identity, item_identity(&bare));
    }

    #[test]
    fn item_id_is_stable_short_hash() {
        let a = item_id("urn:example:embodied-2026");
        let b = item_id("urn:example:embodied-2026");
        assert_eq!(a, b);
        assert_eq!(a.len(), 8);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ---------- 文件名安全 ----------

    #[test]
    fn sanitize_filename_strips_dangerous_characters() {
        assert_eq!(sanitize_filename_part("../../evil"), "evil");
        assert_eq!(sanitize_filename_part("a/b:c*?d"), "a b c d");
        assert_eq!(sanitize_filename_part("..\\..\\win"), "win");
        assert_eq!(sanitize_filename_part("..."), "untitled");
        assert_eq!(sanitize_filename_part("正常标题"), "正常标题");
        let long = "x".repeat(200);
        assert_eq!(sanitize_filename_part(&long).chars().count(), 80);
        let weird = sanitize_filename_part("a<b>c|d:e");
        assert!(!weird.contains('/') && !weird.contains('\\') && !weird.contains(':'));
    }

    #[test]
    fn materialized_path_is_stable_and_never_escapes_root() {
        let feed = parse_fixture(RSS_FIXTURE);
        let source = FeedSource {
            id: "feedid1234".into(),
            name: "测试 Feed".into(),
            url: "https://example.com/feed.xml".into(),
        };
        let when = Local::now();
        let first = materialized_relative_path(&source, &feed.entries[0], when);
        let second = materialized_relative_path(&source, &feed.entries[0], when);
        assert_eq!(first, second, "同一 item 路径必须稳定");
        assert!(first.starts_with("feedid1234/"), "{first}");
        assert!(first.ends_with(".md"));
        assert!(first.contains("2026-08-26"), "{first}");
        assert!(first.contains("具身智能"), "{first}");
        // 不能以绝对路径或 .. 开头
        let path = Path::new(&first);
        assert!(!path.is_absolute());
        assert!(!first.contains(".."));
    }

    // ---------- OPML ----------

    #[test]
    fn parse_opml_reads_recursive_outlines() {
        let entries = parse_opml(OPML_FIXTURE).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0],
            ("源一".to_string(), "https://a.example.com/feed".to_string())
        );
        assert_eq!(entries[1].0, "源二");
        // 没有 xmlUrl 的 outline 被跳过
        assert!(!entries.iter().any(|(name, _)| name == "无 URL"));
        // title 优先于 text
        assert_eq!(entries[0].0, "源一");
    }

    #[test]
    fn parse_opml_rejects_non_opml_and_empty() {
        assert!(parse_opml("<html></html>").is_err());
        assert!(
            parse_opml("<?xml version=\"1.0\"?><opml version=\"2.0\"><body></body></opml>")
                .is_err()
        );
        assert!(parse_opml("not xml at all {{").is_err());
    }

    // ---------- Markdown 物化 ----------

    #[test]
    fn renders_item_markdown_with_headers_and_enclosure() {
        let feed = parse_fixture(RSS_FIXTURE);
        let source = FeedSource {
            id: "feedid1234".into(),
            name: "测试 Feed".into(),
            url: "https://example.com/feed.xml".into(),
        };
        let markdown = render_item_markdown(&source, &feed.entries[0]);
        assert!(markdown.starts_with("# 第一篇：具身智能"));
        assert!(markdown.contains("> 来源：测试 Feed"));
        assert!(markdown.contains("> 发布：2026-08-26"));
        assert!(markdown.contains("> 原文：https://example.com/2026/08/26/embodied"));
        assert!(markdown.contains("> Feed：https://example.com/feed.xml"));
        assert!(markdown.contains("制造现场正在变化"));
        assert!(markdown.contains("**机器人**"));
        // script 被剥离，正文不含恶意脚本
        assert!(!markdown.contains("alert"));
        assert!(markdown.contains("## 附件"));
        assert!(markdown.contains("[audio/mpeg](https://example.com/podcast/ep1.mp3)"));
    }

    #[test]
    fn renders_summary_only_without_faking_full_text() {
        let feed = parse_fixture(RSS_FIXTURE);
        let source = FeedSource {
            id: "feedid1234".into(),
            name: "测试 Feed".into(),
            url: "https://example.com/feed.xml".into(),
        };
        let markdown = render_item_markdown(&source, &feed.entries[1]);
        assert!(markdown.contains("纯文本摘要，没有标签。"));
        assert!(!markdown.contains("## 附件"));
    }

    #[test]
    fn renders_unknown_publish_and_no_section_when_missing() {
        let feed = parse_fixture(ATOM_FIXTURE);
        let source = FeedSource {
            id: "feedid1234".into(),
            name: "Atom 测试".into(),
            url: "https://example.com/atom.xml".into(),
        };
        let markdown = render_item_markdown(&source, &feed.entries[0]);
        assert!(markdown.contains("# Atom 条目"));
        assert!(markdown.contains("> 发布：2026-08-26"));
        assert!(markdown.contains("Atom 正文"));

        let mut bare = feed.entries[0].clone();
        bare.published = None;
        bare.updated = None;
        bare.content = None;
        bare.summary = None;
        let bare_md = render_item_markdown(&source, &bare);
        assert!(bare_md.contains("> 发布：未知"));
        assert!(bare_md.contains("(此 Feed 未提供正文或摘要，请打开原文)"));
    }

    #[test]
    fn strip_executable_html_removes_blocks() {
        assert_eq!(
            strip_executable_html("<p>a</p><script>alert('x')</script><p>b</p>"),
            "<p>a</p><p>b</p>"
        );
        assert_eq!(strip_executable_html("<style>body{}</style>ok"), "ok");
        assert_eq!(
            strip_executable_html("<div><iframe src=\"x\"></iframe></div>"),
            "<div></div>"
        );
        // 自闭合
        assert_eq!(
            strip_executable_html("<img src=\"a\"/><script/><p>t</p>"),
            "<img src=\"a\"/><p>t</p>"
        );
        // 未闭合的 script 清空到末尾
        assert_eq!(strip_executable_html("before<script>after"), "before");
        // 大小写不敏感
        assert_eq!(strip_executable_html("<SCRIPT>a</SCRIPT>ok"), "ok");
    }

    // ---------- 配置 / 状态文件 ----------

    #[test]
    fn config_roundtrip_and_atomic_write() {
        let dir = TempDir::new();
        let path = dir.path().join("rss-sources.json");
        let sources = vec![
            FeedSource {
                id: "abc".into(),
                name: "示例".into(),
                url: "https://example.com/feed".into(),
            },
            FeedSource {
                id: "def".into(),
                name: "示例二".into(),
                url: "https://example2.com/feed".into(),
            },
        ];
        save_sources_file(&path, &sources).unwrap();
        let loaded = load_sources_file(&path).unwrap();
        assert_eq!(loaded, sources);
        // 覆盖写入后无残留临时文件
        save_sources_file(&path, &sources[..1]).unwrap();
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        assert_eq!(load_sources_file(&path).unwrap().len(), 1);
    }

    #[test]
    fn missing_config_is_empty() {
        let dir = TempDir::new();
        assert!(load_sources_file(&dir.path().join("nope.json"))
            .unwrap()
            .is_empty());
    }

    // ---------- 端到端 refresh（本地 HTTP 服务） ----------

    fn setup_sources(url: &str, name: &str) -> FeedSource {
        FeedSource {
            id: source_id(url),
            name: name.into(),
            url: url.into(),
        }
    }

    #[test]
    fn refresh_materializes_items_and_is_idempotent() {
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();
        let server = TestServer::serve(RSS_FIXTURE.as_bytes(), 200, Some("\"v1\""), None);
        let source = setup_sources(&server.url(), &url_host(&server.url()).unwrap());
        let client = test_client();

        let mut state = FetchStateFile::default();
        let outcomes = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        assert_eq!(outcomes.len(), 1);
        let first = &outcomes[0];
        assert_eq!(first.status, "ok");
        assert_eq!(first.added, 2);
        assert_eq!(first.updated, 0);
        assert_eq!(first.unchanged, 0);
        assert_eq!(first.items, 2);
        // 初始名是占位（主机名），采纳 feed 标题
        assert_eq!(first.new_title.as_deref(), Some("测试 Feed"));

        // 文件落盘在 RSS/<id>/ 下
        let source_dir = rss_root.join(&source.id);
        let files: Vec<String> = fs::read_dir(&source_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(files.len(), 2);
        assert_eq!(
            state.sources.get(&source.id).unwrap().etag.as_deref(),
            Some("\"v1\"")
        );

        // 第二次刷新：条件请求命中 304 → 不重新物化、不重写任何文件
        let before = server.hits();
        let outcomes = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        let second = &outcomes[0];
        assert_eq!(second.status, "unchanged");
        assert_eq!(second.added, 0);
        assert_eq!(second.updated, 0);
        assert_eq!(server.hits(), before + 1);
        let files_after: Vec<String> = fs::read_dir(&source_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".md"))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(files_after.len(), 2);
    }

    #[test]
    fn repeated_200_identical_body_does_not_rewrite_or_duplicate() {
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();
        // 无 ETag 的服务器：每次都是 200 + 相同正文
        let server = TestServer::serve(RSS_FIXTURE.as_bytes(), 200, None, None);
        let source = setup_sources(&server.url(), "测试 Feed");
        let client = test_client();

        let mut state = FetchStateFile::default();
        let first = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        assert_eq!(first[0].added, 2);

        let second = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        assert_eq!(second[0].status, "ok");
        assert_eq!(second[0].added, 0);
        assert_eq!(second[0].updated, 0);
        assert_eq!(second[0].unchanged, 2, "正文一致时只计入 unchanged，不重写");

        let files: Vec<_> = fs::read_dir(rss_root.join(&source.id))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(files.len(), 2, "同一 item 不能产生第二个文件");
    }

    #[test]
    fn conditional_request_returns_304_without_rewrite() {
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();
        let server = TestServer::serve(
            RSS_FIXTURE.as_bytes(),
            200,
            Some("\"v1\""),
            Some("Mon, 01 Jan 2024 00:00:00 GMT"),
        );
        let source = setup_sources(&server.url(), "测试 Feed");
        let client = test_client();

        let mut state = FetchStateFile::default();
        let first = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        assert_eq!(first[0].added, 2);

        // 模拟服务器：带了条件头就 304（TestServer 固定行为）
        let second = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        assert_eq!(second[0].status, "unchanged");
        assert_eq!(second[0].added, 0);
        // 304 不应重写任何内容
        let files: Vec<_> = fs::read_dir(rss_root.join(&source.id))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 2);
        // etag 保留
        assert_eq!(
            state.sources.get(&source.id).unwrap().etag.as_deref(),
            Some("\"v1\"")
        );
    }

    #[test]
    fn atom_fixture_refresh_and_dedup() {
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();
        let server = TestServer::serve(ATOM_FIXTURE.as_bytes(), 200, None, None);
        let source = setup_sources(&server.url(), "Atom 测试");
        let client = test_client();

        let mut state = FetchStateFile::default();
        let outcomes = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        assert_eq!(outcomes[0].added, 1);
        let file = fs::read_dir(rss_root.join(&source.id))
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let markdown = fs::read_to_string(file.path()).unwrap();
        assert!(markdown.contains("# Atom 条目"));
        assert!(markdown.contains("> 来源：Atom 测试"));
        // Atom 条目的原标题来自文件名中间段
        assert!(file.file_name().to_string_lossy().contains("Atom 条目"));
    }

    #[test]
    fn one_failing_source_does_not_block_others() {
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();
        let good = TestServer::serve(RSS_FIXTURE.as_bytes(), 200, None, None);
        let bad = TestServer::serve(b"", 500, None, None);
        let timeout_server = TestServer::serve(b"", 200, None, None);
        // 第三个源：响应不结束（漏 Content-Length 但是超时）→ 用不存在的端口模拟连接失败更快
        drop(timeout_server);

        let sources = vec![
            setup_sources(&good.url(), "好源"),
            setup_sources(&bad.url(), "坏源"),
            setup_sources("http://127.0.0.1:1/feed.xml", "连不上"),
        ];
        let client = test_client();
        let mut state = FetchStateFile::default();
        let outcomes = refresh_sources(&client, &rss_root, &sources, &mut state, None);
        assert_eq!(outcomes.len(), 3);

        let good_outcome = outcomes.iter().find(|o| o.name == "好源").unwrap();
        assert_eq!(good_outcome.status, "ok");
        assert_eq!(good_outcome.added, 2);

        let bad_outcome = outcomes.iter().find(|o| o.name == "坏源").unwrap();
        assert_eq!(bad_outcome.status, "error");
        assert_eq!(bad_outcome.http_status, Some(500));

        let unreachable = outcomes.iter().find(|o| o.name == "连不上").unwrap();
        assert_eq!(unreachable.status, "error");

        // 好源的文件仍然物化
        let files: Vec<_> = fs::read_dir(rss_root.join(&sources[0].id))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn malformed_feed_reports_parse_error() {
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();
        let server = TestServer::serve(b"<rss><not-closed", 200, None, None);
        let source = setup_sources(&server.url(), "坏 xml");
        let client = test_client();

        let mut state = FetchStateFile::default();
        let outcomes = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        assert_eq!(outcomes[0].status, "error");
        assert!(outcomes[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("解析失败"));
        assert!(
            !rss_root.join(&source.id).exists()
                || fs::read_dir(rss_root.join(&source.id))
                    .unwrap()
                    .next()
                    .is_none()
        );
    }

    #[test]
    fn hostile_title_does_not_escape_root_and_giant_body_rejected() {
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();

        let hostile = format!(
            r#"<?xml version="1.0"?><rss version="2.0"><channel><title>h</title>
            <item><title>../../evil</title><link>https://e.com/1</link>
            <description>t</description></item>
            <item><title>a/b:c*?d</title><link>https://e.com/2</link>
            <description>t2</description></item>
            <item><link>https://e.com/3</link><description>t3</description></item>
            </channel></rss>"#
        );
        let server = TestServer::serve(hostile.as_bytes(), 200, None, None);
        let source = setup_sources(&server.url(), "敌意源");
        let client = test_client();
        let mut state = FetchStateFile::default();
        let outcomes = refresh_sources(&client, &rss_root, &[source.clone()], &mut state, None);
        assert_eq!(outcomes[0].status, "ok");
        assert_eq!(outcomes[0].added, 3);

        // 所有文件都在 RSS/<id>/ 下，没有逃逸到 rss_root 之外
        let source_dir = rss_root.join(&source.id);
        let names: Vec<String> = fs::read_dir(&source_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names.len(), 3);
        let outside: Vec<_> = fs::read_dir(&rss_root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != source.id.as_str())
            .collect();
        assert!(outside.is_empty(), "{outside:?}");
        for name in &names {
            assert!(!name.contains(".."));
            assert!(!name.contains('/') && !name.contains('\\'));
        }
        // 无标题条目使用占位名，但仍在目录内
        assert!(names.iter().any(|n| n.contains("无标题条目")));

        // 巨型响应拒绝（> MAX_FEED_BYTES）
        let huge = vec![b'x'; (MAX_FEED_BYTES as usize) + 1024];
        let big = TestServer::serve(&huge, 200, None, None);
        let big_source = setup_sources(&big.url(), "巨型源");
        let outcomes = refresh_sources(&client, &rss_root, &[big_source], &mut state, None);
        assert_eq!(outcomes[0].status, "error");
        assert!(outcomes[0].error.as_deref().unwrap_or("").contains("上限"));
    }

    #[test]
    fn redirect_loop_is_bounded_and_reported_as_error() {
        use std::io::Write as _;
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();

        // 302 永远指向自己 → 重定向环
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop_tx, stop_rx) = test_mpsc::channel::<()>();
        thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // 先读完请求再回应：避免客户端仍在写请求时连接被重置。
                        use std::io::BufRead as _;
                        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                        let mut line = String::new();
                        while reader.read_line(&mut line).is_ok()
                            && line != "\r\n"
                            && !line.is_empty()
                        {
                            line.clear();
                        }
                        let _ = stream.write_all(
                            b"HTTP/1.1 302 Found\r\nLocation: /loop\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(3));
                    }
                    Err(_) => break,
                }
            }
        });
        let url = format!("http://{addr}/loop");
        let source = setup_sources(&url, "重定向环");
        let id = source.id.clone();
        let client = test_client();
        let mut state = FetchStateFile::default();
        let outcomes = refresh_sources(&client, &rss_root, &[source], &mut state, None);
        assert_eq!(outcomes[0].status, "error");
        assert!(outcomes[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("重定向"));
        assert!(!rss_root.join(&id).exists());
        let _ = stop_tx.send(());
    }

    #[test]
    fn refresh_only_targets_requested_source() {
        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();
        let server_a = TestServer::serve(RSS_FIXTURE.as_bytes(), 200, None, None);
        let server_b = TestServer::serve(ATOM_FIXTURE.as_bytes(), 200, None, None);
        let a = setup_sources(&server_a.url(), "A");
        let b = setup_sources(&server_b.url(), "B");
        let client = test_client();
        let mut state = FetchStateFile::default();

        let outcomes = refresh_sources(
            &client,
            &rss_root,
            &[a.clone(), b.clone()],
            &mut state,
            Some(&a.id),
        );
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].name, "A");
        assert_eq!(server_b.hits(), 0, "未请求的源不应被访问");
    }

    // -----------------------------------------------------------------------
    // 真实网络验收（E2）：STILLWRITE_OPML=<spec pack 的 opml 路径> 时运行。
    // 与 App 内路径一致：parse_opml → refresh_sources(真实 HTTP 客户端) →
    // 物化 → library 注册/刷新 → 搜索/read_at 命中。
    // -----------------------------------------------------------------------

    #[test]
    #[ignore = "联网验收：需要 STILLWRITE_OPML 环境变量指向真实 OPML"]
    fn real_opml_acceptance() {
        let Some(opml_path) = std::env::var_os("STILLWRITE_OPML") else {
            eprintln!("跳过：未设置 STILLWRITE_OPML");
            return;
        };
        let xml = fs::read_to_string(&opml_path).expect("读取 OPML 失败");
        let entries = parse_opml(&xml).expect("解析 OPML 失败");
        // 按 spec fixture 的 id 命名与 fetch state 记账一致
        let sources: Vec<FeedSource> = entries
            .into_iter()
            .map(|(name, url)| {
                let canonical = canonicalize_feed_url(&url).expect("URL 规范化失败");
                FeedSource {
                    id: source_id(&canonical),
                    name,
                    url: canonical,
                }
            })
            .collect();

        let dir = TempDir::new();
        let rss_root = dir.path().join("RSS");
        fs::create_dir_all(&rss_root).unwrap();
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()
            .unwrap();
        let mut state = FetchStateFile::default();

        println!("== real OPML acceptance: {} sources ==", sources.len());
        let outcomes = refresh_sources(&client, &rss_root, &sources, &mut state, None);
        let mut success = 0usize;
        let mut http_error = 0usize;
        let mut parse_error = 0usize;
        let mut timeout_like = 0usize;
        let mut items = 0usize;
        let mut full_content = 0usize;
        let mut summary_only = 0usize;
        for outcome in &outcomes {
            let code = outcome
                .http_status
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".to_string());
            let result = match outcome.status.as_str() {
                "ok" => {
                    success += 1;
                    items += outcome.added + outcome.updated;
                    "OK".to_string()
                }
                "unchanged" => {
                    success += 1;
                    "UNCHANGED".to_string()
                }
                _ => {
                    let message = outcome.error.as_deref().unwrap_or("");
                    if message.contains("解析失败") {
                        parse_error += 1;
                    } else if message.contains("超时") || message.contains("连接失败") {
                        timeout_like += 1;
                    } else if message.contains("HTTP") {
                        http_error += 1;
                    } else {
                        timeout_like += 1;
                    }
                    "FAIL".to_string()
                }
            };
            println!(
                "{result}\tHTTP {code}\t+{} u{} c{} ({} items)\t{}\t{}",
                outcome.added,
                outcome.updated,
                outcome.unchanged,
                outcome.items,
                outcome.name,
                outcome.error.as_deref().unwrap_or("")
            );
        }

        // 抽查物化全文率：读每个源的前 1 个文件，判断是否含摘要占位声明
        let mut checked = 0usize;
        for outcome in &outcomes {
            if outcome.status != "ok" {
                continue;
            }
            let source_dir = rss_root.join(&outcome.source_id);
            let Ok(entries) = fs::read_dir(&source_dir) else {
                continue;
            };
            let mut first: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "md"))
                .collect();
            first.sort();
            let Some(path) = first.first() else {
                continue;
            };
            let content = fs::read_to_string(path).unwrap_or_default();
            if content.contains("此 Feed 未提供正文或摘要") {
                summary_only += 1;
            } else {
                full_content += 1;
            }
            checked += 1;
        }

        println!(
            "\ntotal={} success={} http_error={} parse_error={} timeout_like={} items_materialized={} full_content-ish={} summary_only-ish={} (checked {})",
            outcomes.len(),
            success,
            http_error,
            parse_error,
            timeout_like,
            items,
            full_content,
            summary_only,
            checked
        );

        // Library 接线：物化目录作为普通 Library source，搜索/read_at 命中
        let db_path = dir.path().join("library.db");
        let registered =
            crate::library::register_source_at(&db_path, &rss_root).expect("注册 RSS 资料源失败");
        println!(
            "library: sources={} documents={} unique={}",
            registered.sources.len(),
            registered.total_documents,
            registered.unique_documents
        );
        assert!(registered.sources.iter().any(|s| s.name == "RSS"));
        if registered.total_documents > 0 {
            let some =
                crate::library::list_source_documents(&db_path, &registered.sources[0].id, 5)
                    .expect("list_source_documents 失败");
            assert!(!some.is_empty());
            let first = &some[0];
            let document =
                crate::library::read_at(&db_path, &first.source_id, &first.relative_path)
                    .expect("read_at 失败");
            assert!(document.content.starts_with("# "));
            // 标题从文件名中间段摘出，不残留日期前缀
            assert!(!first.title.starts_with("20"));
            println!("library→read_at OK: {}", first.title);

            // 批注路径（E2）：对真实物化 material 写入/读取一条批注侧车。
            let annotation_root = dir.path().join("annotations");
            fs::create_dir_all(&annotation_root).unwrap();
            let body = "> 原文（字句）：\n> 测试批注\n\n验收批注正文。";
            let sidecar = crate::annotate::save_library_annotation(
                &annotation_root,
                &first.source_id,
                &first.content_hash,
                &first.title,
                &first.relative_path,
                body,
            )
            .expect("保存资料批注失败");
            assert!(sidecar.exists());
            let data = crate::annotate::read_library_annotation_data(
                &annotation_root,
                &first.source_id,
                &first.content_hash,
                &first.uri,
                &first.title,
                &first.relative_path,
            )
            .expect("读取资料批注失败");
            assert!(data.body.contains("验收批注正文"));
            println!("library→annotation OK: {}", data.title);
        }
    }
}
