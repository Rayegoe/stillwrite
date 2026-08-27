# StillWrite Agent Development Contract

本文件是 StillWrite 仓库级开发契约，面向 Codex、Pi、Claude Code 及其他会读取、修改、测试或演化 StillWrite 的 Coding Agent。

## 1. Product Identity

StillWrite **不是**“Markdown 编辑器 + AI 功能集合”。

StillWrite 是一个 local-first、agent-native 的 Human Workbench：

- 统一人类活动、资料、Agent、日志、Git、决策与成果；
- 将原始高带宽信息压缩成可查询的状态、关系、记忆和工作对象；
- 降低 Human-Agent 与 Agent-Agent 之间的上下文恢复和协调成本；
- 保持人对源文档、最终判断和外部效果的控制。

`Software is Agent` 不意味着为每个能力增加聊天框、按钮、页面或插件。

## 2. Non-Negotiable Rules

### 2.1 UI MUST NOT own durable product state

`localStorage` 只允许保存：

- 宽度
- 布局
- 主题
- 最近视图模式
- 纯展示偏好

凡是需要：

- 跨会话保留
- 被查询
- 被关联
- 被 Agent 消费
- 被 Memory 使用
- 被 Work 协调

的状态，必须进入 domain model + durable storage。

### 2.2 Human content remains portable

Workspace 文档继续以 Markdown / 文件为准。

Agent 面向人的产物优先使用可编辑、可携带的 Artifact（通常是 Markdown）。

不得为了开发方便把普通文档正文整体迁入 SQLite。

### 2.3 SQLite owns structured durable state

vNext 目标中，以下状态应逐步进入 SQLite：

- semantic events
- anchors
- annotation metadata
- relations
- contexts
- agent threads / turns / runs
- memories
- work coordination state
- sources

FTS / trigram 等索引属于 derived/rebuildable data，必须和 durable state 区分。

### 2.4 Git owns software evolution history

Workspace Git 与 Product Git 是不同边界。

StillWrite 自我修改：

- 应在隔离 worktree / branch 中完成；
- 应有 diff；
- 应运行测试；
- 应产生 commit/evidence；
- merge / push / release 属于 adoption/external-effect boundary。

### 2.5 Integrations are adapters, not product modules

RSS、GitHub、audio、transcript、logs、email、external agents 等：

```text
Adapter
→ Canonical Data
→ Agent / Memory / Work
→ Projection
```

不要因为接入新来源就增加永久顶层模块。

### 2.6 Capability appears from context

界面能力由当前对象决定：

- Markdown → write/read/annotate/ask/relate
- Audio → playback + ordinary document surface
- Agent Work → work/evidence projection
- Run → status/evidence
- Dataset → table projection

不要为能力本身增加全局导航入口。

### 2.7 Preserve proven interactions

当前选中文字：

```text
＋批注 | 问 Agent | 联网搜索 | ＋关联
```

是核心 contextual interaction；既有动作语义保持不变，联网搜索作为同一浮层的外部网页检索入口，密钥只由后端管理，优先读取设置页保存值，环境变量作为回退。

右上重复的全局 `问 Agent` 不代表独立能力，后续 UI 清理时删除。

## 3. Canonical Primitives

任何新需求在设计 UI 前，先映射到：

### Entity
具有身份的对象。

例：

- document
- source
- person
- agent
- run
- work
- commit
- term
- audio

### Event
有语义的动作或状态转换。

### Relation
对象之间的 typed edge。

### Artifact
可持久结果：

- Markdown
- report
- code diff
- commit
- transcript
- dataset

### Anchor
对正文选区/范围的稳定引用。

### Annotation
附着在 Anchor / Document 上的批注。

### Context
当前人或 Agent 明确使用的工作集。

### Memory
由证据压缩出来、未来仍有价值的长期知识。

### Work
目标、状态、Agent、Artifact、Evidence、Decision、Risk、Next Action 的协调对象。

### Thread
持续的人-Agent 意图与历史。

### Agent
通过 bounded capabilities 进行 reasoning/execution 的 actor。

**如果已有 primitive 能表达需求，禁止再增加新的 durable state system 或顶层 UI module。**

## 4. Event Model

采用：

> **Relational State + Append-only Semantic Events**

不采用纯 Event Sourcing。

重要 command 应尽可能在同一 backend transaction 中：

1. 更新当前 relational state；
2. append semantic event。

