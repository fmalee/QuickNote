use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::command_book::CommandEntry;
use crate::domain::error::AppError;

use super::map_sqlite_error;

/// Row payload for upserting a note into `notes`, `note_tags`, and `notes_fts`.
#[derive(Debug, Clone)]
pub struct NoteIndexRecord {
    pub key: String,
    pub id: Option<String>,
    pub rel_path: String,
    pub title: String,
    pub doc_type: String,
    pub mtime_ms: i64,
    pub size_bytes: i64,
    pub body_hash: String,
    pub frontmatter_json: String,
    pub excerpt: String,
    pub meta_error: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub is_inbox: bool,
    pub updated_at: i64,
    pub tags: Vec<String>,
    pub title_tokens: String,
    pub tag_tokens: String,
    pub body_tokens: String,
    pub links: Vec<String>,
    pub assets: Vec<String>,
    pub commands: Vec<CommandEntry>,
}

/// Lightweight row returned from `notes` queries.
#[derive(Debug, Clone)]
pub struct IndexedNoteRow {
    pub key: String,
    pub rel_path: String,
    pub title: String,
    pub doc_type: String,
    pub excerpt: String,
    pub pinned: bool,
    pub archived: bool,
    pub is_inbox: bool,
    pub mtime_ms: i64,
    pub meta_error: Option<String>,
    pub tags: Vec<String>,
}

/// FTS5 search hit from `notes_fts` joined with `notes`.
#[derive(Debug, Clone)]
pub struct FtsSearchHit {
    pub key: String,
    pub rel_path: String,
    pub title: String,
    pub rank: f64,
}

/// Replaces all rows in `notes`, `note_tags`, and `notes_fts` atomically inside one transaction.
pub fn replace_all_notes_index(
    conn: &Connection,
    records: &[NoteIndexRecord],
) -> Result<(), AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| map_sqlite_error(e, "Failed to begin index rebuild transaction"))?;

    tx.execute("DELETE FROM note_tags", [])
        .map_err(|e| map_sqlite_error(e, "Failed to clear note_tags"))?;
    tx.execute("DELETE FROM note_links", [])
        .map_err(|e| map_sqlite_error(e, "Failed to clear note_links"))?;
    tx.execute("DELETE FROM note_assets", [])
        .map_err(|e| map_sqlite_error(e, "Failed to clear note_assets"))?;
    tx.execute("DELETE FROM commands", [])
        .map_err(|e| map_sqlite_error(e, "Failed to clear commands"))?;
    tx.execute("DELETE FROM notes_fts", [])
        .map_err(|e| map_sqlite_error(e, "Failed to clear notes_fts"))?;
    tx.execute("DELETE FROM notes", [])
        .map_err(|e| map_sqlite_error(e, "Failed to clear notes"))?;

    for rec in records {
        insert_note_in_tx(&tx, rec)?;
    }

    tx.commit()
        .map_err(|e| map_sqlite_error(e, "Failed to commit index rebuild transaction"))?;
    Ok(())
}

/// Upserts a single note by `rel_path` (removes any previous row with the same `rel_path` or `key`).
pub fn upsert_note_index(conn: &Connection, rec: &NoteIndexRecord) -> Result<(), AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| map_sqlite_error(e, "Failed to begin note upsert transaction"))?;

    delete_note_in_tx(&tx, &rec.rel_path)?;
    tx.execute(
        "DELETE FROM note_tags WHERE note_key = ?1",
        params![rec.key],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to clear note_tags by key"))?;
    tx.execute(
        "DELETE FROM notes_fts WHERE note_key = ?1",
        params![rec.key],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to clear notes_fts by key"))?;
    tx.execute("DELETE FROM notes WHERE key = ?1", params![rec.key])
        .map_err(|e| map_sqlite_error(e, "Failed to clear notes by key"))?;

    insert_note_in_tx(&tx, rec)?;

    tx.commit()
        .map_err(|e| map_sqlite_error(e, "Failed to commit note upsert transaction"))?;
    Ok(())
}

/// Removes a note from `notes`, `note_tags`, and `notes_fts` by `rel_path`.
pub fn delete_note_index_by_rel_path(conn: &Connection, rel_path: &str) -> Result<(), AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| map_sqlite_error(e, "Failed to begin note delete transaction"))?;
    delete_note_in_tx(&tx, rel_path)?;
    tx.commit()
        .map_err(|e| map_sqlite_error(e, "Failed to commit note delete transaction"))?;
    Ok(())
}

