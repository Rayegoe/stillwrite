# StillWrite vNext Architecture

## 1. System Role

StillWrite 是位于人和多个 Agent / 数据源 / 工具之间的 **Cognitive Coordination Layer**。

它解决：

```text
Raw Information Explosion
→ Persistent State
→ Relation
→ Memory
→ Work
→ Compression
→ Projection
→ Human Judgment
```

## 2. High-Level Architecture

```text
┌────────────────────────────────────────────┐
│ Human                                      │
│ read / write / annotate / select / decide  │
└───────────────────┬────────────────────────┘
                    │
                    ▼
┌────────────────────────────────────────────┐
│ UI / Projection Layer                      │
│ Content / Work / Context                   │
└───────────────────┬────────────────────────┘
                    │ commands / queries
                    ▼
┌────────────────────────────────────────────┐
│ Domain Layer                               │
│ Entity / Event / Relation / Artifact       │
│ Anchor / Annotation / Context / Work       │
│ Thread / Memory / Agent                    │
└──────────────┬─────────────────┬───────────┘
               │                 │
               ▼                 ▼
        ┌────────────┐      ┌────────────┐
        │ SQLite     │      │ Markdown   │
        │ state      │      │ artifacts  │
        └─────┬──────┘      └────────────┘
              │
              ▼
┌────────────────────────────────────────────┐
│ Agent Runtime / Capabilities               │
│ Pi + typed tools + future workers          │
└──────────────┬─────────────────────────────┘
               │
       ┌───────┴────────┐
       ▼                ▼
 External Adapters   Product Code Worker
 RSS/Git/Audio/...   Pi / Codex
```

## 3. Canonical Layers

### 3.1 Data ingestion

外部来源通过 Adapter 进入：

```text
RSS
GitHub
Audio
Transcript
Logs
Agent runtimes
Local folders
Future connectors
```

Adapter 只负责：

- ingest
- normalize
- refresh
- provenance

不负责建立独立产品模块。

### 3.2 Canonical primitives

#### Entity

统一标识对象。

建议 URI 风格：

```text
workspace://docs/design.md
library://source-id/path.md
source://rss-simon
agent://...                     (legacy/reserved，Agent Actor canonical URI 待定义)
thread://thread-id
work://work-id
run://run-id
annotation://annotation-id
anchor://anchor-id
memory://memory-id
commit://sha
audio://artifact-id
```

#### Event

Append-only semantic history。

#### Relation

Typed edge：

```text
references
supports
contradicts
derived_from
created_from
continues
about
summarizes
blocks
depends_on
produced_by
```

#### Artifact

真正的工作成果。

### 3.3 Cognitive primitives

#### Anchor

让选中文字成为可持久、可复用的对象。

所有以下对象引用 Anchor，而不是各存一份 `originQuote`：

- Annotation
- Agent request
- Relation
- Context item
- Memory

#### Context

Context 是 Agent 与人共享的显式工作集。

当前“引用篮”迁移目标：

```text
context_set
├─ current document
├─ selected anchor
├─ library refs
├─ annotations
└─ related evidence
```

#### Memory

由 Event / Work / Document / Relation 压缩得到。

不是 raw log。

#### Work

Work 是跨 Agent / 场景通用的协调单位。

```text
Work
├─ intent
├─ state
├─ actors
├─ inputs/context
├─ artifacts
├─ evidence
├─ decisions
├─ risks
├─ blockers
└─ next_actions
```

WO 可以成为 Work 的一种 protocol/projection，而不是独立产品模块。

## 4. Agent Architecture

### 4.1 Pi is runtime, StillWrite is system

Pi 管：

- model loop
- tool call
- session
- compaction
- runtime trace

StillWrite 管：

- context
- memory
- relations
- work
- permissions
- persistence
- projection

### 4.2 Thread continuity

当前 Agent Work 偏向一次请求一个成果。

vNext 目标：

```text
Thread
├─ Turn
│  └─ Run
│     └─ Artifact
├─ Turn
└─ ...
```

Pi session 与 Thread 关联，而不是每个请求重新开始。

### 4.3 Context Compiler

未来 Agent prompt 不应由前端手工拼接巨大文本。

目标接口：

```text
agent_run(
  thread_id,
  anchor_id,
  context_set_id,
  instruction
)
```

Agent 使用 typed tools 查询：

```text
context_get
workspace_read
library_read
relation_neighbors
memory_search
annotation_query
work_query
```

## 5. Work / WO as Agent Coordination

多个 Coding Agent 的问题不是缺少更多聊天窗口，而是缺少压缩。

原始输入：

```text
tool calls
logs
diffs
tests
errors
chat
```

投影给人的 Work：

```text
目标
状态
成果
证据
阻塞
风险
需要决定
下一步
```

这使一人可以监督更多 Agent。

## 6. Library Architecture

Library 是长期知识池。

### UI projection

```text
最近
推荐
来源
```

来源不展开成完整目录树。

点击来源后，在主内容面显示 flat list。

### Recommendation

推荐来自：

```text
current work
+ recent events
+ relations
+ memory
+ FTS
+ source/author affinity
```

第一阶段不需要 embedding。

### RSS

RSS 是一种 Adapter / Source。

Agent 可以：

```text
feed_add
feed_refresh
feed_remove
```

Agent 输出：

- 每日简报
- 重点阅读
- 作者更新摘要
- 与当前 Work 的相关性
- 与既有观点的冲突

这些输出是 Agent Work / Projection，不是 RSS 模块。

## 7. Learning Example

英语精听：

```text
Audio Artifact
+ Transcript Document
+ Dictation Document
+ Anchor
+ Agent Evaluation
+ Learning Memory
```

无需英语专属导航。

打开 Audio，自然出现播放 controls；下面仍可写听写。

全文选中后：

> 问 Agent：loss 有多少？错误类型？下一步重点词汇？

这证明新场景可以通过 primitive + capability + projection 实现。

## 8. Self-Evolution Architecture

```text
Human intent
→ code.change.requested event
→ Product Work
→ isolated worktree
→ Coding Worker (Pi/Codex)
→ tests
→ diff
→ commit
→ evidence
→ adoption decision
```

不要直接让 runtime Agent 对当前 main 无边界写入。

## 9. Two Main Loops

### Cognition Loop

```text
Read
→ Select
→ Annotate
→ Relate
→ Agent
→ Artifact
→ Human Edit
→ Event
→ Memory
→ Better Context
```

### Evolution Loop

```text
Use
→ Friction / Event
→ Change Intent
→ Product Work
→ Coding Agent
→ Git/Test/Evidence
→ Commit
→ Adopt
→ Use
```
