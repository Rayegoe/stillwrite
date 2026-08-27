//! Work：Agent 工作的 durable 协调对象（M1 bridge schema）。
//!
//! Work 是「目标 + 状态 + 产物 + 证据」的查询入口，不复制 Pi 的运行时协议：
//! receipt/session 本体由 Pi runtime 留存，这里只保存 `receipt_ref` 引用；
//! Agent 的最终 Markdown 仍是 Agent Work Artifact，这里只保存 `artifact_uri`。
//!
//! 状态机是 Domain rule：Pi 返回最终文本不等于 `completed`，
//! `completed` 只来自人的明确接受；状态转换全部走 [`transition_work`]，
//! 非法转换（包括终态再变化、LLM 自报完成）都会被拒绝。
//!
//! 事件面固定三个动作：`work.created` / `work.status_changed` / `work.updated`，
//! 与引发它的状态变更在同一事务中落地。

use crate::state_store::{
    append_event, event_action, now_iso, tx_command, ActorKind, NewEvent, ObjectUri,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// Work 协调状态（第一版固定集合）。Run 的执行状态不在这里：
/// Run `succeeded` 不自动等于 Work `completed`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WorkStatus {
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "needs_human")]
    NeedsHuman,
    #[serde(rename = "blocked")]
    Blocked,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl WorkStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkStatus::Queued => "queued",
            WorkStatus::Running => "running",
            WorkStatus::NeedsHuman => "needs_human",
            WorkStatus::Blocked => "blocked",
            WorkStatus::Completed => "completed",
            WorkStatus::Failed => "failed",
            WorkStatus::Cancelled => "cancelled",
        }
    }

    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "queued" => Ok(WorkStatus::Queued),
            "running" => Ok(WorkStatus::Running),
            "needs_human" => Ok(WorkStatus::NeedsHuman),
            "blocked" => Ok(WorkStatus::Blocked),
            "completed" => Ok(WorkStatus::Completed),
            "failed" => Ok(WorkStatus::Failed),
            "cancelled" => Ok(WorkStatus::Cancelled),
            other => Err(format!("未知的 Work 状态: {other}")),
        }
    }
}

/// 合法状态转换表。终态（completed/failed/cancelled）不允许再变化；
/// `blocked` 表达可恢复的输入缺失，允许依赖补齐后重新 `running`。
fn transition_allowed(from: WorkStatus, to: WorkStatus) -> bool {
    use WorkStatus::*;
    match from {
        Queued => matches!(to, Running | Blocked | Failed | Cancelled),
        Running => matches!(to, NeedsHuman | Blocked | Failed | Cancelled),
        NeedsHuman => matches!(to, Completed | Cancelled),
        Blocked => matches!(to, Running | Failed | Cancelled),
        Completed | Failed | Cancelled => false,
    }
}

// ---------------------------------------------------------------------------
// Records & inputs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkRecord {
    pub id: String,
    pub workspace_id: Option<String>,
    pub title: String,
    /// 人的原始意图；不因 Agent 执行过程而改写。
    pub intent: String,
    pub status: WorkStatus,
    pub summary: Option<String>,
    pub next_action: Option<String>,
    /// Agent Work Artifact 的 canonical URI（`agentwork://...`）。
    pub artifact_uri: Option<String>,
    /// 运行收据引用（run id；收据本体在 `<AppData>/agent/runs/<run-id>.jsonl`）。
    pub receipt_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewWork {
    pub workspace_id: Option<String>,
    pub title: String,
    pub intent: String,
    pub receipt_ref: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateWork {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AttachArtifact {
    pub artifact_uri: ObjectUri,
    pub summary: Option<String>,
    pub next_action: Option<String>,
}

// ---------------------------------------------------------------------------
// Commands（全部走 tx_command：状态变更 + 事件同生共死）
// ---------------------------------------------------------------------------

/// 创建 Work：请求产生即 `queued`。
/// `receipt_ref` 唯一——同一运行重复绑定会被唯一索引拒绝。
pub fn create_work(conn: &mut Connection, input: NewWork) -> Result<WorkRecord, String> {
    tx_command(conn, move |tx| {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err("Work 缺少 title".into());
        }
        let intent = input.intent.trim().to_string();
        if intent.is_empty() {
            return Err("Work 缺少 intent".into());
        }
        let receipt_ref = validate_receipt_ref(input.receipt_ref.as_deref())?;
        let id = new_work_id();
        let now = now_iso();
        tx.execute(
            "INSERT INTO works (id, workspace_id, title, intent, status, summary, next_action, artifact_uri, receipt_ref, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'queued', NULL, NULL, NULL, ?5, ?6, ?6)",
            params![id, input.workspace_id, title, intent, receipt_ref, now],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                format!("该运行已绑定过 Work: {receipt_ref:?}")
            } else {
                format!("写入 work 失败: {e}")
            }
        })?;
        let record = get_work_in_tx(tx, &id)?.ok_or_else(|| "回读 work 失败".to_string())?;
        append_event(
            tx,
            NewEvent {
                action: event_action::WORK_CREATED.to_string(),
                workspace_id: record.workspace_id.clone(),
                object_uri: Some(ObjectUri::work(&record.id)),
                work_id: Some(record.id.clone()),
                payload: Some(serde_json::json!({
                    "status": record.status.as_str(),
                    "title": record.title,
                    "receipt_ref": record.receipt_ref,
                })),
                ..NewEvent::default()
            },
        )?;
        Ok(record)
    })
}

