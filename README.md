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
- 不使用数据库、账号、云同步、React、Vite、Node 服务或 localhost。

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

工作区里**任意 Markdown 文档**都可以写批注（不区分来源、不按章节拆分），一键汇总成总笔记：

- 工具栏 `批注` 打开右侧批注栏，当前文档整篇一篇批注（支持 Markdown），650ms 防抖自动保存。
- 批注存放在工作区 `批注/` 文件夹，**按原文路径镜像**（`docs/ch01.md` → `批注/docs/ch01.md`），随 git 同步、可全文搜索、可手工打开编辑。
- 每篇批注文件只标注**来源**（`> 来源：`原文件相对路径）与**时间**（`> 时间：`最近保存时刻）：

  ```markdown
  # 批注：ch01-llm-wiki-是什么
  > 来源：`ch01-llm-wiki-是什么.md`
  > 时间：2026-08-10 17:20

  读完后记：wiki 把知识编译一次，比 RAG 每次现查更省。
  ```

- `汇总批注` 一键把全部批注合并到工作区根目录 `批注汇总.md`（按相对路径顺序、只纳非空、自动生成可重复覆盖；文件菜单里也有入口）。
- 批注清空即删除侧车文件（撤销批注）；批注文件与汇总文件自身不能再批注；删除 `批注/` 文件夹即可整体撤销功能。

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
│   ├── app.js          # 文件树、搜索、同步、自动保存、Markdown 预览、布局、批注
│   └── style.css       # 沉浸式双栏视觉 + 批注栏
└── src-tauri/
    ├── src/lib.rs      # 文件夹/文档选择 + 工作区边界 + 文件读写 + 12 个 command
    ├── src/annotate.rs # 批注侧车：读写 `批注/`（标注来源/时间）+ 汇总 `批注汇总.md`
    ├── src/indexer.rs  # SQLite FTS5 侧车索引（rusqlite bundled）
    ├── src/sync.rs     # git 同步引擎（最后写入者胜 + 单测 + 板子集成测试）
    ├── capabilities/   # 最小 IPC 权限
    └── tauri.conf.json # 直接把 ../ui 嵌入桌面应用
```

### 测试

```bash
cd src-tauri
cargo test                # 21 个默认测试；另有 2 个按需调试/网络测试
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

AI 写作、云同步（远程托管）、数据库内容存储、标签、插件系统、富文本、复杂设置页、本地图片代理。

> 注意：v0.2 原型新增的 SQLite 索引与 git 同步属于本分支实验内容；索引可随时删除重建，同步可随时通过 `git remote remove origin` 解除。
