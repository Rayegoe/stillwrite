//! M1 Durable Work 无 UI integration test（Headless Gate）：
//!
//! ```text
//! create → running → attach artifact → needs_human
//! → query → complete → restart → state preserved
//! ```
//!
//! 不渲染 UI、不依赖 Pi 进程：这里的 domain 函数调用序列与
//! `pi_agent::agent_start` / `lib.rs::create_agent_work` 桥接命令的写入路径
//! 完全一致（请求=queued、accepted=running、Artifact 保存=needs_human、
//! abort=cancelled、依赖缺失=blocked、致命错误=failed、人工接受=completed）。
//!
//! M1 业务验收：重启后仍能可靠判断一个 Agent 请求是 running / failed /
//! needs_human，并能找到 Artifact / receipt。

use std::path::PathBuf;

use stillwrite_lib::state_store::{event_action, list_events, open_state_db, ActorKind, ObjectUri};
use stillwrite_lib::work::{
    attach_work_artifact, create_work, find_work_by_receipt, get_work, list_works, transition_work,
    AttachArtifact, NewWork, WorkStatus,
};

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sw-e2e-work-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 一个 workspace key 形状的 id（durable state 不写绝对路径）。
const WORKSPACE_ID: &str = "5f1e2d3c4b5a6978";
const ARTIFACT_URI: &str = "agentwork://5f1e2d3c4b5a6978/work-abc123";

#[test]
fn pi_request_lifecycle_reaches_needs_human_and_survives_restart() {
    let dir = tmp_dir("lifecycle");
    let db_path = dir.join("state.db");
    let run_id = "local-lifecycle-run";

    // 1. 请求创建 → queued；2. Pi accepted → running；3. Artifact 保存 → needs_human
    let work = {
        let mut conn = open_state_db(&db_path).unwrap();
        let work = create_work(
            &mut conn,
            NewWork {
                workspace_id: Some(WORKSPACE_ID.into()),
                title: "调研 durable work".into(),
                intent: "帮我调研 durable work 现状并给出结论".into(),
                receipt_ref: Some(run_id.into()),
            },
        )
        .unwrap();
        assert_eq!(work.status, WorkStatus::Queued);
        assert_eq!(work.receipt_ref.as_deref(), Some(run_id));

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
                artifact_uri: ObjectUri::parse(ARTIFACT_URI).unwrap(),
                summary: None,
                next_action: Some("等待人工验收".into()),
            },
        )
        .unwrap();
        assert_eq!(needs_human.status, WorkStatus::NeedsHuman);
        drop(conn);
        work
    };

    // 4. query：不渲染 UI 也能回答「什么待审、产物在哪、收据是哪次运行」
    {
        let conn = open_state_db(&db_path).unwrap();
        let by_receipt = find_work_by_receipt(&conn, run_id).unwrap().unwrap();
        assert_eq!(by_receipt.id, work.id);
        assert_eq!(by_receipt.status, WorkStatus::NeedsHuman);
        assert_eq!(by_receipt.artifact_uri.as_deref(), Some(ARTIFACT_URI));
        let workspace_works = list_works(&conn, Some(WORKSPACE_ID), 10).unwrap();
        assert_eq!(workspace_works.len(), 1);
        assert_eq!(workspace_works[0].id, work.id);
    }

    // 5. 人工明确接受 → completed（Pi 返回文本不触发这一步）
    {
        let mut conn = open_state_db(&db_path).unwrap();
        let completed = transition_work(
            &mut conn,
            &work.id,
            WorkStatus::Completed,
            ActorKind::Human,
            None,
        )
        .unwrap();
        assert_eq!(completed.status, WorkStatus::Completed);
    }

    // 6. restart → 状态、Artifact、receipt 全部保留；事件面只有允许的三个动作
    {
        let conn = open_state_db(&db_path).unwrap();
        let reopened = get_work(&conn, &work.id).unwrap().unwrap();
        assert_eq!(reopened.status, WorkStatus::Completed);
        assert_eq!(reopened.artifact_uri.as_deref(), Some(ARTIFACT_URI));
        assert_eq!(reopened.receipt_ref.as_deref(), Some(run_id));
        assert_eq!(reopened.intent, "帮我调研 durable work 现状并给出结论");
        assert_eq!(reopened.next_action.as_deref(), Some("等待人工验收"));

        let events = list_events(&conn, 20).unwrap();
        let actions: Vec<_> = events.iter().map(|event| event.action.as_str()).collect();
        assert_eq!(
            actions,
            vec![
                event_action::WORK_STATUS_CHANGED, // completed（人工接受）
                event_action::WORK_STATUS_CHANGED, // needs_human（artifact 绑定）
                event_action::WORK_STATUS_CHANGED, // running（pi accepted）
                event_action::WORK_CREATED,        // 请求创建
            ]
        );
        // needs_human 事件必须携带 Artifact 指纹，供 M2 证据投影使用
        let attached = events
            .iter()
            .find(|event| {
                event.action == event_action::WORK_STATUS_CHANGED
                    && event.payload.as_ref().unwrap()["to"] == "needs_human"
            })
            .unwrap();
        assert_eq!(
            attached.payload.as_ref().unwrap()["artifact_uri"],
            ARTIFACT_URI
        );
    }
}

