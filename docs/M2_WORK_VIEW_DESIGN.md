# M2 — Work View 设计（设计文档，非契约）

> 状态：设计稿（PREP 后、M1 开发中产出）。本文件是 M2 实现前的设计输入，
> 不修改任何产品代码；实现的唯一数据依据是 M1 落地后的真实 schema/API。
> 对齐：[`02_MILESTONES.md` M2]（spec）、`AGENTS.md §6`、`docs/UX_MODEL.md §2/§9/§10`、
> `03_RULES_ACCEPTANCE.md`。

## 0. 目标与边界

**目标**：在不改主导航（`文件 | 资料 | Agent`）的前提下，让 Agent 区直接回答三个问题：
哪些 Agent 工作需要我、哪些在跑、结果在哪里——不用逐个打开 raw Agent Work。

**边界（本阶段不做）**：

- 不切换 Shell（`工作 | 资料 | 文件` 属 M3）；
- 不做 RunSurface、不重做 SourceSurface；
- 不显示 tests/diff/commit 证据（当前无真实 adapter）；
- 不新增 Thread / Run / Artifact 表（M1 只有 `works`）；
- 不引入 mock 数据；Gate 要求真实 M1 数据。

## 1. 数据依赖（截至 M1 真实实现，只读核对）

### 1.1 `works` 表（state.db migration v4）

| 字段 | 说明 |
| --- | --- |
| `id` TEXT PK | `work-<pid>-<nanos>` |
| `workspace_id` TEXT | opaque workspace key（不写绝对路径） |
| `title` TEXT NOT NULL | 展示标题；M1 bridge 取 prompt 前 120 字符 |
| `intent` TEXT NOT NULL | 人的原始意图（=prompt 全量），不被 Agent 改写 |
| `status` TEXT NOT NULL | 7 态，见 1.2 |
| `summary` TEXT | 可空；Artifact 绑定/更新时写入 |
| `next_action` TEXT | 可空；留给人的下一步提示 |
| `artifact_uri` TEXT | 可空；`agentwork://...` canonical URI |
| `receipt_ref` TEXT UNIQUE | run id；收据本体在 `<AppData>/agent/runs/<run-id>.jsonl` |
| `created_at` / `updated_at` TEXT | ISO；状态/字段变更时 updated_at 更新 |

### 1.2 状态机（`work.rs`，Domain rule）

`queued → running → needs_human → completed`，另含 `blocked / failed / cancelled`：

```text
queued      → running | blocked | failed | cancelled
running     → needs_human | blocked | failed | cancelled
needs_human → completed | cancelled
blocked     → running | failed | cancelled
```

- 终态（completed/failed/cancelled）冻结；
- `completed` 只来自 Human actor 的明确接受；Pi 返回最终文本不触发；
- `attach_work_artifact`：running → needs_human（幂等；绑定非同一 Artifact 报错）。

### 1.3 查询 API（`work.rs`）+ 前端通路缺口

- `list_works(workspace_id, limit)`：`ORDER BY updated_at DESC, id DESC`；
- `get_work(id)` / `find_work_by_receipt(receipt_ref)`；
- **缺口**：`lib.rs invoke_handler` 目前无任何 work command。
  M2 实现需新增后端 command（见 §4），属于 M2 的正常实现范围，不是 M1 缺陷。

### 1.4 事件面（供详情"证据"投影）

- `work.created`（payload: status/title/receipt_ref）
- `work.status_changed`（payload: from/to/reason?/artifact_uri?/next_action?；`to=needs_human` 携带 Artifact 指纹，已为 M2 证据投影预留）
- `work.updated`（payload: changed[]）

## 2. Agent 区信息架构

### 2.1 分组规则（建议，待 M2 冲刺确认）

```text
需要你        needs_human | blocked
进行中        queued | running
最近完成      completed | failed | cancelled
```

- 组内与跨组排序统一 `updated_at DESC, id DESC`（与 `list_works` 一致）；
- 一次查询全量（limit 由 UI 传，默认 100），分组在 projection 层完成；
- 组标题带计数；空组不渲染标题。

### 2.2 行展示（02 M2：只展示四要素）

