//! 批注侧车。
//!
//! 设计遵循应用哲学「文件是唯一内容源」：批注就是工作区里的普通 Markdown，
//! 按原文相对路径镜像放在 `批注/` 文件夹下，随 git 同步、可全文搜索、可手工打开编辑。
//!
//! 工作区里任意 Markdown 文档都能批注（不区分来源、不按章节拆分），
//! 每篇批注是一个文件，元信息只标注来源与时间：
//! ```text
//! # 批注：<文档名>
//! > 来源：`<相对路径>`
//! > 时间：2026-08-10 17:20
//!
//! <批注正文>
//! ```
//!
//! 汇总文件：工作区根目录 `批注汇总.md`，由「汇总批注」一键生成，手工修改会在下次汇总被覆盖。

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const ANNOTATE_DIR: &str = "批注";
pub const AGGREGATE_NAME: &str = "批注汇总.md";
const STRUCTURED_HEADER: &str = "<!-- stillwrite-annotations:v1 -->";
const ITEM_PREFIX: &str = "<!-- stillwrite-annotation:";
const ITEM_PREFIX_END: &str = " -->";
const QUOTE_END: &str = "<!-- /stillwrite-quote -->";

#[derive(Serialize)]
pub struct AnnotationData {
    /// 原文绝对路径（供前端与当前打开的文档匹配）
    pub doc_path: String,
    /// 源文档文件名去扩展名（侧车 `# 批注：<title>` 与汇总 `## <title>` 用）
    pub title: String,
    /// 最近一次批注时间 `YYYY-MM-DD HH:MM`
    pub updated_at: String,
    /// 批注正文
    pub body: String,
}

#[derive(Serialize)]
pub struct AggregateResult {
    /// 汇总文件绝对路径
    pub path: String,
    /// 纳入汇总的批注篇数（只统计非空）
    pub count: usize,
}

/// 源文档相对路径 → 批注侧车相对路径（`批注/<rel>`，目录结构镜像）。
pub fn annotation_rel(rel: &Path) -> PathBuf {
    PathBuf::from(ANNOTATE_DIR).join(rel)
}

/// 判断某个相对路径是否可以作为批注的源文档。
/// 批注文件（`批注/**`）与汇总文件（`批注汇总.md`）自身不能再被批注。
pub fn is_annotation_target(rel: &Path) -> bool {
    let rel = rel.to_string_lossy().replace('\\', "/");
    !(rel == ANNOTATE_DIR
        || rel.starts_with(&format!("{ANNOTATE_DIR}/"))
        || rel == AGGREGATE_NAME)
}

