//! 本地 SQLite 侧车索引：文件仍是唯一内容源，这里只存派生的搜索/元数据。
//! 索引放在应用数据目录，不进入工作区、不参与 git 同步，任何时刻可重建。

use rusqlite::{Connection, params};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
}

pub fn open_index(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files (
            path    TEXT PRIMARY KEY,
            title   TEXT NOT NULL,
            mtime   INTEGER NOT NULL,
            size    INTEGER NOT NULL,
            words   INTEGER NOT NULL,
            snippet TEXT NOT NULL
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS fts USING fts5(
            path UNINDEXED, title, body,
            tokenize = 'unicode61'
        );",
    )?;
    Ok(conn)
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn title_of(name: &str) -> String {
    let stem = name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(name);
    stem.to_string()
}

fn word_count(text: &str) -> usize {
    // 简单统计：拉丁词 + CJK 字符块
    let mut words = 0usize;
    let mut in_latin = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if !in_latin {
                words += 1;
                in_latin = true;
            }
        } else {
            in_latin = false;
            if (0x4E00..=0x9FFF).contains(&(ch as u32))
                || (0x3400..=0x4DBF).contains(&(ch as u32))
                || (0x3000..=0x303F).contains(&(ch as u32))
            {
                words += 1;
            }
        }
    }
    words
}

fn snippet_of(text: &str) -> String {
    let text = text.trim();
    let mut out = String::new();
    let mut count = 0;
    for ch in text.chars() {
        out.push(ch);
        count += 1;
        if count >= 160 {
            break;
        }
    }
    if text.chars().count() > count {
        out.push('…');
    }
    out
}

/// 遍历工作区里的 markdown 文件，返回 (path, mtime_secs, size)。
fn walk(root: &Path) -> Vec<(PathBuf, i64, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    stack.push(path);
                } else if ft.is_file() && is_markdown(&path) {
                    if let Ok(meta) = entry.metadata() {
                        out.push((path, meta.modified().map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)).unwrap_or(0), meta.len()));
                    }
                }
            }
        }
    }
    out
}

