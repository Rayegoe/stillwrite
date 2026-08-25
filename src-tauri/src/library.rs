//! StillWrite Library：注册在工作区之外的 Markdown 资料源。
//!
//! 资料正文仍保留在原始目录；这里的 SQLite 只保存来源、元数据和可重建的 FTS
//! 侧车索引。Library 与 Workspace 使用不同的数据库和路径边界，资料不会进入
//! Workspace 文件树、批注或 git 同步。

use crate::indexer;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

#[derive(Clone)]
struct SourceRecord {
    id: String,
    root: PathBuf,
}

#[derive(Serialize, Clone)]
pub struct LibrarySource {
    pub id: String,
    pub name: String,
    pub root: String,
    pub documents: usize,
    pub available: bool,
}

#[derive(Serialize)]
pub struct LibraryRefreshResult {
    pub sources: Vec<LibrarySource>,
    pub total_documents: usize,
    pub unique_documents: usize,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub duplicates: usize,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct LibrarySearchHit {
    pub uri: String,
    pub source_id: String,
    pub source_name: String,
    pub relative_path: String,
    pub content_hash: String,
    pub title: String,
    pub snippet: String,
    pub duplicate_count: usize,
}

#[derive(Serialize)]
pub struct LibraryDocument {
    pub uri: String,
    pub source_id: String,
    pub source_name: String,
    pub relative_path: String,
    pub content_hash: String,
    pub title: String,
    pub content: String,
}

struct RefreshStats {
    added: usize,
    updated: usize,
    removed: usize,
    warnings: Vec<String>,
}

pub fn resolve_index_db(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位应用数据目录: {e}"))?
        .join("library");
    fs::create_dir_all(&dir).map_err(|e| format!("创建资料库索引目录失败: {e}"))?;
    Ok(dir.join("index.db"))
}

pub fn open_index(db_path: &Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS library_sources (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            root       TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS library_documents (
            source_id    TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            title        TEXT NOT NULL,
            mtime        INTEGER NOT NULL,
            size         INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            words        INTEGER NOT NULL,
            snippet      TEXT NOT NULL,
            PRIMARY KEY (source_id, relative_path),
            FOREIGN KEY (source_id) REFERENCES library_sources(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS library_documents_hash
            ON library_documents(content_hash);
        CREATE VIRTUAL TABLE IF NOT EXISTS library_fts USING fts5(
            content_hash UNINDEXED, title, body,
            tokenize = 'unicode61'
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS library_related_fts USING fts5(
            content_hash UNINDEXED, title, body,
            tokenize = 'trigram'
        );
        CREATE TABLE IF NOT EXISTS library_related_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;

    let related_ready: Option<String> = conn
        .query_row(
            "SELECT value FROM library_related_meta WHERE key = 'backfilled'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if related_ready.as_deref() != Some("1") {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO library_related_fts (content_hash, title, body)
             SELECT f.content_hash, f.title, f.body
             FROM library_fts f
             WHERE NOT EXISTS (
                 SELECT 1 FROM library_related_fts r
                 WHERE r.content_hash = f.content_hash
             )",
            [],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO library_related_meta (key, value)
             VALUES ('backfilled', '1')",
            [],
        )?;
        tx.commit()?;
    }
    Ok(conn)
}

pub fn register_source_at(
    db_path: &Path,
    root: &Path,
) -> Result<LibraryRefreshResult, String> {
    let root = canonical_source_root(root)?;
    let mut conn = open_index(db_path).map_err(|e| format!("打开资料库索引失败: {e}"))?;
    let id = source_id(&root);
    let name = source_name(&root);
    conn.execute(
        "INSERT OR IGNORE INTO library_sources (id, name, root, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            id,
            name,
            root.to_string_lossy().to_string(),
            unix_timestamp()
        ],
    )
    .map_err(|e| format!("注册资料源失败: {e}"))?;
    refresh(&mut conn)
}

pub fn refresh_at(db_path: &Path) -> Result<LibraryRefreshResult, String> {
    let mut conn = open_index(db_path).map_err(|e| format!("打开资料库索引失败: {e}"))?;
    refresh(&mut conn)
}

pub fn search_at(
    db_path: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<LibrarySearchHit>, String> {
    let conn = open_index(db_path).map_err(|e| format!("打开资料库索引失败: {e}"))?;
    search(&conn, query, limit)
}

pub fn search_related_at(
    db_path: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<LibrarySearchHit>, String> {
    let conn = open_index(db_path).map_err(|e| format!("打开资料库索引失败: {e}"))?;
    search_related(&conn, query, limit)
}

pub fn read_at(
    db_path: &Path,
    source_id: &str,
    relative_path: &str,
) -> Result<LibraryDocument, String> {
    let conn = open_index(db_path).map_err(|e| format!("打开资料库索引失败: {e}"))?;
    read_document(&conn, source_id, relative_path)
}

pub fn canonical_source_root(root: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|e| format!("资料目录不可用: {e}"))?;
    if !root.is_dir() {
        return Err("选择的资料源不是目录".into());
    }
    if root.parent().is_none() {
        return Err("不能将文件系统根目录作为资料源".into());
    }
    Ok(root)
}

pub fn roots_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn source_id(root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(root.to_string_lossy().as_bytes());
    hasher
        .finalize()
        .iter()
        .take(12)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn source_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "资料源".to_string())
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn load_sources(conn: &Connection) -> Result<Vec<SourceRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT id, root FROM library_sources ORDER BY name COLLATE NOCASE, id")
        .map_err(|e| format!("读取资料源失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SourceRecord {
                id: row.get(0)?,
                root: PathBuf::from(row.get::<_, String>(1)?),
            })
        })
        .map_err(|e| format!("读取资料源失败: {e}"))?;
    rows.map(|row| row.map_err(|e| format!("读取资料源失败: {e}")))
        .collect()
}

fn refresh(conn: &mut Connection) -> Result<LibraryRefreshResult, String> {
    let sources = load_sources(conn)?;
    let mut stats = RefreshStats {
        added: 0,
        updated: 0,
        removed: 0,
        warnings: Vec::new(),
    };
    let mut orphan_candidates = HashSet::new();

    let tx = conn
        .transaction()
        .map_err(|e| format!("资料库索引事务失败: {e}"))?;
    for source in &sources {
        let Ok(current_root) = fs::canonicalize(&source.root) else {
            stats
                .warnings
                .push(format!("资料源不可用：{}", source.root.display()));
            continue;
        };
        if current_root != source.root || !current_root.is_dir() {
            stats.warnings.push(format!(
                "资料源路径已变化，请重新注册：{}",
                source.root.display()
            ));
            continue;
        }

        let files = indexer::walk(&source.root);
        let mut present = HashSet::new();
        for (path, mtime, size) in files {
            let relative_path = path
                .strip_prefix(&source.root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            present.insert(relative_path.clone());

            let previous: Option<(i64, i64, String)> = tx
                .query_row(
                    "SELECT mtime, size, content_hash
                     FROM library_documents
                     WHERE source_id = ?1 AND relative_path = ?2",
                    params![source.id, relative_path],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(|e| format!("读取资料元数据失败: {e}"))?;
            if previous
                .as_ref()
                .is_some_and(|(old_mtime, old_size, _)| {
                    *old_mtime == mtime && *old_size == size as i64
                })
            {
                continue;
            }

            let text = match fs::read_to_string(&path) {
                Ok(text) => normalize_text(&text),
                Err(error) => {
                    stats.warnings.push(format!(
                        "跳过无法读取的资料：{}（{error}）",
                        path.display()
                    ));
                    continue;
                }
            };
            let content_hash = content_hash(&text);
            if let Some((_, _, old_hash)) = &previous {
                if old_hash != &content_hash {
                    orphan_candidates.insert(old_hash.clone());
                }
            }
            let title = path
                .file_name()
                .map(|name| indexer::title_of(&name.to_string_lossy()))
                .unwrap_or_default();
            let words = indexer::word_count(&text) as i64;
            let snippet = indexer::snippet_of(&text);

            tx.execute(
                "INSERT INTO library_documents
                    (source_id, relative_path, title, mtime, size, content_hash, words, snippet)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(source_id, relative_path) DO UPDATE SET
                    title = excluded.title,
                    mtime = excluded.mtime,
                    size = excluded.size,
                    content_hash = excluded.content_hash,
                    words = excluded.words,
                    snippet = excluded.snippet",
                params![
                    source.id,
                    relative_path,
                    title,
                    mtime,
                    size as i64,
                    content_hash,
                    words,
                    snippet
                ],
            )
            .map_err(|e| format!("写入资料元数据失败: {e}"))?;

            let fts_exists = tx
                .query_row(
                    "SELECT 1 FROM library_fts WHERE content_hash = ?1 LIMIT 1",
                    params![content_hash],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| format!("读取资料全文索引失败: {e}"))?
                .is_some();
            if !fts_exists {
                tx.execute(
                    "INSERT INTO library_fts (content_hash, title, body) VALUES (?1, ?2, ?3)",
                    params![content_hash, title, text],
                )
                .map_err(|e| format!("写入资料全文索引失败: {e}"))?;
            }
            let related_fts_exists = tx
                .query_row(
                    "SELECT 1 FROM library_related_fts WHERE content_hash = ?1 LIMIT 1",
                    params![content_hash],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| format!("读取关联资料索引失败: {e}"))?
                .is_some();
            if !related_fts_exists {
                tx.execute(
                    "INSERT INTO library_related_fts (content_hash, title, body)
                     VALUES (?1, ?2, ?3)",
                    params![content_hash, title, text],
                )
                .map_err(|e| format!("写入关联资料索引失败: {e}"))?;
            }

            if previous.is_some() {
                stats.updated += 1;
            } else {
                stats.added += 1;
            }
        }

        let known: Vec<(String, String)> = tx
            .prepare(
                "SELECT relative_path, content_hash
                 FROM library_documents WHERE source_id = ?1",
            )
            .map_err(|e| format!("读取资料清单失败: {e}"))?
            .query_map(params![source.id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| format!("读取资料清单失败: {e}"))?
            .filter_map(|row| row.ok())
            .collect();
        for (relative_path, old_hash) in known {
            if present.contains(&relative_path) {
                continue;
            }
            tx.execute(
                "DELETE FROM library_documents WHERE source_id = ?1 AND relative_path = ?2",
                params![source.id, relative_path],
            )
            .map_err(|e| format!("清理资料元数据失败: {e}"))?;
            orphan_candidates.insert(old_hash);
            stats.removed += 1;
        }
    }

    for hash in orphan_candidates {
        let still_used = tx
            .query_row(
                "SELECT 1 FROM library_documents WHERE content_hash = ?1 LIMIT 1",
                params![hash],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| format!("检查重复资料失败: {e}"))?
            .is_some();
        if !still_used {
            tx.execute(
                "DELETE FROM library_fts WHERE content_hash = ?1",
                params![hash],
            )
            .map_err(|e| format!("清理资料全文索引失败: {e}"))?;
            tx.execute(
                "DELETE FROM library_related_fts WHERE content_hash = ?1",
                params![hash],
            )
            .map_err(|e| format!("清理关联资料索引失败: {e}"))?;
        }
    }
    tx.commit()
        .map_err(|e| format!("资料库索引提交失败: {e}"))?;

    let (source_views, total_documents, unique_documents) = snapshot(conn)?;
    Ok(LibraryRefreshResult {
        duplicates: total_documents.saturating_sub(unique_documents),
        sources: source_views,
        total_documents,
        unique_documents,
        added: stats.added,
        updated: stats.updated,
        removed: stats.removed,
        warnings: stats.warnings,
    })
}

fn snapshot(
    conn: &Connection,
) -> Result<(Vec<LibrarySource>, usize, usize), String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.name, s.root, COUNT(d.relative_path)
             FROM library_sources s
             LEFT JOIN library_documents d ON d.source_id = s.id
             GROUP BY s.id, s.name, s.root
             ORDER BY s.name COLLATE NOCASE, s.id",
        )
        .map_err(|e| format!("读取资料库概览失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let root: String = row.get(2)?;
            Ok(LibrarySource {
                id: row.get(0)?,
                name: row.get(1)?,
                root: root.clone(),
                documents: row.get::<_, i64>(3)? as usize,
                available: fs::canonicalize(&root)
                    .map(|current| current == Path::new(&root) && current.is_dir())
                    .unwrap_or(false),
            })
        })
        .map_err(|e| format!("读取资料库概览失败: {e}"))?;
    let sources: Vec<LibrarySource> = rows
        .map(|row| row.map_err(|e| format!("读取资料库概览失败: {e}")))
        .collect::<Result<_, _>>()?;
    let total_documents: usize = conn
        .query_row("SELECT COUNT(*) FROM library_documents", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|e| format!("统计资料数量失败: {e}"))? as usize;
    let unique_documents: usize = conn
        .query_row(
            "SELECT COUNT(DISTINCT content_hash) FROM library_documents",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("统计资料去重数量失败: {e}"))? as usize;
    Ok((sources, total_documents, unique_documents))
}

fn search(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<LibrarySearchHit>, String> {
    search_with_fts(conn, query, limit, "library_fts", false)
}

fn search_related(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<LibrarySearchHit>, String> {
    search_with_fts(conn, query, limit, "library_related_fts", true)
}

fn search_with_fts(
    conn: &Connection,
    query: &str,
    limit: usize,
    fts_table: &str,
    related: bool,
) -> Result<Vec<LibrarySearchHit>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 100);
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect();
    let match_expr = tokens.join(" AND ");
    let mut corpus_hits = Vec::new();
    let match_sql = format!(
        "SELECT content_hash, snippet({fts_table}, 2, '[[', ']]', '…', 14)
         FROM {fts_table} WHERE {fts_table} MATCH ?1 ORDER BY rank LIMIT ?2"
    );
    if let Ok(mut stmt) = conn.prepare(&match_sql) {
        if let Ok(rows) = stmt.query_map(params![match_expr, (limit * 3) as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                if !corpus_hits.iter().any(|(hash, _)| hash == &row.0) {
                    corpus_hits.push(row);
                }
            }
        }
    }

    if corpus_hits.is_empty() {
        let like = format!("%{}%", query.trim());
        let fallback_sql = if related {
            format!(
                "SELECT f.content_hash, COALESCE(MIN(d.snippet), '')
                 FROM {fts_table} f
                 LEFT JOIN library_documents d ON d.content_hash = f.content_hash
                 WHERE f.title LIKE ?1 OR f.body LIKE ?1
                 GROUP BY f.content_hash LIMIT ?2"
            )
        } else {
            "SELECT content_hash, snippet FROM library_documents
             WHERE title LIKE ?1 OR snippet LIKE ?1
             GROUP BY content_hash LIMIT ?2"
                .to_string()
        };
        let mut stmt = conn.prepare(&fallback_sql).map_err(|e| {
            if related {
                format!("关联资料搜索准备失败: {e}")
            } else {
                format!("资料搜索准备失败: {e}")
            }
        })?;
        let rows = stmt
            .query_map(params![like, limit as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| {
                if related {
                    format!("关联资料搜索失败: {e}")
                } else {
                    format!("资料搜索失败: {e}")
                }
            })?;
        corpus_hits = rows.flatten().collect();
    }

    let hits = corpus_hits
        .into_iter()
        .take(limit)
        .filter_map(|(hash, snippet)| {
            let document = conn
                .query_row(
                    "SELECT d.source_id, s.name, d.relative_path, d.content_hash, d.title
                     FROM library_documents d
                     JOIN library_sources s ON s.id = d.source_id
                     WHERE d.content_hash = ?1
                     ORDER BY s.name COLLATE NOCASE, d.relative_path
                     LIMIT 1",
                    params![hash],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                        ))
                    },
                )
                .ok()?;
            let duplicate_count = conn
                .query_row(
                    "SELECT COUNT(*) FROM library_documents WHERE content_hash = ?1",
                    params![hash],
                    |row| row.get::<_, i64>(0),
                )
                .ok()? as usize;
            let (source_id, source_name, relative_path, content_hash, title) = document;
            Some(LibrarySearchHit {
                uri: document_uri(&source_id, &relative_path),
                source_id,
                source_name,
                relative_path,
                content_hash,
                title,
                snippet,
                duplicate_count,
            })
        })
        .collect::<Vec<_>>();
    Ok(hits)
}

