//! P1 durable state 端到端契约：从公开 API 验证
//! 「migration → 复合命令原子性 → 事件留存 → durable 数据跨重启」。
//!
//! `selected_quote_to_relation` 是未来 vertical slice 的命令模板：
//! 一个用户动作涉及的 anchors / relations / events 必须在同一事务中生效，
//! 任何一步失败都不能留下半截状态。

use std::path::PathBuf;
use stillwrite_lib::state_store::{
    attach_context_item, create_anchor_in_tx, create_context_set, create_relation,
    create_relation_in_tx, event_action, list_context_items, list_events, neighbors, open_state_db,
    tx_command, AnchorRecord, NeighborHit, NewAnchor, NewContextSet, NewRelation, ObjectUri,
    RelationDirection,
};

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sw-e2e-state-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 未来「选区 → ＋关联」的命令雏形（P2 迁移时只需挪进 backend command）。
/// anchor + relation + 语义事件在一个事务中同时落地。
fn selected_quote_to_relation(
    conn: &mut rusqlite::Connection,
    document_uri: ObjectUri,
    quote: &str,
    target_uri: ObjectUri,
) -> Result<(AnchorRecord, i64), String> {
    tx_command(conn, move |tx| {
        let anchor = create_anchor_in_tx(
            tx,
            NewAnchor {
                document_uri: document_uri.clone(),
                kind: "字句".into(),
                start_offset: 0,
                end_offset: quote.chars().count() as i64,
                quote: quote.to_string(),
                prefix: None,
                suffix: None,
            },
        )?;
        let relation = create_relation_in_tx(
            tx,
            NewRelation {
                // 选区级 Relation 的 source 必须是 anchor URI；document_uri
                // 只记录在 anchor.document_uri，不参与选区关系的唯一性。
                source_uri: ObjectUri::anchor(anchor.id),
                predicate: "related_to".into(),
                target_uri: target_uri.clone(),
                anchor_id: Some(anchor.id),
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        )?;
        Ok((anchor, relation.id))
    })
}

#[test]
fn composite_command_commits_state_and_event_together() {
    let mut conn = open_state_db(&tmp_dir("composite").join("state.db")).unwrap();

    let document = ObjectUri::workspace("草稿/第一章.md").unwrap();
    let library_doc = ObjectUri::library("arxiv-src", "papers/agent-memory.md").unwrap();
    let (_anchor, relation_id) = selected_quote_to_relation(
        &mut conn,
        document.clone(),
        "把资料关联到我的正文",
        library_doc.clone(),
    )
    .unwrap();
    // 把返回的 relation 变成可导航 URI：relation://<id>
    assert!(ObjectUri::relation(relation_id)
        .as_str()
        .starts_with("relation://"));

    // 状态与事件同时可见
    let anchor_uri = ObjectUri::anchor(_anchor.id);
    let hits: Vec<NeighborHit> = neighbors(&conn, anchor_uri.as_str(), None, 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].direction, RelationDirection::Outgoing);
    assert_eq!(hits[0].neighbor_uri, library_doc.as_str());
    assert_eq!(hits[0].relation.anchor_id, Some(_anchor.id));

    let last = list_events(&conn, 1).unwrap().remove(0);
    assert_eq!(last.action, event_action::RELATION_CREATED);
    assert_eq!(
        last.payload.as_ref().unwrap()["source_uri"].as_str(),
        Some(anchor_uri.as_str())
    );
}

#[test]
fn distinct_selected_anchors_can_link_to_the_same_target() {
    let mut conn = open_state_db(&tmp_dir("anchor-relation-grain").join("state.db")).unwrap();
    let document = ObjectUri::workspace("草稿/第一章.md").unwrap();
    let target = ObjectUri::library("arxiv-src", "papers/agent-memory.md").unwrap();

    let (first_anchor, _) =
        selected_quote_to_relation(&mut conn, document.clone(), "第一处选区", target.clone())
            .unwrap();
    let (second_anchor, _) =
        selected_quote_to_relation(&mut conn, document, "第二处选区", target.clone()).unwrap();

    assert_ne!(first_anchor.id, second_anchor.id);
    assert_eq!(
        neighbors(&conn, ObjectUri::anchor(first_anchor.id).as_str(), None, 10)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        neighbors(
            &conn,
            ObjectUri::anchor(second_anchor.id).as_str(),
            None,
            10,
        )
        .unwrap()
        .len(),
        1
    );
    // P1 暂不为 anchor 自身追加事件；两次 Relation 各留下一个 created 事件。
    assert_eq!(list_events(&conn, 10).unwrap().len(), 2);
}