/// 解析侧车内容 → (最近批注时间, 正文)。
/// 跳过 `# 批注：` 头、`> 来源：` / `> 时间：` 元信息行与开头空行，其余为正文。
/// 兼容旧版（按章节）格式：`> 来源：… · 更新于 <时间>` 单行合并写法。
pub fn parse_note(content: &str) -> (String, String) {
    let mut updated_at = String::new();
    let mut body = Vec::new();
    let mut started = false;
    for line in content.lines() {
        if let Some(t) = line.strip_prefix("> 时间：") {
            updated_at = t.trim().to_string();
            continue;
        }
        if line.starts_with("> 来源：") {
            // 旧版把更新时间合并在来源行：`> 来源：`x.md` · 更新于 <时间>`
            if let Some(t) = line.split("· 更新于 ").nth(1) {
                updated_at = t.trim().to_string();
            }
            continue;
        }
        if !started {
            if line.starts_with("# 批注：") {
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            started = true;
        }
        body.push(line);
    }
    (updated_at, body.join("\n").trim().to_string())
}

/// 判断侧车是否为旧版（按章节）格式：来源行带 `· 更新于`，且正文按 `## 章节` 分段。
fn is_old_format(content: &str) -> bool {
    content.lines().any(|l| l.contains("· 更新于"))
}

/// 旧版（按章节）格式 → 新版（按文档）格式。
/// 旧格式的每个 `## 章节` 段合并进正文（保留章节标记，读者仍能看到结构），
/// 更新时间从 `> 来源：… · 更新于 <时间>` 中提取。
pub fn migrate_old_format(content: &str) -> (String, String) {
    let mut updated_at = String::new();
    let mut body = Vec::new();
    let mut started = false;
    for line in content.lines() {
        if let Some(t) = line.split("· 更新于 ").nth(1) {
            updated_at = t.trim().to_string();
            continue;
        }
        if line.starts_with("# 批注：") || line.starts_with("> 来源：") {
            continue;
        }
        if !started {
            if line.trim().is_empty() {
                continue;
            }
            started = true;
        }
        body.push(line);
    }
    (updated_at, body.join("\n").trim().to_string())
}

/// 渲染侧车 Markdown（来源 + 时间 + 正文）。
pub fn render_note(title: &str, doc_rel: &str, body: &str, updated_at: &str) -> String {
    let mut out = String::from("# 批注：");
    out.push_str(title);
    out.push_str("\n\n> 来源：`");
    out.push_str(doc_rel);
    out.push_str("`\n> 时间：");
    out.push_str(updated_at);
    out.push_str("\n\n");
    out.push_str(body.trim());
    out.push('\n');
    out
}

/// 汇总条目：文档标题、来源相对路径、批注时间、正文。
#[derive(Debug)]
pub struct AggregateEntry {
    pub title: String,
    pub source_rel: String,
    pub updated_at: String,
    pub body: String,
}

#[derive(Deserialize)]
struct StructuredAnnotationMeta {
    #[serde(rename = "updatedAt", default)]
    updated_at: String,
    #[serde(rename = "createdAt", default)]
    created_at: String,
}

/// 结构化批注把每条时间保存在 item 元数据里。汇总时将它还原成可读 Markdown，
/// 避免所有批注都错误地显示为侧车文件最后一次保存的时间。
fn aggregate_body_with_item_times(body: &str) -> (String, bool) {
    if !body.contains(STRUCTURED_HEADER) {
        return (body.to_string(), false);
    }

    let mut out = String::new();
    let mut pending_time = None;
    let mut found_item_time = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(encoded) = trimmed
            .strip_prefix(ITEM_PREFIX)
            .and_then(|value| value.strip_suffix(ITEM_PREFIX_END))
        {
            pending_time = URL_SAFE_NO_PAD
                .decode(encoded)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<StructuredAnnotationMeta>(&bytes).ok())
                .map(|meta| {
                    if meta.updated_at.is_empty() {
                        meta.created_at
                    } else {
                        meta.updated_at
                    }
                })
                .filter(|time| !time.is_empty());
        }

        // 汇总里引用块本身已经表达“原文”，无需重复显示类型说明。
        if trimmed.starts_with("> 原文（") && trimmed.ends_with("）：") {
            continue;
        }

        out.push_str(line);
        out.push('\n');
        if trimmed == QUOTE_END {
            if let Some(time) = pending_time.take() {
                out.push_str("\n> 批注时间：");
                out.push_str(&time);
                out.push_str("\n");
                found_item_time = true;
            }
        }
    }
    (out.trim().to_string(), found_item_time)
}

fn push_source_link(out: &mut String, source_rel: &str) {
    let label = source_rel.replace('\\', "\\\\").replace(']', "\\]");
    let href = source_rel.replace('>', "%3E");
    out.push('[');
    out.push_str(&label);
    out.push_str("](<");
    out.push_str(&href);
    out.push_str(">)");
}

/// 把汇总条目渲染成 `批注汇总.md` 的内容。
pub fn render_aggregate(ws_name: &str, entries: &[AggregateEntry]) -> String {
    let mut out = String::from("# 《");
    out.push_str(ws_name);
    out.push_str("》批注汇总\n\n> 此文件由「汇总批注」自动生成，手工修改会在下次汇总时被覆盖。\n\n> 共 ");
    out.push_str(&entries.len().to_string());
    out.push_str(" 篇批注\n\n");
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            out.push_str("---\n\n");
        }
        out.push_str("## ");
        out.push_str(&entry.title);
        out.push_str("\n\n> 来源：");
        push_source_link(&mut out, &entry.source_rel);
        let (aggregate_body, has_item_times) = aggregate_body_with_item_times(&entry.body);
        if !has_item_times && !entry.updated_at.is_empty() {
            out.push_str(" · 批注于 ");
            out.push_str(&entry.updated_at);
        }
        out.push_str("\n\n");
        out.push_str(&aggregate_body);
        out.push_str("\n\n");
    }
    if entries.is_empty() {
        out.push_str("_还没有任何批注。打开文档点工具栏「批注」写笔记，再回到这里汇总。_\n\n");
    }
    out
}

