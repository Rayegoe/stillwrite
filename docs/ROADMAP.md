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
- [ ] related pinned items → relations
- [ ] citation basket → context sets
- [ ] Agent Work metadata → thread/turn/work
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

降低监督多 Agent 的人类带宽。

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

文档落地后，第一张真正的工程 Work 应只做：

> **P1: SQLite Core Durable State Foundation**

验收：

1. schema version + migrations；
2. `events / anchors / relations / context` 最小表先落地；
3. durable/derived 数据分层；
4. command transaction helper；
5. tests；
6. 不改变现有选区问 Agent / 批注 / Library 使用方式。

完成后再进入 P2，而不是顺手开发 Daily Brief、英语、RSS 推荐或 Self-Evolution。
