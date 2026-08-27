# StillWrite vNext Data Model

## 1. Source of Truth Matrix

| Data | Source of Truth |
|---|---|
| Workspace document body | Markdown/file |
| Library source body | original file / materialized artifact |
| Agent human-facing artifact | Markdown / explicit artifact |
| Events | SQLite durable |
| Anchors | SQLite durable |
| Annotation metadata | SQLite durable |
| Relations | SQLite durable |
| Context | SQLite durable |
| Web search history / result snapshots | SQLite durable |
| Threads / turns | SQLite durable |
| Work | SQLite durable |
| Memory | SQLite durable |
| Source registrations | SQLite durable |
| FTS / trigram | SQLite derived/rebuildable |
| layout/theme/width | localStorage presentation only |
| Pi raw session/trace | Pi/runtime evidence |
| Product evolution | Git |

## 2. Initial Durable Tables

### entities

Optional registry for objects needing uniform metadata.

```text
id
uri UNIQUE
kind
title
metadata_json
created_at
updated_at
```

### events

```text
id
workspace_id
actor_type
actor_id
action
object_uri
target_uri
thread_id
work_id
payload_json
created_at
```

Append-only by default.

### anchors

```text
id
document_uri
document_hash
kind
start_offset
end_offset
quote
prefix
suffix
created_at
updated_at
```

Current annotation selection logic already provides useful basis for range/quote recovery.

### annotations

```text
id
anchor_id
document_uri
body_md
created_by
created_at
updated_at
status
```

Existing Markdown annotations migrate into this table.

Markdown sidecars remain portable projection/export.

### relations

```text
id
source_uri
predicate
target_uri
anchor_id
evidence_event_id
created_by
confidence
created_at
updated_at
```

### context_sets

```text
id
workspace_id
thread_id
work_id
purpose
created_at
updated_at
```

### context_items

```text
id
context_id
object_uri
anchor_id
position
added_by
created_at
```

Current citation basket is migrated here.

### web_searches / web_search_results

每次 Brave 互联网搜索都是一条可重读的 `web_searches` 历史记录；返回的标题、URL、摘要和时间信息以快照写入 `web_search_results`，不依赖再次请求互联网。网页结果可通过通用 `relations` 以 `search-result://<id>` 作为 target 关联到当前笔记。

### agent_threads

```text
id
workspace_id
title
pi_session_ref
status
created_at
updated_at
```

### agent_turns

```text
id
thread_id
role
instruction
artifact_uri
created_at
```

UI history should prioritize latest human instruction, not origin quote.

### agent_runs

```text
id
thread_id
turn_id
runtime
model
status
started_at
completed_at
receipt_ref
error
```

Raw Pi receipts can remain evidence; DB stores product projection.

### works

```text
id
workspace_id
kind
intent
status
owner
summary
created_at
updated_at
```

### work_relations

Can initially use generic `relations` rather than a separate table unless query patterns require specialization.

### memories

```text
id
workspace_id
thread_id
work_id
kind
content
source_uri
source_event_ids_json
confidence
created_at
updated_at
```

Initial kinds:

```text
fact
decision
rule
hypothesis
todo
workflow_pattern
```

### sources

Long-term source abstraction:

```text
id
kind
name
locator
metadata_json
created_at
updated_at
```

Existing library_sources / feed state may migrate incrementally rather than through a big-bang rewrite.

## 3. Derived Tables

Examples:

- workspace FTS
- library FTS
- related trigram indexes
- recommendation cache
- computed work summaries

Derived data must be rebuildable from durable state/artifacts.

## 3.1 P1 实施现状（state.db 已落地）

vNext P1 已建立独立于派生索引的 durable 数据库，位置为 `<AppData>/state.db`（`src-tauri/src/state_store.rs`）：

- 物理边界：durable（state.db：schema_migrations / events / anchors / relations / context_sets / context_items / web_searches / web_search_results）与 derived（indexer/library 的 index.db）分文件存放。派生索引可整库删除重建，durable 数据不允许；
- migration runner 按版本递增应用 `MIGRATIONS`，每版本一个事务；重启即 no-op；
- events 通过 SQLite trigger 物理禁止 UPDATE/DELETE（append-only），写入只发生在命令事务内；
- command transaction pattern：所有状态变更经 `tx_command` 在同一事务内完成 mutate + append event；复合命令直接组合 `*_in_tx` 级别函数（见 `tests/state_flow.rs`）；
- P1 事件仅四个动作：`relation.created`、`relation.removed`、`context.attached`、`context.detached`；anchor 自身的事件语义留给批注迁移 slice；
- **Relation URI 粒度原则**：选区级 Relation 的 `source_uri` 直接使用 `anchor://<id>`；文档级 Relation 才使用 `workspace://...` / `library://...`。`relations.anchor_id` 只是回溯辅助，不参与 `(source_uri, predicate, target_uri)` 唯一性判定；
- `relation.removed` 事件 payload 携带完整快照（predicate / source_uri / target_uri / removed_evidence_event_id）——relation 行删除后不可查询，撤销审计只能依赖事件本身；
- URI 经 `ObjectUri` 统一校验后入库（`workspace://` `library://` `anchor://` 等 validated wrapper，不做封闭 enum）；
- **P2a 固定关联已迁移**（第一个 vertical slice）：用户的 ☆固定/取消固定 不再写入 `localStorage['stillwrite.relatedPinned.v1']`，改经 `pin_related / unpin_related / list_related_pins / import_related_pins` 命令落入 relations + events。作用域与旧 UI 一致为工作区级共享，建模为 `ws://<workspace-key>` 根对象（key 复用 index 目录的 short_hash）；卡片展示快照存在 `relation.created` 事件 payload 的 `snapshot` 字段里；legacy 值按工作区前缀分批幂等导入、校验投影后才移除（重复启动不产生重复关系或事件）；
- **P2c 网页搜索已持久化**：`search_web` 在 Brave 返回后把搜索历史和结果快照原子写入 `web_searches / web_search_results`，并追加 `web_search.completed` 事件；右侧“搜索”视图按历史条目展示，点击后展开结果，结果可打开网页或通过 `relations` 关联当前笔记。
- 尚未迁移：annotations metadata、agent threads/turns/runs、works、memories、sources——各自对应的 vertical slice 迁移时再接入同一套事务模式。

## 4. Transaction Rule

Important product command:

```text
command
→ state mutation
→ relation mutation
→ append semantic event
→ COMMIT
```

UI receives projection/query result after commit.

## 5. Migration Rules

1. No big-bang conversion.
2. Preserve existing Markdown content.
3. Import existing annotation sidecars.
4. Move current durable `localStorage`/JS Map state only after DB representation exists.
5. Keep compatibility readers until migrated data is verified.
6. Add migration tests before deleting legacy paths.
7. Do not move presentation-only state into DB.

## 6. Event vs Memory

### Event

```text
2026-08-27 10:35
annotation.created
object=annotation://17
target=workspace://design.md
```

### Memory

```text
decision
StillWrite 保留选区“问 Agent”，删除右上重复入口。
evidence=[event://..., thread://...]
```

Memory Compiler consumes evidence; it must not turn every event into memory.

## 7. Implicit Outcome / Reward

Future derived evaluation can use:

- Agent output retained percentage
- human edit distance
- deleted content
- reused artifact
- follow-up correction
- relation/reference reuse
- time-to-action

This is derived evaluation, not a new UI module.
