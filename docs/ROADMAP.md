# StillWrite vNext Roadmap

目标：从当前“多个原型能力 + 前端状态”迁移到统一的 Agent-native platform。

原则：

- Contract first
- Data before UI
- Migration before deletion
- No big-bang rewrite
- Preserve successful interactions
- No new module unless primitives fail

---

## Workbench compatibility gate（A0，文档契约）

在 Work 重构前，以下四个兼容接缝必须先固定；本阶段不修改 Rust/JS/CSS 产品代码：

- [x] URI legacy/canonical resolver：`agent://` 暂时保持 legacy/reserved（本阶段不重定义为 Agent Actor），匹配旧 Agent Work sidecar 的双段 URI 映射到 `agentwork://` Artifact；保留原始 URI；`ws://` 仅为既有固定关联 scope；
- [x] Document Context 明确为 `批注 | 关联 | 搜索`，搜索历史/结果快照继续来自 `state.db`；
- [x] 明确 Run status 与 Work status 分离，Run `succeeded` 不自动等于 Work `completed`；
- [x] 定义 legacy Agent Work → Work/Run/Artifact 的幂等 bridge，不重写或删除 Markdown/sidecar。

本阶段（PREP/A0/A1）同时冻结：Work 是可选协调对象（不是根领域对象）；Pi receipt/session 是 runtime evidence（durable state 只保存 `receipt_ref` 引用）；Pi 返回最终文本不等于 Work `completed`（`completed` 只来自人的明确接受）；暂不要求 durable Thread/Run。

### Reframed execution order

```text
A0 compatibility contract
→ A1 minimal Work primitive
→ A2 Pi → Work bridge
→ A3 Work-first Shell
→ A4 Work Detail + Context/Evidence
→ A5 Needs Human
```

原 P9 的 Work/WO 协调目标已前移到 A1–A5；P9 只保留后续多 Agent、WO protocol 和证据压缩的扩展工作。

---

## P0 — Foundation Contract

### Goal

先让所有后续 Coding Agent 对产品方向达成一致。

### Deliverables

- [x] vNext `README.md`
- [x] `AGENTS.md`
- [x] `docs/PRINCIPLES.md`
- [x] `docs/ARCHITECTURE.md`
- [x] `docs/DATA_MODEL.md`
- [x] `docs/UX_MODEL.md`
- [x] `docs/ROADMAP.md`

### No product behavior change

这一阶段只定宪，不顺手重构代码。

---

## P1 — Core Durable State

### Goal

建立 SQLite durable state 与 migration 基础。

### First schema

- [x] events（append-only 由 trigger 强制；P1 事件面仅 relation/context 四动作）
- [x] anchors
- [ ] annotations
- [x] relations
- [x] context_sets
- [x] context_items
- [ ] agent_threads
- [ ] agent_turns
- [ ] agent_runs
- [ ] works
- [ ] memories

### Requirements

- [x] schema version / migration mechanism（`state_store::migrate`，每版本一事务）
- [x] durable vs derived tables clearly separated（`<AppData>/state.db` vs 各 index.db）
- [x] transaction helper（`tx_command` + `*_in_tx` 两层）
- [x] query/command boundary（查询走 `&Connection`，写命令只经 `*_in_tx`/`tx_command`）
- [x] migration tests（fresh / 增量 / 重启 no-op；另有事件原子性、邻居、有序 context 契约测试）

### Do not

- no embedding
- no graph DB
- no React rewrite
- no plugin system

### Work slice boundary

`works`、Work semantic events 和 Work projection 属于 A1，不得在 A0 直接改 UI。A2 才把 Pi Run 与 Work/Artifact 连接起来；A3 之后才改变默认导航和启动面。

A1 + A2 已完成（M1，backend-only）：`works` bridge schema（migration v4）、`work://` URI、`work.created / work.status_changed / work.updated` 三个语义事件，以及 Pi 桥接（请求=queued、accepted=running、Artifact 保存=needs_human、abort=cancelled、依赖缺失=blocked、致命错误=failed、人工接受=completed）。Headless Gate：`create → running → attach artifact → needs_human → query → complete → restart` 由 `src-tauri/tests/work_flow.rs` 无 UI 覆盖。UI 不变，Work 视图（M2）必须使用这些真实数据。

---

## P2 — Eventize Existing Human Actions

### Goal

现有成功交互不变，但动作写入统一 backend transaction/event。

优先顺序：

1. [ ] annotation create/update/delete
2. [ ] selection/anchor used by annotation
3. [ ] relation create/remove
4. [ ] context/reference attach/detach
5. [ ] library source add/refresh
6. [ ] agent requested/run/work
7. [ ] meaningful document open/save events

### Evidence

每个动作可查询对应 state + semantic event。

---

## P3 — Migrate Current Durable UI State

### Goal

把“产品状态在 JS/localStorage/JSON sidecar”逐步移出 UI。

### Candidates

- [ ] annotation metadata → SQLite
- [ ] selection anchors → SQLite
- [x] related pinned items → relations（P2a 已完成：`ws://<workspace-key>` scope + 幂等 legacy 导入）
- [x] web search history/results → SQLite（P2c：Brave 快照、历史展开、网页结果关联当前笔记）
- [ ] citation basket → context sets
- [ ] Agent Work metadata → thread/turn/work（A2 先做兼容 bridge，完整 Thread/Turn 迁移留在 P6）
- [ ] Agent history → turns
- [ ] feed durable state → source model where appropriate

