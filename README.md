# Stillwrite

一个极简、本地优先、**运行时零端口**的 Markdown 沉浸式写作工具。

## 第一版只做这些

- 左侧：可隐藏、可拖动缩放的 Markdown 文件树。
- 中间：纯 Markdown 源码写作区。
- 右侧：实时阅读区。
- 写作 / 双栏 / 阅读三种模式，写作区和阅读区都能独占主窗口。
- 主双栏比例可拖动；侧边栏宽度可拖动。
- 打开真实本地文件夹，直接读写 `.md` / `.markdown`。
- 文件菜单支持打开文件夹、直接打开 Markdown、新建、保存和刷新目录。
- 650ms 防抖自动保存，支持 `Ctrl+S`。
- 记住上次目录、侧栏宽度、双栏比例和视图模式。
- 正文仍是唯一内容源；SQLite 只做可删除重建的 Workspace / Library 侧车索引。
- 不使用账号、React、Vite、Node 服务或 localhost。

## v0.2 原型：全文搜索 + git 同步（本分支新增）

**文件仍是唯一内容源**。SQLite 只是本地侧车索引，可随时删除重建；同步走 git（最后写入者胜）。

### 全文搜索

- 侧栏顶部搜索框，FTS5 全文索引（标题 + 正文）。
- 打开工作区后先显示文件树，再在后台增量索引；保存 / 同步后自动更新。
- 索引存放于应用数据目录（`~/.local/share/com.stillevo.stillwrite/workspaces/<hash>/index.db`），不进入工作区、不参与 git。

### git 同步（最后写入者胜）

- 工具栏 `⟳ 同步`：自动 `commit → fetch → merge → push`。
- 冲突时按 **merge 前的工作区 mtime 与远端提交时间** 逐文件裁决，较新者胜。
- 首次手动同步成功后，自动保存（含 `Ctrl+S`）4 秒后自动同步。
- 同步使用**独立 `sync` remote**，绝不改写工作区已有的 `origin`（例如工作区若关联了 GitHub，两套 remote 互不干扰）。
- 需要系统装有 `git` 并配置过身份（`git config --global user.name / user.email`）。

#### 在远程设备上建同步仓库（一次性）

```bash
# 板子上：
git init --bare ~/stillwrite.git
git symbolic-ref HEAD refs/heads/main   # 保证 HEAD 指向 main

# 本机：验证 SSH 免密（可选，方便测试）
ssh user@example.invalid "git --version"
```

首次在工作区点 `⟳ 同步` 时，应用会自动执行 `git init` 并配置独立 `sync` remote（默认 `user@example.invalid:~/stillwrite.git`，可用 `localStorage['stillwrite.remote']` 覆盖）。实际使用时请替换为自己的远程地址。若工作区已有指向同一地址的 `origin`，则直接复用 `origin`。

## v0.3 原型：批注 + 自动汇总（本分支新增）

工作区里**任意 Markdown 文档**都可以按字句或段落写批注，并一键汇总成总笔记：

- 在写作区选中字句后点 `＋ 批注`；没有选区时，右栏 `新批注` 会取光标所在段落。阅读区选中文字也会出现轻量 `＋ 批注` 按钮。
- 阅读区用高亮和编号标出原文，点击编号展开右侧批注栏并定位对应卡片；点卡片里的原文可回到正文位置。
- 每篇文档可有多条批注，右栏仅显示类型、原文、时间和批注正文；正文编辑仍采用 650ms 防抖自动保存。
- 汇总时保留每条批注自己的更新时间，不再把同一文档内的批注统一成侧车文件最后保存时间。
- 汇总中的来源路径可点击回到原 Markdown；正文中的普通 URL、项目内路径、完整文件名，以及唯一对应某个 Markdown 文件的名称会自动变成链接。
- 批注存放在工作区 `批注/` 文件夹，**按原文路径镜像**（`docs/ch01.md` → `批注/docs/ch01.md`），随 git 同步、可全文搜索、可手工打开编辑。
- 侧车仍是普通 Markdown：文件头标注**来源**与**最近保存时间**，每条批注保留可读的原文引用和正文；结构锚点用 Markdown 注释保存。
- 已有的旧版整篇自由文本批注会无损显示为一条“全文批注”，首次编辑后自动升级格式。

  ```markdown
  # 批注：ch01-llm-wiki-是什么
  > 来源：`ch01-llm-wiki-是什么.md`
  > 时间：2026-08-10 17:20

  <!-- stillwrite-annotation:… -->
  > 原文（字句）：
  > wiki 把知识编译一次

  这个说法比“每次现查”更准确，可以保留。
  <!-- /stillwrite-annotation -->
  ```

