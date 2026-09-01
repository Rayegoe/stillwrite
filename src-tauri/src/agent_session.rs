// M5 Durable Agent Thread & Cognitive Lineage。
//
// AgentSession = 连续性（"我们之前聊到哪里"）；AgentMessage = thread 上的最小
// 事实（谁说了什么、run 证据、当时的来源与引文快照）。与 Work/Run/Artifact
// 的语义边界见 spec 01：Session 不是任务状态，Run success != Work completed。
//
// 关系复用既有原语，不新造 link 表：
//   turn 来源/引用        → context_sets(thread_id = session id) + context_items
//   insert/save/delegate  → relations（inserted_into / derived_into / promoted_to）
//   一切变更              → append-only events

use crate::state_store::{
    append_event, create_relation, event_action, now_iso, tx_command, ActorKind, NewContextSet,
    NewEvent, NewRelation, ObjectUri,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;

const SESSION_ID_PREFIX: &str = "as-";
const MESSAGE_ID_PREFIX: &str = "am-";

/// 每轮继续问随 runtime input 带上的历史轮数上限。
/// 历史保存在本表里，prompt 端只投影最近的窗口；完整线程永远可查。
pub const HISTORY_WINDOW_MESSAGES: usize = 12;

pub fn new_session_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static N: AtomicU32 = AtomicU32::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{SESSION_ID_PREFIX}{}-{}-{}",
        nonce,
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn new_message_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{MESSAGE_ID_PREFIX}{}-{}-{}",
        nonce,
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

// ---------------------------------------------------------------------------
// Records & inputs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionRecord {
    pub id: String,
    pub workspace_id: Option<String>,
    pub title: String,
    pub provider: Option<String>,
    pub provider_session_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionSummary {
    #[serde(flatten)]
    pub session: AgentSessionRecord,
    pub message_count: i64,
    pub last_origin_uri: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMessageRole {
    User,
    Assistant,
}

impl AgentMessageRole {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentMessageRole::User => "user",
            AgentMessageRole::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageRecord {
    pub id: String,
    pub session_id: String,
    pub role: AgentMessageRole,
    pub content: String,
    pub run_ref: Option<String>,
    pub origin_uri: Option<String>,
    pub quote_snapshot: Option<String>,
    pub context_set_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewAgentSession {
    pub workspace_id: Option<String>,
    pub title: String,
}

/// 一轮 turn 的 user message：人的原始要求 + 当时的来源/引文证据。
pub struct NewAgentMessage {
    pub session_id: String,
    pub role: AgentMessageRole,
    pub content: String,
    pub run_ref: Option<String>,
    pub origin_uri: Option<String>,
    pub quote_snapshot: Option<String>,
    /// turn 的上下文引用（当前文档、选区 anchor、引用资料 URI）。
    /// M7 Context Compiler 落地后由 backend 统一编译；这里只保存"当时用了什么"。
    pub context_item_uris: Vec<ObjectUri>,
}

/// 人的后续动作 → 认知链 predicate 白名单。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomePredicate {
    InsertedInto,
    DerivedInto,
    PromotedTo,
}

impl OutcomePredicate {
    pub fn as_str(self) -> &'static str {
        match self {
            OutcomePredicate::InsertedInto => "inserted_into",
            OutcomePredicate::DerivedInto => "derived_into",
            OutcomePredicate::PromotedTo => "promoted_to",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "inserted_into" => Ok(OutcomePredicate::InsertedInto),
            "derived_into" => Ok(OutcomePredicate::DerivedInto),
            "promoted_to" => Ok(OutcomePredicate::PromotedTo),
            other => Err(format!("未知的 Agent 结果动作: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Row mapping
// ---------------------------------------------------------------------------

const SESSION_COLUMNS: &str =
    "id, workspace_id, title, provider, provider_session_ref, created_at, updated_at";
const MESSAGE_COLUMNS: &str =
    "id, session_id, role, content, run_ref, origin_uri, quote_snapshot, context_set_id, created_at";

fn row_to_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSessionRecord> {
    Ok(AgentSessionRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        title: row.get(2)?,
        provider: row.get(3)?,
        provider_session_ref: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentMessageRecord> {
    let role: String = row.get(2)?;
    Ok(AgentMessageRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        role: match role.as_str() {
            "user" => AgentMessageRole::User,
            _ => AgentMessageRole::Assistant,
        },
        content: row.get(3)?,
        run_ref: row.get(4)?,
        origin_uri: row.get(5)?,
        quote_snapshot: row.get(6)?,
        context_set_id: row.get(7)?,
        created_at: row.get(8)?,
    })
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

pub fn create_session(
    conn: &mut Connection,
    input: NewAgentSession,
) -> Result<AgentSessionRecord, String> {
    tx_command(conn, move |tx| {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err("Agent session 缺少标题".into());
        }
        let now = now_iso();
        let id = new_session_id();
        tx.execute(
            "INSERT INTO agent_sessions (id, workspace_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, input.workspace_id, title, now],
        )
        .map_err(|e| format!("写入 agent session 失败: {e}"))?;
        append_event(
            tx,
            NewEvent {
                action: event_action::AGENT_SESSION_CREATED.to_string(),
                workspace_id: input.workspace_id.clone(),
                actor_type: Some(ActorKind::Human),
                object_uri: Some(ObjectUri::agent_session(&id)),
                payload: Some(serde_json::json!({ "title": title })),
                ..NewEvent::default()
            },
        )?;
        let sql = format!("SELECT {SESSION_COLUMNS} FROM agent_sessions WHERE id = ?1");
        tx.query_row(&sql, params![id], row_to_session)
            .map_err(|e| format!("回读 agent session 失败: {e}"))
    })
}

/// 追加一条 message（事务内）：写行 + touch session + 建 turn context set +
/// 追加 `agent_message.appended` 事件。role=user 由人在发起时产生，
/// role=assistant 由 run settle 侧产生；同一 run_ref 最多绑定一条。
pub fn append_message(
    conn: &mut Connection,
    input: NewAgentMessage,
) -> Result<AgentMessageRecord, String> {
    tx_command(conn, move |tx| append_message_in_tx(tx, input))
}

pub fn append_message_in_tx(
    tx: &Transaction,
    input: NewAgentMessage,
) -> Result<AgentMessageRecord, String> {
    let content = input.content.trim().to_string();
    if content.is_empty() {
        return Err("Agent message 内容不能为空".into());
    }
    let session: AgentSessionRecord = {
        let sql = format!("SELECT {SESSION_COLUMNS} FROM agent_sessions WHERE id = ?1");
        tx.query_row(&sql, params![input.session_id], row_to_session)
            .optional()
            .map_err(|e| format!("查询 agent session 失败: {e}"))?
            .ok_or_else(|| format!("Agent session 不存在: {}", input.session_id))?
    };
    // turn context set：thread_id 指回 session，purpose 标注来源
    let mut context_set_id = None;
    if !input.context_item_uris.is_empty() {
        let set = crate::state_store::create_context_set_in_tx(
            tx,
            NewContextSet {
                workspace_id: session.workspace_id.clone(),
                thread_id: Some(session.id.clone()),
                work_id: None,
                purpose: Some("agent_turn".into()),
            },
        )?;
        for (position, uri) in input.context_item_uris.iter().enumerate() {
            let next_position = (position + 1) as i64;
            tx.execute(
                "INSERT INTO context_items (context_id, object_uri, position, added_by, created_at)
                 VALUES (?1, ?2, ?3, 'human', ?4)",
                params![set.id, uri.as_str(), next_position, now_iso()],
            )
            .map_err(|e| format!("写入 context item 失败: {e}"))?;
        }
        context_set_id = Some(set.id);
    }
    let now = now_iso();
    let id = new_message_id();
    tx.execute(
        "INSERT INTO agent_messages
            (id, session_id, role, content, run_ref, origin_uri, quote_snapshot, context_set_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            input.session_id,
            input.role.as_str(),
            content,
            input.run_ref,
            input.origin_uri,
            input.quote_snapshot,
            context_set_id,
            now,
        ],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            format!("run {} 已绑定过 message", input.run_ref.as_deref().unwrap_or("?"))
        } else {
            format!("写入 agent message 失败: {e}")
        }
    })?;
    tx.execute(
        "UPDATE agent_sessions SET updated_at = ?1 WHERE id = ?2",
        params![now, input.session_id],
    )
    .map_err(|e| format!("更新 agent session 时间失败: {e}"))?;
    append_event(
        tx,
        NewEvent {
            action: event_action::AGENT_MESSAGE_APPENDED.to_string(),
            workspace_id: session.workspace_id.clone(),
            actor_type: Some(match input.role {
                AgentMessageRole::User => ActorKind::Human,
                AgentMessageRole::Assistant => ActorKind::Agent,
            }),
            object_uri: Some(ObjectUri::agent_message(&id)),
            target_uri: Some(ObjectUri::agent_session(&input.session_id)),
            thread_id: Some(input.session_id.clone()),
            payload: Some(serde_json::json!({
                "role": input.role.as_str(),
                "runRef": input.run_ref,
                "originUri": input.origin_uri,
                "contextSetId": context_set_id,
                "quoteSnapshot": input.quote_snapshot,
            })),
            ..NewEvent::default()
        },
    )?;
    let sql = format!("SELECT {MESSAGE_COLUMNS} FROM agent_messages WHERE id = ?1");
    tx.query_row(&sql, params![id], row_to_message)
        .map_err(|e| format!("回读 agent message 失败: {e}"))
}

/// run settle / 启动桥：按 run_ref 反查所属 session 的 user message。
pub fn find_user_message_by_run(
    conn: &Connection,
    run_ref: &str,
) -> Result<Option<AgentMessageRecord>, String> {
    let sql = format!(
        "SELECT {MESSAGE_COLUMNS} FROM agent_messages WHERE run_ref = ?1 AND role = 'user'"
    );
    conn.query_row(&sql, params![run_ref], row_to_message)
        .optional()
        .map_err(|e| format!("查询 run 对应 message 失败: {e}"))
}

/// 同一 run 的 assistant message 是否已落（幂等防重放写入）。
pub fn find_assistant_message_by_run(
    conn: &Connection,
    run_ref: &str,
) -> Result<Option<AgentMessageRecord>, String> {
    let sql = format!(
        "SELECT {MESSAGE_COLUMNS} FROM agent_messages WHERE run_ref = ?1 AND role = 'assistant'"
    );
    conn.query_row(&sql, params![run_ref], row_to_message)
        .optional()
        .map_err(|e| format!("查询 run 回执 message 失败: {e}"))
}

pub fn get_session(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let sql = format!("SELECT {SESSION_COLUMNS} FROM agent_sessions WHERE id = ?1");
    conn.query_row(&sql, params![session_id], row_to_session)
        .optional()
        .map_err(|e| format!("查询 agent session 失败: {e}"))
}

/// 更新 provider 侧信息（run settle 时尽力而为，不影响 run 本身）。
pub fn set_session_provider(
    conn: &mut Connection,
    session_id: &str,
    provider: Option<&str>,
    provider_session_ref: Option<&str>,
) -> Result<(), String> {
    tx_command(conn, move |tx| {
        tx.execute(
            "UPDATE agent_sessions
             SET provider = COALESCE(?2, provider),
                 provider_session_ref = COALESCE(?3, provider_session_ref),
                 updated_at = ?1
             WHERE id = ?4",
            params![now_iso(), provider, provider_session_ref, session_id],
        )
        .map_err(|e| format!("更新 agent session provider 失败: {e}"))?;
        Ok(())
    })
}

/// 当前 workspace 的会话列表（updated_at 倒序），带 message 计数与最近来源。
pub fn list_sessions(
    conn: &Connection,
    workspace_id: &str,
    limit: usize,
) -> Result<Vec<AgentSessionSummary>, String> {
    let sql = format!(
        "SELECT {SESSION_COLUMNS},
                (SELECT COUNT(*) FROM agent_messages m WHERE m.session_id = agent_sessions.id) AS message_count,
                (SELECT m.origin_uri FROM agent_messages m WHERE m.session_id = agent_sessions.id
                  AND m.origin_uri IS NOT NULL ORDER BY m.created_at DESC, m.id DESC LIMIT 1) AS last_origin_uri
         FROM agent_sessions WHERE workspace_id = ?1
         ORDER BY updated_at DESC, id DESC LIMIT ?2"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询 agent sessions 失败: {e}"))?;
    let rows = stmt
        .query_map(params![workspace_id, limit as i64], |row| {
            Ok(AgentSessionSummary {
                session: row_to_session(row)?,
                message_count: row.get(7)?,
                last_origin_uri: row.get(8)?,
            })
        })
        .map_err(|e| format!("查询 agent sessions 失败: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// 与某来源文档相关的会话（任一 turn 的 origin_uri 命中），排除当前展开的
/// session 由调用方处理。相关优先的右栏排序靠这条查询。
pub fn related_sessions(
    conn: &Connection,
    workspace_id: &str,
    origin_uri: &str,
    limit: usize,
) -> Result<Vec<AgentSessionSummary>, String> {
    let sql = format!(
        "SELECT {SESSION_COLUMNS},
                (SELECT COUNT(*) FROM agent_messages m WHERE m.session_id = agent_sessions.id) AS message_count,
                (SELECT m.origin_uri FROM agent_messages m WHERE m.session_id = agent_sessions.id
                  AND m.origin_uri IS NOT NULL ORDER BY m.created_at DESC, m.id DESC LIMIT 1) AS last_origin_uri
         FROM agent_sessions
         WHERE workspace_id = ?1
           AND id IN (
               SELECT DISTINCT session_id FROM agent_messages WHERE origin_uri = ?2
           )
         ORDER BY updated_at DESC, id DESC LIMIT ?3"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询相关 agent sessions 失败: {e}"))?;
    let rows = stmt
        .query_map(params![workspace_id, origin_uri, limit as i64], |row| {
            Ok(AgentSessionSummary {
                session: row_to_session(row)?,
                message_count: row.get(7)?,
                last_origin_uri: row.get(8)?,
            })
        })
        .map_err(|e| format!("查询相关 agent sessions 失败: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_messages(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<AgentMessageRecord>, String> {
    let sql = format!(
        "SELECT {MESSAGE_COLUMNS} FROM agent_messages WHERE session_id = ?1 ORDER BY created_at ASC, id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询 agent messages 失败: {e}"))?;
    let rows = stmt
        .query_map(params![session_id], row_to_message)
        .map_err(|e| format!("查询 agent messages 失败: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// 人的后续动作入认知链：agentmsg://<id> -predicate-> target。
/// relation.created 事件自动留档；同一三元组重复提交由唯一索引拒绝。
pub fn link_message_outcome(
    conn: &mut Connection,
    message_id: &str,
    predicate: OutcomePredicate,
    target_uri: ObjectUri,
    workspace_id: Option<String>,
) -> Result<(), String> {
    let source = ObjectUri::agent_message(message_id);
    create_relation(
        conn,
        NewRelation {
            source_uri: source,
            predicate: predicate.as_str().to_string(),
            target_uri,
            anchor_id: None,
            created_by: Some("human".into()),
            confidence: None,
            workspace_id,
            snapshot: None,
        },
    )?;
    Ok(())
}

/// 会话历史窗口（继续问时随 runtime input 带给模型的最近上下文）。
pub fn recent_history(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<AgentMessageRecord>, String> {
    let mut all = list_messages(conn, session_id)?;
    let start = all.len().saturating_sub(HISTORY_WINDOW_MESSAGES);
    all.drain(..start);
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_store::{list_context_items, list_events, open_state_db};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "sw-agent-session-{name}-{}-{}",
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

    fn open_fresh(name: &str) -> (TempDir, Connection) {
        let dir = TempDir::new(name);
        let conn = open_state_db(&dir.path().join("state.db")).unwrap();
        (dir, conn)
    }

    fn sample_session(conn: &mut Connection, title: &str) -> AgentSessionRecord {
        create_session(
            conn,
            NewAgentSession {
                workspace_id: Some("ws-hash".into()),
                title: title.into(),
            },
        )
        .unwrap()
    }

    fn user_turn(
        conn: &mut Connection,
        session_id: &str,
        instruction: &str,
        origin: Option<&str>,
        run: Option<&str>,
    ) -> AgentMessageRecord {
        let context_item_uris = origin
            .and_then(|uri| ObjectUri::parse(uri).ok())
            .into_iter()
            .collect();
        append_message(
            conn,
            NewAgentMessage {
                session_id: session_id.into(),
                role: AgentMessageRole::User,
                content: instruction.into(),
                run_ref: run.map(|r| r.into()),
                origin_uri: origin.map(|o| o.into()),
                quote_snapshot: None,
                context_item_uris,
            },
        )
        .unwrap()
    }

    fn assistant_turn(
        conn: &mut Connection,
        session_id: &str,
        content: &str,
        run: &str,
    ) -> AgentMessageRecord {
        append_message(
            conn,
            NewAgentMessage {
                session_id: session_id.into(),
                role: AgentMessageRole::Assistant,
                content: content.into(),
                run_ref: Some(run.into()),
                origin_uri: None,
                quote_snapshot: None,
                context_item_uris: vec![],
            },
        )
        .unwrap()
    }

    #[test]
    fn create_session_emits_event_and_survives_reopen() {
        let (dir, mut conn) = open_fresh("create");
        let session = sample_session(&mut conn, "Work 放在哪里");
        assert!(session.id.starts_with(SESSION_ID_PREFIX));
        assert_eq!(session.title, "Work 放在哪里");
        assert_eq!(session.provider, None);
        let events = list_events(&conn, 10).unwrap();
        assert_eq!(events[0].action, event_action::AGENT_SESSION_CREATED);
        assert_eq!(
            events[0].object_uri.as_deref(),
            Some(ObjectUri::agent_session(&session.id).as_str())
        );
        drop(conn);
        let conn = open_state_db(&dir.path().join("state.db")).unwrap();
        let loaded = get_session(&conn, &session.id).unwrap().unwrap();
        assert_eq!(loaded.title, "Work 放在哪里");
    }

    #[test]
    fn create_rejects_blank_title_and_missing_session() {
        let (_dir, mut conn) = open_fresh("reject");
        assert!(create_session(
            &mut conn,
            NewAgentSession {
                workspace_id: None,
                title: "   ".into()
            }
        )
        .is_err());
        assert!(append_message(
            &mut conn,
            NewAgentMessage {
                session_id: "as-missing".into(),
                role: AgentMessageRole::User,
                content: "问".into(),
                run_ref: None,
                origin_uri: None,
                quote_snapshot: None,
                context_item_uris: vec![],
            }
        )
        .is_err());
    }

    #[test]
    fn thread_roundtrip_preserves_order_and_run_binding() {
        let (_dir, mut conn) = open_fresh("thread");
        let session = sample_session(&mut conn, "讨论");
        user_turn(&mut conn, &session.id, "第一问", None, Some("run-1"));
        assistant_turn(&mut conn, &session.id, "第一答", "run-1");
        user_turn(&mut conn, &session.id, "第二问", None, Some("run-2"));
        assistant_turn(&mut conn, &session.id, "第二答", "run-2");
        let messages = list_messages(&conn, &session.id).unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, AgentMessageRole::User);
        assert_eq!(messages[0].content, "第一问");
        assert_eq!(messages[3].content, "第二答");
        // settle 侧幂等：同 run 再落 assistant 会被唯一 run 索引拒绝
        assert!(append_message(
            &mut conn,
            NewAgentMessage {
                session_id: session.id.clone(),
                role: AgentMessageRole::Assistant,
                content: "重复".into(),
                run_ref: Some("run-1".into()),
                origin_uri: None,
                quote_snapshot: None,
                context_item_uris: vec![],
            }
        )
        .is_err());
        let found = find_user_message_by_run(&conn, "run-2").unwrap().unwrap();
        assert_eq!(found.content, "第二问");
        assert!(find_assistant_message_by_run(&conn, "run-2")
            .unwrap()
            .is_some());
    }

    #[test]
    fn turn_context_set_is_linked_via_thread_id() {
        let (_dir, mut conn) = open_fresh("context");
        let session = sample_session(&mut conn, "带资料讨论");
        let message = user_turn(
            &mut conn,
            &session.id,
            "这个判断有什么问题",
            Some("workspace://docs/agent-design.md"),
            Some("run-ctx"),
        );
        let context_id = message.context_set_id.expect("turn 应建立 context set");
        let items = list_context_items(&conn, context_id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].object_uri, "workspace://docs/agent-design.md");
        let set = crate::state_store::get_context_set(&conn, context_id)
            .unwrap()
            .unwrap();
        assert_eq!(set.thread_id.as_deref(), Some(session.id.as_str()));
    }

    #[test]
    fn list_and_related_cover_workspace_scoping() {
        let (_dir, mut conn) = open_fresh("related");
        let ws_a = sample_session(&mut conn, "A 会话");
        let ws_b = create_session(
            &mut conn,
            NewAgentSession {
                workspace_id: Some("other-ws".into()),
                title: "B 会话".into(),
            },
        )
        .unwrap();
        user_turn(
            &mut conn,
            &ws_a.id,
            "问 M3",
            Some("workspace://docs/m3.md"),
            Some("run-a"),
        );
        user_turn(
            &mut conn,
            &ws_b.id,
            "别的工作区",
            Some("workspace://docs/m3.md"),
            Some("run-b"),
        );
        let listed = list_sessions(&conn, "ws-hash", 10).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].session.id, ws_a.id);
        assert_eq!(listed[0].message_count, 1);
        let related = related_sessions(&conn, "ws-hash", "workspace://docs/m3.md", 10).unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(
            related[0].last_origin_uri.as_deref(),
            Some("workspace://docs/m3.md")
        );
    }

    #[test]
    fn outcome_relation_records_lineage_and_rejects_repeat() {
        let (_dir, mut conn) = open_fresh("outcome");
        let session = sample_session(&mut conn, "链路");
        let message = user_turn(&mut conn, &session.id, "问", None, Some("run-o"));
        link_message_outcome(
            &mut conn,
            &message.id,
            OutcomePredicate::PromotedTo,
            ObjectUri::work("w-42"),
            Some("ws-hash".into()),
        )
        .unwrap();
        assert!(link_message_outcome(
            &mut conn,
            &message.id,
            OutcomePredicate::PromotedTo,
            ObjectUri::work("w-42"),
            Some("ws-hash".into()),
        )
        .is_err());
        let events = list_events(&conn, 20).unwrap();
        assert!(events
            .iter()
            .any(|event| event.action == event_action::RELATION_CREATED));
        // 白名单外的 predicate 不允许进入链路
        assert!(OutcomePredicate::parse("likes").is_err());
    }

    #[test]
    fn recent_history_returns_bounded_window_in_order() {
        let (_dir, mut conn) = open_fresh("history");
        let session = sample_session(&mut conn, "长会话");
        for i in 0..20 {
            user_turn(&mut conn, &session.id, &format!("问 {i}"), None, None);
            assistant_turn(
                &mut conn,
                &session.id,
                &format!("答 {i}"),
                &format!("run-h{i}"),
            );
        }
        let history = recent_history(&conn, &session.id).unwrap();
        assert_eq!(history.len(), HISTORY_WINDOW_MESSAGES);
        assert_eq!(history[0].content, "问 14");
        assert_eq!(history.last().unwrap().content, "答 19");
    }
}
