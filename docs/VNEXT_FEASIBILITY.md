# StillWrite vNext Foundation 可行性分析

## 结论

可行，但应把这套 foundation 作为“演进契约 + 分阶段迁移路线”，而不是一次性重写方案。

当前最合适的落地方式是：

1. P0：将契约、架构、数据模型、UX 和路线图纳入 `stillwrite` 仓库；
2. P1：增加独立的 SQLite durable-state 数据库与 migration 基础；
3. P2–P3：按现有成功交互逐项 eventize、迁移 durable state；
4. 后续再扩展 Context、Thread、Memory、Work projection。

本次只完成 P0 文档纳入，不改变产品行为。这一边界是必要的：foundation 明确要求 Foundation / Contract 阶段“不顺手重构代码”，而当前实现尚未达到 P1 的统一持久化条件。

## 当前实现与 foundation 的接缝

代码知识图谱刷新后，当前仓库包含 28 个代码/配置文件、562 个函数、11 个 Rust 文件和 9 个 JavaScript 文件。现有模块已经形成若干清晰边界：`src-tauri` 负责文件、索引、资料、批注、Agent 和同步，`ui` 负责投影与交互，`resources/pi` 负责 Agent runtime 的提示词与工具。

| Foundation primitive / boundary | 当前实现 | 判断 |
| --- | --- | --- |
| Markdown / Artifact | Workspace 正文、批注侧车、Agent Work 正文都保留为 Markdown 或文件 | 高度兼容，应继续保持可携带 |
| SQLite derived index | `src-tauri/src/indexer.rs` 的 `files`、`fts`、`related_fts`；`library.rs` 另有 `library_sources`、`library_documents` 与 FTS | 已有 SQLite 基础，但这些数据库主要是可重建索引，不能直接冒充 durable state |
| Annotation / Anchor | `annotate.rs` 从选区生成结构化元数据，批注正文写入工作区 `批注/` 或应用数据目录 | 可作为 P2 迁移起点；需要先保留兼容读写，再把 metadata / anchor 写入 DB |
| Library / Source adapter | `library.rs` 已有 source 注册、增量扫描、内容 hash、只读读取；`feeds.rs` 将 RSS/Atom 物化为 Library Markdown | 与 Adapter → Canonical Data → Projection 方向一致，不应再造 RSS 专属核心模型 |
| Agent / Run / Work | `agent_work.rs` 以 `<AppData>/agent/works/<workspace-key>` 保存 Markdown + JSON 元数据；`pi_agent.rs` 以 `<AppData>/agent/runs/<run-id>.jsonl` 保存运行收据，并维护 Pi 进程/session | 已有天然迁移对象，但仍是 sidecar / receipt 投影，尚不是 `threads / turns / runs / works` 的统一 durable model |
| Context | `ui/app.js` 有当前引用篮 `citationBasket`，并由 `buildAgentPrompt` 手工拼接资料上下文；`loadWorksetContext` 负责读取工作集 | 是 P5 的明确接缝；先定义 context_set/context_items，再替换前端拼 prompt |
| Presentation state | `localStorage` 保存目录、布局、宽度、比例、视图模式、同步地址，以及关联卡片固定状态 | 布局等属于合法 presentation state；固定关联卡片和引用篮需按 foundation 重新分类，不能默认永久留在 UI |
| Product Git boundary | 当前 `stillwrite` 自身通过 Git 演进，已有测试与提交流程 | 可作为 P11 的 adoption boundary；仍需后续为 product change 能力定义隔离 worktree/branch，不应直接写 main |

关键证据位置：

- [`AppState` 与索引路径](../src-tauri/src/lib.rs) 当前只持有 workspace root、索引路径及运行态状态；`resolve_index_db` 将索引放在应用数据目录的 workspace hash 下。
- [`indexer::open_index`](../src-tauri/src/indexer.rs) 建立的是文件表和 FTS/trigram 派生索引。
- [`annotate::read_annotation_data`](../src-tauri/src/annotate.rs) / [`annotate::save_annotation`](../src-tauri/src/annotate.rs) 仍以 Markdown sidecar 为兼容与可携带边界。
- [`agent_work::AgentWorkMeta`](../src-tauri/src/agent_work.rs) 目前保存 prompt、origin quote、时间和 Pi session 引用；[`agent_work::list`](../src-tauri/src/agent_work.rs) 通过遍历 Markdown 文件生成工作列表。
- [`pi_agent::runs_dir`](../src-tauri/src/pi_agent.rs) 将 raw/半结构化运行收据写到 JSONL；[`pi_agent::agent_start`](../src-tauri/src/pi_agent.rs) 负责启动 session、发送 prompt 和记录阶段。
- [`ui/app.js`](../ui/app.js) 的 98–149 行与 276–300 行分别体现 UI 偏好、引用/运行态和关联固定状态的存储边界；1189 行附近的 `buildAgentPrompt` 是 Context Compiler 的迁移接缝。

## 可行性分级

### 高可行：文档契约与架构基线

这部分已经完成并纳入仓库。它不需要新增依赖、不改变数据格式，也不会影响当前 Tauri 的 zero-port 运行方式。根目录 `AGENTS.md` 让后续 Coding Agent 默认读取同一套规则，减少“每个 Agent 重新解释产品方向”的漂移。