例如创建批注：

```text
BEGIN
create/update anchor
insert annotation
insert relation
append annotation.created
COMMIT
```

不要：

```text
frontend state
→ localStorage
→ Markdown
→ later scan into DB
```

### Semantic Events

推荐：

- `document.opened`
- `document.saved`
- `annotation.created`
- `annotation.updated`
- `annotation.deleted`
- `relation.created`
- `relation.removed`
- `context.attached`
- `context.detached`
- `source.added`
- `source.refreshed`
- `agent.requested`
- `agent.run.started`
- `agent.run.completed`
- `agent.work.created`
- `agent.work.edited`
- `work.blocked`
- `work.completed`
- `decision.made`
- `code.change.requested`
- `code.change.committed`

不要把每个 keypress / mousemove / scroll pixel 记录成 semantic history。

## 5. Memory Rules

Event Log ≠ Memory。

Memory 是：

> 对未来决策仍有价值的、可查询的、带来源证据的压缩状态。

初期 Memory kind 控制在：

- `fact`
- `decision`
- `rule`
- `hypothesis`
- `todo`
- `workflow_pattern`

推断 Memory 应保留：

- source events
- source URI / Work
- confidence（适用时）
- created/updated time

不得静默发明长期用户偏好。

## 6. Work Is a Primitive

WO 不是一个必须独立做 UI 的模块。

可将其抽象为：

```text
Work
├─ intent
├─ owner / agents
├─ state
├─ artifacts
├─ evidence
├─ decisions
├─ risks
└─ next_actions
```

Coding Task、Research、Writing、Daily Brief、Learning Session、Bug Investigation 都可以是 Work 的不同 projection。

## 7. Agent Capability Boundary

Runtime Agent 应使用 typed bounded tools，不开放：

- raw SQL
- unrestricted shell
- arbitrary filesystem write

优先提供：

```text
workspace_search/read
library_search/read
anchor_read
annotation_query
context_get/attach
relation_neighbors/put
memory_search/put
work_query/update
thread_query/update
agent_work_read/write
```

### Product Code Capability

修改 StillWrite 自身源码属于独立 capability family：

```text
product_repo_status
product_repo_search
product_repo_read
product_change_begin
product_change_write
product_change_diff
product_change_test
product_change_commit
product_change_discard
```

merge / push / release 是额外 adoption boundary。

## 8. Human Authority

Agent 内部工作在用户明确提出意图后，可以连续执行，不需要把每个中间步骤都变成审批。

但以下动作必须有明确边界：

- destructive change
- send/publish
- push
- release
- replace active product version
- other external effects

## 9. UI Rules

UI 是 projection layer。

### Top toolbar

只放：

- application state
- document/view state

不是 feature shelf。

### Selection popup

保留 contextual actions：

```text
批注 | 问 Agent | 联网搜索 | ＋关联
```

### Left navigation

负责：

- object navigation
- source navigation
- work/thread navigation

### Right support surface

负责：

- annotations
- relations
- evidence
- contextual projections

### Library

必须可浏览，不能强迫用户先搜索。

优先模型：

```text
最近
推荐
来源
```

来源使用 main-surface flat list，不展开巨大目录树。

### RSS

RSS 是 Source/ingestion capability。

Daily Brief、重点阅读、作者追踪、阅读建议由 Agent 生成，不做 RSS Reader 产品 UI。

## 10. Development Workflow

开始代码前：

1. 读 `README.md`
2. 读 `docs/PRINCIPLES.md`
3. 读 `docs/ARCHITECTURE.md`
4. 读 `docs/DATA_MODEL.md`
5. 读 `docs/ROADMAP.md`
6. 明确涉及哪些 canonical primitives
7. 明确每个新状态的 Source of Truth
8. 明确测试与 evidence

实现时：

- 只做当前 roadmap phase 的最小闭环；
- 不顺手加入 embedding、graph DB、plugin framework、大前端框架；
- 没有真实 failure，不新增架构；
- 先迁移 durable state，再缩减 UI state；
- 保留当前已证明有效的交互习惯。

## 11. Definition of Done

一个改动完成至少应满足：

- durable state 有唯一 Source of Truth；
- UI 不成为第二数据库；
- 需要的 event/relation 可查询；
- Agent capability boundary 明确；
- migration/regression 有测试；
- evidence 可以被 Work/Agent/人读取；
- 架构边界变化同步更新文档。
