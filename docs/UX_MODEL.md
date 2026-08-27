# StillWrite UX / Projection Model

## 1. UI is not the product model

UI 只负责把 canonical state 呈现给人。

不以“模块”设计导航。

## 2. Primary Navigation

当前保持：

```text
文件 | 资料 | Agent
```

### 文件

人的 Workspace。

### 资料

外部知识池。

### Agent

Thread / Work / Agent history 的入口。

这三个是“对象来源/工作平面”。

## 3. View Mode

保持：

```text
写 | 双 | 读
```

它只描述当前内容如何显示。

不要让 `libraryMode` / `agentMode` 成为决定所有 UI 行为的核心业务状态。

未来可由 Document/Entity capability 推导：

```text
editable
renderable
annotatable
referenceable
playable
```

## 4. Toolbar

目标：

```text
☰文件   ⟳同步        当前文档        已保存    ☰批注    写 双 读
```

已完成清理：

- 顶部 `问 Agent` 已移除；Agent 入口保留在左侧导航与选区动作里；
- 顶部 `＋批注` 改为 `☰` 图标，仅表示右侧批注栏的显示/隐藏（active 态高亮），
  创建批注本身仍由选区动作发起；
- 顶部 `关联 n` 已移除；查看关联走右侧支持栏的“关联”视图。

保留选区动作：

```text
＋批注 | 问 Agent | 联网搜索 | ＋关联
```

`联网搜索` 将选区发送到 Brave Search API，在右侧支持栏的“搜索”视图把每次搜索保留为历史条目，与“批注”“关联”并列；点击历史条目展开已保存的网页结果，可重新打开网页或关联当前笔记。API 密钥通过左下角设置由后端保存，未保存时回退到环境变量；搜索历史和结果快照写入应用数据目录的 `state.db`，不写入 Markdown。

## 5. Library

### Problem

当前 Library 更偏搜索导向：

- 不搜索时难以发现具体资料；
- 如果改成完整目录树又会爆炸；
- RSS 如果继续增加专属 UI，会演化成 RSS Reader。

### Target

左栏只展示入口与 Sources：

```text
资料
────────────────
搜索资料…

最近
推荐

来源
三联生活周刊     386
Simon Willison    84
Pydantic          42
项目资料          61
```

点击 `资料` 默认进入 Library Home：

```text
今日建议阅读
继续阅读
最近加入
```

点击 Source：

```text
Simon Willison · 84 篇

[搜索这个来源…]

最新文章 A
summary...
[打开] [引用]

文章 B
summary...
```

主内容区使用 flat list / pagination / virtualized list。

Source 的物理磁盘目录不是主信息架构。

## 6. Recommendation Projection

每一条 Agent 推荐都应尽量解释：

```text
为什么推荐给你
```

例如：

- 与昨天 3 条 StillWrite Memory 批注相关；
- 与当前 `design.md` 有直接关系；
- 你最近连续引用了 4 篇 Agent evaluation 资料；
- 与既有判断相反，值得检查。

优化目标不是 engagement，而是：

> **提高当前认知任务的信息价值密度。**

## 7. RSS

不新增 RSS 顶层页面。

RSS Source 进入 Library。

Agent 生成：

- Daily Brief
- 今日重点
- 作者更新
- 建议跳过内容
- 与当前 Work 的相关性
- 冲突观点

Daily Brief 是普通 Agent Work Markdown，可继续选字、批注、问 Agent。

## 8. Audio / Learning

没有“英语学习” icon。

当当前 Artifact 可播放时显示音频 controls：

```text
▶ 00:42 ━━━━━━━━━━━ 18:32
-5s  0.8x  1.0x  1.2x
```

下方仍是普通 Document Surface。

用户听写结束后全文选择：

```text
问 Agent
```

Agent 基于：

- dictation
- reference transcript
- past learning memory

输出 evaluation / next focus。

## 9. Agent History

Thread row 的主要文字必须是人的意图：

```text
StillWrite 架构重构
把批注和关系迁进 SQLite…
刚刚
```

不要让 `originQuote` 代替 prompt/instruction。

origin / refs / tools / run status 进入详情。

## 10. Work Projection

多个 Agent 不显示成十几个聊天窗口。

压缩成：

```text
StillWrite DB Migration
Codex
实施中 · tests pending

ZUAEF Console
Pi
阻塞 · missing fixture

Website Rewrite
Codex
完成 · tests PASS
```

点开后再看：

- intent
- artifacts
- evidence
- blockers
- decisions
- next actions
- raw trace（必要时）

## 11. Long-Term Three Projection Families

内部可逐渐归纳为：

### Content

我正在读/写什么？

### Work

人和 Agent 正在做什么？

### Context

依据、关系、记忆、证据是什么？

当前不需要立即把 UI 改名成这三项，但所有新界面应能映射到其中之一。
