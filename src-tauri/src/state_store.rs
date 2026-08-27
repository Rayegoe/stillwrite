//! StillWrite vNext P1：durable state 基础（`state.db`）。
//!
//! 与 `indexer.rs` / `library.rs` 的派生索引物理分离：
//! index.db 里全是可重建的 FTS/trigram，而本模块的表（events / anchors /
//! relations / context_* / web_search_*）是产品事实，不允许当作 sidecar 删除。
//!
//! 约定：
//! - durable state 位于应用数据目录下的单个 `state.db`，与 workspace 解耦
//!   （行内用 URI/workspace_id 区分来源，跨域 relation 才可能成立）；
//! - 所有写操作走 [`tx_command`]：同一事务内完成状态变更 + 追加语义事件，
//!   失败则整体回滚，事件不允许先于状态落库；
//! - P1 只建立 primitive 与事务边界，不改 UI、不迁移现有批注/Agent Work；
//!   P2 的 vertical slice 再逐一接入 Tauri command。
//!
//! 事件遵循“宁少勿多”：只为已经接入的 durable vertical slice 定义动作常量；
//! anchors 在批注迁移进 DB 前不产生事件。

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

// ---------------------------------------------------------------------------
// Migration runner
// ---------------------------------------------------------------------------

/// 单个 schema 版本。新版本只能追加到 MIGRATIONS 尾部，禁止修改历史 SQL。
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// 当前 schema 版本；测试可以据此构造“旧版本数据库再增量迁移”的场景。
pub const LATEST_SCHEMA_VERSION: i64 = 3;

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "core_events_anchors",
        sql: r#"
        CREATE TABLE events (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace_id  TEXT,
            actor_type    TEXT NOT NULL DEFAULT 'human',
            actor_id      TEXT,
            action        TEXT NOT NULL,
            object_uri    TEXT,
            target_uri    TEXT,
            thread_id     TEXT,
            work_id       TEXT,
            payload_json  TEXT,
            created_at    TEXT NOT NULL
        );
        CREATE INDEX idx_events_action ON events(action);
        CREATE INDEX idx_events_object_uri ON events(object_uri);
        CREATE TRIGGER events_no_update BEFORE UPDATE ON events
        BEGIN
            SELECT RAISE(ABORT, 'events 是 append-only，禁止 UPDATE');
        END;
        CREATE TRIGGER events_no_delete BEFORE DELETE ON events
        BEGIN
            SELECT RAISE(ABORT, 'events 是 append-only，禁止 DELETE');
        END;

        CREATE TABLE anchors (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            document_uri TEXT NOT NULL,
            kind         TEXT NOT NULL,
            start_offset INTEGER NOT NULL,
            end_offset   INTEGER NOT NULL,
            quote        TEXT NOT NULL DEFAULT '',
            prefix       TEXT,
            suffix       TEXT,
            created_at   TEXT NOT NULL,
            updated_at   TEXT NOT NULL
        );
        CREATE INDEX idx_anchors_document_uri ON anchors(document_uri);
        "#,
    },
    Migration {
        version: 2,
        name: "relations_context",
        sql: r#"
        CREATE TABLE relations (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            source_uri        TEXT NOT NULL,
            predicate         TEXT NOT NULL,
            target_uri        TEXT NOT NULL,
            anchor_id         INTEGER REFERENCES anchors(id),
            evidence_event_id INTEGER,
            created_by        TEXT NOT NULL DEFAULT 'human',
            confidence        REAL,
            created_at        TEXT NOT NULL,
            updated_at        TEXT NOT NULL
        );
        -- 同一三元组视为误操作重复提交，直接拒绝；不同 predicate 表达不同关系类型。
        CREATE UNIQUE INDEX idx_relations_triple ON relations(source_uri, predicate, target_uri);
        CREATE INDEX idx_relations_source ON relations(source_uri);
        CREATE INDEX idx_relations_target ON relations(target_uri);

        CREATE TABLE context_sets (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace_id TEXT,
            thread_id    TEXT,
            work_id      TEXT,
            purpose      TEXT,
            created_at   TEXT NOT NULL,
            updated_at   TEXT NOT NULL
        );

        CREATE TABLE context_items (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            context_id  INTEGER NOT NULL REFERENCES context_sets(id) ON DELETE CASCADE,
            object_uri  TEXT NOT NULL,
            anchor_id   INTEGER REFERENCES anchors(id),
            position    INTEGER NOT NULL,
            added_by    TEXT NOT NULL DEFAULT 'human',
            created_at  TEXT NOT NULL
        );
        CREATE UNIQUE INDEX idx_context_items_position ON context_items(context_id, position);
        CREATE INDEX idx_context_items_context ON context_items(context_id);
        "#,
    },
    Migration {
        version: 3,
        name: "web_search_history",
        sql: r#"
        CREATE TABLE web_searches (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace_id  TEXT,
            document_uri  TEXT,
            selected_text TEXT,
            query         TEXT NOT NULL,
            created_at    TEXT NOT NULL
        );
        CREATE INDEX idx_web_searches_workspace_created
            ON web_searches(workspace_id, id);
        CREATE INDEX idx_web_searches_document_created
            ON web_searches(document_uri, id);

        CREATE TABLE web_search_results (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            search_id   INTEGER NOT NULL REFERENCES web_searches(id) ON DELETE CASCADE,
            position    INTEGER NOT NULL,
            title       TEXT NOT NULL,
            url         TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            age         TEXT,
            created_at  TEXT NOT NULL
        );
        CREATE UNIQUE INDEX idx_web_search_results_position
            ON web_search_results(search_id, position);
        CREATE INDEX idx_web_search_results_search
            ON web_search_results(search_id, position);
        "#,
    },
];

#[derive(Debug)]
pub struct MigrationReport {
    pub applied_versions: Vec<i64>,
}

impl MigrationReport {
    pub fn applied_any(&self) -> bool {
        !self.applied_versions.is_empty()
    }
}

pub fn current_schema_version(conn: &Connection) -> Result<i64, String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("检查 schema_migrations 失败: {e}"))?;
    if !exists {
        return Ok(0);
    }
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .map_err(|e| format!("读取 schema 版本失败: {e}"))
}

/// 按版本顺序应用未执行的 migration，每个 migration 单独成一个事务：
/// 中途失败时已应用的版本保留，重启后从未完成的版本继续。
pub fn migrate(conn: &mut Connection) -> Result<MigrationReport, String> {
    // 版本登记表本身不进 migration 列表：runner 需要在任何版本执行前就能记录
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version     INTEGER PRIMARY KEY,
            name        TEXT NOT NULL,
            applied_at  TEXT NOT NULL
        );",
    )
    .map_err(|e| format!("创建 schema_migrations 失败: {e}"))?;
    let mut applied = Vec::new();
    for migration in MIGRATIONS {
        if migration.version <= current_schema_version(conn)? {
            continue;
        }
        let tx = conn
            .transaction()
            .map_err(|e| format!("开启 migration 事务失败: {e}"))?;
        tx.execute_batch(migration.sql).map_err(|e| {
            format!(
                "应用 migration v{}({}) 失败: {e}",
                migration.version, migration.name
            )
        })?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![migration.version, migration.name, now_iso()],
        )
        .map_err(|e| format!("记录 migration 版本失败: {e}"))?;
        tx.commit()
            .map_err(|e| format!("提交 migration v{} 失败: {e}", migration.version))?;
        applied.push(migration.version);
    }
    Ok(MigrationReport {
        applied_versions: applied,
    })
}

#[cfg(test)]
fn validate_migration_list() {
    for (index, migration) in MIGRATIONS.iter().enumerate() {
        assert_eq!(
            migration.version,
            index as i64 + 1,
            "migration 版本必须从 1 开始连续递增"
        );
        assert!(!migration.name.is_empty());
    }
}