/// 状态转换。同状态重复转换视为幂等 no-op（直接返回当前记录，不产生事件）；
/// 非法转换返回错误——状态机归 Domain rule，不听 LLM 自报。
pub fn transition_work(
    conn: &mut Connection,
    work_id: &str,
    to: WorkStatus,
    actor: ActorKind,
    reason: Option<&str>,
) -> Result<WorkRecord, String> {
    tx_command(conn, move |tx| {
        let record = get_work_in_tx(tx, work_id)?.ok_or_else(|| format!("work 不存在: {work_id}"))?;
        if record.status == to {
            return Ok(record);
        }
        if !transition_allowed(record.status, to) {
            return Err(format!(
                "非法 Work 状态转换: {} → {}",
                record.status.as_str(),
                to.as_str()
            ));
        }
        tx.execute(
            "UPDATE works SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![to.as_str(), now_iso(), work_id],
        )
        .map_err(|e| format!("更新 work 状态失败: {e}"))?;
        let mut payload = serde_json::json!({
            "from": record.status.as_str(),
            "to": to.as_str(),
        });
        if let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) {
            payload["reason"] = serde_json::Value::String(reason.to_string());
        }
        append_event(
            tx,
            NewEvent {
                action: event_action::WORK_STATUS_CHANGED.to_string(),
                actor_type: Some(actor),
                workspace_id: record.workspace_id.clone(),
                object_uri: Some(ObjectUri::work(&record.id)),
                work_id: Some(record.id.clone()),
                payload: Some(payload),
                ..NewEvent::default()
            },
        )?;
        get_work_in_tx(tx, work_id)?.ok_or_else(|| "回读 work 失败".to_string())
    })
}

/// 绑定 Artifact（Agent Work Markdown 已保存后调用）：`running → needs_human`。
/// 同一 Artifact 重复绑定视为幂等 no-op；Artifact 已固化后状态仍不变。
pub fn attach_work_artifact(
    conn: &mut Connection,
    work_id: &str,
    attach: AttachArtifact,
) -> Result<WorkRecord, String> {
    tx_command(conn, move |tx| {
        let record = get_work_in_tx(tx, work_id)?.ok_or_else(|| format!("work 不存在: {work_id}"))?;
        if record.status == WorkStatus::NeedsHuman {
            if record.artifact_uri.as_deref() == Some(attach.artifact_uri.as_str()) {
                return Ok(record);
            }
            return Err(format!(
                "Work 已绑定其它 Artifact: {work_id} ({:?})",
                record.artifact_uri
            ));
        }
        if record.status != WorkStatus::Running {
            return Err(format!(
                "只有 running 中的 Work 才能绑定 Artifact，当前 {}",
                record.status.as_str()
            ));
        }
        let now = now_iso();
        tx.execute(
            "UPDATE works SET status = 'needs_human', summary = ?1, next_action = ?2, artifact_uri = ?3, updated_at = ?4 WHERE id = ?5",
            params![
                attach.summary.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                attach.next_action.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                attach.artifact_uri.as_str(),
                now,
                work_id,
            ],
        )
        .map_err(|e| format!("绑定 work artifact 失败: {e}"))?;
        append_event(
            tx,
            NewEvent {
                action: event_action::WORK_STATUS_CHANGED.to_string(),
                actor_type: Some(ActorKind::Agent),
                workspace_id: record.workspace_id.clone(),
                object_uri: Some(ObjectUri::work(&record.id)),
                work_id: Some(record.id.clone()),
                payload: Some(serde_json::json!({
                    "from": record.status.as_str(),
                    "to": WorkStatus::NeedsHuman.as_str(),
                    "artifact_uri": attach.artifact_uri.as_str(),
                    "next_action": attach.next_action,
                })),
                ..NewEvent::default()
            },
        )?;
        get_work_in_tx(tx, work_id)?.ok_or_else(|| "回读 work 失败".to_string())
    })
}

