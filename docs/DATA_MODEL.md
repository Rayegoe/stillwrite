# StillWrite vNext Data Model

## 1. Source of Truth Matrix

| Data | Source of Truth |
| --- | --- |
| Workspace document body | Markdown/file |
| Library source body | original file / materialized artifact |
| Agent human-facing artifact | Markdown / explicit artifact |
| Events | SQLite durable |
| Anchors | SQLite durable |
| Annotation metadata | SQLite durable |
| Relations | SQLite durable |
| Context | SQLite durable |
| Web search history / result snapshots | SQLite durable |
| Threads / turns（未来 milestone） | SQLite durable |
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

每次 Brave 互联网搜索都是一条可重读的 `web_searches` 历史记录；返回的标题、URL、摘要和时间信息以快照写入 `web_search_results`，不依赖再次请求互联网。它在 Document Context 中投影为 `搜索`，与 `批注`、`关联` 并列。网页结果可通过通用 `relations` 以 `search-result://<id>` 作为 target 关联到当前笔记。

### Future Candidate：Thread / Turn / Run

以下 `agent_threads` / `agent_turns` / `agent_runs` 字段设计是**未来草图**，不是当前 durable schema：

- **不属于当前 durable schema**：M1 的 durable schema 只有 `works`（bridge schema），state.db 不创建这三张表；
- **不属于 M1**：它们不出现在 M1 的 migration 边界里；
- **不得因 DATA_MODEL 中存在草图而提前实现**：草图不构成实现任务，不进入任何未到期 milestone；
- 只有真实的多轮 Work 使用证明需要后，才进入未来 milestone（候选：ROADMAP P6 Thread Continuity）。

字段设计本轮不调整，仅作未来方向参考。

#### agent_threads（Future Candidate: Thread）

```text
id
workspace_id
title
pi_session_ref
status
created_at
updated_at
```

#### agent_turns（Future Candidate: Turn）

```text
id
thread_id
role
instruction
artifact_uri
created_at
```

UI history should prioritize latest human instruction, not origin quote.

#### agent_runs（Future Candidate: Run）

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

Run status 只描述一次执行：

```text
queued | running | succeeded | failed | cancelled
```

不要把 `needs_human` 或 `blocked` 写入 Run；它们属于 Work 协调状态。

这三张表见上方 **Future Candidate：Thread / Turn / Run**（当前阶段不落地）。Pi receipt/session 属于 runtime evidence，DB 只保存 `receipt_ref` 引用，receipt 本体由 Pi runtime 留存；Pi 返回最终文本不等于 Work `completed`。

### works

M1 已落地的 bridge schema（state.db migration v4）。每条新的 Pi 请求对应一行 Work：

```text
id
workspace_id
title
intent
status
summary
next_action
artifact_uri
receipt_ref
created_at
updated_at
```

`kind`、`owner` 只有在查询确实需要时再作为可选元数据加入；它们不能替代人类可读的 `title/intent`。Work status 只描述目标协调结果：

```text
queued | running | needs_human | blocked | completed | failed | cancelled
```

Run `succeeded` 不自动等于 Work `completed`。只有验收条件满足时，Work 才能完成；可恢复的 Run failure 通常使 Work `blocked`，不可恢复 failure 才使 Work `failed`。

Pi 桥接映射（`receipt_ref` 保存 run id，收据本体留在 `<AppData>/agent/runs/`）：请求创建=`queued`；Pi accepted=`running`；Agent Work Artifact 保存=`needs_human`（`artifact_uri` 记录 canonical `agentwork://` URI）；abort=`cancelled`；依赖缺失（缺 Pi/模型）=`blocked`；致命错误=`failed`；人明确接受=`completed`。Pi 返回最终文本（settled）不是 Work 状态变化。

Work 语义事件只有三个动作：`work.created` / `work.status_changed` / `work.updated`，与状态变更同事务落地（`work://<work-id>` 为 object URI）。

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

## 2.1 URI 与 legacy Agent Work bridge

| 对象 | canonical URI | legacy / 兼容规则 |
| --- | --- | --- |
| Workspace Markdown | `workspace://<relative-path>` | 不转换正文 |
| legacy Agent Work URI | `agent://<workspace-key>/<id>` | 匹配 sidecar 时映射到 `agentwork://...` |
| `agent://`（其余形式） | 待定义 | 暂时 legacy/reserved，本阶段不重定义为 Agent Actor |
| Agent Work Markdown Artifact | `agentwork://<workspace-key>/<id>` | 匹配 sidecar 的 `agent://<workspace-key>/<id>` 映射到这里 |
| Work | `work://<work-id>` | 新模型对象 |
| Run | `run://<run-id>` | 新模型对象 |
| Brave result snapshot | `search-result://<result-id>` | 结果快照可重读 |
| Existing pinned scope | `ws://<workspace-key>` | 只表示固定关联 scope，不是 Work |

兼容 resolver 必须先按 Workspace + legacy Agent Work sidecar 校验双段 `agent://`，再决定是否把它解释为 Agent Work；不能全局替换前缀，本阶段不把 `agent://` 重定义为 Agent Actor。canonical URI 用于新 Relation/Event/Projection，原始 legacy URI 作为 provenance 保留。

### Legacy bridge mapping

旧 Agent Work 的 Markdown 正文是 Artifact，sidecar 是迁移输入，不是新的事实源：

```text
legacy Agent Work sidecar + Markdown
  ├─ title / prompt       → Work title / intent
  ├─ run_id / session_ref → existing Run；缺失时可建 legacy completed Run
  ├─ Markdown body        → agentwork://... Artifact
  └─ origin_uri / quote   → provenance / Context reference
```

新请求的顺序是 `create Work → create Run → persist Artifact → attach relations`。至少建立：

```text
run://<run>  belongs_to  work://<work>
work://<work> produces    agentwork://<artifact>
run://<run>  produces     agentwork://<artifact>
```

桥接以 `(workspace_id, legacy_agent_work_id)` 为幂等键；重复扫描不得复制对象或 semantic events。不得删除、覆盖旧 Markdown/JSON；只有 canonical projection 验证成功后才可隐藏旧入口。

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