/// 打开 durable state 数据库：WAL、外键约束、自动迁移到最新 schema。
pub fn open_state_db(path: &Path) -> Result<Connection, String> {
    let mut conn = Connection::open(path).map_err(|e| format!("打开 state.db 失败: {e}"))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {e}"))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| format!("启用外键约束失败: {e}"))?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// durable state 数据库路径：`<AppData>/state.db`。
///
/// 刻意不放在 `<AppData>/workspaces/<hash>/` 下——那里的 index.db 属于派生索引，
/// 用户或未来清理逻辑可能整目录删除；durable 数据不能随之丢失。
pub fn resolve_state_db(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位应用数据目录: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建应用数据目录失败: {e}"))?;
    Ok(dir.join("state.db"))
}

// ---------------------------------------------------------------------------
// ObjectUri
// ---------------------------------------------------------------------------

/// 跨模块共享的对象引用。P1 只做形状校验（validated wrapper），不做封闭 enum：
/// 保证入库的 URI 至少经过统一规则，避免各模块自由拼接出不可解析的字符串。
///
/// 形如 `scheme://rest`，scheme 为小写字母开头的小写字母/数字/短横线组合。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectUri(String);

impl ObjectUri {
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw != raw.trim() || raw.is_empty() {
            return Err(format!("对象 URI 不能为空或含首尾空白: '{raw}'"));
        }
        let Some((scheme, rest)) = raw.split_once("://") else {
            return Err(format!("对象 URI 必须形如 scheme://..., 实际为: '{raw}'"));
        };
        let valid_scheme = scheme
            .chars()
            .next()
            .map(|c| c.is_ascii_lowercase())
            .unwrap_or(false)
            && scheme
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid_scheme {
            return Err(format!("对象 URI scheme 非法: '{scheme}'"));
        }
        if rest.is_empty() || rest.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(format!("对象 URI 主体非法: '{raw}'"));
        }
        Ok(ObjectUri(raw.to_string()))
    }

    /// 工作区文档。路径使用工作区相对路径并统一 `/` 分隔符。
    pub fn workspace(relative_path: &str) -> Result<Self, String> {
        let rel = relative_path.trim().replace('\\', "/");
        let rel = rel.trim_start_matches('/');
        Self::parse(&format!("workspace://{rel}"))
    }

    /// 资料库文档：`library://<source_id>/<relative_path>`。
    pub fn library(source_id: &str, relative_path: &str) -> Result<Self, String> {
        let rel = relative_path
            .trim()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        Self::parse(&format!("library://{}/{}", source_id.trim(), rel))
    }

    pub fn anchor(anchor_id: i64) -> Self {
        ObjectUri(format!("anchor://{anchor_id}"))
    }

    pub fn relation(relation_id: i64) -> Self {
        ObjectUri(format!("relation://{relation_id}"))
    }

    pub fn context(context_set_id: i64) -> Self {
        ObjectUri(format!("context://{context_set_id}"))
    }

    pub fn web_search(search_id: i64) -> Self {
        ObjectUri(format!("search://{search_id}"))
    }

    pub fn web_search_result(result_id: i64) -> Self {
        ObjectUri(format!("search-result://{result_id}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn scheme(&self) -> &str {
        self.0.split_once("://").expect("ObjectUri 构造时已校验").0
    }

    /// `scheme://` 之后的部分。
    pub fn subject(&self) -> &str {
        self.0.split_once("://").expect("ObjectUri 构造时已校验").1
    }
}

impl std::fmt::Display for ObjectUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// 已接入 durable vertical slice 的语义事件。宁少勿多：其余动作在对应 slice
/// 迁移时再加入。
pub mod event_action {
    pub const RELATION_CREATED: &str = "relation.created";
    pub const RELATION_REMOVED: &str = "relation.removed";
    pub const CONTEXT_ATTACHED: &str = "context.attached";
    pub const CONTEXT_DETACHED: &str = "context.detached";
    pub const WEB_SEARCH_COMPLETED: &str = "web_search.completed";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Human,
    Agent,
}

impl ActorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ActorKind::Human => "human",
            ActorKind::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRecord {
    pub id: i64,
    pub workspace_id: Option<String>,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub action: String,
    pub object_uri: Option<String>,
    pub target_uri: Option<String>,
    pub thread_id: Option<String>,
    pub work_id: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewEvent {
    pub action: String,
    pub actor_type: Option<ActorKind>,
    pub actor_id: Option<String>,
    pub workspace_id: Option<String>,
    pub object_uri: Option<ObjectUri>,
    pub target_uri: Option<ObjectUri>,
    pub thread_id: Option<String>,
    pub work_id: Option<String>,
    pub payload: Option<serde_json::Value>,
}

/// 追加语义事件。只能在事务内调用——事件必须与引发它的状态变更同生共死。
pub fn append_event(tx: &Transaction, event: NewEvent) -> Result<i64, String> {
    if event.action.trim().is_empty() {
        return Err("语义事件缺少 action".into());
    }
    let payload_json = match &event.payload {
        Some(value) => {
            Some(serde_json::to_string(value).map_err(|e| format!("payload 序列化失败: {e}"))?)
        }
        None => None,
    };
    tx.execute(
        "INSERT INTO events (workspace_id, actor_type, actor_id, action, object_uri, target_uri, thread_id, work_id, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            event.workspace_id,
            event.actor_type.unwrap_or(ActorKind::Human).as_str(),
            event.actor_id,
            event.action.trim(),
            event.object_uri.as_ref().map(|u| u.as_str()),
            event.target_uri.as_ref().map(|u| u.as_str()),
            event.thread_id,
            event.work_id,
            payload_json,
            now_iso(),
        ],
    )
    .map_err(|e| format!("写入语义事件失败: {e}"))?;
    Ok(tx.last_insert_rowid())
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    let payload_json: Option<String> = row.get(9)?;
    Ok(EventRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        actor_type: row.get(2)?,
        actor_id: row.get(3)?,
        action: row.get(4)?,
        object_uri: row.get(5)?,
        target_uri: row.get(6)?,
        thread_id: row.get(7)?,
        work_id: row.get(8)?,
        payload: payload_json.and_then(|json| serde_json::from_str(&json).ok()),
        created_at: row.get(10)?,
    })
}

const EVENT_COLUMNS: &str = "id, workspace_id, actor_type, actor_id, action, object_uri, target_uri, thread_id, work_id, payload_json, created_at";

