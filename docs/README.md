# StillWrite

StillWrite 是一个本地优先的 **Agent-native Human Workbench**。

它从 Markdown 写作工具出发，但长期目标不是不断追加功能、页面或插件，而是把人的阅读、写作、批注、资料、Agent 工作、运行日志、Git 历史和外部信息统一为可查询的数据，再通过关系、记忆、上下文与工作对象进行压缩和呈现。

> **Software is Agent. Agent is Software.**
>
> StillWrite 的核心不是“在软件里放一个 Agent”，而是让 Agent 成为软件控制平面的一部分；让 UI 退化为数据与工作状态的投影。

## 为什么做 StillWrite

Agent 的执行能力正在快速提高，但人的注意力和认知带宽没有同步增长。

典型问题已经不是“Agent 能不能做”，而是：

- 十几个 Coding Agent 窗口同时运行，人无法持续跟踪；
- 每个 Agent 都重复读取项目规则、历史决策、失败经验；
- RSS、资料库、日志、Git、聊天、WO、测试证据分散在不同界面；
- 人不得不不断恢复上下文、搬运信息、比较结果；
- Agent 输出越来越多，但真正需要人的判断通常很少。

StillWrite 要做的是：

**把高带宽原始信息压缩为低带宽、高信息密度、可追溯的状态与判断。**

## 产品表面保持简单

当前最稳定的交互模型继续保留：

```text
文件 | 资料 | Agent
```

- **文件**：人的工作区，可编辑 Markdown。
- **资料**：外部知识池，只读为主，可浏览、引用、批注。
- **Agent**：持续工作、历史、成果与运行状态。

文档呈现方式仍然是：

```text
写 | 双 | 读
```

它和 `文件 / 资料 / Agent` 是两个正交维度：

- 左侧决定“当前在看什么对象”；
- 右上决定“当前对象如何呈现”。

选中文字后的核心交互保留，并增加联网搜索入口：

```text
＋批注 | 问 Agent | 联网搜索 | ＋关联
```

`问 Agent` 是上下文动作，因此不需要再在右上工具栏保留一个重复入口。

左下角齿轮“设置”用于填写 Brave Search API Key；每次互联网搜索都会成为右侧支持栏“搜索”视图中的历史条目，与“批注”“关联”并列。点开历史条目即可展开已写入 `state.db` 的结果快照，结果可重新打开网页或关联当前笔记。密钥由后端保存到应用数据目录，未保存设置时也可使用 `BRAVE_SEARCH_API_KEY` 环境变量。

## 不再用“模块”组织产品

StillWrite 不把 RSS、英语学习、Coding Agent、日报、WO、日志分析当作需要独立 UI 的模块。

它们首先被表达为统一数据：

```text
Entity
Event
Relation
Artifact
Anchor
Annotation
Context
Memory
Work
Thread
Agent
```

然后由同一套界面进行投影。

例如：

### RSS

不是 RSS 阅读器：

```text
RSS Adapter
→ Source / Documents / Events
→ Agent
→ 今日简报 / 推荐阅读 / 作者重点
```

### 英语精听

不是英语学习页面：

```text
Audio Artifact
+ Transcript
+ Dictation Document
+ Evaluation
+ Learning Memory
```

用户直接播放音频、在主界面听写，完成后 Ctrl+A：

> 问 Agent：这次 loss 有多少？主要错误是什么？下一步重点词汇是什么？

### Coding Agent

不是再开十几个聊天窗口：

```text
Agent Runs
+ Work
+ Git
+ Tests
+ Evidence
→ 压缩为状态 / 阻塞 / 决策 / 下一步
```

## Library：知识池，不是文件树

资料库必须同时支持“搜索”和“浏览”。

搜索回答：

> 我知道我要找什么。

Library Home 回答：

> 我不知道库里有什么，什么值得看。

资料默认采用：

```text
最近
推荐
来源
```

来源只作为一级书架，不展开成几千篇目录树。点击来源后，在主内容区显示 flat list。

示意：

```text
资料
────────────────
最近
推荐

来源
三联生活周刊   386
Simon Willison  84
Pydantic        42
项目资料        61
```

主内容区：

```text
今日建议阅读
继续阅读
最近加入
```

RSS 只是 `Source` 的一种，不获得专属顶层 UI。

## 核心架构

```text
External World / Human Activity / Agent Runs
                    ↓
                 Adapters
                    ↓
        Entity / Event / Relation / Artifact
                    ↓
       Anchor / Annotation / Context / Work
                    ↓
              Memory / Compression
                    ↓
                   Agent
                    ↓
           Artifacts / Actions / Code
                    ↓
                 Projection
                    ↓
                  Human
```

### Markdown

承载人类可读、可携带内容：

- Workspace 文档
- Agent Work
- 报告
- 听写
- 研究成果
- 可导出的批注投影

### SQLite

承载结构化、可查询的长期状态：

- semantic events
- anchors
- annotations metadata
- relations
- web search history / result snapshots
- context sets
- threads / turns / runs
- memories
- work coordination
- sources

FTS 等索引属于可重建派生数据；上述 durable state 不是“随时可删的索引”。

### Pi

Pi 是 Agent Runtime：

- model loop
- tools
- session
- compaction
- runtime events

StillWrite 才是完整 Agent System：

- durable state
- memory
- context
- permissions
- work
- projection

### Git

Git 有两种不同职责：

1. **Workspace Git**：用户内容同步；
2. **Product Git**：StillWrite 自身演化。

产品自修改必须通过独立代码变更边界，优先使用 Git worktree/branch，形成 diff、测试和 commit 证据后再决定是否采用。

## Semantic Event Log

StillWrite 记录有意义的行为，而不是噪声 telemetry。

例如：

```text
document.opened
document.saved
annotation.created
relation.created
source.added
context.attached
agent.requested
agent.run.completed
agent.work.edited
decision.made
work.blocked
code.change.requested
code.change.committed
```

不记录每次鼠标移动或每个按键。

Event 回答“发生过什么”。

Memory 回答“未来还值得记住什么”。

两者不能混为一谈。

## 当前实现基础

仓库当前已经具备重要基础：

- 本地 Markdown Workspace；
- 写 / 双 / 读；
- FTS 搜索；
- Git 同步；
- 字句/段落批注；
- Library；
- RSS / Atom ingestion；
- 资料引用；
- 关联；
- Pi RPC Agent；
- Agent Work Markdown；
- Agent run events / receipts。

vNext 的重点不是继续加更多入口，而是把这些能力背后的 durable state 收束到统一数据模型，并让 Agent 直接消费 Context / Relation / Memory。

## 开发文档

- [`AGENTS.md`](../AGENTS.md) — Coding Agent / Pi / Codex 的仓库级开发契约
- [`PRINCIPLES.md`](./PRINCIPLES.md) — 产品与工程理念
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — vNext 统一架构
- [`DATA_MODEL.md`](./DATA_MODEL.md) — 数据与 Source of Truth
- [`UX_MODEL.md`](./UX_MODEL.md) — UI/Projection 模型
- [`ROADMAP.md`](./ROADMAP.md) — 分阶段迁移计划

## 当前阶段

当前进入 **Foundation / Contract Phase**。

原则：

> 先定数据、关系、记忆、Agent 和权限边界，再迁移现有交互；不先增加新的业务模块。

下一步详见 [`ROADMAP.md`](./ROADMAP.md)。