/// 增量重建索引：只重读 mtime/size 变化的文件；清理已删除文件。
pub fn build_index(conn: &mut Connection, root: &Path) -> Result<(usize, usize), String> {
    let files = walk(root);
    let mut updated = 0usize;
    let mut removed = 0usize;

    let tx = conn.transaction().map_err(|e| format!("索引事务失败: {e}"))?;
    {
        let mut upsert_file = tx
            .prepare(
                "INSERT OR REPLACE INTO files (path, title, mtime, size, words, snippet) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(|e| format!("索引准备失败: {e}"))?;
        let mut upsert_fts = tx
            .prepare("INSERT INTO fts (path, title, body) VALUES (?1, ?2, ?3)")
            .map_err(|e| format!("索引准备失败: {e}"))?;
        let mut del_fts = tx
            .prepare("DELETE FROM fts WHERE path = ?1")
            .map_err(|e| format!("索引准备失败: {e}"))?;

        for (path, mtime, size) in &files {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let title = title_of(
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
                    .as_str(),
            );

            // 跳过未变化的文件
            let unchanged = tx
                .query_row(
                    "SELECT 1 FROM files WHERE path = ?1 AND mtime = ?2 AND size = ?3",
                    params![rel, mtime, *size as i64],
                    |_| Ok(()),
                )
                .is_ok();
            if unchanged {
                continue;
            }

            let text = fs::read_to_string(path).unwrap_or_default();
            let words = word_count(&text);
            let snippet = snippet_of(&text);

            upsert_file
                .execute(params![rel, title, mtime, *size as i64, words as i64, snippet])
                .map_err(|e| format!("索引写入失败: {e}"))?;
            del_fts
                .execute(params![rel])
                .map_err(|e| format!("索引写入失败: {e}"))?;
            upsert_fts
                .execute(params![rel, title, text])
                .map_err(|e| format!("索引写入失败: {e}"))?;
            updated += 1;
        }

        // 清理磁盘上已不存在的文件
        let known: Vec<String> = tx
            .prepare("SELECT path FROM files")
            .map_err(|e| format!("索引读取失败: {e}"))?
            .query_map([], |row| row.get(0))
            .map_err(|e| format!("索引读取失败: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        for rel in known {
            if !files.iter().any(|(p, _, _)| {
                p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/") == rel
            }) {
                tx.execute("DELETE FROM files WHERE path = ?1", params![rel])
                    .map_err(|e| format!("索引清理失败: {e}"))?;
                del_fts
                    .execute(params![rel])
                    .map_err(|e| format!("索引清理失败: {e}"))?;
                removed += 1;
            }
        }
    }
    tx.commit().map_err(|e| format!("索引提交失败: {e}"))?;
    Ok((updated, removed))
}

/// 单文件增量入索引（文件新建/保存后调用）。
/// 与 build_index 的字段约定一致：path 存工作区相对路径，正文存 FTS5。
pub fn index_single(conn: &mut Connection, root: &Path, path: &Path) -> Result<(), String> {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let text = fs::read_to_string(path).unwrap_or_default();
    let title = title_of(
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
            .as_str(),
    );
    let words = word_count(&text) as i64;
    let snippet = snippet_of(&text);
    let meta = fs::metadata(path).map_err(|e| format!("索引读取失败: {e}"))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let size = meta.len() as i64;

    let tx = conn.transaction().map_err(|e| format!("索引事务失败: {e}"))?;
    tx.execute(
        "INSERT OR REPLACE INTO files (path, title, mtime, size, words, snippet) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![rel, title, mtime, size, words, snippet],
    )
    .map_err(|e| format!("索引写入失败: {e}"))?;
    tx.execute("DELETE FROM fts WHERE path = ?1", params![rel])
        .map_err(|e| format!("索引写入失败: {e}"))?;
    tx.execute(
        "INSERT INTO fts (path, title, body) VALUES (?1, ?2, ?3)",
        params![rel, title, text],
    )
    .map_err(|e| format!("索引写入失败: {e}"))?;
    tx.commit().map_err(|e| format!("索引提交失败: {e}"))?;
    Ok(())
}

/// FTS5 搜索。用户输入被切成词元逐词短语化，避免 MATCH 语法注入报错。
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    let match_expr = tokens.join(" AND ");

    let mut stmt = conn
        .prepare(
            "SELECT path, title, snippet(fts, 2, '[[', ']]', '…', 14)
             FROM fts WHERE fts MATCH ?1 ORDER BY rank LIMIT ?2",
        )
        .map_err(|e| format!("搜索准备失败: {e}"))?;

    let hits: Vec<SearchHit> = stmt
        .query_map(params![match_expr, limit as i64], |row| {
            Ok(SearchHit {
                path: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get(2)?,
            })
        })
        .map_err(|e| format!("搜索失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // MATCH 语法错误时降级为 LIKE
    if hits.is_empty() {
        if let Ok(mut like_stmt) = conn.prepare(
            "SELECT path, title, snippet FROM files WHERE title LIKE ?1 OR snippet LIKE ?1 LIMIT ?2",
        ) {
            if let Ok(rows) = like_stmt.query_map(
                params![format!("%{}%", query), limit as i64],
                |row| {
                    Ok(SearchHit {
                        path: row.get(0)?,
                        title: row.get(1)?,
                        snippet: row.get(2)?,
                    })
                },
            ) {
                let like_hits: Vec<SearchHit> = rows.filter_map(|r| r.ok()).collect();
                if !like_hits.is_empty() {
                    return Ok(like_hits);
                }
            }
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "sw-test-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_workspace() -> (TempDir, PathBuf) {
        let dir = TempDir::new();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        (dir, root)
    }

    #[test]
    fn build_and_search_roundtrip() {
        let (_keep, root) = temp_workspace();
        let db = root.join("test-index.db");

        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes/idea.md"), "# 标题\n\n毛泽东选集 读书笔记\n\n关于农村包围城市\n").unwrap();
        std::fs::write(root.join("essay.md"), "# Essay\n\nhello world, this is a test.\n").unwrap();

        let mut conn = open_index(&db).unwrap();
        let (updated, removed) = build_index(&mut conn, &root).unwrap();
        assert_eq!(updated, 2);
        assert_eq!(removed, 0);

        // FTS5 命中
        let hits = search(&conn, "毛泽东", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("idea.md"));
        assert!(hits[0].title.contains("idea"));

        let hits2 = search(&conn, "world", 10).unwrap();
        assert_eq!(hits2.len(), 1);
        assert!(hits2[0].path.ends_with("essay.md"));

        // 增量：不变文件不重读
        std::fs::write(root.join("essay.md"), "# Essay\n\nchanged content\n").unwrap();
        let (updated2, _) = build_index(&mut conn, &root).unwrap();
        assert_eq!(updated2, 1); // 只有 essay.md 变化

        // 删除文件后索引清理
        std::fs::remove_file(root.join("notes/idea.md")).unwrap();
        let (_, removed2) = build_index(&mut conn, &root).unwrap();
        assert_eq!(removed2, 1);
        assert!(search(&conn, "毛泽东", 10).unwrap().is_empty());
    }

    #[test]
    fn word_count_basics() {
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count("你好世界"), 4);
        assert_eq!(word_count("毛泽东选集"), 5);
        assert_eq!(word_count(""), 0);
    }
}
