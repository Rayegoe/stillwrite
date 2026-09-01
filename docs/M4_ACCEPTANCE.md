# M4 — Interaction Contract Reset 验收矩阵

> 状态：自动化部分已落地（2026-09-01）。L1/L2 由 `cargo test` + `node --test`
> 覆盖；L3 是真人操作 gate（spec 06），需要带真实 Pi 的 GUI 会话执行一次。
> 数据规则沿用 03 Rule 8：不允许 mock Work/Session 数据通过业务验收。

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

## L3. 真人 GUI gate（spec 06 M4 验收，待执行）

前置：真实 workspace + 可用 Pi。

- [ ] 启动 1：上次停在某个文档 → 重启 StillWrite → 恢复该文档（非 Work Home）。
- [ ] 启动 2：删除上次文档后重启 → 恢复最近可用 Workspace 文档。
- [ ] 启动 3：清空 recents + 无有效 surface → 进入 Work Home/empty state。
- [ ] 启动 4：上次停在 Work Detail → 重启恢复该 Work 详情。
- [ ] 连续 5 次选区「问 Agent」（assist）→ Work Inbox 新增 0 个 Work；回答以
      Answer Card 呈现；[插入正文] 写入当前文档；[保存为文档] 生成 Workspace md；
      [委派成工作] 打开委派 composer 且预填原始 instruction。
- [ ] 2 次显式委派（Work Home「发起 Agent 工作」/ 侧栏 ＋）→ 恰好 2 个 Work；
      按钮/对话框呈现「委派 Agent 工作 / 开始工作」。
- [ ] 两个新 Work 的 intent 与输入逐字一致，不含
      `# Current source` / `# Selected text` / `# Explicit references`。
- [ ] assist 失败一次（可拔掉 Pi 配置）→ 有失败提示；`agent_recent_runs`/receipt
      可查到该次失败证据；Work Inbox 无新增。
- [ ] legacy：旧 Agent Work 列表/打开/编辑正常；旧 Work 详情正常。

## 回归（03 Rules 7，本里程碑不得破坏）

- [ ] Rust tests（142+）；frontend JS tests（92）；`node --check`；`git diff --check`
- [ ] Workspace open/save/autosave；写/双/读；format toolbar；Git sync
- [ ] 批注；relations/pins；Brave search/history；Library search/open/reference；RSS
- [ ] Pi start/abort/result；Agent Work open/edit；Work list/detail/evidence