fn read_document(
    conn: &Connection,
    source_id: &str,
    relative_path: &str,
) -> Result<LibraryDocument, String> {
    let relative_path = normalize_relative_path(relative_path)?;
    let (source_name, root, title): (String, String, String) = conn
        .query_row(
            "SELECT s.name, s.root, d.title
             FROM library_documents d
             JOIN library_sources s ON s.id = d.source_id
             WHERE d.source_id = ?1 AND d.relative_path = ?2",
            params![source_id, relative_path],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "资料尚未索引，请先刷新资料库".to_string())?;
    let stored_root = PathBuf::from(root);
    let root = fs::canonicalize(&stored_root).map_err(|e| format!("资料源不可用: {e}"))?;
    if root != stored_root {
        return Err("资料源路径已变化，请重新注册".into());
    }
    let candidate = root.join(Path::new(&relative_path));
    let path = fs::canonicalize(&candidate).map_err(|e| format!("资料文件不可用: {e}"))?;
    if !path.starts_with(&root) || !path.is_file() || !indexer::is_markdown(&path) {
        return Err("拒绝读取资料源之外的文件".into());
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("读取资料失败: {e}"))?;
    let current_hash = content_hash(&normalize_text(&content));
    Ok(LibraryDocument {
        uri: document_uri(source_id, &relative_path),
        source_id: source_id.to_string(),
        source_name,
        relative_path,
        content_hash: current_hash,
        title,
        content,
    })
}