/// Unix 秒 → `YYYY-MM-DD HH:MM`（本地时区；非 unix 平台回退到 UTC）。
pub fn format_timestamp(secs: u64) -> String {
    #[cfg(unix)]
    {
        let t = secs as libc::time_t;
        let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
        unsafe {
            libc::localtime_r(&t, tm.as_mut_ptr());
        }
        let tm = unsafe { tm.assume_init() };
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min
        )
    }
    #[cfg(not(unix))]
    {
        // Howard Hinnant civil_from_days 逆算法（UTC）
        let days = (secs / 86400) as i64;
        let rem = secs % 86400;
        let (hh, mm) = ((rem / 3600) as i64, ((rem % 3600) / 60) as i64);
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}")
    }
}

/// 源文档文件名去扩展名 → 侧车 `# 批注：` 标题。
pub fn title_of(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.rsplit_once('.').map(|(stem, _)| stem.to_string()))
        .unwrap_or_else(|| {
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "文档".to_string())
        })
}

/// 遍历工作区里可作为批注源文档的 Markdown（排除 `批注/` 目录与 `批注汇总.md` 自身）。
fn walk_sources(root: &Path) -> Vec<PathBuf> {
    fn is_markdown(p: &Path) -> bool {
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown"))
            .unwrap_or(false)
    }
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == ANNOTATE_DIR || name == AGGREGATE_NAME {
                continue;
            }
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                if matches!(name.as_str(), "target" | "node_modules") {
                    continue;
                }
                stack.push(path);
            } else if ft.is_file() && is_markdown(&path) {
                out.push(path);
            }
        }
    }
    out.sort_by(|a, b| {
        let ra = a.strip_prefix(root).unwrap_or(a);
        let rb = b.strip_prefix(root).unwrap_or(b);
        ra.to_string_lossy().cmp(&rb.to_string_lossy())
    });
    out
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 读取某源文档的批注（侧车不存在时返回空正文）。
/// 旧版（按章节）格式的侧车会被就地升级为新版（按文档）格式。
pub fn read_annotation_data(root: &Path, doc_path: &Path) -> Result<AnnotationData, String> {
    let rel_str = rel_of(root, doc_path);
    let rel = Path::new(&rel_str);
    let sidecar = root.join(annotation_rel(rel));
    if !sidecar.is_file() {
        return Ok(AnnotationData {
            doc_path: doc_path.to_string_lossy().to_string(),
            title: title_of(doc_path),
            updated_at: String::new(),
            body: String::new(),
        });
    }
    let content = fs::read_to_string(&sidecar).map_err(|e| format!("读取批注失败: {e}"))?;
    let (updated_at, body) = if is_old_format(&content) {
        let (updated_at, body) = migrate_old_format(&content);
        // 就地升级为新版格式，后续读写一致
        if let Some(parent) = sidecar.parent() {
            let _ = crate::create_dir_all_inside(root, parent);
        }
        let upgraded = render_note(&title_of(doc_path), &rel_str, &body, &updated_at);
        let _ = crate::atomic_write(&sidecar, &upgraded);
        (updated_at, body)
    } else {
        parse_note(&content)
    };
    Ok(AnnotationData {
        doc_path: doc_path.to_string_lossy().to_string(),
        title: title_of(doc_path),
        updated_at,
        body,
    })
}

/// 保存某源文档的批注（镜像路径下建目录，原子写；正文为空则删除侧车 = 撤销批注）。
/// 返回侧车绝对路径（可能已删除），供调用方更新全文索引。
pub fn save_annotation(root: &Path, doc_path: &Path, body: &str) -> Result<PathBuf, String> {
    let rel_str = rel_of(root, doc_path);
    let rel = Path::new(&rel_str);
    let sidecar = root.join(annotation_rel(rel));

    if body.trim().is_empty() {
        if sidecar.is_file() {
            fs::remove_file(&sidecar).map_err(|e| format!("删除批注失败: {e}"))?;
        }
        return Ok(sidecar);
    }

    if let Some(parent) = sidecar.parent() {
        crate::create_dir_all_inside(root, parent)?;
    }
    let updated_at = format_timestamp(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    );
    let content = render_note(&title_of(doc_path), &rel_of(root, doc_path), body, &updated_at);
    crate::atomic_write(&sidecar, &content)?;
    Ok(sidecar)
}