### 高可行：P1 最小 durable state

项目已有 `rusqlite` bundled、WAL、事务和应用数据目录隔离机制。可以在当前 `index.db` 之外新增一个明确命名的 `state.db`（或等价 durable DB），通过版本化 migration 建立最小表：

```text
events
anchors
relations
context_sets
context_items
```

这样不会把正文迁进 SQLite，也不会把 FTS 与业务事实混在一起。第一版只需要稳定的 ID、时间、URI、JSON payload 和事务 helper，不需要 embedding、graph DB 或新的前端框架。

### 中等可行：P2–P3 迁移

难点不在建表，而在兼容已有用户数据和同时维护多个投影：

- 批注 sidecar 已经是用户可读、可 Git 同步的数据，不能直接删除；
- Agent Work 的 Markdown 正文与 JSON 元数据需要双读或一次性导入校验；
- Library 外部源不能被 DB 接管正文写入；
- 引用篮是当前会话行为，而 foundation 目标中的 Context 是可查询工作集，二者需要先定生命周期；
- 关联固定项有用户明确操作，不能因为它在 `localStorage` 就直接丢弃。

因此必须采用“兼容读取 → 导入/双写 → 校验 → 再删除旧路径”的迁移顺序，并为每一步添加测试。

### 中等可行：P5 之后的 Agent-native 能力

Pi runtime 的 session、RPC、流式事件和运行收据已经存在，说明 Thread / Run 不是从零开始。但目前 Agent 请求仍有较多前端编排，Work metadata 也没有与 Event / Relation / Context 统一。只有 P1–P3 稳定后，`agent_run(thread_id, context_set_id, anchor_id, instruction)` 才能成为可靠接口。

### 有边界可行：产品自演化

现有 Git、测试和 Agent 运行链路足以支撑 Product Work，但必须把 `product_repo_*`、隔离 worktree、diff/test/commit 与 merge/push/release 分成不同 capability。foundation 对 adoption boundary 的要求不能被“Agent 已能调用 Pi”替代。

## 推荐的实施顺序

### P1.1：只建立基础设施

- 新建 durable state DB 路径与 schema version；
- 加载 migration，并提供单一 transaction helper；
- 先落 `events / anchors / relations / context_sets / context_items`；
- 添加 migration、重启、损坏/缺失 DB 和空 workspace 测试；
- 不改变现有批注、引用篮、Ask Agent 和 Library 的 UI。

### P1.2：先接一个最小闭环

优先选择批注：

```text
选区
→ anchor
→ annotation metadata
→ annotation.created event
→ 现有 Markdown sidecar projection
```

此闭环能同时验证 Anchor、Event、Relation/Artifact 边界，并保留现有可携带 Markdown 交互。

### P2：逐项 eventize

按路线图顺序迁移 annotation、relation、context/reference、source、agent run/work，最后再记录 document open/save 等有意义事件。不要记录 keypress、mousemove 或滚动噪声。

### P3：收缩 UI 的事实所有权

将可查询的引用、固定关联、Agent Work metadata 和历史迁移到 backend query/command；localStorage 只保留布局、宽度、比例、主题和视图偏好。

## 主要风险与控制点

| 风险 | 控制点 |
| --- | --- |
| 文档/批注被迁移破坏 | Markdown 继续是正文与人类 artifact 的 source of truth；兼容读写和 golden migration tests |
| FTS 与 durable state 混淆 | `index.db`（derived）与 `state.db`（durable）分离，文档明确标注可重建性 |
| URI/锚点失效 | 统一 URI 规则；保存 document hash、quote、prefix、suffix；内容变更时显式标记不确定性 |
| 前端第二数据库 | 所有新持久化 command 由 Rust backend transaction + event 完成，UI 只接收 projection |
| Agent 权限过宽 | 使用 typed bounded tools；product code 的写入、commit、push 另设 adoption boundary |
| 大爆炸重写导致无法回退 | 每个 phase 保持现有 UI；先双读/双写和测试，再删除旧路径 |

## 验收标准

在开始 P2 之前，至少应满足：

- durable state 有唯一 Source of Truth；
- FTS 删除后可以重建，且不会丢失 events/anchors/relations/context；
- 一个重要 command 能在同一事务中更新 state 并追加 semantic event；
- 现有 Markdown 正文、批注和 Agent Work 仍可读取；
- migration/regression 测试覆盖旧数据；
- Agent 的 Context 来源可查询，而不是依赖 `ui/app.js` 手工拼接完整 prompt；
- Work/Agent 能看到 artifact、evidence、status 和 blocker；
- merge / push / release 仍保持明确的 adoption boundary。

## 最终判断

把 foundation 维护进 `stillwrite` 是可行且值得做的；它能立即提升后续 Agent 的上下文一致性，成本主要是文档治理而不是运行时改造。

完整 vNext 也可行，但应视为一项分阶段数据迁移工程：P1–P3 是决定成败的核心，P4 之后的 Library/Agent/Memory/Work 投影建立在 durable state 稳定之上。当前不建议直接开发 Daily Brief、embedding、Graph DB、插件系统或 React 重写。