/// 更新展示/协调字段（title/summary/next_action）。只有字段真的变化才写
/// `work.updated`；状态机不由本命令触碰。
pub fn update_work(
    conn: &mut Connection,
    work_id: &str,
    update: UpdateWork,
) -> Result<WorkRecord, String> {
    tx_command(conn, move |tx| {
        let record = get_work_in_tx(tx, work_id)?.ok_or_else(|| format!("work 不存在: {work_id}"))?;
        let normalize = |value: Option<String>| -> Option<String> {
            value
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let title = normalize(update.title);
        let summary = normalize(update.summary);
        let next_action = normalize(update.next_action);
        let title_changed = title.as_deref().is_some_and(|value| value != record.title);
        let summary_changed = summary.is_some() && summary != record.summary;
        let next_action_changed = next_action.is_some() && next_action != record.next_action;
        if !title_changed && !summary_changed && !next_action_changed {
            return Ok(record);
        }
        let mut changed = Vec::new();
        if title_changed {
            changed.push("title");
        }
        if summary_changed {
            changed.push("summary");
        }
        if next_action_changed {
            changed.push("next_action");
        }
        tx.execute(
            "UPDATE works SET
                title = COALESCE(?1, title),
                summary = COALESCE(?2, summary),
                next_action = COALESCE(?3, next_action),
                updated_at = ?4
             WHERE id = ?5",
            params![
                title.filter(|_| title_changed),
                summary.filter(|_| summary_changed),
                next_action.filter(|_| next_action_changed),
                now_iso(),
                work_id,
            ],
        )
        .map_err(|e| format!("更新 work 失败: {e}"))?;
        append_event(
            tx,
            NewEvent {
                action: event_action::WORK_UPDATED.to_string(),
                workspace_id: record.workspace_id.clone(),
                object_uri: Some(ObjectUri::work(&record.id)),
                work_id: Some(record.id.clone()),
                payload: Some(serde_json::json!({
                    "changed": changed,
                })),
                ..NewEvent::default()
            },
        )?;
        get_work_in_tx(tx, work_id)?.ok_or_else(|| "回读 work 失败".to_string())
    })
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

const WORK_COLUMNS: &str = "id, workspace_id, title, intent, status, summary, next_action, artifact_uri, receipt_ref, created_at, updated_at";

fn row_to_work(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkRecord> {
    let status: String = row.get(4)?;
    let status = WorkStatus::parse(&status).map_err(|_| {
        rusqlite::Error::InvalidColumnType(4, status.clone(), rusqlite::types::Type::Text)
    })?;
    Ok(WorkRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        title: row.get(2)?,
        intent: row.get(3)?,
        status,
        summary: row.get(5)?,
        next_action: row.get(6)?,
        artifact_uri: row.get(7)?,
        receipt_ref: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn get_work_in_tx(tx: &rusqlite::Transaction, work_id: &str) -> Result<Option<WorkRecord>, String> {
    let sql = format!("SELECT {WORK_COLUMNS} FROM works WHERE id = ?1");
    tx.query_row(&sql, params![work_id], row_to_work)
        .optional()
        .map_err(|e| format!("查询 work 失败: {e}"))
}

pub fn get_work(conn: &Connection, work_id: &str) -> Result<Option<WorkRecord>, String> {
    let sql = format!("SELECT {WORK_COLUMNS} FROM works WHERE id = ?1");
    conn.query_row(&sql, params![work_id], row_to_work)
        .optional()
        .map_err(|e| format!("查询 work 失败: {e}"))
}

/// 按运行收据引用定位 Work（Pi 桥接用：run id → work）。
pub fn find_work_by_receipt(
    conn: &Connection,
    receipt_ref: &str,
) -> Result<Option<WorkRecord>, String> {
    let sql = format!("SELECT {WORK_COLUMNS} FROM works WHERE receipt_ref = ?1");
    conn.query_row(&sql, params![receipt_ref.trim()], row_to_work)
        .optional()
        .map_err(|e| format!("按收据查询 work 失败: {e}"))
}

/// 最近更新的 Work（新的在前），可按 workspace 过滤。
pub fn list_works(
    conn: &Connection,
    workspace_id: Option<&str>,
    limit: usize,
) -> Result<Vec<WorkRecord>, String> {
    let sql = format!(
        "SELECT {WORK_COLUMNS} FROM works
         WHERE (?1 IS NULL OR workspace_id = ?1)
         ORDER BY updated_at DESC, id DESC LIMIT ?2"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询 works 失败: {e}"))?;
    let rows = stmt
        .query_map(params![workspace_id, limit as i64], row_to_work)
        .map_err(|e| format!("查询 works 失败: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// receipt_ref 只是引用键（run id），不允许空白/控制字符；
/// 本体文件由 Pi runtime 持有，这里不做路径语义。
fn validate_receipt_ref(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 128 || value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("运行收据引用非法: {value:?}"));
    }
    Ok(Some(value.to_string()))
}

fn new_work_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("work-{:x}-{:x}", std::process::id(), nanos)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_store::{list_events, open_state_db};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "sw-work-{name}-{}-{}",
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

    fn sample_work(conn: &mut Connection, receipt: &str) -> WorkRecord {
        create_work(
            conn,
            NewWork {
                workspace_id: Some("ws-hash".into()),
                title: "调研 durable work".into(),
                intent: "帮我调研并总结 durable work 的设计".into(),
                receipt_ref: Some(receipt.into()),
            },
        )
        .unwrap()
    }

    #[test]
    fn create_starts_queued_and_emits_work_created() {
        let (_dir, mut conn) = open_fresh("create");
        let work = sample_work(&mut conn, "run-1");
        assert_eq!(work.status, WorkStatus::Queued);
        assert_eq!(work.receipt_ref.as_deref(), Some("run-1"));
        assert_eq!(work.artifact_uri, None);
        assert_eq!(work.summary, None);
        assert_eq!(work.created_at, work.updated_at);

        let events = list_events(&conn, 10).unwrap();
        assert_eq!(events.len(), 1);
        let created = &events[0];
        assert_eq!(created.action, event_action::WORK_CREATED);
        assert_eq!(created.work_id.as_deref(), Some(work.id.as_str()));
        assert_eq!(
            created.object_uri.as_deref(),
            Some(ObjectUri::work(&work.id).as_str())
        );
        assert_eq!(created.payload.as_ref().unwrap()["status"], "queued");
        assert_eq!(created.workspace_id.as_deref(), Some("ws-hash"));
    }

    #[test]
    fn create_rejects_missing_title_intent_and_duplicate_receipt() {
        let (_dir, mut conn) = open_fresh("validate");
        let error = create_work(
            &mut conn,
            NewWork {
                workspace_id: None,
                title: "  ".into(),
                intent: "intent".into(),
                receipt_ref: None,
            },
        )
        .unwrap_err();
        assert!(error.contains("title"));
        assert!(create_work(
            &mut conn,
            NewWork {
                workspace_id: None,
                title: "t".into(),
                intent: "".into(),
                receipt_ref: None,
            },
        )
        .unwrap_err()
        .contains("intent"));
        assert!(create_work(
            &mut conn,
            NewWork {
                workspace_id: None,
                title: "t".into(),
                intent: "i".into(),
                receipt_ref: Some("bad ref".into()),
            },
        )
        .unwrap_err()
        .contains("收据引用非法"));

        sample_work(&mut conn, "run-dup");
        let duplicate = create_work(
            &mut conn,
            NewWork {
                workspace_id: None,
                title: "另一份".into(),
                intent: "同 run 重复绑定".into(),
                receipt_ref: Some("run-dup".into()),
            },
        )
        .unwrap_err();
        assert!(duplicate.contains("已绑定过 Work"));
    }

    #[test]
    fn full_lifecycle_records_status_events() {
        let (_dir, mut conn) = open_fresh("lifecycle");
        let work = sample_work(&mut conn, "run-2");

        let running = transition_work(
            &mut conn,
            &work.id,
            WorkStatus::Running,
            ActorKind::Agent,
            Some("pi accepted"),
        )
        .unwrap();
        assert_eq!(running.status, WorkStatus::Running);

        let needs_human = attach_work_artifact(
            &mut conn,
            &work.id,
            AttachArtifact {
                artifact_uri: ObjectUri::parse("agentwork://ws-key/work-1").unwrap(),
                summary: Some("完成了调研".into()),
                next_action: Some("等待人工验收".into()),
            },
        )
        .unwrap();
        assert_eq!(needs_human.status, WorkStatus::NeedsHuman);
        assert_eq!(
            needs_human.artifact_uri.as_deref(),
            Some("agentwork://ws-key/work-1")
        );
        assert_eq!(needs_human.next_action.as_deref(), Some("等待人工验收"));

        let completed = transition_work(
            &mut conn,
            &work.id,
            WorkStatus::Completed,
            ActorKind::Human,
            None,
        )
        .unwrap();
        assert_eq!(completed.status, WorkStatus::Completed);

        let events = list_events(&conn, 10).unwrap();
        let actions: Vec<_> = events
            .iter()
            .map(|event| event.action.as_str())
            .collect();
        assert_eq!(
            actions,
            vec![
                event_action::WORK_STATUS_CHANGED,
                event_action::WORK_STATUS_CHANGED,
                event_action::WORK_STATUS_CHANGED,
                event_action::WORK_CREATED,
            ]
        );
        // list_events 新的在前：completed ← needs_human ← running ← created
        let accepted = &events[0];
        assert_eq!(accepted.payload.as_ref().unwrap()["from"], "needs_human");
        assert_eq!(accepted.payload.as_ref().unwrap()["to"], "completed");
        assert_eq!(accepted.actor_type, "human");
        let attached = &events[1];
        assert_eq!(
            attached.payload.as_ref().unwrap()["artifact_uri"],
            "agentwork://ws-key/work-1"
        );
        let running = &events[2];
        assert_eq!(running.payload.as_ref().unwrap()["from"], "queued");
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        let (_dir, mut conn) = open_fresh("invalid");
        let work = sample_work(&mut conn, "run-3");
        // LLM 自报完成不等于验收：queued 不能直接 completed
        let error = transition_work(
            &mut conn,
            &work.id,
            WorkStatus::Completed,
            ActorKind::Agent,
            Some("self report"),
        )
        .unwrap_err();
        assert!(error.contains("非法 Work 状态转换"));

        transition_work(
            &mut conn,
            &work.id,
            WorkStatus::Running,
            ActorKind::Agent,
            None,
        )
        .unwrap();
        transition_work(
            &mut conn,
            &work.id,
            WorkStatus::Cancelled,
            ActorKind::Human,
            None,
        )
        .unwrap();
        // 终态冻结
        for to in [
            WorkStatus::Running,
            WorkStatus::Failed,
            WorkStatus::Completed,
        ] {
            assert!(transition_work(&mut conn, &work.id, to, ActorKind::Human, None).is_err());
        }
        assert_eq!(
            get_work(&conn, &work.id).unwrap().unwrap().status,
            WorkStatus::Cancelled
        );
    }

    #[test]
    fn same_status_transition_is_idempotent_without_event() {
        let (_dir, mut conn) = open_fresh("idempotent");
        let work = sample_work(&mut conn, "run-4");
        let again = transition_work(
            &mut conn,
            &work.id,
            WorkStatus::Queued,
            ActorKind::Agent,
            None,
        )
        .unwrap();
        assert_eq!(again.status, WorkStatus::Queued);
        assert_eq!(list_events(&conn, 10).unwrap().len(), 1);
    }

    #[test]
    fn attach_artifact_is_idempotent_and_validates_running_state() {
        let (_dir, mut conn) = open_fresh("attach");
        let work = sample_work(&mut conn, "run-5");
        let uri = ObjectUri::parse("agentwork://ws-key/work-2").unwrap();
        // queued 时不能绑定
        assert!(attach_work_artifact(
            &mut conn,
            &work.id,
            AttachArtifact {
                artifact_uri: uri.clone(),
                summary: None,
                next_action: None,
            },
        )
        .is_err());

        transition_work(
            &mut conn,
            &work.id,
            WorkStatus::Running,
            ActorKind::Agent,
            None,
        )
        .unwrap();
        let first = attach_work_artifact(
            &mut conn,
            &work.id,
            AttachArtifact {
                artifact_uri: uri.clone(),
                summary: Some("摘要".into()),
                next_action: Some("等待人工验收".into()),
            },
        )
        .unwrap();
        let repeat = attach_work_artifact(
            &mut conn,
            &work.id,
            AttachArtifact {
                artifact_uri: uri.clone(),
                summary: Some("摘要".into()),
                next_action: Some("等待人工验收".into()),
            },
        )
        .unwrap();
        assert_eq!(repeat.updated_at, first.updated_at);
        assert_eq!(list_events(&conn, 10).unwrap().len(), 3);
        // 绑定别的 Artifact 属于数据冲突
        assert!(attach_work_artifact(
            &mut conn,
            &work.id,
            AttachArtifact {
                artifact_uri: ObjectUri::parse("agentwork://ws-key/work-3").unwrap(),
                summary: None,
                next_action: None,
            },
        )
        .is_err());
    }

    #[test]
    fn update_work_emits_event_only_when_fields_change() {
        let (_dir, mut conn) = open_fresh("update");
        let work = sample_work(&mut conn, "run-6");
        let unchanged = update_work(
            &mut conn,
            &work.id,
            UpdateWork {
                summary: Some("  ".into()),
                ..UpdateWork::default()
            },
        )
        .unwrap();
        assert_eq!(unchanged.summary, None);
        assert_eq!(list_events(&conn, 10).unwrap().len(), 1);

        let updated = update_work(
            &mut conn,
            &work.id,
            UpdateWork {
                summary: Some("新摘要".into()),
                next_action: Some("等待运行结束".into()),
                ..UpdateWork::default()
            },
        )
        .unwrap();
        assert_eq!(updated.summary.as_deref(), Some("新摘要"));
        assert_eq!(updated.next_action.as_deref(), Some("等待运行结束"));
        assert!(updated.updated_at >= updated.created_at);
        let events = list_events(&conn, 10).unwrap();
        assert_eq!(events[0].action, event_action::WORK_UPDATED);
        assert_eq!(
            events[0].payload.as_ref().unwrap()["changed"],
            serde_json::json!(["summary", "next_action"])
        );
    }

    #[test]
    fn queries_cover_receipt_and_workspace_listing() {
        let (_dir, mut conn) = open_fresh("queries");
        let first = sample_work(&mut conn, "run-7");
        let second = create_work(
            &mut conn,
            NewWork {
                workspace_id: Some("ws-other".into()),
                title: "另一个工作区".into(),
                intent: "不同 workspace 的意图".into(),
                receipt_ref: Some("run-8".into()),
            },
        )
        .unwrap();

        assert_eq!(
            find_work_by_receipt(&conn, " run-7 ").unwrap().unwrap().id,
            first.id
        );
        assert!(find_work_by_receipt(&conn, "run-missing").unwrap().is_none());
        assert!(get_work(&conn, "missing").unwrap().is_none());

        let mine = list_works(&conn, Some("ws-hash"), 10).unwrap();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].id, first.id);
        let all = list_works(&conn, None, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, second.id, "新的在前");
        let limited = list_works(&conn, None, 1).unwrap();
        assert_eq!(limited.len(), 1);
    }

    #[test]
    fn work_state_survives_reopen() {
        let dir = TempDir::new("reopen");
        let db_path = dir.path().join("state.db");
        let work = {
            let mut conn = open_state_db(&db_path).unwrap();
            let work = sample_work(&mut conn, "run-9");
            transition_work(
                &mut conn,
                &work.id,
                WorkStatus::Running,
                ActorKind::Agent,
                None,
            )
            .unwrap();
            attach_work_artifact(
                &mut conn,
                &work.id,
                AttachArtifact {
                    artifact_uri: ObjectUri::parse("agentwork://ws-key/work-9").unwrap(),
                    summary: None,
                    next_action: Some("等待人工验收".into()),
                },
            )
            .unwrap();
            drop(conn);
            work
        };
        let conn = open_state_db(&db_path).unwrap();
        let reopened = get_work(&conn, &work.id).unwrap().unwrap();
        assert_eq!(reopened.status, WorkStatus::NeedsHuman);
        assert_eq!(
            reopened.artifact_uri.as_deref(),
            Some("agentwork://ws-key/work-9")
        );
        assert_eq!(reopened.receipt_ref.as_deref(), Some("run-9"));
        assert_eq!(reopened.created_at, work.created_at);
        let events = list_events(&conn, 10).unwrap();
        assert_eq!(events.len(), 3);
    }
}