/// 汇总所有批注到工作区根目录 `批注汇总.md`，返回结果与汇总文件路径。
/// 只纳入非空批注；顺序与文件树一致（按相对路径排序）。
/// 旧格式侧车经 read_annotation_data 自动升级后再汇入。
pub fn aggregate(root: &Path) -> Result<AggregateResult, String> {
    let mut entries = Vec::new();
    for source in walk_sources(root) {
        let rel = rel_of(root, &source);
        let Ok(data) = read_annotation_data(root, &source) else {
            continue;
        };
        if data.body.trim().is_empty() {
            continue;
        }
        entries.push(AggregateEntry {
            title: data.title,
            source_rel: rel,
            updated_at: data.updated_at,
            body: data.body,
        });
    }

    let ws_name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "工作区".to_string());
    let content = render_aggregate(&ws_name, &entries);
    let aggregate_path = root.join(AGGREGATE_NAME);
    crate::atomic_write(&aggregate_path, &content)?;

    Ok(AggregateResult {
        path: aggregate_path.to_string_lossy().to_string(),
        count: entries.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sw-annotate-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parse_note_skips_meta_and_keeps_body() {
        let content = "# 批注：ch01\n\n> 来源：`ch01.md`\n> 时间：2026-08-10 12:00\n\n这是第一条批注。\n\n第二段。\n";
        let (updated_at, body) = parse_note(content);
        assert_eq!(updated_at, "2026-08-10 12:00");
        assert_eq!(body, "这是第一条批注。\n\n第二段。");
    }

    #[test]
    fn render_then_parse_roundtrip() {
        let md = render_note("ch01", "docs/ch01.md", "批注甲\n\n第二段", "2026-08-10 12:00");
        let (updated_at, body) = parse_note(&md);
        assert_eq!(updated_at, "2026-08-10 12:00");
        assert_eq!(body, "批注甲\n\n第二段");
    }

    #[test]
    fn annotation_rel_mirrors_subdirs() {
        assert_eq!(
            annotation_rel(Path::new("docs/ch01.md")),
            PathBuf::from("批注/docs/ch01.md")
        );
        assert_eq!(
            annotation_rel(Path::new("ch01.md")),
            PathBuf::from("批注/ch01.md")
        );
    }

    #[test]
    fn is_annotation_target_rules() {
        assert!(is_annotation_target(Path::new("ch01.md")));
        assert!(is_annotation_target(Path::new("docs/ch01.md")));
        assert!(is_annotation_target(Path::new("笔记.md")));
        assert!(!is_annotation_target(Path::new("批注/ch01.md")));
        assert!(!is_annotation_target(Path::new("批注/docs/ch01.md")));
        assert!(!is_annotation_target(Path::new(AGGREGATE_NAME)));
        assert!(!is_annotation_target(Path::new("批注")));
    }

    #[test]
    fn save_and_read_roundtrip() {
        let root = tmp_root("save");
        fs::create_dir_all(root.join("docs")).unwrap();
        let doc = root.join("docs/ch01.md");
        fs::write(&doc, "# 第一章\n").unwrap();

        let sidecar = save_annotation(&root, &doc, "读完后记：文件即内容源。").unwrap();
        assert_eq!(sidecar, root.join("批注/docs/ch01.md"));
        assert!(sidecar.is_file());

        let data = read_annotation_data(&root, &doc).unwrap();
        assert_eq!(data.title, "ch01");
        assert_eq!(data.body, "读完后记：文件即内容源。");
        assert!(!data.updated_at.is_empty());
    }

    #[test]
    fn save_empty_body_deletes_sidecar() {
        let root = tmp_root("empty");
        let doc = root.join("a.md");
        fs::write(&doc, "# A\n").unwrap();
        save_annotation(&root, &doc, "旧批注").unwrap();
        assert!(root.join("批注/a.md").is_file());

        let sidecar = save_annotation(&root, &doc, "   ").unwrap();
        assert_eq!(sidecar, root.join("批注/a.md"));
        assert!(!sidecar.exists(), "空正文应删除侧车");
        let data = read_annotation_data(&root, &doc).unwrap();
        assert_eq!(data.body, "");
    }

    #[test]
    fn aggregate_collects_sorted_notes() {
        let root = tmp_root("agg");
        fs::write(root.join("ch10.md"), "# 第十章\n").unwrap();
        fs::write(root.join("ch02.md"), "# 第二章\n").unwrap();

        save_annotation(&root, &root.join("ch10.md"), "结尾的思考").unwrap();
        save_annotation(&root, &root.join("ch02.md"), "中段的思考").unwrap();

        let result = aggregate(&root).unwrap();
        assert_eq!(result.count, 2);
        assert_eq!(result.path, root.join(AGGREGATE_NAME).to_string_lossy().to_string());

        let md = fs::read_to_string(root.join(AGGREGATE_NAME)).unwrap();
        let idx02 = md.find("ch02").unwrap();
        let idx10 = md.find("ch10").unwrap();
        assert!(idx02 < idx10, "应保持相对路径字典序");
        assert!(md.contains("来源：[ch02.md](<ch02.md>)"));
        assert!(md.contains("中段的思考"));
        assert!(md.contains("批注于 "));
    }

    #[test]
    fn aggregate_structured_annotations_use_each_item_time() {
        let meta_one = URL_SAFE_NO_PAD.encode(
            r#"{"id":"one","createdAt":"2026-08-25 09:10","updatedAt":"2026-08-25 09:12"}"#,
        );
        let meta_two = URL_SAFE_NO_PAD.encode(
            r#"{"id":"two","createdAt":"2026-08-25 10:20","updatedAt":"2026-08-25 10:25"}"#,
        );
        let body = format!(
            "{STRUCTURED_HEADER}\n\n{ITEM_PREFIX}{meta_one}{ITEM_PREFIX_END}\n<!-- stillwrite-quote -->\n> 原文（字句）：\n> 第一处原文\n{QUOTE_END}\n\n第一条批注\n<!-- /stillwrite-annotation -->\n\n{ITEM_PREFIX}{meta_two}{ITEM_PREFIX_END}\n<!-- stillwrite-quote -->\n> 原文（段落）：\n> 第二处原文\n{QUOTE_END}\n\n第二条批注\n<!-- /stillwrite-annotation -->"
        );
        let md = render_aggregate(
            "测试工作区",
            &[AggregateEntry {
                title: "测试文档".into(),
                source_rel: "测试文档.md".into(),
                updated_at: "2026-08-25 11:59".into(),
                body,
            }],
        );

        assert!(md.contains("> 批注时间：2026-08-25 09:12"), "{md}");
        assert!(md.contains("> 批注时间：2026-08-25 10:25"), "{md}");
        assert!(!md.contains("原文（字句）"), "{md}");
        assert!(!md.contains("原文（段落）"), "{md}");
        assert!(md.contains("来源：[测试文档.md](<测试文档.md>)"), "{md}");
        assert!(
            !md.contains("批注于 2026-08-25 11:59"),
            "结构化批注不应再使用侧车的最后保存时间: {md}"
        );
    }

    #[test]
    fn aggregate_skips_empty_notes_and_annotate_dir_and_aggregate_itself() {
        let root = tmp_root("agg-skip");
        fs::write(root.join("a.md"), "# A\n").unwrap();
        fs::write(root.join("b.md"), "# B\n").unwrap();

        // a 的批注为空 → 直接删除侧车（等效无批注）；b 写一个只有头部的侧车文件模拟脏数据
        let side = root.join("批注/b.md");
        fs::create_dir_all(side.parent().unwrap()).unwrap();
        fs::write(&side, "# 批注：b\n\n> 来源：`b.md`\n> 时间：2026-08-10 12:00\n\n").unwrap();

        let result = aggregate(&root).unwrap();
        assert_eq!(result.count, 0);

        // 汇总文件自身不被当作源文档（重复汇总幂等）
        let again = aggregate(&root).unwrap();
        assert_eq!(again.count, 0);
        assert_eq!(fs::read_to_string(root.join(AGGREGATE_NAME)).unwrap(), fs::read_to_string(root.join(AGGREGATE_NAME)).unwrap());
    }

    #[test]
    fn aggregate_any_doc_anywhere_in_workspace() {
        let root = tmp_root("anywhere");
        // 普通文档、子目录文档、无标题文档都能批注
        fs::write(root.join("随手记.md"), "随便写的东西，没有标题结构\n").unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/idea.md"), "# Idea\n").unwrap();

        save_annotation(&root, &root.join("随手记.md"), "散记批注").unwrap();
        save_annotation(&root, &root.join("notes/idea.md"), "子目录批注").unwrap();

        let result = aggregate(&root).unwrap();
        assert_eq!(result.count, 2);
        let md = fs::read_to_string(root.join(AGGREGATE_NAME)).unwrap();
        assert!(md.contains("## 随手记"));
        assert!(md.contains("## idea"));
        assert!(md.contains("来源：[notes/idea.md](<notes/idea.md>)"));
    }

    #[test]
    fn migrate_old_format_to_new() {
        let old = "# 批注：AI创业思考\n\n> 来源：`AI创业思考.md` · 更新于 2026-08-10 10:01\n\n## 创业思考\n\n我来试一下这个章节批注的功能\n";
        let (updated_at, body) = migrate_old_format(old);
        assert_eq!(updated_at, "2026-08-10 10:01");
        assert!(body.contains("## 创业思考"));
        assert!(body.contains("我来试一下这个章节批注的功能"));
    }

    #[test]
    fn parse_note_extracts_time_from_old_combined_source_line() {
        let old = "# 批注：AI创业思考\n\n> 来源：`AI创业思考.md` · 更新于 2026-08-10 10:01\n\n## 创业思考\n\n我来试一下这个章节批注的功能\n";
        let (updated_at, body) = parse_note(old);
        assert_eq!(updated_at, "2026-08-10 10:01");
        assert!(body.starts_with("## 创业思考"));
    }

    #[test]
    fn read_annotation_data_upgrades_old_sidecar_in_place() {
        let root = tmp_root("upgrade");
        let doc = root.join("AI创业思考.md");
        fs::write(&doc, "# 创业思考\n").unwrap();
        let sidecar = root.join("批注/AI创业思考.md");
        fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        fs::write(
            &sidecar,
            "# 批注：AI创业思考\n\n> 来源：`AI创业思考.md` · 更新于 2026-08-10 10:01\n\n## 创业思考\n\n我来试一下这个章节批注的功能\n",
        )
        .unwrap();

        let data = read_annotation_data(&root, &doc).unwrap();
        assert_eq!(data.body, "## 创业思考\n\n我来试一下这个章节批注的功能");
        assert_eq!(data.updated_at, "2026-08-10 10:01");
        // 侧车已被就地升级为新版格式
        let upgraded = fs::read_to_string(&sidecar).unwrap();
        assert!(upgraded.contains("> 来源：`AI创业思考.md`\n> 时间：2026-08-10 10:01"), "{upgraded}");
        assert!(!upgraded.contains("· 更新于"));
        // 再读是幂等的
        let again = read_annotation_data(&root, &doc).unwrap();
        assert_eq!(again.body, data.body);
    }

    #[test]
    fn aggregate_handles_old_format_sidecar_cleanly() {
        let root = tmp_root("agg-old");
        fs::write(root.join("AI创业思考.md"), "# 创业思考\n").unwrap();
        let sidecar = root.join("批注/AI创业思考.md");
        fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        fs::write(
            &sidecar,
            "# 批注：AI创业思考\n\n> 来源：`AI创业思考.md` · 更新于 2026-08-10 10:01\n\n## 创业思考\n\n我来试一下这个章节批注的功能\n",
        )
        .unwrap();

        let result = aggregate(&root).unwrap();
        assert_eq!(result.count, 1);
        let md = fs::read_to_string(root.join(AGGREGATE_NAME)).unwrap();
        assert!(md.contains("批注于 2026-08-10 10:01"), "旧格式时间应被迁移: {md}");
        assert!(md.contains("我来试一下这个章节批注的功能"));
        // 侧车已被就地升级
        assert!(!fs::read_to_string(&sidecar).unwrap().contains("· 更新于"));
    }

    #[test]
    fn format_timestamp_shape() {
        let ts = format_timestamp(1_750_000_000);
        assert_eq!(ts.len(), 16, "应为 YYYY-MM-DD HH:MM，实际 {ts}");
        assert!(ts.chars().nth(4) == Some('-'));
        assert!(ts.chars().nth(10) == Some(' '));
    }

    #[test]
    fn format_timestamp_is_local_time() {
        // 本地时区偏移非零时，输出必须等于 localtime_r 的小时（UTC 实现会差 offset 小时）
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let local = format_timestamp(now);
        let hh: i32 = local[11..13].parse().unwrap();
        unsafe {
            let t = now as libc::time_t;
            let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
            libc::localtime_r(&t, tm.as_mut_ptr());
            let tm = tm.assume_init();
            assert_eq!(
                hh, tm.tm_hour as i32,
                "format_timestamp 应输出本地时间（当前实现输出 {local}，本地应为 {:02}）",
                tm.tm_hour
            );
        }
    }
}
