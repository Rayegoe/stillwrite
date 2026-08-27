# M2 — 验收测试矩阵（设计文档，非契约）

> 状态：设计稿。执行条件：M1 数据结构稳定（works 表 + work.rs API 冻结）之后，
> 由 M2 冲刺按本矩阵产出测试。**所有"数据前置"必须来自真实 M1 流程**
> （headless gate：create → running → attach artifact → …，或真实 Pi 请求），
> 禁止 mock 卡片/伪造 DB 行（03 Rule 8 + M2 Gate）。

## A. 七状态展示矩阵

| # | work 状态 | 数据前置（真实 M1 流程） | 列表行断言 | 详情断言 | 分组 |
| --- | --- | --- | --- | --- | --- |
| A1 | `queued` | `create_work` 后不做任何转换 | badge「排队」；可见 title；无 summary 时摘要行不渲染 | 状态=queued；无「由X转为Y」；成果=尚无成果；证据=receipt_ref 存在 | 进行中 |
| A2 | `running` | create → transition(running, actor=Agent, reason="pi accepted") | badge「进行中」；summary 为空 | 状态=running；「由 queued 转为 running」+ reason="pi accepted" | 进行中 |
| A3 | `needs_human` | create → running → attach_work_artifact(uri, summary, next_action) | badge「需要你」；summary 一行截断显示 | 目标=intent 全文；状态=由 running 转为 needs_human；下一步=next_action；成果=可点击打开 agentwork:// 的 Markdown；证据=事件 payload 含 artifact_uri | 需要你 |
| A4 | `blocked` | create → transition(blocked, reason="依赖缺失") | badge「受阻」；reason 见详情 | 状态转换带 reason；无重试按钮 | 需要你 |
| A5 | `failed` | create → running → transition(failed) | badge「失败」 | 终态；无任何操作按钮 | 最近完成 |
| A6 | `cancelled` | create → running → transition(cancelled, actor=Human) | badge「已取消」 | 终态；无操作按钮 | 最近完成 |
| A7 | `completed` | create → running → attach → transition(completed, actor=Human) | badge「完成」；进"最近完成" | 状态来自 Human 接受事件（actor_type=human, from=needs_human） | 最近完成 |

## B. 行为/门禁矩阵

| # | 场景 | 前置 | 操作 | 期望 |
| --- | --- | --- | --- | --- |
| B1 | 重启持久 | A3 数据集 | 关闭 state.db 连接 → 重开 → 重新查询并渲染 | 分组、状态、artifact_uri、receipt_ref 全保留；事件链完整 |
| B2 | 真实数据门禁 | 无手工 DB 行 | 代码评审 + 数据源检查 | 所有测试数据均由 work::* 命令或真实 Pi 请求产生；无 mock |
| B3 | 排序 | 混合状态 5+ 条，updated_at 交错 | 打开 Agent 区 | 全列表 `updated_at DESC, id DESC`；组内同序 |
| B4 | 空状态 | 全新 workspace，无任何 Work | 打开 Agent 区 | 三组均不渲染标题；显示一条中性空文案（含「发起问 Agent / 选区动作」提示路径，不新增入口） |
| B5 | receipt 存在性 | A3 + 删除 `<AppData>/agent/runs/<id>.jsonl` | 打开详情证据区 | 标记「receipt 缺失」；其余字段不受影响；不崩溃 |
| B6 | 未知 status 数据 | 直接 SQL 写入非法 status（仅测试夹具） | 打开列表 | 后端已拒绝写入（migration/API 层）；若残留，前端降级为中性 badge「未知」且不崩溃 |
| B7 | 终态不可再操作 | A7 数据集 | 尝试点操作按钮 | 无按钮可点；`transition_work` 对终态转换返回错误且 UI 不展示错误态 |
| B8 | LLM 自报完成防线 | `queued` work | 尝试通过任何 UI 路径标记完成 | 不存在该路径；后端 `queued→completed` 被拒绝（回归 work.rs 单测） |
| B9 | 双工作区隔离 | ws-A、ws-B 各有 Work | 切换工作区 | 只显示当前 workspace 的 Work（list_works 按 workspace_id 过滤） |
| B10 | 取消路径 | A2 运行中 + 真实 Pi 进程 | 取消 | 状态=cancelled；Pi 进程收到 abort（若 M1 通路存在）；事件 actor=human |

## C. 全量回归（03 Rules，M2 不得破坏）

- [ ] Workspace 打开；Markdown 写/双/读与保存/autosave；Git sync
- [ ] 批注；关联与固定；`批注 | 关联 | 搜索` 右栏
- [ ] Brave 搜索历史；Library search/open/reference；RSS
- [ ] Pi start/abort/result；Agent Work open/edit
- [ ] Rust/frontend tests 全绿；无新增 warning
- [ ] 旧 Agent 列表兼容入口仍可打开（UX_MODEL §2：不先删除再迁移）

## D. M2 业务验收（03 原文映射）

> 不用逐个打开 raw Agent Work，就能知道 active / review / completed 工作和对应产物。

- 验证方式：A1–A7 矩阵全绿 + 一次真实使用演示（≥1 个「需要你」Work 从列表直接看到产物）。

## E. 明确不测 / 不做（防止越界）

- 不测 Shell 切换（M3）、RunSurface（M3 后）、Context 持久化（M4）、Attention（M5）；
- 不测 tests/diff/commit 证据（当前无 adapter，显示即违规）；
- 不测 Thread/Run/Artifact 表（本阶段不建表）。