- `汇总批注` 一键把全部批注合并到工作区根目录 `批注汇总.md`（按相对路径顺序、只纳非空、自动生成可重复覆盖；文件菜单里也有入口）。
- 批注清空即删除侧车文件（撤销批注）；批注文件与汇总文件自身不能再批注；删除 `批注/` 文件夹即可整体撤销功能。

## v0.4 原型：Library / 资料库

Library 是 Workspace 之外的只读资料层，不是 RSS 阅读器，也不会把外部资料塞进工作区文件树：

- 侧栏切换到 `资料`，点击 `＋` 注册一个外部 Markdown 目录（资料源不能与当前工作区重叠）。
- StillWrite 只扫描已注册目录，按 `mtime + size` 增量更新独立的 `library/index.db`；正文继续留在原始 `.md` 文件。
- Library 使用 SHA-256 对规范化后的 Markdown 内容去重；重复文件仍保留在原目录，但搜索结果只显示一个 corpus item。
- 搜索结果勾选“引用”即可加入当前引用篮。打开资料时正文仍只读，但可以使用与 Workspace 完全相同的选取、批注、高亮和批注面板。
- Library 批注使用同一套 Markdown 批注格式，写入 StillWrite 应用数据目录，不修改外部资料，也不进入 Workspace 文件树、git 同步或 Workspace 汇总。
- 发送 Agent 请求时，StillWrite 才读取当前引用篮中的资料正文，并把它们作为有边界的只读引用传入；勾选原文不会自动携带批注。
- 刷新会清理已删除文件的索引；SQLite 可直接删除，下一次刷新会从资料源重建。

Library 的目标边界是：`Library ≠ Workspace`、`Index ≠ Content`。引用篮是当前会话内的临时选择，不是持久化对象；发送时读取当前资料正文。当前版本暂不包含 Embedding、标签、自动分类和摘要；RSS 网络抓取见下方 v0.7 小节。

## v0.5 原型：Agent Work / Agent 工作

Agent 不是右侧常驻聊天框，而是由文档选区触发、最终落成 Markdown 的工作文档：

- 侧栏现在有 `文件 / 资料 / Agent` 三个平面；`Agent` 列表只显示工作标题、来源选区和运行状态，不把 Workspace 文件树或 Library 展开成第二棵目录树。
- 在阅读区选中文字后，浮层会同时提供 `＋批注` 与 `问 Agent`。工具栏里的 `问 Agent` 也可围绕当前段落发起请求；`Agent` 侧栏的 `＋` 用于新建独立工作。
- Agent 结果保存为当前 Workspace 对应的独立 Markdown 工作文档，正文在 StillWrite 应用数据目录，不进入 Workspace 文件树、git 或 Library。打开后与其他文档共用同一个编辑器、阅读区、选区、高亮和批注系统。
- Agent 工作的来源选区、请求和 Pi session 相对引用只保存在最小 JSON 侧车；正文仍是 Markdown。工作运行时列表显示状态，完成后用户点击列表项再打开结果，不打断当前写作。
- StillWrite 通过本机 Pi 的 `--mode rpc` 持久进程运行 Agent；一个 Workspace 对应一个 Pi 进程和独立 session 目录。流式预览只留在 Agent 列表，只有收到权威最终文本后才保存一次 Agent Work。
- Pi 只能通过 StillWrite 显式加载的 `workspace_list`、`workspace_read`、`workspace_search` 三个只读工具访问当前 Workspace；它没有 shell、编辑或写文件工具。当前源文档不会被 Pi 修改。

安装 Pi（也可以使用已安装的 standalone `pi` 可执行文件）：

```bash
npm install -g @mariozechner/pi-coding-agent
pi --version
```

provider、模型和认证在 Pi 外部配置，StillWrite 不保存凭据。必要时可以设置以下环境变量：

- `STILLWRITE_PI_EXECUTABLE`：明确的 Pi 可执行文件路径；否则按 `PATH` 中的 `pi` 查找。
- `STILLWRITE_PI_AGENT_DIR`：Pi 的独立配置目录。
- `STILLWRITE_PI_PROVIDER`、`STILLWRITE_PI_MODEL`、`STILLWRITE_PI_THINKING`：可选的启动参数覆盖。

## v0.6 原型：Ambient Related / 关联

`关联` = 当前作品周围的静默本地材料准备。