/// 最近事件（新的在前）。
pub fn list_events(conn: &Connection, limit: usize) -> Result<Vec<EventRecord>, String> {
    let sql = format!("SELECT {EVENT_COLUMNS} FROM events ORDER BY id DESC LIMIT ?1");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询事件失败: {e}"))?;
    let rows = stmt
        .query_map(params![limit as i64], row_to_event)
        .map_err(|e| format!("查询事件失败: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// 按对象 URI 过滤事件（object 或 target 任一端命中）。
pub fn events_for_object(
    conn: &Connection,
    uri: &str,
    limit: usize,
) -> Result<Vec<EventRecord>, String> {
    let sql = format!(
        "SELECT {EVENT_COLUMNS} FROM events WHERE object_uri = ?1 OR target_uri = ?1 ORDER BY id DESC LIMIT ?2"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询事件失败: {e}"))?;
    let rows = stmt
        .query_map(params![uri, limit as i64], row_to_event)
        .map_err(|e| format!("查询事件失败: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ---------------------------------------------------------------------------
// Web search history
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchHistoryRecord {
    pub id: i64,
    pub workspace_id: Option<String>,
    pub document_uri: Option<String>,
    pub selected_text: Option<String>,
    pub query: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewWebSearchResult {
    pub title: String,
    pub url: String,
    pub description: String,
    pub age: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewWebSearch {
    pub workspace_id: Option<String>,
    pub document_uri: Option<ObjectUri>,
    pub selected_text: Option<String>,
    pub query: String,
    pub results: Vec<NewWebSearchResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResultRecord {
    pub id: i64,
    pub search_id: i64,
    pub position: i64,
    pub title: String,
    pub url: String,
    pub description: String,
    pub age: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchHistoryView {
    pub id: i64,
    pub workspace_id: Option<String>,
    pub document_uri: Option<String>,
    pub selected_text: Option<String>,
    pub query: String,
    pub created_at: String,
    pub results: Vec<WebSearchResultRecord>,
}

fn row_to_web_search_history(row: &rusqlite::Row<'_>) -> rusqlite::Result<WebSearchHistoryRecord> {
    Ok(WebSearchHistoryRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        document_uri: row.get(2)?,
        selected_text: row.get(3)?,
        query: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn row_to_web_search_result(row: &rusqlite::Row<'_>) -> rusqlite::Result<WebSearchResultRecord> {
    Ok(WebSearchResultRecord {
        id: row.get(0)?,
        search_id: row.get(1)?,
        position: row.get(2)?,
        title: row.get(3)?,
        url: row.get(4)?,
        description: row.get(5)?,
        age: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn get_web_search_record(
    conn: &Connection,
    search_id: i64,
) -> Result<Option<WebSearchHistoryRecord>, String> {
    conn.query_row(
        "SELECT id, workspace_id, document_uri, selected_text, query, created_at
         FROM web_searches WHERE id = ?1",
        params![search_id],
        row_to_web_search_history,
    )
    .optional()
    .map_err(|e| format!("查询网页搜索历史失败: {e}"))
}

fn list_web_search_results(
    conn: &Connection,
    search_id: i64,
) -> Result<Vec<WebSearchResultRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, search_id, position, title, url, description, age, created_at
             FROM web_search_results WHERE search_id = ?1 ORDER BY position ASC",
        )
        .map_err(|e| format!("查询网页搜索结果失败: {e}"))?;
    let rows = stmt
        .query_map(params![search_id], row_to_web_search_result)
        .map_err(|e| format!("查询网页搜索结果失败: {e}"))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("读取网页搜索结果失败: {e}"))
}

pub fn get_web_search(
    conn: &Connection,
    search_id: i64,
) -> Result<Option<WebSearchHistoryView>, String> {
    let Some(history) = get_web_search_record(conn, search_id)? else {
        return Ok(None);
    };
    let results = list_web_search_results(conn, search_id)?;
    Ok(Some(WebSearchHistoryView {
        id: history.id,
        workspace_id: history.workspace_id,
        document_uri: history.document_uri,
        selected_text: history.selected_text,
        query: history.query,
        created_at: history.created_at,
        results,
    }))
}

pub fn list_web_search_history(
    conn: &Connection,
    workspace_id: Option<&str>,
    limit: usize,
) -> Result<Vec<WebSearchHistoryView>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, workspace_id, document_uri, selected_text, query, created_at
             FROM web_searches
             WHERE (?1 IS NULL OR workspace_id = ?1)
             ORDER BY id DESC LIMIT ?2",
        )
        .map_err(|e| format!("查询网页搜索历史失败: {e}"))?;
    let rows = stmt
        .query_map(
            params![workspace_id, limit as i64],
            row_to_web_search_history,
        )
        .map_err(|e| format!("查询网页搜索历史失败: {e}"))?;
    let histories = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("读取网页搜索历史失败: {e}"))?;
    histories
        .into_iter()
        .map(|history| {
            let results = list_web_search_results(conn, history.id)?;
            Ok(WebSearchHistoryView {
                id: history.id,
                workspace_id: history.workspace_id,
                document_uri: history.document_uri,
                selected_text: history.selected_text,
                query: history.query,
                created_at: history.created_at,
                results,
            })
        })
        .collect()
}

fn create_web_search_in_tx(
    tx: &Transaction,
    input: NewWebSearch,
) -> Result<WebSearchHistoryView, String> {
    let NewWebSearch {
        workspace_id,
        document_uri,
        selected_text,
        query,
        results,
    } = input;
    let query = query.trim().to_string();
    if query.is_empty() {
        return Err("网页搜索历史缺少 query".into());
    }
    if results.len() > 100 {
        return Err("网页搜索结果数量超过限制".into());
    }
    let selected_text = selected_text.and_then(|text| {
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    let created_at = now_iso();
    tx.execute(
        "INSERT INTO web_searches
            (workspace_id, document_uri, selected_text, query, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            workspace_id,
            document_uri.as_ref().map(ObjectUri::as_str),
            selected_text,
            query,
            created_at,
        ],
    )
    .map_err(|e| format!("写入网页搜索历史失败: {e}"))?;
    let search_id = tx.last_insert_rowid();
    for (position, result) in results.iter().enumerate() {
        let title = result.title.trim();
        let url = result.url.trim();
        if title.is_empty() {
            return Err("网页搜索结果缺少标题".into());
        }
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(format!("网页搜索结果 URL 非法: {url}"));
        }
        tx.execute(
            "INSERT INTO web_search_results
                (search_id, position, title, url, description, age, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                search_id,
                position as i64,
                title,
                url,
                result.description.trim(),
                result
                    .age
                    .as_deref()
                    .map(str::trim)
                    .filter(|age| !age.is_empty()),
                created_at,
            ],
        )
        .map_err(|e| format!("写入网页搜索结果失败: {e}"))?;
    }

    let mut payload = serde_json::json!({
        "query": query,
        "result_count": results.len(),
    });
    if let Some(selected_text) = selected_text.as_deref() {
        payload["selected_text"] = serde_json::Value::String(selected_text.to_string());
    }
    append_event(
        tx,
        NewEvent {
            action: event_action::WEB_SEARCH_COMPLETED.to_string(),
            workspace_id: workspace_id.clone(),
            object_uri: Some(ObjectUri::web_search(search_id)),
            target_uri: document_uri.clone(),
            payload: Some(payload),
            ..NewEvent::default()
        },
    )?;
    get_web_search(tx, search_id)?.ok_or_else(|| "回读网页搜索历史失败".into())
}

pub fn create_web_search(
    conn: &mut Connection,
    input: NewWebSearch,
) -> Result<WebSearchHistoryView, String> {
    tx_command(conn, move |tx| create_web_search_in_tx(tx, input))
}

pub fn get_web_search_result(
    conn: &Connection,
    result_id: i64,
) -> Result<Option<(WebSearchHistoryView, WebSearchResultRecord)>, String> {
    let search_id: Option<i64> = conn
        .query_row(
            "SELECT search_id FROM web_search_results WHERE id = ?1",
            params![result_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("查询网页搜索结果归属失败: {e}"))?;
    let Some(search_id) = search_id else {
        return Ok(None);
    };
    let Some(history) = get_web_search(conn, search_id)? else {
        return Ok(None);
    };
    let result = history
        .results
        .iter()
        .find(|item| item.id == result_id)
        .cloned();
    let Some(result) = result else {
        return Ok(None);
    };
    Ok(Some((history, result)))
}

/// 查询某篇笔记已经关联的网页搜索结果 ID。
///
/// `relations` 本身沿用跨域 URI 模型，没有重复存 workspace_id；查询时通过
/// search-result 的所属历史回连 workspace，避免两个工作区使用同一相对路径时串出关联。
pub fn web_search_result_links(
    conn: &Connection,
    source_uri: &str,
    workspace_id: Option<&str>,
) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT r.target_uri
             FROM relations r
             JOIN web_search_results sr
               ON sr.id = CAST(substr(r.target_uri, 17) AS INTEGER)
             JOIN web_searches ws ON ws.id = sr.search_id
             WHERE r.source_uri = ?1 AND r.predicate = 'related_to'
               AND r.target_uri LIKE 'search-result://%'
               AND (?2 IS NULL OR ws.workspace_id = ?2)
             ORDER BY r.id ASC",
        )
        .map_err(|e| format!("查询网页搜索关联失败: {e}"))?;
    let rows = stmt
        .query_map(params![source_uri, workspace_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| format!("查询网页搜索关联失败: {e}"))?;
    let ids = rows
        .filter_map(|row| row.ok())
        .filter_map(|uri| uri.strip_prefix("search-result://")?.parse::<i64>().ok())
        .collect::<Vec<_>>();
    Ok(ids)
}

// ---------------------------------------------------------------------------
// Anchors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct AnchorRecord {
    pub id: i64,
    pub document_uri: String,
    pub kind: String,
    pub start_offset: i64,
    pub end_offset: i64,
    pub quote: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewAnchor {
    pub document_uri: ObjectUri,
    /// 选区种类，沿用现有批注行为：'字句' | '段落' 等。
    pub kind: String,
    pub start_offset: i64,
    pub end_offset: i64,
    pub quote: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
}

/// 事务内创建 Anchor。P1 不产生事件：anchor 的事件语义归属未来的批注 vertical slice。
/// 需要与其它 primitive 在同一事务中组合时（如未来的批注迁移命令），直接调用本函数。
pub fn create_anchor_in_tx(tx: &Transaction, anchor: NewAnchor) -> Result<AnchorRecord, String> {
    if anchor.kind.trim().is_empty() {
        return Err("Anchor 缺少 kind".into());
    }
    if anchor.end_offset < anchor.start_offset {
        return Err(format!(
            "Anchor 范围非法: start={} > end={}",
            anchor.start_offset, anchor.end_offset
        ));
    }
    let now = now_iso();
    tx.execute(
        "INSERT INTO anchors (document_uri, kind, start_offset, end_offset, quote, prefix, suffix, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            anchor.document_uri.as_str(),
            anchor.kind.trim(),
            anchor.start_offset,
            anchor.end_offset,
            anchor.quote,
            anchor.prefix,
            anchor.suffix,
            now,
        ],
    )
    .map_err(|e| format!("写入 anchor 失败: {e}"))?;
    get_anchor_in_tx(tx, tx.last_insert_rowid())
}

/// 单命令入口：单独事务中创建 anchor。
pub fn create_anchor(conn: &mut Connection, anchor: NewAnchor) -> Result<AnchorRecord, String> {
    tx_command(conn, move |tx| create_anchor_in_tx(tx, anchor))
}

fn row_to_anchor(row: &rusqlite::Row<'_>) -> rusqlite::Result<AnchorRecord> {
    Ok(AnchorRecord {
        id: row.get(0)?,
        document_uri: row.get(1)?,
        kind: row.get(2)?,
        start_offset: row.get(3)?,
        end_offset: row.get(4)?,
        quote: row.get(5)?,
        prefix: row.get(6)?,
        suffix: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

const ANCHOR_COLUMNS: &str =
    "id, document_uri, kind, start_offset, end_offset, quote, prefix, suffix, created_at, updated_at";

fn get_anchor_in_tx(tx: &Transaction, anchor_id: i64) -> Result<AnchorRecord, String> {
    let sql = format!("SELECT {ANCHOR_COLUMNS} FROM anchors WHERE id = ?1");
    tx.query_row(&sql, params![anchor_id], row_to_anchor)
        .optional()
        .map_err(|e| format!("查询 anchor 失败: {e}"))?
        .ok_or_else(|| format!("anchor 不存在: {anchor_id}"))
}

pub fn get_anchor(conn: &Connection, anchor_id: i64) -> Result<Option<AnchorRecord>, String> {
    let sql = format!("SELECT {ANCHOR_COLUMNS} FROM anchors WHERE id = ?1");
    conn.query_row(&sql, params![anchor_id], row_to_anchor)
        .optional()
        .map_err(|e| format!("查询 anchor 失败: {e}"))
}

/// 列出一个文档上的全部 anchor（按创建顺序）。同一选区重复创建不去重：
/// 上游 UI 明确触发两次就是两条记录，是否合并留给批注 slice 的产品设计。
pub fn anchors_for_document(
    conn: &Connection,
    document_uri: &str,
) -> Result<Vec<AnchorRecord>, String> {
    let sql =
        format!("SELECT {ANCHOR_COLUMNS} FROM anchors WHERE document_uri = ?1 ORDER BY id ASC");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询 anchor 失败: {e}"))?;
    let rows = stmt
        .query_map(params![document_uri], row_to_anchor)
        .map_err(|e| format!("查询 anchor 失败: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ---------------------------------------------------------------------------
// Relations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RelationRecord {
    pub id: i64,
    pub source_uri: String,
    pub predicate: String,
    pub target_uri: String,
    pub anchor_id: Option<i64>,
    pub evidence_event_id: Option<i64>,
    pub created_by: String,
    pub confidence: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewRelation {
    /// 粒度原则：选区级 Relation 直接用 `anchor://<id>` 作为 source_uri；
    /// 只有整篇文档级别的 Relation 才用 `workspace://...` / `library://...`。
    /// 不要用 `document_uri + anchor_id` 组合表达选区关系——三元组唯一索引
    /// 会把同一文档不同选区对同一目标的关系错误合并。
    pub source_uri: ObjectUri,
    pub predicate: String,
    pub target_uri: ObjectUri,
    /// 仅作展示/回溯辅助（例如从锚点详情跳回正文），不参与唯一性判定。
    pub anchor_id: Option<i64>,
    pub created_by: Option<String>,
    pub confidence: Option<f64>,
    pub workspace_id: Option<String>,
    /// 卡片等 UI 投影快照。只进 relation.created 事件的 payload，
    /// 不污染 relations 行本身（行永远只有 URI 与事实字段）。
    pub snapshot: Option<serde_json::Value>,
}

const RELATION_COLUMNS: &str = "id, source_uri, predicate, target_uri, anchor_id, evidence_event_id, created_by, confidence, created_at, updated_at";

fn row_to_relation(row: &rusqlite::Row<'_>) -> rusqlite::Result<RelationRecord> {
    Ok(RelationRecord {
        id: row.get(0)?,
        source_uri: row.get(1)?,
        predicate: row.get(2)?,
        target_uri: row.get(3)?,
        anchor_id: row.get(4)?,
        evidence_event_id: row.get(5)?,
        created_by: row.get(6)?,
        confidence: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// 事务内创建 relation 并追加 `relation.created` 事件。
/// 事件 object=relation://<id>，target=对端对象，便于从任一端回溯成因。
pub fn create_relation_in_tx(
    tx: &Transaction,
    relation: NewRelation,
) -> Result<RelationRecord, String> {
    let predicate = relation.predicate.trim().to_string();
    if predicate.is_empty() {
        return Err("Relation 缺少 predicate".into());
    }
    if relation.source_uri == relation.target_uri {
        return Err("Relation 不能指向自身".into());
    }
    let now = now_iso();
    tx.execute(
        "INSERT INTO relations (source_uri, predicate, target_uri, anchor_id, created_by, confidence, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, COALESCE(?5, 'human'), ?6, ?7, ?7)",
        params![
            relation.source_uri.as_str(),
            predicate,
            relation.target_uri.as_str(),
            relation.anchor_id,
            relation.created_by,
            relation.confidence,
            now,
        ],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            format!("相同的关联已存在: {} -{}-> {}", relation.source_uri, predicate, relation.target_uri)
        } else {
            format!("写入 relation 失败: {e}")
        }
    })?;
    let record = {
        let sql = format!("SELECT {RELATION_COLUMNS} FROM relations WHERE id = ?1");
        tx.query_row(&sql, params![tx.last_insert_rowid()], row_to_relation)
            .map_err(|e| format!("回读 relation 失败: {e}"))?
    };
    // 同一事务内追加事件；insert 失败会让整个命令一起回滚。
    let event_id = append_event(
        tx,
        NewEvent {
            action: event_action::RELATION_CREATED.to_string(),
            workspace_id: relation.workspace_id.clone(),
            object_uri: Some(ObjectUri::relation(record.id)),
            target_uri: Some(relation.target_uri.clone()),
            payload: Some({
                let mut payload = serde_json::json!({
                    "predicate": record.predicate,
                    "source_uri": record.source_uri,
                    "target_uri": record.target_uri,
                });
                if let Some(snapshot) = relation.snapshot {
                    payload["snapshot"] = snapshot;
                }
                payload
            }),
            ..NewEvent::default()
        },
    )?;
    tx.execute(
        "UPDATE relations SET evidence_event_id = ?1 WHERE id = ?2",
        params![event_id, record.id],
    )
    .map_err(|e| format!("绑定 relation 证据事件失败: {e}"))?;
    Ok(RelationRecord {
        evidence_event_id: Some(event_id),
        ..record
    })
}

/// 单命令入口：创建 relation 并在同一事务中追加 `relation.created` 事件。
pub fn create_relation(
    conn: &mut Connection,
    relation: NewRelation,
) -> Result<RelationRecord, String> {
    tx_command(conn, move |tx| create_relation_in_tx(tx, relation))
}

/// 事务内删除 relation 并追加 `relation.removed` 事件。
/// 关系状态消失后事件仍可查询——append-only log 是删除操作的唯一痕迹。
pub fn remove_relation_in_tx(tx: &Transaction, relation_id: i64) -> Result<(), String> {
    let sql = format!("SELECT {RELATION_COLUMNS} FROM relations WHERE id = ?1");
    let record = tx
        .query_row(&sql, params![relation_id], row_to_relation)
        .optional()
        .map_err(|e| format!("查询 relation 失败: {e}"))?
        .ok_or_else(|| format!("relation 不存在: {relation_id}"))?;
    tx.execute("DELETE FROM relations WHERE id = ?1", params![relation_id])
        .map_err(|e| format!("删除 relation 失败: {e}"))?;
    append_event(
        tx,
        NewEvent {
            action: event_action::RELATION_REMOVED.to_string(),
            object_uri: Some(ObjectUri::relation(record.id)),
            target_uri: ObjectUri::parse(&record.target_uri).ok(),
            // relation 行删除后即不可查询，快照必须留在事件里供未来 Memory/Work 审计
            payload: Some(serde_json::json!({
                "predicate": record.predicate,
                "source_uri": record.source_uri,
                "target_uri": record.target_uri,
                "removed_evidence_event_id": record.evidence_event_id,
            })),
            ..NewEvent::default()
        },
    )?;
    Ok(())
}

/// 单命令入口：删除 relation（状态消失，removed 事件留存）。
pub fn remove_relation(conn: &mut Connection, relation_id: i64) -> Result<(), String> {
    tx_command(conn, move |tx| remove_relation_in_tx(tx, relation_id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationDirection {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone, Serialize)]
pub struct NeighborHit {
    pub relation: RelationRecord,
    /// 对端对象：outgoing 时是 target，incoming 时是 source。
    pub neighbor_uri: String,
    pub direction: RelationDirection,
}

/// 查询某对象的全部邻居边（双向），支持按 predicate 过滤；结果按 relation.id 排序。
pub fn neighbors(
    conn: &Connection,
    uri: &str,
    predicate: Option<&str>,
    limit: usize,
) -> Result<Vec<NeighborHit>, String> {
    // P1 数据量下在 Rust 侧合并/截断即可，避免动态拼 LIMIT 占位符带来的绑定错误
    let fetch_side = |column: &str,
                      direction: RelationDirection|
     -> Result<Vec<NeighborHit>, String> {
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<NeighborHit> {
            let record = row_to_relation(row)?;
            let neighbor_uri = if direction == RelationDirection::Outgoing {
                record.target_uri.clone()
            } else {
                record.source_uri.clone()
            };
            Ok(NeighborHit {
                relation: record,
                neighbor_uri,
                direction,
            })
        };
        let rows = match predicate {
            Some(predicate) => {
                let sql = format!(
                    "SELECT {RELATION_COLUMNS} FROM relations WHERE {column} = ?1 AND predicate = ?2 ORDER BY id ASC"
                );
                let mut stmt = conn
                    .prepare(&sql)
                    .map_err(|e| format!("查询邻居失败: {e}"))?;
                let rows = stmt
                    .query_map(params![uri, predicate], map_row)
                    .map_err(|e| format!("查询邻居失败: {e}"))?;
                rows.filter_map(|r| r.ok()).collect()
            }
            None => {
                let sql = format!(
                    "SELECT {RELATION_COLUMNS} FROM relations WHERE {column} = ?1 ORDER BY id ASC"
                );
                let mut stmt = conn
                    .prepare(&sql)
                    .map_err(|e| format!("查询邻居失败: {e}"))?;
                let rows = stmt
                    .query_map(params![uri], map_row)
                    .map_err(|e| format!("查询邻居失败: {e}"))?;
                rows.filter_map(|r| r.ok()).collect()
            }
        };
        Ok(rows)
    };

    let mut hits = fetch_side("source_uri", RelationDirection::Outgoing)?;
    hits.extend(fetch_side("target_uri", RelationDirection::Incoming)?);
    hits.sort_by_key(|hit| hit.relation.id);
    hits.truncate(limit);
    Ok(hits)
}

/// 按 (source, predicate, target) 精确查找 relation；不存在返回 None。
/// unpin / 幂等导入都依赖它，避免依赖 SQLite 错误字符串做控制流。
pub fn find_relation(
    conn: &Connection,
    source_uri: &str,
    predicate: &str,
    target_uri: &str,
) -> Result<Option<RelationRecord>, String> {
    let sql = format!(
        "SELECT {RELATION_COLUMNS} FROM relations WHERE source_uri = ?1 AND predicate = ?2 AND target_uri = ?3"
    );
    conn.query_row(
        &sql,
        params![source_uri, predicate, target_uri],
        row_to_relation,
    )
    .optional()
    .map_err(|e| format!("查询 relation 失败: {e}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedRelationView {
    pub relation: RelationRecord,
    /// 创建关系时随 relation.created 事件保存的展示快照（标题/摘要/来源等）。
    /// relation 行只存 URI；卡片渲染所需的投影数据以事件 payload 为证据留档。
    pub snapshot: Option<serde_json::Value>,
}

/// 列出某 scope/source 下指定 predicate 的全部关系（按创建顺序），
/// 并从各自的证据事件中还原展示快照。
pub fn list_relation_snapshots(
    conn: &Connection,
    source_uri: &str,
    predicate: &str,
) -> Result<Vec<PinnedRelationView>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT r.id, r.source_uri, r.predicate, r.target_uri, r.anchor_id, r.evidence_event_id,
                    r.created_by, r.confidence, r.created_at, r.updated_at
             FROM relations r
             WHERE r.source_uri = ?1 AND r.predicate = ?2
             ORDER BY r.id ASC"
        ))
        .map_err(|e| format!("查询固定关联失败: {e}"))?;
    let rows = stmt
        .query_map(params![source_uri, predicate], |row| {
            let relation = row_to_relation(row)?;
            Ok(relation)
        })
        .map_err(|e| format!("查询固定关联失败: {e}"))?;
    let relations: Vec<RelationRecord> = rows.filter_map(|r| r.ok()).collect();

    let mut views = Vec::with_capacity(relations.len());
    for relation in relations {
        let snapshot = match relation.evidence_event_id {
            Some(event_id) => {
                let payload_json: Option<String> = conn
                    .query_row(
                        "SELECT payload_json FROM events WHERE id = ?1",
                        params![event_id],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|e| format!("读取证据事件失败: {e}"))?;
                payload_json
                    .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
                    .and_then(|payload| payload.get("snapshot").cloned().filter(|s| !s.is_null()))
            }
            None => None,
        };
        views.push(PinnedRelationView { relation, snapshot });
    }
    Ok(views)
}

#[derive(Debug, Clone)]
pub struct RelationLinkImport {
    pub target_uri: ObjectUri,
    pub created_by: Option<String>,
    pub workspace_id: Option<String>,
    pub snapshot: Option<serde_json::Value>,
}

/// 幂等批量导入「scope → predicate → target」关系：已存在的三元组静默跳过。
/// legacy localStorage 迁移必须满足“重复执行不产生重复关系、也不产生重复事件”，
/// 引用篮等后续迁移复用同一入口。返回实际新增条数（整个批次单事务提交）。
pub fn import_relation_links(
    conn: &mut Connection,
    source_uri: ObjectUri,
    predicate: &str,
    items: Vec<RelationLinkImport>,
) -> Result<usize, String> {
    if predicate.trim().is_empty() {
        return Err("批量导入缺少 predicate".into());
    }
    // 计数器必须在闭包内维护并经返回值传出：move 闭包会拷贝外部变量，
    // 在外部累加会永远读到初始值。
    let imported = tx_command(conn, move |tx| {
        let mut imported = 0usize;
        for item in items {
            let RelationLinkImport {
                target_uri,
                created_by,
                workspace_id,
                snapshot,
            } = item;
            if find_relation(tx, source_uri.as_str(), predicate, target_uri.as_str())?.is_some() {
                continue;
            }
            create_relation_in_tx(
                tx,
                NewRelation {
                    source_uri: source_uri.clone(),
                    predicate: predicate.to_string(),
                    target_uri,
                    anchor_id: None,
                    created_by,
                    confidence: None,
                    workspace_id,
                    snapshot,
                },
            )?;
            imported += 1;
        }
        Ok(imported)
    })?;
    Ok(imported)
}

// ---------------------------------------------------------------------------
// Context sets / items
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ContextSetRecord {
    pub id: i64,
    pub workspace_id: Option<String>,
    pub thread_id: Option<String>,
    pub work_id: Option<String>,
    pub purpose: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewContextSet {
    pub workspace_id: Option<String>,
    pub thread_id: Option<String>,
    pub work_id: Option<String>,
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextItemRecord {
    pub id: i64,
    pub context_id: i64,
    pub object_uri: String,
    pub anchor_id: Option<i64>,
    pub position: i64,
    pub added_by: String,
    pub created_at: String,
}

const CONTEXT_SET_COLUMNS: &str =
    "id, workspace_id, thread_id, work_id, purpose, created_at, updated_at";
const CONTEXT_ITEM_COLUMNS: &str =
    "id, context_id, object_uri, anchor_id, position, added_by, created_at";

fn row_to_context_set(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContextSetRecord> {
    Ok(ContextSetRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        thread_id: row.get(2)?,
        work_id: row.get(3)?,
        purpose: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn row_to_context_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContextItemRecord> {
    Ok(ContextItemRecord {
        id: row.get(0)?,
        context_id: row.get(1)?,
        object_uri: row.get(2)?,
        anchor_id: row.get(3)?,
        position: row.get(4)?,
        added_by: row.get(5)?,
        created_at: row.get(6)?,
    })
}

/// 事务内创建 context set。
pub fn create_context_set_in_tx(
    tx: &Transaction,
    input: NewContextSet,
) -> Result<ContextSetRecord, String> {
    let now = now_iso();
    tx.execute(
        "INSERT INTO context_sets (workspace_id, thread_id, work_id, purpose, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![input.workspace_id, input.thread_id, input.work_id, input.purpose, now],
    )
    .map_err(|e| format!("写入 context set 失败: {e}"))?;
    let sql = format!("SELECT {CONTEXT_SET_COLUMNS} FROM context_sets WHERE id = ?1");
    tx.query_row(&sql, params![tx.last_insert_rowid()], row_to_context_set)
        .map_err(|e| format!("回读 context set 失败: {e}"))
}

/// 单命令入口：创建 context set。
pub fn create_context_set(
    conn: &mut Connection,
    input: NewContextSet,
) -> Result<ContextSetRecord, String> {
    tx_command(conn, move |tx| create_context_set_in_tx(tx, input))
}

fn get_context_set_in_tx(tx: &Transaction, context_id: i64) -> Result<ContextSetRecord, String> {
    let sql = format!("SELECT {CONTEXT_SET_COLUMNS} FROM context_sets WHERE id = ?1");
    tx.query_row(&sql, params![context_id], row_to_context_set)
        .optional()
        .map_err(|e| format!("查询 context set 失败: {e}"))?
        .ok_or_else(|| format!("context set 不存在: {context_id}"))
}

pub fn get_context_set(
    conn: &Connection,
    context_id: i64,
) -> Result<Option<ContextSetRecord>, String> {
    let sql = format!("SELECT {CONTEXT_SET_COLUMNS} FROM context_sets WHERE id = ?1");
    conn.query_row(&sql, params![context_id], row_to_context_set)
        .optional()
        .map_err(|e| format!("查询 context set 失败: {e}"))
}

/// 事务内向 context set 追加一项，排在末尾（position 递增），并记录
/// `context.attached`。object_uri 统一为全局对象 URI，因此 item 可以引用
/// library、workspace、anchor 或未来任何 registered 对象而不改表结构。
pub fn attach_context_item_in_tx(
    tx: &Transaction,
    context_id: i64,
    object_uri: ObjectUri,
    anchor_id: Option<i64>,
    added_by: Option<String>,
) -> Result<ContextItemRecord, String> {
    get_context_set_in_tx(tx, context_id)?;
    let next_position: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM context_items WHERE context_id = ?1",
            params![context_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("计算 context position 失败: {e}"))?;
    tx.execute(
        "INSERT INTO context_items (context_id, object_uri, anchor_id, position, added_by, created_at)
         VALUES (?1, ?2, ?3, ?4, COALESCE(?5, 'human'), ?6)",
        params![
            context_id,
            object_uri.as_str(),
            anchor_id,
            next_position,
            added_by,
            now_iso(),
        ],
    )
    .map_err(|e| {
        if e.to_string().contains("FOREIGN KEY") {
            format!("context item 引用的对象不存在: {e}")
        } else {
            format!("写入 context item 失败: {e}")
        }
    })?;
    touch_context_set(tx, context_id)?;
    let record = {
        let sql = format!("SELECT {CONTEXT_ITEM_COLUMNS} FROM context_items WHERE id = ?1");
        tx.query_row(&sql, params![tx.last_insert_rowid()], row_to_context_item)
            .map_err(|e| format!("回读 context item 失败: {e}"))?
    };
    append_event(
        tx,
        NewEvent {
            action: event_action::CONTEXT_ATTACHED.to_string(),
            object_uri: Some(ObjectUri::context(context_id)),
            target_uri: Some(object_uri),
            payload: Some(serde_json::json!({
                "item_id": record.id,
                "position": record.position,
                "anchor_id": record.anchor_id,
            })),
            ..NewEvent::default()
        },
    )?;
    Ok(record)
}

/// 单命令入口：向 context set 追加一项。
pub fn attach_context_item(
    conn: &mut Connection,
    context_id: i64,
    object_uri: ObjectUri,
    anchor_id: Option<i64>,
    added_by: Option<String>,
) -> Result<ContextItemRecord, String> {
    tx_command(conn, move |tx| {
        attach_context_item_in_tx(tx, context_id, object_uri, anchor_id, added_by)
    })
}

/// 事务内从 context set 移除一项并补齐后续 position（保持 1..N 连续有序），
/// 同时记录 `context.detached`。
pub fn detach_context_item_in_tx(
    tx: &Transaction,
    context_id: i64,
    item_id: i64,
) -> Result<(), String> {
    let sql = format!("SELECT {CONTEXT_ITEM_COLUMNS} FROM context_items WHERE id = ?1");
    let record = tx
        .query_row(&sql, params![item_id], row_to_context_item)
        .optional()
        .map_err(|e| format!("查询 context item 失败: {e}"))?
        .filter(|record| record.context_id == context_id)
        .ok_or_else(|| format!("context item 不存在或不属于该 context: {item_id}"))?;
    tx.execute("DELETE FROM context_items WHERE id = ?1", params![item_id])
        .map_err(|e| format!("删除 context item 失败: {e}"))?;
    tx.execute(
        "UPDATE context_items SET position = position - 1 WHERE context_id = ?1 AND position > ?2",
        params![context_id, record.position],
    )
    .map_err(|e| format!("整理 context position 失败: {e}"))?;
    touch_context_set(tx, context_id)?;
    append_event(
        tx,
        NewEvent {
            action: event_action::CONTEXT_DETACHED.to_string(),
            object_uri: Some(ObjectUri::context(context_id)),
            target_uri: ObjectUri::parse(&record.object_uri).ok(),
            payload: Some(serde_json::json!({
                "item_id": record.id,
                "position_before": record.position,
            })),
            ..NewEvent::default()
        },
    )?;
    Ok(())
}

/// 单命令入口：移除 context set 中的一项。
pub fn detach_context_item(
    conn: &mut Connection,
    context_id: i64,
    item_id: i64,
) -> Result<(), String> {
    tx_command(conn, move |tx| {
        detach_context_item_in_tx(tx, context_id, item_id)
    })
}

fn touch_context_set(tx: &Transaction, context_id: i64) -> Result<(), String> {
    tx.execute(
        "UPDATE context_sets SET updated_at = ?1 WHERE id = ?2",
        params![now_iso(), context_id],
    )
    .map_err(|e| format!("更新 context set 时间失败: {e}"))?;
    Ok(())
}

/// 按 position 升序列出 context 内容。
pub fn list_context_items(
    conn: &Connection,
    context_id: i64,
) -> Result<Vec<ContextItemRecord>, String> {
    let sql = format!(
        "SELECT {CONTEXT_ITEM_COLUMNS} FROM context_items WHERE context_id = ?1 ORDER BY position ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询 context items 失败: {e}"))?;
    let rows = stmt
        .query_map(params![context_id], row_to_context_item)
        .map_err(|e| format!("查询 context items 失败: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ---------------------------------------------------------------------------
// Command transaction pattern
// ---------------------------------------------------------------------------

/// 所有会修改 durable state 的命令都必须经由本函数执行：
///
/// ```text
/// BEGIN
///   mutate durable state（本模块 *_in_tx / 直接 SQL）
///   append semantic event
/// COMMIT
/// ```
///
/// 命令闭包返回 Err 时整体回滚：状态变更和事件要么同时可见、要么都不可见。
pub fn tx_command<T>(
    conn: &mut Connection,
    command: impl FnOnce(&Transaction) -> Result<T, String>,
) -> Result<T, String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启命令事务失败: {e}"))?;
    let outcome = command(&tx)?;
    tx.commit().map_err(|e| format!("提交命令事务失败: {e}"))?;
    Ok(outcome)
}

/// 单独记录一条事件时使用的便捷封装（同样走完整事务）。
pub fn record_event(conn: &mut Connection, event: NewEvent) -> Result<i64, String> {
    tx_command(conn, |tx| append_event(tx, event))
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// Contract tests（P1 验收标准逐条落地）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "sw-state-{name}-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn open_fresh() -> (TempDir, Connection) {
        let dir = TempDir::new("state-store");
        let conn = open_state_db(&dir.path().join("state.db")).unwrap();
        (dir, conn)
    }

    fn reopen(dir: &TempDir) -> Connection {
        open_state_db(&dir.path().join("state.db")).unwrap()
    }

    // ---- Migration ----

    #[test]
    fn fresh_database_migrates_to_latest_schema() {
        let (_dir, conn) = open_fresh();
        assert_eq!(
            current_schema_version(&conn).unwrap(),
            LATEST_SCHEMA_VERSION
        );
        for table in [
            "schema_migrations",
            "events",
            "anchors",
            "relations",
            "context_sets",
            "context_items",
            "web_searches",
            "web_search_results",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "缺少表 {table}");
        }
    }

    #[test]
    fn restart_migration_is_a_noop_and_preserves_data() {
        let (dir, mut conn) = open_fresh();
        create_relation(
            &mut conn,
            NewRelation {
                source_uri: ObjectUri::workspace("a.md").unwrap(),
                predicate: "related_to".into(),
                target_uri: ObjectUri::library("src1", "papers/x.md").unwrap(),
                anchor_id: None,
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        )
        .unwrap();

        let mut reopened = reopen(&dir);
        let report = migrate(&mut reopened).unwrap();
        assert!(!report.applied_any(), "重启不应执行任何 migration");
        assert_eq!(
            current_schema_version(&reopened).unwrap(),
            LATEST_SCHEMA_VERSION
        );
        // 数据原样保留
        assert_eq!(list_events(&reopened, 10).unwrap().len(), 1);
        assert_eq!(
            neighbors(&reopened, "workspace://a.md", None, 10)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn partially_migrated_database_continues_from_pending_version() {
        let dir = TempDir::new("state-partial");
        let db_path = dir.path().join("state.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL);",
            )
            .unwrap();
            let first = &MIGRATIONS[0];
            conn.execute_batch(first.sql).unwrap();
            // 放一条事件，确保后面的 DELETE 真正命中触发器（空表上触发器不触发行为）
            conn.execute(
                "INSERT INTO events (action, created_at) VALUES ('relation.created', 'seed')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![first.version, first.name, now_iso()],
            )
            .unwrap();
        }
        let conn = open_state_db(&db_path).unwrap();
        assert_eq!(
            current_schema_version(&conn).unwrap(),
            LATEST_SCHEMA_VERSION
        );
        // v2 的表已经补齐，v1 的事件数据与 append-only 触发器也都保留
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('relations','context_sets','context_items')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 3);
        assert_eq!(list_events(&conn, 10).unwrap().len(), 1);
        let delete = conn.execute("DELETE FROM events", []);
        assert!(delete.is_err(), "v1 触发器必须继续生效");
    }

    #[test]
    fn migration_versions_are_contiguous() {
        validate_migration_list();
        assert_eq!(MIGRATIONS.last().unwrap().version, LATEST_SCHEMA_VERSION);
    }

    // ---- Events ----

    #[test]
    fn appended_events_are_queryable_with_payload() {
        let (_dir, mut conn) = open_fresh();
        record_event(
            &mut conn,
            NewEvent {
                action: event_action::RELATION_CREATED.into(),
                workspace_id: Some("ws-hash".into()),
                actor_type: Some(ActorKind::Agent),
                actor_id: Some("pi".into()),
                object_uri: Some(ObjectUri::relation(7)),
                target_uri: Some(ObjectUri::workspace("notes.md").unwrap()),
                payload: Some(serde_json::json!({"predicate": "related_to"})),
                ..NewEvent::default()
            },
        )
        .unwrap();

        let events = list_events(&conn, 10).unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.id, 1);
        assert_eq!(event.action, "relation.created");
        assert_eq!(event.actor_type, "agent");
        assert_eq!(event.actor_id.as_deref(), Some("pi"));
        assert!(event.thread_id.is_none());
        assert!(event.work_id.is_none());
        assert_eq!(event.workspace_id.as_deref(), Some("ws-hash"));
        assert_eq!(event.object_uri.as_deref(), Some("relation://7"));
        assert_eq!(event.payload.as_ref().unwrap()["predicate"], "related_to");
        assert!(!event.created_at.is_empty());

        let by_object = events_for_object(&conn, "workspace://notes.md", 10).unwrap();
        assert_eq!(by_object.len(), 1);
        assert_eq!(by_object[0].id, 1);
    }

    #[test]
    fn web_search_history_roundtrips_with_results_and_links() {
        let (dir, mut conn) = open_fresh();
        let document = ObjectUri::workspace("notes/search.md").unwrap();
        let history = create_web_search(
            &mut conn,
            NewWebSearch {
                workspace_id: Some("ws-search".into()),
                document_uri: Some(document.clone()),
                selected_text: Some("上下文工程".into()),
                query: "上下文工程 durable state".into(),
                results: vec![
                    NewWebSearchResult {
                        title: "StillWrite context state".into(),
                        url: "https://example.com/context".into(),
                        description: "A durable search result snapshot".into(),
                        age: Some("2 days ago".into()),
                    },
                    NewWebSearchResult {
                        title: "SQLite events".into(),
                        url: "https://example.com/events".into(),
                        description: "Append-only evidence".into(),
                        age: None,
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(history.results.len(), 2);
        assert_eq!(history.results[0].position, 0);
        assert_eq!(history.results[1].url, "https://example.com/events");

        let linked_result = &history.results[0];
        create_relation(
            &mut conn,
            NewRelation {
                source_uri: document.clone(),
                predicate: "related_to".into(),
                target_uri: ObjectUri::web_search_result(linked_result.id),
                anchor_id: None,
                created_by: Some("human".into()),
                confidence: None,
                workspace_id: Some("ws-search".into()),
                snapshot: Some(serde_json::json!({"title": linked_result.title})),
            },
        )
        .unwrap();
        assert_eq!(
            web_search_result_links(&conn, document.as_str(), Some("ws-search")).unwrap(),
            vec![linked_result.id]
        );

        let other_history = create_web_search(
            &mut conn,
            NewWebSearch {
                workspace_id: Some("ws-other".into()),
                document_uri: Some(document.clone()),
                selected_text: None,
                query: "other workspace".into(),
                results: vec![NewWebSearchResult {
                    title: "Other workspace result".into(),
                    url: "https://example.com/other".into(),
                    description: "Must stay scoped to its workspace".into(),
                    age: None,
                }],
            },
        )
        .unwrap();
        let other_result = &other_history.results[0];
        create_relation(
            &mut conn,
            NewRelation {
                source_uri: document.clone(),
                predicate: "related_to".into(),
                target_uri: ObjectUri::web_search_result(other_result.id),
                anchor_id: None,
                created_by: Some("human".into()),
                confidence: None,
                workspace_id: Some("ws-other".into()),
                snapshot: None,
            },
        )
        .unwrap();
        assert_eq!(
            web_search_result_links(&conn, document.as_str(), Some("ws-search")).unwrap(),
            vec![linked_result.id]
        );
        assert_eq!(
            web_search_result_links(&conn, document.as_str(), Some("ws-other")).unwrap(),
            vec![other_result.id]
        );

        let reopened = reopen(&dir);
        let histories = list_web_search_history(&reopened, Some("ws-search"), 10).unwrap();
        assert_eq!(histories.len(), 1);
        assert_eq!(histories[0].query, "上下文工程 durable state");
        assert_eq!(histories[0].results.len(), 2);
        assert_eq!(
            histories[0].document_uri.as_deref(),
            Some(document.as_str())
        );
        let events = list_events(&reopened, 10).unwrap();
        assert!(events.iter().any(|event| {
            event.action == event_action::WEB_SEARCH_COMPLETED
                && event.object_uri.as_deref() == Some(ObjectUri::web_search(history.id).as_str())
        }));
    }

    #[test]
    fn failed_state_operation_leaves_neither_state_nor_event() {
        let (_dir, mut conn) = open_fresh();
        // 先建一条已有关联，制造后续必然失败的条件
        create_relation(
            &mut conn,
            NewRelation {
                source_uri: ObjectUri::workspace("a.md").unwrap(),
                predicate: "related_to".into(),
                target_uri: ObjectUri::workspace("b.md").unwrap(),
                anchor_id: None,
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        )
        .unwrap();

        // 手写一个会在“追加事件之后”才失败的命令：事件已写入事务但必须随回滚消失
        let outcome: Result<(), String> = tx_command(&mut conn, |tx| {
            append_event(
                tx,
                NewEvent {
                    action: "annotation.created".into(),
                    object_uri: Some(ObjectUri::workspace("c.md").unwrap()),
                    ..NewEvent::default()
                },
            )?;
            // 强制失败：违反唯一约束（同一三元组第二次插入）
            tx.execute(
                "INSERT INTO relations (source_uri, predicate, target_uri, created_at, updated_at)
                 VALUES ('workspace://a.md', 'related_to', 'workspace://b.md', 'x', 'x')",
                [],
            )
            .map_err(|e| format!("预期失败: {e}"))?;
            Ok(())
        });
        assert!(outcome.is_err());

        let events = list_events(&conn, 50).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| e.action == "relation.created")
                .count(),
            1,
            "只有第一次成功创建留下的事件"
        );
        assert!(
            !events.iter().any(|e| e.action == "annotation.created"),
            "失败命令中的事件必须被回滚"
        );

        // 生产路径同理：create_relation 自身失败时不留状态也不留事件
        let duplicate = create_relation(
            &mut conn,
            NewRelation {
                source_uri: ObjectUri::workspace("a.md").unwrap(),
                predicate: "related_to".into(),
                target_uri: ObjectUri::workspace("b.md").unwrap(),
                anchor_id: None,
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        );
        assert!(duplicate.is_err());
        assert_eq!(list_events(&conn, 50).unwrap().len(), 1);
    }

    #[test]
    fn events_table_rejects_update_and_delete_at_sql_level() {
        let (_dir, mut conn) = open_fresh();
        record_event(
            &mut conn,
            NewEvent {
                action: "relation.created".into(),
                ..NewEvent::default()
            },
        )
        .unwrap();
        let update = conn.execute("UPDATE events SET action = 'tampered'", []);
        assert!(update.is_err(), "events 必须禁止 UPDATE");
        let delete = conn.execute("DELETE FROM events", []);
        assert!(delete.is_err(), "events 必须禁止 DELETE");
        assert_eq!(
            list_events(&conn, 10).unwrap()[0].action,
            "relation.created"
        );
    }

    // ---- Anchors ----

    #[test]
    fn create_anchor_then_query_by_document() {
        let (_dir, mut conn) = open_fresh();
        let doc = ObjectUri::workspace("章节/第一章.md").unwrap();
        let other_doc = ObjectUri::library("rss-src", "daily/post.md").unwrap();

        let with_context = create_anchor(
            &mut conn,
            NewAnchor {
                document_uri: doc.clone(),
                kind: "字句".into(),
                start_offset: 12,
                end_offset: 34,
                quote: "上下文工程正在成为核心问题".into(),
                prefix: Some("前面的文字".into()),
                suffix: Some("后面的文字".into()),
            },
        )
        .unwrap();
        let without_context = create_anchor(
            &mut conn,
            NewAnchor {
                document_uri: doc.clone(),
                kind: "段落".into(),
                start_offset: 100,
                end_offset: 240,
                quote: "第二段原文".into(),
                prefix: None,
                suffix: None,
            },
        )
        .unwrap();
        create_anchor(
            &mut conn,
            NewAnchor {
                document_uri: other_doc.clone(),
                kind: "段落".into(),
                start_offset: 0,
                end_offset: 20,
                quote: "别的文档".into(),
                prefix: None,
                suffix: None,
            },
        )
        .unwrap();

        let mine = anchors_for_document(&conn, doc.as_str()).unwrap();
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].id, with_context.id);
        assert_eq!(mine[0].prefix.as_deref(), Some("前面的文字"));
        // 创建时 created/updated 一致；漂移由未来的批注 slice 维护
        assert_eq!(mine[0].created_at, mine[0].updated_at);
        assert_eq!(mine[1].id, without_context.id);
        assert!(mine[1].prefix.is_none() && mine[1].suffix.is_none());
        assert_eq!(
            anchors_for_document(&conn, other_doc.as_str())
                .unwrap()
                .len(),
            1
        );
        assert!(get_anchor(&conn, with_context.id).unwrap().is_some());
        assert!(get_anchor(&conn, 999).unwrap().is_none());
    }

    #[test]
    fn anchor_scope_validation_rejects_bad_ranges() {
        let (_dir, mut conn) = open_fresh();
        let result = create_anchor(
            &mut conn,
            NewAnchor {
                document_uri: ObjectUri::workspace("a.md").unwrap(),
                kind: "字句".into(),
                start_offset: 50,
                end_offset: 10,
                quote: String::new(),
                prefix: None,
                suffix: None,
            },
        );
        assert!(result.is_err());
    }

    // ---- Relations ----

    #[test]
    fn relation_is_queryable_from_both_ends() {
        let (_dir, mut conn) = open_fresh();
        let a = ObjectUri::workspace("a.md").unwrap();
        let b = ObjectUri::library("lib1", "docs/b.md").unwrap();
        let created = create_relation(
            &mut conn,
            NewRelation {
                source_uri: a.clone(),
                predicate: "related_to".into(),
                target_uri: b.clone(),
                anchor_id: None,
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        )
        .unwrap();

        let from_a = neighbors(&conn, a.as_str(), None, 10).unwrap();
        assert_eq!(from_a.len(), 1);
        assert_eq!(from_a[0].neighbor_uri, b.as_str());
        assert_eq!(from_a[0].direction, RelationDirection::Outgoing);
        assert_eq!(from_a[0].relation.id, created.id);
        // 未显式提供时的默认值
        assert_eq!(created.created_by, "human");
        assert!(created.confidence.is_none());
        assert!(!created.created_at.is_empty());

        let from_b = neighbors(&conn, b.as_str(), None, 10).unwrap();
        assert_eq!(from_b.len(), 1);
        assert_eq!(from_b[0].neighbor_uri, a.as_str());
        assert_eq!(from_b[0].direction, RelationDirection::Incoming);

        // predicate 过滤
        assert!(neighbors(&conn, a.as_str(), Some("contradicts"), 10)
            .unwrap()
            .is_empty());
        assert_eq!(
            neighbors(&conn, a.as_str(), Some("related_to"), 10)
                .unwrap()
                .len(),
            1
        );

        // 创建即有事件，且 relation 行上绑定了证据事件
        let events = list_events(&conn, 10).unwrap();
        assert_eq!(events[0].action, event_action::RELATION_CREATED);
        assert_eq!(created.evidence_event_id.unwrap(), events[0].id);
    }

    #[test]
    fn remove_relation_keeps_removed_event_but_state_gone() {
        let (_dir, mut conn) = open_fresh();
        let a = ObjectUri::workspace("a.md").unwrap();
        let b = ObjectUri::workspace("b.md").unwrap();
        let created = create_relation(
            &mut conn,
            NewRelation {
                source_uri: a.clone(),
                predicate: "related_to".into(),
                target_uri: b.clone(),
                anchor_id: None,
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        )
        .unwrap();
        remove_relation(&mut conn, created.id).unwrap();

        assert!(neighbors(&conn, a.as_str(), None, 10).unwrap().is_empty());
        let events = list_events(&conn, 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, event_action::RELATION_REMOVED);
        assert_eq!(
            events[0].object_uri.as_deref(),
            Some(format!("relation://{}", created.id).as_str())
        );
        assert_eq!(
            events[0].payload.as_ref().unwrap()["source_uri"],
            a.as_str()
        );

        remove_relation(&mut conn, created.id).unwrap_err();
    }

    #[test]
    fn find_relation_and_snapshot_views_roundtrip() {
        let (_dir, mut conn) = open_fresh();
        let scope = ObjectUri::parse("ws://7f3a9c1e2b4d5a6f").unwrap();
        let target_a = ObjectUri::library("arxiv", "papers/memory.md").unwrap();
        let target_b = ObjectUri::workspace("notes/ideas.md").unwrap();
        let card = serde_json::json!({
            "key": "library:arxiv/papers/memory.md",
            "kind": "library",
            "title": "Agent Memory 论文",
            "snippet": "记忆应当压缩自证据",
            "source": "arxiv · papers/memory.md",
        });

        create_relation(
            &mut conn,
            NewRelation {
                source_uri: scope.clone(),
                predicate: "related_to".into(),
                target_uri: target_a.clone(),
                anchor_id: None,
                created_by: Some("human".into()),
                confidence: None,
                workspace_id: None,
                snapshot: Some(card.clone()),
            },
        )
        .unwrap();
        // 第二条不带快照字段差异（payload 结构一致，只是内容不同）
        create_relation(
            &mut conn,
            NewRelation {
                source_uri: scope.clone(),
                predicate: "related_to".into(),
                target_uri: target_b.clone(),
                anchor_id: None,
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        )
        .unwrap();

        let found = find_relation(&conn, scope.as_str(), "related_to", target_a.as_str())
            .unwrap()
            .expect("刚创建的关系必须能被三元组查回");
        assert!(
            find_relation(&conn, scope.as_str(), "contradicts", target_a.as_str())
                .unwrap()
                .is_none()
        );

        let views = list_relation_snapshots(&conn, scope.as_str(), "related_to").unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].relation.target_uri, target_a.as_str());
        // 展示快照原样从证据事件还原
        assert_eq!(views[0].snapshot.as_ref(), Some(&card));
        assert_eq!(
            views[1].relation.target_uri,
            target_b.as_str(),
            "按创建顺序返回"
        );
        // 未携带快照的关系返回 None 而不是报错
        assert!(views[1].snapshot.is_none());

        // 删除后 find 归零、视图同步减少
        remove_relation(&mut conn, found.id).unwrap();
        assert!(
            find_relation(&conn, scope.as_str(), "related_to", target_a.as_str())
                .unwrap()
                .is_none()
        );
        assert_eq!(
            list_relation_snapshots(&conn, scope.as_str(), "related_to")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn duplicate_triple_is_rejected_but_new_predicate_allowed() {
        let (_dir, mut conn) = open_fresh();
        let mut new_relation = |predicate: &str| {
            create_relation(
                &mut conn,
                NewRelation {
                    source_uri: ObjectUri::workspace("a.md").unwrap(),
                    predicate: predicate.into(),
                    target_uri: ObjectUri::workspace("b.md").unwrap(),
                    anchor_id: None,
                    created_by: None,
                    confidence: None,
                    workspace_id: None,
                    snapshot: None,
                },
            )
        };
        new_relation("related_to").unwrap();
        assert!(new_relation("related_to").is_err());
        assert!(new_relation("elaborates").is_ok());
        assert!(create_relation(
            &mut conn,
            NewRelation {
                source_uri: ObjectUri::workspace("same.md").unwrap(),
                predicate: "related_to".into(),
                target_uri: ObjectUri::workspace("same.md").unwrap(),
                anchor_id: None,
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        )
        .is_err());
    }

    // ---- Context ----

    #[test]
    fn context_attach_preserves_order_across_detach() {
        let (_dir, mut conn) = open_fresh();
        let set = create_context_set(
            &mut conn,
            NewContextSet {
                workspace_id: Some("ws-hash".into()),
                purpose: Some("写作本章的资料集".into()),
                ..NewContextSet::default()
            },
        )
        .unwrap();

        let lib = ObjectUri::library("src1", "books/a.md").unwrap();
        let ws = ObjectUri::workspace("notes.md").unwrap();
        // context_items.anchor_id 带 FK：先有真实 anchor 才能挂上 anchor:// 引用
        let anchored = create_anchor(
            &mut conn,
            NewAnchor {
                document_uri: ws.clone(),
                kind: "字句".into(),
                start_offset: 0,
                end_offset: 8,
                quote: "工作集里的选区".into(),
                prefix: None,
                suffix: None,
            },
        )
        .unwrap();
        let anchor_uri = ObjectUri::anchor(anchored.id);
        let first = attach_context_item(&mut conn, set.id, lib.clone(), None, None).unwrap();
        let second = attach_context_item(
            &mut conn,
            set.id,
            anchor_uri.clone(),
            Some(anchored.id),
            None,
        )
        .unwrap();
        let third = attach_context_item(&mut conn, set.id, ws.clone(), None, None).unwrap();

        let items = list_context_items(&conn, set.id).unwrap();
        assert_eq!(
            items
                .iter()
                .map(|i| i.object_uri.as_str())
                .collect::<Vec<_>>(),
            vec![lib.as_str(), anchor_uri.as_str(), ws.as_str()]
        );
        assert_eq!(
            items.iter().map(|i| i.position).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );

        // 移除中间项后仍保持紧密有序
        detach_context_item(&mut conn, set.id, second.id).unwrap();
        let items = list_context_items(&conn, set.id).unwrap();
        assert_eq!(
            items.iter().map(|i| (i.id, i.position)).collect::<Vec<_>>(),
            vec![(first.id, 1), (third.id, 2)]
        );

        // 附着/脱离都留下了可查询事件
        assert_eq!(
            get_context_set(&conn, set.id)
                .unwrap()
                .unwrap()
                .purpose
                .as_deref(),
            Some("写作本章的资料集")
        );
        let events = list_events(&conn, 20).unwrap();
        let actions: Vec<_> = events.iter().map(|e| e.action.as_str()).collect();
        assert_eq!(
            actions,
            vec![
                event_action::CONTEXT_DETACHED,
                event_action::CONTEXT_ATTACHED,
                event_action::CONTEXT_ATTACHED,
                event_action::CONTEXT_ATTACHED,
            ]
        );
        let detached = &events[0];
        assert_eq!(
            detached.object_uri.as_deref(),
            Some(ObjectUri::context(set.id).as_str())
        );
        assert_eq!(detached.payload.as_ref().unwrap()["item_id"], second.id);

        // 移除首项：其余位置必须继续逐级前移（unique(position) 下不允许出现中间态冲突）
        detach_context_item(&mut conn, set.id, first.id).unwrap();
        let items = list_context_items(&conn, set.id).unwrap();
        assert_eq!(
            items.iter().map(|i| (i.id, i.position)).collect::<Vec<_>>(),
            vec![(third.id, 1)]
        );

        detach_context_item(&mut conn, set.id, second.id).unwrap_err();
        attach_context_item(&mut conn, 404, ws.clone(), None, None).unwrap_err();
    }
}
