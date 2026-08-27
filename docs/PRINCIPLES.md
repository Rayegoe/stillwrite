# StillWrite Principles

## 1. Human cognitive bandwidth is the scarce resource

StillWrite 不以“让 Agent 说更多、做更多”为终点。

真正稀缺的是人的：

- 注意力
- 上下文恢复能力
- 比较能力
- 判断能力

因此优化目标是：

> **降低 Human-Agent Interface Bandwidth，同时保留决策所需证据。**

## 2. Information compression, not feature accumulation

新场景首先被视为数据问题，而不是 UI 功能问题。

错误路径：

```text
新需求
→ 新模块
→ 新页面
→ 新按钮
→ 新插件
```

默认路径：

```text
新来源 / 新工作
→ Adapter
→ Canonical Data
→ Relation / Memory / Work
→ Agent
→ Projection
```

## 3. Integration is an adapter, not a module

RSS、GitHub、Coding Agent、audio、email、logs 等首先进入统一数据模型。

不要让 integration 决定产品信息架构。

## 4. Contextual action over global feature entry

能力尽量出现在它真正有意义的上下文。

例如选中文本时：

```text
批注
问 Agent
关联
```

而不是为每种能力做永久 toolbar icon。

## 5. Read / Think / Write remains the human center

Agent 的作用不是把人从认知循环移走。

StillWrite 要保留并加强：

```text
Read
→ Select
→ Annotate
→ Relate
→ Ask / Instruct
→ Evaluate
→ Write
```

## 6. Memory is compressed evidence

Memory 不是聊天历史，也不是原始日志。

Memory 必须：

- 对未来有价值；
- 可查询；
- 可追溯；
- 尽量由 evidence 支撑。

## 7. Work is the coordination compression protocol

Agent 的 tool calls、logs、diff、tests 不应该全部推给人。

Work 应压缩成：

- 目标
- 当前状态
- 结果
- 证据
- 阻塞
- 风险
- 决策
- 下一步

人的注意力只进入真正需要判断的位置。

## 8. UI is projection

UI 不拥有业务事实。

同一份 canonical data 可以被投影成：

- Library Home
- Daily Brief
- Agent History
- Work Board
- Related panel
- Learning evaluation

Projection 可以随时变化，底层事实不应跟着 UI 重构。

## 9. Portable human artifacts

人的正文、Agent 面向人的成果尽量保持可读、可导出。

Markdown 仍然是 StillWrite 的重要人类接口。

## 10. Evidence before architecture

不要因为“以后可能需要”而提前加入：

- embedding
- vector DB
- graph DB
- plugin framework
- React rewrite
- complex orchestration

只有真实 failure 证明当前 primitive 无法承载，才升级架构。

## 11. Agent observability should become semantic observability

Raw trace 对调试有价值，但对日常监督 Agent 的人过载。

StillWrite 应从：

```text
10000 lines raw log
```

压缩为：

```text
状态
原因
证据
风险
需要人的决定
```

## 12. Software can evolve through the Agent, but adoption is bounded

Agent 可以读代码、建 worktree、修改、测试、commit。

最终 merge / push / release 仍应有明确 adoption boundary。

这保证：

> 软件可以自迭代，但不会失去人类控制。