### Keep in localStorage

- layout
- sidebar width
- split ratio
- presentation preferences

---

## P4 — Library Becomes Browsable

### Goal

解决“资料必须先搜索才可读”，同时避免目录树爆炸。

### UX

- [ ] Library Home
- [ ] 最近
- [ ] 推荐（第一版可先不用模型）
- [ ] 来源
- [ ] Source flat list in main content surface
- [ ] source-scoped search

### Backend

- [ ] list recent documents across sources
- [ ] list documents by source with pagination
- [ ] source metadata projection
- [ ] recent/read/reference events

### RSS

停止扩展 RSS Reader UI。

保留 ingestion/source capability。

---

## P5 — Agent Context Becomes Data-Native

### Goal

停止由前端手工拼接完整 Agent prompt/context。

### Add typed tools

- [ ] context_get
- [ ] anchor_read
- [ ] annotation_query
- [ ] relation_neighbors
- [ ] memory_search
- [ ] library_read/search
- [ ] work_query

### Agent call

目标：

```text
agent_run(
  thread_id,
  context_set_id,
  anchor_id,
  instruction
)
```

### Preserve

选中文字 → `问 Agent` 的 UI 逻辑不变。

---

## P6 — Thread Continuity

### Goal

Agent 从一次 Work 变成持续工作。

### Deliverables

- [ ] thread list
- [ ] latest human instruction visible in sidebar
- [ ] turns
- [ ] Pi session linked to thread
- [ ] continuation/switch session
- [ ] Artifact/Work attached to turn/run

---

## P7 — Memory Compiler

### Goal

从 semantic events / Work / decisions 中形成可追溯 Memory。

### Initial memory kinds

- fact
- decision
- rule
- hypothesis
- todo
- workflow_pattern

### Rules

- [ ] provenance required
- [ ] confidence for inferred memory
- [ ] event ≠ memory
- [ ] retrieval by scope/FTS/relation/recency
- [ ] no embedding until failure demands it

---

## P8 — Agent Reading / RSS Intelligence

### Goal

证明“新能力 ≠ 新模块”。

### Outputs

- [ ] Daily Brief as Agent Work
- [ ] recommended reading
- [ ] author/source affinity
- [ ] key sections
- [ ] conflicts with current memory/decisions
- [ ] skip recommendations

No RSS-specific top-level UI.

---

## P9 — Work / WO Coordination

### Goal

降低监督多 Agent 的人类带宽。Work 的最小 primitive、Pi bridge 和 Work-first Shell 已前移到 A1–A5；本阶段只做跨 Agent/WO 的协调扩展。

### Work projection

- intent
- status
- artifacts
- evidence
- blockers
- decisions
- risks
- next actions

### Integrate

- Agent runs
- Git diff/commit
- tests
- logs
- existing WO concepts

Raw traces remain available as evidence, but default projection is compressed.

---

## P10 — Learning / Multimodal Proof

### Goal

证明同一架构可以承载英语精听等场景而无需新增业务模块。

### Minimal proof

- [ ] audio Artifact renderer/playback
- [ ] transcript relation
- [ ] dictation Markdown
- [ ] full-selection Ask Agent
- [ ] generic evaluation
- [ ] learning memory

No permanent “英语学习” icon.

---

## P11 — Product Self-Evolution

### Goal

让 StillWrite Agent 在明确用户意图下修改 StillWrite 本身。

### Product capability

- repo status/search/read
- begin isolated change
- write/edit
- run tests
- diff
- commit
- discard

### Worker

Pi 或 Codex 是内部 Coding Worker，不成为新的产品 UI。

### Adoption boundary

merge / push / release / replace active version 保留明确人类边界。

---

# Immediate Next Work

兼容契约落地后，第一张真正的工程 Work 应只做：

> **A1: SQLite-backed Minimal Work Primitive**

验收：

1. 在现有 migration runner 上增加 `works`；
2. 增加 `work://` 与 Work semantic events；
3. 保持 Relational State + Append-only Events；
4. 增加 fresh/restart/update/rollback/event atomicity tests；
5. 不修改 Workbench UI，不改变现有选区问 Agent / 批注 / 搜索 / Library 使用方式；
6. A1 完成后再进入 A2，不在同一阶段顺手做 Shell 改造。

**状态（2026-08，M1 交付）：** A1 + A2 已按上述边界完成——`works`（migration v4）、`work://` URI、三个 Work 语义事件、Pi→Work 状态桥接与无 UI lifecycle 测试全部落地；UI 未做任何修改，未建 Thread/Run/Artifact 表，历史 Agent Work 未迁移。下一步是 M2：在现有 Agent 区用真实 `works` 数据做 Work 视图（需要你 / 进行中 / 最近完成），不引入 mock 卡片。

完成 A 系列、证明 Work/Run/Artifact 能在一个窗口中被监督后，再进入 Thread、Memory、Library 推荐、RSS 智能或 Self-Evolution。