- 在可编辑的 Workspace Markdown 文档中写下标题或开头后，StillWrite 会从 H1、文件名和（标题信号不足时的）首段提取 3–6 个关键词/短语，分别检索 Workspace / Library，并按多关键词共现、短语命中和标题命中重排，最多展示 5 条材料。
- 关联使用独立的 trigram 侧车索引；普通手工搜索仍保留原有 unicode61 语义，二字中文关键词另有正文 LIKE 兜底。
- 结果出现在右侧支持栏的 `关联` 视图，分类为 `过去的批注`、`工作区` 和 `资料`；普通后台刷新不自动打开面板、不抢焦点、不改写正文，点击结果后才打开原始材料。
- 在写作区或阅读区选中字句/段落后，浮层提供 `批注`、`问 Agent` 和 `＋关联`；`＋关联` 会把选区作为当前作品的临时检索补充，最多保留最近 3 个选区，并立即展示更新后的关联材料。
- 每张关联卡片都可以在 `引用` 旁点击 `☆ 固定`；固定卡片会保存到本机、优先显示，并且不受关联结果 Top 5 限制。固定状态按 Workspace 保存，不写入 Markdown 或 git。
- 关联只使用本地搜索和现有 Markdown / 索引，不调用模型、不访问网络、不自动写入文档；除用户主动固定的卡片状态外，不产生持久化关系数据。
- Library 结果可以由用户显式加入当前 `引用`；关联本身不会自动加入引用篮。查看 Library 或 Agent Work 时，关联结果会清空。

## v0.7 原型：RSS / Atom 作为资料源（本分支新增）

RSS 只是一个 **Library 输入适配器**，不是第四个顶层平面，也不是新阅读器：

- `资料 → ＋ → 添加 RSS` 粘贴一个 RSS / Atom URL，或 `导入 OPML` 批量导入（`*.opml / *.xml`，merge 语义：已存在 URL 跳过，坏项不影响其他项）。每个源只有 `刷新` 与 `删除` 两个操作，不做已读/未读、文件夹、标签或推荐。
- 抓取发生在 Rust 后端（`feeds.rs`）：ETag / Last-Modified 条件请求、连接与读取超时、最多 5 次重定向、5 MiB 响应体上限、明确的 User-Agent。刷新全部源时最多 4 路并发，单个源失败不会阻塞其他源。
- 条目物化为本地 Markdown：`<AppData>/library/RSS/<feed-id>/<YYYY-MM-DD>__<标题>__<短id>.md`。内容优先使用 Feed 自带全文；只有摘要时保存摘要并标注原文链接，不伪装成全文。Feed 正文是 HTML 时由成熟 `html2md` 转换；script / style / iframe 等可执行内容不进入 Markdown。enclosure 只保留为普通链接，不下载。
- 物化目录作为普通 Library source 注册（名为 `RSS`），随后走现有 Library 索引 / 搜索 / 只读阅读 / 批注 / 引用篮 / Ask Agent / 关联。`最近 RSS` 直接读现有 `library_documents`，不建第二套全文库或批注库。
- `rss-sources.json` 只保存用户订阅（id / name / url）；ETag、上次抓取时间、错误等派生状态在 `rss-fetch-state.json`，删除后只导致下一次完整抓取，不丢订阅。删除源会删除本地 Markdown 缓存并刷新索引，但**保留**已有批注。
- 手动刷新是主路径；打开 `资料` 面板时若距上次全局刷新超过 30 分钟，会在后台静默触发一次。
- **不做**：网页全文抓取 / headless browser、EPUB / Kindle 导出、已读未读、定时 daemon、RSS 专属数据库、RSS 专属批注或 Agent 管线。summary-only feed 的影响留待真实使用数据再决定是否引入正文提取。

## 为什么不是单 HTML `file://`

浏览器的目录读写 API 仍受安全上下文和兼容性限制。Stillwrite 使用 Tauri 的桌面 WebView + Rust 文件层，前端文件直接嵌入桌面应用，不需要浏览器目录授权模型。

## 快捷键

| 快捷键 | 功能 |
| --- | --- |
| Ctrl/Cmd + O | 打开文件夹 |
| Ctrl/Cmd + Shift + O | 打开 Markdown 文档 |
| Ctrl/Cmd + N | 新建 Markdown |
| Ctrl/Cmd + S | 立即保存 |
| Ctrl/Cmd + R | 刷新目录 |
| Ctrl/Cmd + B | 显示/隐藏文件栏 |
| Ctrl/Cmd + Shift + M | 新建字句/段落批注 |
| Ctrl/Cmd + 1 | 仅写作 |
| Ctrl/Cmd + 2 | 双栏 |
| Ctrl/Cmd + 3 | 仅阅读 |

## Linux / Ubuntu 构建

Stillwrite 前端没有 npm 依赖。只需要 Rust + Tauri CLI。

### 1. Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 2. Tauri Linux 系统依赖

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### 3. 安装 Tauri CLI

```bash
cargo install tauri-cli --version '^2.0.0' --locked
```

### 4. 直接开发运行（无 localhost）