```text
[状态badge] title（intent 超出时同文本回退）    updated_at 相对时间
            summary（可空，一行截断）
```

- 主文字 = `title`；title 为空（防御）时回退到 `intent` 截断；
- status badge 颜色建议（可访问性：不只用颜色，badge 带文字）：

| status | badge 文案 | 语义色 |
| --- | --- | --- |
| queued | 排队 | 中性 |
| running | 进行中 | 蓝 |
| needs_human | 需要你 | 橙（最高优先级） |
| blocked | 受阻 | 琥珀 |
| completed | 完成 | 绿 |
| failed | 失败 | 红 |
| cancelled | 已取消 | 灰 |

## 3. Work Detail（点击行展开/进入）

| 区块 | 数据来源 | 展示 |
| --- | --- | --- |
| 目标 | `intent` | 全文，不被 Agent 改写；空则显示 title |
| 当前状态 | `status` + 最近 `work.status_changed`（from/to/reason） | badge + "由 X 转为 Y" + reason |
| 下一步 | `next_action` | 空显示"见成果/等待验收"类中性文案 |
| 成果 | `artifact_uri` | 可点击，**打开现有可编辑 Agent Work Markdown**（复用现有 MarkdownSurface/阅读路径）；为空显示"尚无成果" |
| 证据 | `receipt_ref` + 本地 events | 引用显示；receipt 本体文件存在性标记（存在/缺失），不解析；events 列表最多 N 条（倒序） |

### 3.1 证据展示边界（02 M2 原文）

当前 Pi 证据只允许：Artifact、receipt、session ref、runtime settlement/failure、Work events。
没有真实 adapter 就不显示 tests/diff/commit。

### 3.2 操作（建议最小闭环，待确认）

- `needs_human` → **接受完成**（Human actor → completed）：`completed` 只来自人的明确接受，UI 若无此入口则 completed 不可达；
- `queued / running` → **取消**（Human actor → cancelled；运行中取消需同时通知 Pi 进程 abort，若 M1 已有 abort 通路则复用，否则仅状态落库并标注）；
- `blocked` → 不提供重试（重试属 A2 运行层），仅展示。

## 4. 后端 command 契约（M2 新增，设计层面定义）

```text
list_works(workspace_id)            → WorkRecord[]（updated_at DESC, id DESC）
get_work(work_id)                   → WorkRecord
work_accept(work_id)                → transition_work(→ completed, actor=Human)
work_cancel(work_id)                → transition_work(→ cancelled, actor=Human)
work_events(work_id, limit)         → events[]（仅 work.*，倒序）
receipt_probe(receipt_ref)          → { exists: bool, path: <AppData>/agent/runs/<id>.jsonl }
```

- 全部走现有 command transaction 模式；UI 只做 projection；
- `list_works` 返回完整 WorkRecord（intent 可能很大，M2 冲刺评估是否需要 list 专用轻量投影列，否则直接复用 WORK_COLUMNS）。

## 5. 兼容入口（UX_MODEL §2 要求）

> "旧 Agent 列表在迁移期间保留为兼容路径，不能先删除再迁移数据。"

- Agent 区第一版 = Work 三组（works 表为主源）；
- 仍存在但没有 Work 记录的 legacy Agent Work（尚未走 bridge 的旧条目）：
  折叠入口「旧 Agent 工作」→ 现有 Agent Work 只读打开路径；
- 不隐藏、不删除旧入口，直到 bridge 覆盖验证完成（03 Rule 5）。

## 6. 刷新策略（建议）

1. 打开 Agent 区 / 切换回来时查询；
2. 存在 `queued/running` 时 5s 轻轮询仅刷新该组；
3. 手动刷新按钮；
4. 不做实时推送（无 push 基础设施；M1 无事件订阅协议）。

## 7. 开放问题（需 M2 冲刺前裁决，均未在本文件擅自决定）

1. 分组映射是否采纳 §2.1（failed/cancelled 归"最近完成" vs 独立"已结束"组）；
2. 操作按钮范围（§3.2）是否最小化到"接受完成 + 取消"；
3. legacy 条目折叠入口的样式与归组；
4. `list_works` 是否需要轻量投影（intent 不参与列表查询）；
5. 轮询间隔与上限（防过度设计）。
