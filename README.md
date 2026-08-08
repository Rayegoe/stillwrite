# Stillwrite

一个极简、本地优先、**运行时零端口**的 Markdown 沉浸式写作工具。

## 第一版只做这些

- 左侧：可隐藏、可拖动缩放的 Markdown 文件树。
- 中间：纯 Markdown 源码写作区。
- 右侧：实时阅读区。
- 写作 / 双栏 / 阅读三种模式，写作区和阅读区都能独占主窗口。
- 主双栏比例可拖动；侧边栏宽度可拖动。
- 打开真实本地文件夹，直接读写 `.md` / `.markdown`。
- 650ms 防抖自动保存，支持 `Ctrl+S`。
- 记住上次目录、侧栏宽度、双栏比例和视图模式。
- 不使用数据库、账号、云同步、React、Vite、Node 服务或 localhost。

## 为什么不是单 HTML `file://`

浏览器的目录读写 API 仍受安全上下文和兼容性限制。Stillwrite 使用 Tauri 的桌面 WebView + Rust 文件层，前端文件直接嵌入桌面应用，不需要浏览器目录授权模型。

## 快捷键

| 快捷键 | 功能 |
| --- | --- |
| Ctrl/Cmd + O | 打开文件夹 |
| Ctrl/Cmd + N | 新建 Markdown |
| Ctrl/Cmd + S | 立即保存 |
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

构建产物通常位于：

```text
src-tauri/target/release/bundle/appimage/
src-tauri/target/release/bundle/deb/
```

## 源码结构

```text
stillwrite/
├── ui/
│   ├── index.html      # 静态界面
│   ├── app.js          # 文件树、自动保存、Markdown 预览、布局
│   └── style.css       # 沉浸式双栏视觉
└── src-tauri/
    ├── src/lib.rs      # 目录选择 + 工作区边界 + 文件读写
    ├── capabilities/   # 最小 IPC 权限
    └── tauri.conf.json # 直接把 ../ui 嵌入桌面应用
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

AI 写作、云同步、数据库、标签、插件系统、Git、富文本、复杂设置页、本地图片代理。