```bash
cd src-tauri
cargo tauri dev
```

### 5. 构建 AppImage / deb

```bash
cd src-tauri
cargo tauri build
```

日常 Ubuntu 安装优先只构建体积更小的 deb：

```bash
cargo tauri build --bundles deb
```

构建产物通常位于：

```text
src-tauri/target/release/bundle/appimage/
src-tauri/target/release/bundle/deb/
```

## 资源占用说明

- 源码不足 1 MB；开发目录变大主要来自 Rust/Tauri 编译缓存，不是写作数据。
- 优化前 `target` 约 7.7 GB；关闭调试符号、增量缓存和移动端专用重复库后，冷启动测试构建约 1.2 GB，同时保留测试与 release 构建约 2.2 GB。
- release 主程序由约 14 MB 降至 5.5 MB；deb 由约 4.1 MB 降至 2.2 MB，安装后约 5.6 MB（WebKit/GTK 由系统共享）。
- AppImage 为可移植性自带 GTK/WebKit 运行库，仍会接近 80 MB；在 Ubuntu 上优先使用 deb。
- WebKit 进程显示的数十 GB `VSZ` 是预留虚拟地址空间，不是实际 RAM；判断内存请看 `RSS` 或 `PSS`。Stillwrite 还使用操作系统文件锁避免重复实例叠加内存。

需要立即回收所有可再生编译缓存时：

```bash
cd src-tauri
cargo clean
```

## 源码结构

```text
stillwrite/
├── ui/
│   ├── index.html      # 静态界面（含搜索框、同步按钮、批注栏）
│   ├── annotations.js  # 结构化批注编解码、选区/段落捕获与锚点恢复
│   ├── annotations.test.js # 批注格式与锚点单元测试
│   ├── document-links.js # URL 与项目内 Markdown 名称/路径识别和跳转
│   ├── document-links.test.js # 内外链接识别与相对路径解析测试
│   ├── agent-events.js # Agent 流式事件归约（运行态，不落盘）
│   ├── agent-events.test.js # Agent 事件归约测试
│   ├── app.js          # 文件树、Library、搜索、关联、引用篮、同步、预览、布局、批注
│   └── style.css       # 沉浸式双栏视觉 + 批注栏 + 资料库
└── src-tauri/
    ├── src/lib.rs      # 文件夹/文档选择 + 工作区边界 + 文件读写 + Tauri command 注册
    ├── src/annotate.rs # 批注侧车：读写 `批注/`（标注来源/时间）+ 汇总 `批注汇总.md`
    ├── src/pi_agent.rs # Pi launcher、持久 JSONL RPC、流式事件、取消与 Workspace 生命周期
    ├── src/agent_work.rs # Agent Markdown 工作文档与来源/运行侧车
    ├── src/indexer.rs  # SQLite FTS5 侧车索引（rusqlite bundled）
    ├── src/library.rs   # 外部 Markdown 资料源、增量索引、SHA-256 去重、只读读取
    ├── src/feeds.rs    # RSS/Atom 源：抓取 + 解析 + 物化 Markdown + OPML 导入 + 单测
    ├── src/sync.rs     # git 同步引擎（最后写入者胜 + 单测 + 板子集成测试）
    ├── resources/pi/  # StillWrite system prompt 与只读 Workspace 工具扩展
    ├── capabilities/   # 最小 IPC 权限
    └── tauri.conf.json # 直接把 ../ui 嵌入桌面应用
```

### 测试

```bash
node ui/annotations.test.js # 结构化批注前端单元测试
node ui/document-links.test.js # 项目内文档链接与 URL 单元测试
node ui/agent-events.test.js # Agent 流式事件归约与结束状态测试
cd src-tauri
cargo test                # 默认单测与批注流程测试；另有 2 个按需调试/网络测试
cargo test -- --ignored live   # 需要配置的远程设备在线：真实跨设备推送/拉取/冲突收敛
```

## 当前 Markdown 支持

为保持零前端依赖，当前阅读区内置一个小型安全渲染器，支持：

- H1–H6
- 段落
- 粗体 / 斜体 / 删除线 / 行内代码
- 有序 / 无序列表
- 引用
- 分隔线
- fenced code block
- http/https/mailto/相对链接

第二阶段如果需要完整 CommonMark/GFM，再将解析器替换为本地打包的成熟 Markdown parser；这不会改变 Tauri 的零端口架构。

## 当前刻意不做

云同步（远程托管）、数据库内容存储、标签、插件系统、富文本、复杂设置页、本地图片代理。

> 注意：v0.2 原型新增的 SQLite 索引与 git 同步属于本分支实验内容；索引可随时删除重建，同步可随时通过 `git remote remove origin` 解除。