#[test]
fn failed_composite_command_rolls_back_every_primitive() {
    let mut conn = open_state_db(&tmp_dir("rollback").join("state.db")).unwrap();

    let document = ObjectUri::workspace("笔记.md").unwrap();
    let existing_target = ObjectUri::workspace("已有.md").unwrap();
    create_relation(
        &mut conn,
        NewRelation {
            source_uri: document.clone(),
            predicate: "related_to".into(),
            target_uri: existing_target.clone(),
            anchor_id: None,
            created_by: None,
            confidence: None,
            workspace_id: None,
            snapshot: None,
        },
    )
    .unwrap();
    let events_before = list_events(&conn, 50).unwrap().len();

    // 复合命令中途撞上唯一约束：此前写入的 anchor 和事件都必须一起回滚
    let outcome = tx_command(&mut conn, |tx| {
        create_anchor_in_tx(
            tx,
            NewAnchor {
                document_uri: document.clone(),
                kind: "字句".into(),
                start_offset: 0,
                end_offset: 5,
                quote: "注定要回滚的选区".into(),
                prefix: None,
                suffix: None,
            },
        )?;
        create_relation_in_tx(
            tx,
            NewRelation {
                source_uri: document.clone(),
                predicate: "related_to".into(),
                target_uri: existing_target.clone(), // 与开头已存在的三元组重复 → 必败
                anchor_id: None,
                created_by: None,
                confidence: None,
                workspace_id: None,
                snapshot: None,
            },
        )?;
        Ok(())
    });
    assert!(
        outcome.unwrap_err().contains("相同的关联已存在"),
        "应报唯一约束错误"
    );

    let anchors_left: i64 = conn
        .query_row("SELECT COUNT(*) FROM anchors", [], |row| row.get(0))
        .unwrap();
    assert_eq!(anchors_left, 0, "回滚后不允许有 anchor 残留");
    assert_eq!(
        neighbors(&conn, document.as_str(), None, 10).unwrap().len(),
        1
    );
    assert_eq!(list_events(&conn, 50).unwrap().len(), events_before);
}

#[test]
fn context_and_relation_survive_reopen_like_durable_facts() {
    let dir = tmp_dir("durable");
    let db_path = dir.join("state.db");
    let mut conn = open_state_db(&db_path).unwrap();

    let set = create_context_set(
        &mut conn,
        NewContextSet {
            purpose: Some("本会话引用篮（P1 仅数据能力）".into()),
            ..NewContextSet::default()
        },
    )
    .unwrap();
    attach_context_item(
        &mut conn,
        set.id,
        ObjectUri::library("rss", "daily/brief.md").unwrap(),
        None,
        None,
    )
    .unwrap();

    // 重开数据库：派生索引可以随时重建，durable 数据必须原样还在
    drop(conn);
    let conn = open_state_db(&db_path).unwrap();
    assert_eq!(list_events(&conn, 10).unwrap().len(), 1);
    assert_eq!(list_context_items(&conn, set.id).unwrap().len(), 1);
}

/// P2a legacy 迁移契约：固定项从 localStorage 导入后，重启两次必须零重复——
/// 关系不重复，relation.created 事件也不重复（幂等导入连事件一起跳过）。
#[test]
fn legacy_pin_import_is_idempotent_across_restarts() {
    use stillwrite_lib::state_store::{
        import_relation_links, list_relation_snapshots, RelationLinkImport,
    };

    let dir = tmp_dir("pin-import");
    let db_path = dir.join("state.db");
    let scope = ObjectUri::parse("ws://7f3a9c1e2b4d5a6f").unwrap();
    let legacy_items = vec![
        (
            "library://rss/daily/brief.md",
            serde_json::json!({"key": "library:rss/daily/brief.md", "kind": "library", "title": "日报"}),
        ),
        (
            "workspace://笔记/常驻.md",
            serde_json::json!({"key": "workspace:笔记/常驻.md", "kind": "workspace", "title": "常驻参考"}),
        ),
        // 非法 URI 必须让整批失败（单事务），而不是悄悄丢数据
    ];

    let run_import = || -> Result<usize, String> {
        let mut conn = open_state_db(&db_path)?;
        let links = legacy_items
            .iter()
            .map(|(uri, snapshot)| {
                Ok(RelationLinkImport {
                    target_uri: ObjectUri::parse(uri)?,
                    created_by: Some("human".into()),
                    workspace_id: Some(scope.subject().to_string()),
                    snapshot: Some(snapshot.clone()),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        import_relation_links(&mut conn, scope.clone(), "related_to", links)
    };

    assert_eq!(run_import().unwrap(), 2, "首次导入全部新增");
    drop(open_state_db(&db_path).unwrap());

    // 模拟第二次启动再次触发迁移
    assert_eq!(run_import().unwrap(), 0, "重启后重复导入必须是 no-op");

    let conn = open_state_db(&db_path).unwrap();
    let views = list_relation_snapshots(&conn, scope.as_str(), "related_to").unwrap();
    assert_eq!(views.len(), 2);
    assert_eq!(
        views[0].snapshot.as_ref().unwrap()["title"],
        serde_json::Value::String("日报".into())
    );
    // 两条新增关系恰好各带一个 relation.created：没有第三个事件 = 没有重复导入痕迹
    let events = list_events(&conn, 20).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.action == event_action::RELATION_CREATED)
            .count(),
        2
    );
}
