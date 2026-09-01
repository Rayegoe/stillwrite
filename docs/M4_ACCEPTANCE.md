# M4 — Interaction Contract Reset 验收矩阵

> 状态：自动化部分已落地（2026-09-01）。L1/L2 由 `cargo test` + `node --test`
> 覆盖；L3 是真人操作 gate（spec 06 + 2026-09-01 UI projection 修正），
> 需要带真实 Pi 的 GUI 会话执行一次。
> 数据规则沿用 03 Rule 8：不允许 mock Work/Session 数据通过业务验收。

## UI projection（2026-09-01 修正）

普通 Agent 回答的投影 = **右栏第四个 Context 视图**（`批注|关联|搜索|Agent`），
不是独立 modal/Answer Card：

- 选区「问 Agent」→ 提交后右栏自动切到 Agent 视图，流式显示回答；
- 回答锚定当前文档/选区（显示「针对选中的内容」+ 引文），不离开当前文档；
- 处置动作：`继续问`（M5 Session 后开放）/ `插入正文` / `保存为笔记` / `委派成工作`；
- 切回批注再切回 Agent，回答保留在当前界面状态（不落盘、不丢）；
- 换文档/换工作区即清空（回答锚在当时的文档上下文上）；
- **边界：复用批注栏的位置与交互范式，不把回答写进 Annotation schema**——
  批注是 Human-authored durable annotation，Agent 回答的
  `Document → Anchor → AgentSession → Response` 结构随 M5/M6 落地。
- rewrite 类任务的 inline diff、Work Artifact 的右栏 `工作` Review 视图
  分别属于后续里程碑，不在本版。

## 契约（本里程碑冻结）

```text
agent_start input = {
  runId,
  mode: "assist" | "work",     # 缺失/空 → assist（宁可少建 Work）
  instruction: String,          # 人的原始要求，逐字
  title?: String,               # 仅展示/会话名，永不写入 intent
  context?: { originUri?, originQuote?, citationContext? }
}
```

- runtime input（Pi 收到的消息）由 backend `compose_runtime_message` 组装；
  `instruction` 不再与 host context 拼成一个 `prompt` 领域字段。
- `mode=assist`：不创建 Work；Pi→Work 桥接对该 run 全部跳过；receipt 保留。
- `mode=work`：创建 Work，`Work.intent = instruction` 原文；
  queued → running（pi accepted）→ needs_human（artifact 固化）链路不变。
- 旧 Agent Work / 旧 Work 数据不迁移、不删除，正常打开。

## L1. Rust 单测（已覆盖）

| # | 断言 | 测试 |
| --- | --- | --- |
| R1 | mode 缺失/空/assist → Assist；work → Work；未知值报错 | `mode_parses_assist_work_and_defaults_to_assist` |
| R2 | runtime message 分节含来源/选区/引用；instruction 原文完整收尾；无 context 时用占位符 | `runtime_message_keeps_instruction_verbatim_and_context_in_sections` |
| R3 | 既有 Work 状态机不受影响（终态冻结、幂等、事件链） | work.rs 既有 142 个测试全绿 |

## L2. Frontend 纯函数（node --test）

| # | 断言 | 测试 |
| --- | --- | --- |
| F1 | buildStartInput 分离 instruction/context/mode；无 `prompt` 字段；缺省 assist | `ui/agent-request.test.js` |
| F2 | displayTitle 取首行/剥标记/截断，不回写 intent | 同上 |
| F3 | surface resume 三级优先：上次 Surface → 最近可用文档 → Work Home；坏数据不抛错 | `ui/surface-resume.test.js` |
| F4 | run settled 的 UI 状态语言 =「已生成」，不是「已完成」 | `ui/agent-events.test.js` |

## L3. 真人 GUI gate（待执行）

前置：真实 workspace + 可用 Pi。

1. [ ] 不创建 Work（5 次选区 Ask 后 Work Inbox 新增 0）。
2. [ ] 不创建 Agent Work Markdown（assist 不产生 artifact 文件）。
3. [ ] 回答不离开当前 Document（右栏 Agent 视图，无全屏/独立页面）。
4. [ ] 当前 selection/anchor 保持关联（卡片显示「针对选中的内容」+ 引文）。
5. [ ] 可插入正文（回答写入当前文档光标处，走正常保存/预览管线）。
6. [ ] 可保存为文档/笔记（生成 Workspace md 并打开）。
7. [ ] 可委派成 Work（composer 预填原始 instruction，委派后恰新增 1 个 Work）。
8. [ ] 切回「批注」再切回「Agent」，回答仍保留在当前界面状态。
9. [x] `继续问`——已由 M5（Durable Agent Thread）兑现为 thread composer，
   见 `docs/M5_ACCEPTANCE.md`。
10. [ ] 不修改 Annotation schema（回答不出现在批注列表/批注文件中）。

> 2026-09-01 M5 修正：右栏 Agent 的数据源已从临时 `lastAgentAnswer` 升级为
> durable `agent_sessions`/`agent_messages`；切文档不再清空回答，只重排
> 相关/最近会话。第 8 条由 durable thread 天然满足。

另验（M4 原有 gate）：

- [ ] 启动恢复：上次文档 / 删除后回退最近文档 / 无对象进 Work Home /
      上次 Work Detail，四种场景符合 02_M3_HANDOFF 优先级。
- [ ] 2 次显式委派（Work Home「发起 Agent 工作」/ 侧栏 ＋）→ 恰好 2 个 Work；
      intent 与输入逐字一致，不含 host context 段落。
- [ ] assist 失败一次 → 右栏显示失败与证据指引；receipt 可查；Work Inbox 无新增。
- [ ] legacy：旧 Agent Work 列表/打开/编辑正常；旧 Work 详情正常。

## 回归（03 Rules 7，本里程碑不得破坏）

- [ ] Rust tests（142+）；frontend JS tests（92）；`node --check`；`git diff --check`
- [ ] Workspace open/save/autosave；写/双/读；format toolbar；Git sync
- [ ] 批注；relations/pins；Brave search/history；Library search/open/reference；RSS
- [ ] Pi start/abort/result；Agent Work open/edit；Work list/detail/evidence