fn delete_note_in_tx(tx: &rusqlite::Transaction<'_>, rel_path: &str) -> Result<(), AppError> {
    tx.execute(
        "DELETE FROM note_links WHERE source_rel_path = ?1",
        params![rel_path],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to delete note_links"))?;
    tx.execute(
        "DELETE FROM note_assets WHERE source_rel_path = ?1",
        params![rel_path],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to delete note_assets"))?;
    tx.execute(
        "DELETE FROM commands WHERE note_rel_path = ?1",
        params![rel_path],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to delete commands"))?;

    let existing_key: Option<String> = tx
        .query_row(
            "SELECT key FROM notes WHERE rel_path = ?1",
            params![rel_path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "Failed to lookup note key by rel_path"))?;

    if let Some(key) = existing_key {
        tx.execute("DELETE FROM note_tags WHERE note_key = ?1", params![key])
            .map_err(|e| map_sqlite_error(e, "Failed to delete note_tags"))?;
        tx.execute("DELETE FROM notes_fts WHERE note_key = ?1", params![key])
            .map_err(|e| map_sqlite_error(e, "Failed to delete notes_fts"))?;
        tx.execute("DELETE FROM notes WHERE key = ?1", params![key])
            .map_err(|e| map_sqlite_error(e, "Failed to delete note"))?;
    }
    Ok(())
}

fn insert_note_in_tx(
    tx: &rusqlite::Transaction<'_>,
    rec: &NoteIndexRecord,
) -> Result<(), AppError> {
    tx.execute(
        "INSERT INTO notes (
            key, id, rel_path, title, doc_type, project,
            mtime_ms, size_bytes, body_hash, frontmatter_json, excerpt,
            meta_error, pinned, archived, is_inbox, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            rec.key,
            rec.id,
            rec.rel_path,
            rec.title,
            rec.doc_type,
            Option::<String>::None,
            rec.mtime_ms,
            rec.size_bytes,
            rec.body_hash,
            rec.frontmatter_json,
            rec.excerpt,
            rec.meta_error,
            i64::from(rec.pinned),
            i64::from(rec.archived),
            i64::from(rec.is_inbox),
            rec.updated_at,
        ],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to insert into notes"))?;

    for tag in &rec.tags {
        tx.execute(
            "INSERT OR IGNORE INTO note_tags (note_key, tag) VALUES (?1, ?2)",
            params![rec.key, tag],
        )
        .map_err(|e| map_sqlite_error(e, "Failed to insert into note_tags"))?;
    }

    tx.execute(
        "INSERT INTO notes_fts (note_key, title_tokens, tag_tokens, body_tokens) VALUES (?1, ?2, ?3, ?4)",
        params![rec.key, rec.title_tokens, rec.tag_tokens, rec.body_tokens],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to insert into notes_fts"))?;

    for target in &rec.links {
        tx.execute(
            "INSERT OR IGNORE INTO note_links (source_rel_path, target_rel_path) VALUES (?1, ?2)",
            params![rec.rel_path, target],
        )
        .map_err(|e| map_sqlite_error(e, "Failed to insert into note_links"))?;
    }

    for asset in &rec.assets {
        tx.execute(
            "INSERT OR IGNORE INTO note_assets (source_rel_path, asset_rel_path) VALUES (?1, ?2)",
            params![rec.rel_path, asset],
        )
        .map_err(|e| map_sqlite_error(e, "Failed to insert into note_assets"))?;
    }

    for cmd in &rec.commands {
        let keywords_json =
            serde_json::to_string(&cmd.keywords).unwrap_or_else(|_| "[]".to_string());
        tx.execute(
            "INSERT INTO commands (
                id, note_rel_path, section, title, description,
                code, language, keywords, is_danger, line
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                cmd.id,
                cmd.note_rel_path,
                cmd.section,
                cmd.title,
                cmd.description,
                cmd.code,
                cmd.language,
                keywords_json,
                i64::from(cmd.is_danger),
                cmd.line as i64,
            ],
        )
        .map_err(|e| map_sqlite_error(e, "Failed to insert into commands"))?;
    }

    Ok(())
}

/// Loads all indexed notes with their tags, ordered by `pinned DESC, mtime_ms DESC`.
pub fn list_all_indexed_notes(conn: &Connection) -> Result<Vec<IndexedNoteRow>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT key, rel_path, title, doc_type, excerpt,
                    pinned, archived, is_inbox, mtime_ms, meta_error
             FROM notes
             ORDER BY pinned DESC, mtime_ms DESC",
        )
        .map_err(|e| map_sqlite_error(e, "Failed to prepare list_all_indexed_notes"))?;

    let mut rows = stmt
        .query_map([], |row| {
            Ok(IndexedNoteRow {
                key: row.get(0)?,
                rel_path: row.get(1)?,
                title: row.get(2)?,
                doc_type: row.get(3)?,
                excerpt: row.get(4)?,
                pinned: row.get::<_, i64>(5)? != 0,
                archived: row.get::<_, i64>(6)? != 0,
                is_inbox: row.get::<_, i64>(7)? != 0,
                mtime_ms: row.get(8)?,
                meta_error: row.get(9)?,
                tags: Vec::new(),
            })
        })
        .map_err(|e| map_sqlite_error(e, "Failed to query notes"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to read note rows"))?;

    let mut tag_stmt = conn
        .prepare("SELECT note_key, tag FROM note_tags")
        .map_err(|e| map_sqlite_error(e, "Failed to prepare note_tags query"))?;
    let tag_pairs = tag_stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| map_sqlite_error(e, "Failed to query note_tags"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to read note_tags"))?;

    let mut tag_map: HashMap<String, Vec<String>> = HashMap::new();
    for (key, tag) in tag_pairs {
        tag_map.entry(key).or_default().push(tag);
    }

    for row in &mut rows {
        if let Some(tags) = tag_map.remove(&row.key) {
            row.tags = tags;
        }
    }

    Ok(rows)
}

/// Queries `notes_fts` with a prebuilt FTS5 `MATCH` expression.
pub fn search_notes_fts(
    conn: &Connection,
    fts_expr: &str,
    limit: usize,
) -> Result<Vec<FtsSearchHit>, AppError> {
    if fts_expr.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT n.key, n.rel_path, n.title, bm25(notes_fts, 10.0, 5.0, 1.0) AS rank
             FROM notes_fts
             JOIN notes n ON n.key = notes_fts.note_key
             WHERE notes_fts MATCH ?1
             ORDER BY rank ASC
             LIMIT ?2",
        )
        .map_err(|e| map_sqlite_error(e, "Failed to prepare FTS search"))?;

    let hits = stmt
        .query_map(params![fts_expr, limit as i64], |row| {
            Ok(FtsSearchHit {
                key: row.get(0)?,
                rel_path: row.get(1)?,
                title: row.get(2)?,
                rank: row.get(3)?,
            })
        })
        .map_err(|e| map_sqlite_error(e, "Failed to execute FTS search"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to read FTS search hits"))?;

    Ok(hits)
}

/// Looks up the indexed `key` for a library-relative path, if present.
pub fn get_note_key_by_rel_path(
    conn: &Connection,
    rel_path: &str,
) -> Result<Option<String>, AppError> {
    conn.query_row(
        "SELECT key FROM notes WHERE rel_path = ?1",
        params![rel_path],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| map_sqlite_error(e, "Failed to query note key by rel_path"))
}

/// Lists all indexed command entries ordered by `note_rel_path ASC, line ASC`.
pub fn list_all_commands(conn: &Connection) -> Result<Vec<CommandEntry>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, note_rel_path, section, title, description,
                    code, language, keywords, is_danger, line
             FROM commands
             ORDER BY note_rel_path ASC, line ASC",
        )
        .map_err(|e| map_sqlite_error(e, "Failed to prepare list_all_commands"))?;

    let rows = stmt
        .query_map([], |row| {
            let raw_keywords: String = row.get(7)?;
            let keywords = parse_stored_keywords(&raw_keywords);
            let line_i64: i64 = row.get(9)?;
            Ok(CommandEntry {
                id: row.get(0)?,
                note_rel_path: row.get(1)?,
                section: row.get(2)?,
                title: row.get(3)?,
                description: row.get(4)?,
                code: row.get(5)?,
                language: row.get(6)?,
                keywords,
                is_danger: row.get::<_, i64>(8)? != 0,
                line: line_i64.max(1) as u32,
            })
        })
        .map_err(|e| map_sqlite_error(e, "Failed to query commands"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to read command rows"))?;

    Ok(rows)
}

fn parse_stored_keywords(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
        return parsed;
    }
    trimmed
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Lists all `source_rel_path` values in `note_links` pointing to `target_rel_path`.
pub fn list_backlink_sources(
    conn: &Connection,
    target_rel_path: &str,
) -> Result<HashSet<String>, AppError> {
    let mut stmt = conn
        .prepare("SELECT source_rel_path FROM note_links WHERE target_rel_path = ?1")
        .map_err(|e| map_sqlite_error(e, "Failed to prepare list_backlink_sources"))?;

    let sources = stmt
        .query_map(params![target_rel_path], |row| row.get::<_, String>(0))
        .map_err(|e| map_sqlite_error(e, "Failed to query backlink sources"))?
        .collect::<Result<HashSet<String>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to read backlink source rows"))?;

    Ok(sources)
}

/// Lists all `asset_rel_path` values in `note_assets` referenced by `source_rel_path`.
pub fn list_note_assets_by_source(
    conn: &Connection,
    source_rel_path: &str,
) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare("SELECT asset_rel_path FROM note_assets WHERE source_rel_path = ?1")
        .map_err(|e| map_sqlite_error(e, "Failed to prepare list_note_assets_by_source"))?;

    let assets = stmt
        .query_map(params![source_rel_path], |row| row.get::<_, String>(0))
        .map_err(|e| map_sqlite_error(e, "Failed to query note assets by source"))?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to read note asset rows"))?;

    Ok(assets)
}

/// Lists all distinct `asset_rel_path` values currently referenced by any indexed note.
pub fn list_all_referenced_assets(conn: &Connection) -> Result<HashSet<String>, AppError> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT asset_rel_path FROM note_assets")
        .map_err(|e| map_sqlite_error(e, "Failed to prepare list_all_referenced_assets"))?;

    let assets = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| map_sqlite_error(e, "Failed to query all referenced assets"))?
        .collect::<Result<HashSet<String>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to read referenced asset rows"))?;

    Ok(assets)
}