pub fn document_uri(source_id: &str, relative_path: &str) -> String {
    format!("library://{source_id}/{relative_path}")
}

fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn normalize_relative_path(path: &str) -> Result<String, String> {
    let normalized = path.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("资料路径包含不允许的路径片段".into());
    }
    if !indexer::is_markdown(path) {
        return Err("只允许读取 Markdown 资料".into());
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "stillwrite-library-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn refreshes_incrementally_and_deduplicates_content() {
        let keep = TempDir::new();
        let source = keep.path().join("WhereMyLife");
        fs::create_dir_all(source.join("2026-08-25")).unwrap();
        fs::write(
            source.join("2026-08-25/001.md"),
            "# 具身智能\r\n\r\n制造现场正在变化。",
        )
        .unwrap();
        fs::write(
            source.join("2026-08-25/002.md"),
            "# 具身智能\n\n制造现场正在变化。",
        )
        .unwrap();
        let db = keep.path().join("library.db");
        let source = canonical_source_root(&source).unwrap();

        let first = register_source_at(&db, &source).unwrap();
        assert_eq!(first.added, 2);
        assert_eq!(first.total_documents, 2);
        assert_eq!(first.unique_documents, 1);
        assert_eq!(first.duplicates, 1);

        let second = refresh_at(&db).unwrap();
        assert_eq!(second.added, 0);
        assert_eq!(second.updated, 0);

        fs::write(source.join("2026-08-25/002.md"), "# 新文章\n\n完全不同的内容。\n").unwrap();
        let third = refresh_at(&db).unwrap();
        assert_eq!(third.updated, 1);
        assert_eq!(third.unique_documents, 2);

        let hits = search_at(&db, "完全不同", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].relative_path.ends_with("002.md"));
        assert_eq!(hits[0].duplicate_count, 1);
    }

    #[test]
    fn trigram_retrieves_chinese_substrings_and_two_character_terms() {
        let keep = TempDir::new();
        let source = keep.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("context.md"),
            "# 上下文工程\n\n上下文工程正在成为 Agent Harness 的核心问题，模型能力也会因此重新分层。\n",
        )
        .unwrap();
        let db = keep.path().join("library.db");
        let source = canonical_source_root(&source).unwrap();
        register_source_at(&db, &source).unwrap();

        for query in ["上下文", "上下文工程", "模型", "Harness"] {
            let hits = search_related_at(&db, query, 10).unwrap();
            assert_eq!(hits.len(), 1, "query={query}");
            assert!(hits[0].relative_path.ends_with("context.md"), "query={query}");
        }
    }

    #[test]
    fn backfills_related_trigram_index_from_library_fts() {
        let keep = TempDir::new();
        let db = keep.path().join("legacy-library.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE library_sources (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                root TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE library_documents (
                source_id TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                title TEXT NOT NULL,
                mtime INTEGER NOT NULL,
                size INTEGER NOT NULL,
                content_hash TEXT NOT NULL,
                words INTEGER NOT NULL,
                snippet TEXT NOT NULL,
                PRIMARY KEY (source_id, relative_path)
            );
            CREATE VIRTUAL TABLE library_fts USING fts5(
                content_hash UNINDEXED, title, body,
                tokenize = 'unicode61'
            );
            INSERT INTO library_sources VALUES ('s', '旧资料', '/legacy', 0);
            INSERT INTO library_documents VALUES
                ('s', 'legacy.md', '旧文档', 0, 10, 'hash', 4, '上下文工程仍然值得检索');
            INSERT INTO library_fts VALUES ('hash', '旧文档', '上下文工程仍然值得检索');",
        )
        .unwrap();
        drop(conn);

        let hits = search_related_at(&db, "上下文", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].relative_path, "legacy.md");
    }

    #[test]
    fn reads_only_registered_markdown_inside_source() {
        let keep = TempDir::new();
        let source = keep.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("note.md"), "资料正文").unwrap();
        fs::write(keep.path().join("outside.md"), "外部正文").unwrap();
        let db = keep.path().join("library.db");
        let source = canonical_source_root(&source).unwrap();
        let result = register_source_at(&db, &source).unwrap();
        let id = result.sources[0].id.clone();

        let document = read_at(&db, &id, "note.md").unwrap();
        assert_eq!(document.content, "资料正文");
        assert!(read_at(&db, &id, "../outside.md").is_err());
        assert!(read_at(&db, &id, "note.txt").is_err());
    }

    #[test]
    fn removes_deleted_documents_and_orphaned_fts_rows() {
        let keep = TempDir::new();
        let source = keep.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("note.md"), "要删除的资料").unwrap();
        let db = keep.path().join("library.db");
        register_source_at(&db, &canonical_source_root(&source).unwrap()).unwrap();
        fs::remove_file(source.join("note.md")).unwrap();

        let result = refresh_at(&db).unwrap();
        assert_eq!(result.removed, 1);
        assert_eq!(search_at(&db, "要删除", 10).unwrap().len(), 0);
    }
}
