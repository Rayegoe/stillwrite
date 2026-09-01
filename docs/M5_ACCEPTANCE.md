# M5 — Durable Agent Thread & Cognitive Lineage 验收矩阵

> 状态：自动化部分已落地（2026-09-01）。M5.1–M5.5 一个里程碑一个 commit。
> L1/L2 由 `cargo test` + `node --test` 覆盖；L3 真人 gate 需带真实 Pi 跑一遍。
> 数据规则沿用 03 Rule 8：不允许 mock 会话数据通过业务验收。

## 核心语义（本里程碑冻结）

```text
AgentSession = 连续性（我们之前聊到哪里）
agent_sessions:    id, workspace_id, title, provider, provider_session_ref,
                   created_at, updated_at                    (schema v6)
agent_messages:    id, session_id, role(user|assistant), content,
                   run_ref, origin_uri, quote_snapshot,
                   context_set_id, created_at                (schema v6)
```

- 复用既有原语，不新造 link 表：turn 上下文 → `context_sets(thread_id=session)`
  + `context_items`；结果动作 → `relations(inserted_into/derived_into/
  promoted_to)`；一切变更 → append-only events（`agent_session.created`、
  `agent_message.appended`、`relation.created`）。
- Session title = 首条提问截断（`AgentRequest.displayTitle`），不再调模型。
- 回答由 backend 在 `agent_settled` 事件**先落库后广播**（幂等：assistant
  run_ref 唯一索引吸收重放）；重开应用 thread 完整可查。
- 继续问 = same Session + new Run；runtime input 携带最近 12 条历史窗口
  （每条截断 1600 字符），`instruction` 始终独立成段。
- `Work.intent` 不变仍是原始 instruction；thread 里的委派 turn 由 backend
  记录 `agentmsg:// → work://` 的 `promoted_to` relation。

## L1. Rust 单测（已覆盖）

| # | 断言 | 测试 |
| --- | --- | --- |
| R1 | session 创建落事件；重开数据库不丢 | `create_session_emits_event_and_survives_reopen` |
| R2 | 空 title / 不存在的 session 拒绝 | `create_rejects_blank_title_and_missing_session` |
| R3 | user/assistant 按 runRef 配对；重复 settle 被 run 唯一索引幂等吸收 | `thread_roundtrip_preserves_order_and_run_binding` |
| R4 | turn 建立 context set，thread_id 指回 session，item 为来源 URI | `turn_context_set_is_linked_via_thread_id` |
| R5 | 列表按 workspace 隔离；related 反查 origin_uri 命中 | `list_and_related_cover_workspace_scoping` |
| R6 | promoted_to 落 relation + 事件；重复三元组被唯一索引拒绝；白名单外 predicate 拒绝 | `outcome_relation_records_lineage_and_rejects_repeat` |
| R7 | 历史窗口有界且有序 | `recent_history_returns_bounded_window_in_order` |
| R8 | runtime message：历史窗口投影为「用户/Agent」节，instruction 原文仍独立收尾；超长截断 | `runtime_message_projects_history_window_without_replacing_instruction` |

## L2. Frontend 纯函数（node --test，ui/agent-thread.test.js）

| # | 断言 | 测试点 |
| --- | --- | --- |
| F1 | durable messages + live run 合成 turn；流式/失败/缺失三态 | `buildTurns` |
| F2 | 会话三分：当前/相关/最近；换文档只重排不删除 | `partitionSessions` |
| F3 | 会话行摘要 | `sessionRowMeta` |

## L3. 真人 GUI gate（待执行）

Streaming Integrity（M5.1）：

- [ ] Pi 真流式：chunk 到达即渐进显示（markdown 实时渲染），无打字机动画、
      无固定间隔投递；settled 只定稿（权威全文 + 开启动作按钮），不清空重播。
- [ ] 无流式返回时：显示「生成中…」，结果一次性完整出现。

Durable Thread（M5.2/M5.4）：

- [ ] 首次问 Agent 自动建立会话（标题=首问截断）；右栏出现
      当前会话（thread）/ 相关会话 / 最近会话三层。
- [ ] 继续问（thread composer，Enter 发送）→ same Session 新 Run，
      Agent 记得上文。
- [ ] 切换文档：会话不消失；相关会话排前；切回来依然在。
- [ ] 正在生成的会话切到别的文档 → 会话行显示「正在生成…」，完成后不丢。
- [ ] 点击相关/最近会话行 → 展开完整 thread。
- [ ] ＋ 新会话 → 下一条提问开启新线程。
- [ ] 关闭重开 StillWrite → thread 完整；上次会话自动展开（按 workspace 记忆）。

Context Lineage（M5.3）：

- [ ] 每轮 turn 保存 quote snapshot：文档改写后，旧 thread 仍显示
      「当时针对的原文」。
- [ ] state.db 检查：每 turn 有 context_set（thread_id=会话，items 含
      当前文档/引用资料 URI）；不含 giant compiled prompt。

Outcome Relations（M5.5）：

- [ ] 插入正文 → relation `agentmsg://… -inserted_into-> workspace://…`
- [ ] 保存为笔记 → `-derived_into->` 新文档
- [ ] thread 里委派 → backend 记录 `-promoted_to-> work://…`，
      Work 照常走 queued→running→needs_human。
- [ ] state.db `relations`/`events` 可查到以上链路；Annotation schema 未动。

## 回归（03 Rules 7）

- [ ] Rust tests（161+）；frontend JS tests（97+）；`node --check`；`git diff --check`
- [ ] M4 全部 gate 不回退（assist 不落 Work、启动恢复、委派计数）
- [ ] Workspace open/save/autosave；写/双/读；批注；relations/pins；
      Brave search；Library；RSS；Pi start/abort/result；Agent Work open/edit；
      Work list/detail/evidence