#[test]
fn start_failures_and_abort_end_in_queryable_terminal_states() {
    let dir = tmp_dir("terminals");
    let db_path = dir.join("state.db");
    let mut ids = Vec::new();
    {
        let mut conn = open_state_db(&db_path).unwrap();
        let seed = |conn: &mut rusqlite::Connection, receipt: &str, title: &str| {
            create_work(
                conn,
                NewWork {
                    workspace_id: Some(WORKSPACE_ID.into()),
                    title: title.into(),
                    intent: "intent".into(),
                    receipt_ref: Some(receipt.into()),
                },
            )
            .unwrap()
        };
        // 依赖缺失（缺 Pi / 缺模型）→ blocked，错误原因进事件 payload
        let blocked = seed(&mut conn, "run-blocked", "依赖缺失样本");
        transition_work(
            &mut conn,
            &blocked.id,
            WorkStatus::Blocked,
            ActorKind::Agent,
            Some("未找到 Pi：请安装 Pi，或配置 STILLWRITE_PI_EXECUTABLE"),
        )
        .unwrap();
        // 致命错误 → failed
        let failed = seed(&mut conn, "run-failed", "致命错误样本");
        transition_work(
            &mut conn,
            &failed.id,
            WorkStatus::Running,
            ActorKind::Agent,
            Some("pi accepted"),
        )
        .unwrap();
        transition_work(
            &mut conn,
            &failed.id,
            WorkStatus::Failed,
            ActorKind::Agent,
            Some("Pi 进程已退出"),
        )
        .unwrap();
        // abort → cancelled
        let cancelled = seed(&mut conn, "run-cancelled", "用户中止样本");
        transition_work(
            &mut conn,
            &cancelled.id,
            WorkStatus::Running,
            ActorKind::Agent,
            Some("pi accepted"),
        )
        .unwrap();
        transition_work(
            &mut conn,
            &cancelled.id,
            WorkStatus::Cancelled,
            ActorKind::Human,
            Some("用户停止 Agent"),
        )
        .unwrap();
        ids.extend([blocked.id.clone(), failed.id.clone(), cancelled.id.clone()]);
    }

    // 重启后三种终态仍可按 workspace 聚合查询（M2「需要你/进行中/最近完成」的数据基础）
    let conn = open_state_db(&db_path).unwrap();
    let statuses: Vec<_> = ["run-blocked", "run-failed", "run-cancelled"]
        .iter()
        .map(|receipt| {
            find_work_by_receipt(&conn, receipt)
                .unwrap()
                .unwrap()
                .status
        })
        .collect();
    assert_eq!(
        statuses,
        vec![
            WorkStatus::Blocked,
            WorkStatus::Failed,
            WorkStatus::Cancelled
        ]
    );
    assert_eq!(list_works(&conn, Some(WORKSPACE_ID), 10).unwrap().len(), 3);
    assert_eq!(
        get_work(&conn, &ids[0]).unwrap().unwrap().status,
        WorkStatus::Blocked
    );
}

#[test]
fn blocked_work_can_reenter_running_after_dependency_fixed() {
    let dir = tmp_dir("blocked-retry");
    let db_path = dir.join("state.db");
    let mut conn = open_state_db(&db_path).unwrap();
    let work = create_work(
        &mut conn,
        NewWork {
            workspace_id: Some(WORKSPACE_ID.into()),
            title: "重试样本".into(),
            intent: "依赖补齐后重试".into(),
            receipt_ref: Some("run-retry".into()),
        },
    )
    .unwrap();
    transition_work(
        &mut conn,
        &work.id,
        WorkStatus::Blocked,
        ActorKind::Agent,
        Some("没有可用模型"),
    )
    .unwrap();
    let retried = transition_work(
        &mut conn,
        &work.id,
        WorkStatus::Running,
        ActorKind::Agent,
        Some("pi accepted after retry"),
    )
    .unwrap();
    assert_eq!(retried.status, WorkStatus::Running);
    // blocked → running 有两次 status_changed，原因分别可查（list_events 新的在前）
    let events = list_events(&conn, 10).unwrap();
    let reasons: Vec<_> = events
        .iter()
        .filter_map(|event| {
            event
                .payload
                .as_ref()
                .and_then(|payload| payload.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect();
    assert_eq!(reasons[0], "pi accepted after retry");
    assert!(reasons[1].contains("没有可用模型"));
}

#[test]
fn settled_text_without_saved_artifact_keeps_work_running() {
    // Pi 返回最终文本不是任何 Work 状态变化：agent_settled 之后、Artifact
    // 保存之前，Work 必须仍是 running，既不是 needs_human 也不是 completed。
    let dir = tmp_dir("settled-not-completed");
    let db_path = dir.join("state.db");
    let mut conn = open_state_db(&db_path).unwrap();
    let work = create_work(
        &mut conn,
        NewWork {
            workspace_id: Some(WORKSPACE_ID.into()),
            title: "settled 样本".into(),
            intent: "验证文本不等于完成".into(),
            receipt_ref: Some("run-settled".into()),
        },
    )
    .unwrap();
    transition_work(
        &mut conn,
        &work.id,
        WorkStatus::Running,
        ActorKind::Agent,
        Some("pi accepted"),
    )
    .unwrap();

    // —— agent_settled 发生：Work 侧没有任何调用 ——
    let after_settle = get_work(&conn, &work.id).unwrap().unwrap();
    assert_eq!(after_settle.status, WorkStatus::Running);
    assert_eq!(after_settle.artifact_uri, None);

    // 事件面停留在 created + running：settled 没有伪造任何 Work 事件
    let events = list_events(&conn, 10).unwrap();
    let actions: Vec<_> = events.iter().map(|event| event.action.as_str()).collect();
    assert_eq!(
        actions,
        vec![
            event_action::WORK_STATUS_CHANGED,
            event_action::WORK_CREATED
        ]
    );
}
