use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::error::{AppError, ErrorCode};
use crate::infra::fs::map_io_error;

/// Initial SQLite schema migration (`C5.2`).
pub const MIGRATION_0001: &str = include_str!("../../migrations/0001_init.sql");

fn map_sqlite_error(err: rusqlite::Error, context: &str) -> AppError {
    AppError::with_detail(ErrorCode::InternalError, context, err.to_string())
}

/// Opens `index.sqlite`, enables `WAL` and `foreign_keys = ON`, and runs pending migrations (`C5.1`, `C5.2`).
pub fn init_sqlite(db_path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| map_io_error(e, "Failed to create SQLite parent directory"))?;
    }

    let mut conn =
        Connection::open(db_path).map_err(|e| map_sqlite_error(e, "Failed to open SQLite"))?;

    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
        .map_err(|e| map_sqlite_error(e, "Failed to configure SQLite pragmas"))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
           id TEXT PRIMARY KEY,
           applied_at INTEGER NOT NULL
         );",
    )
    .map_err(|e| map_sqlite_error(e, "Failed to ensure schema_migrations table"))?;

    let applied: Option<String> = conn
        .query_row(
            "SELECT id FROM schema_migrations WHERE id = ?1",
            params!["0001_init"],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "Failed to query schema_migrations"))?;

    if applied.is_none() {
        let tx = conn
            .transaction()
            .map_err(|e| map_sqlite_error(e, "Failed to begin migration transaction"))?;
        tx.execute_batch(MIGRATION_0001)
            .map_err(|e| map_sqlite_error(e, "Failed to execute 0001_init.sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (id, applied_at) VALUES (?1, ?2)",
            params!["0001_init", Utc::now().timestamp_millis()],
        )
        .map_err(|e| map_sqlite_error(e, "Failed to record 0001_init migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "Failed to commit 0001_init migration"))?;
    }

    Ok(conn)
}

/// Reads a value from `library_meta`.
pub fn get_library_meta(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    conn.query_row(
        "SELECT value FROM library_meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| map_sqlite_error(e, "Failed to read library_meta"))
}

/// Upserts a value in `library_meta`.
pub fn set_library_meta(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO library_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to write library_meta"))?;
    Ok(())
}

/// Returns total indexed notes count in `notes`.
pub fn count_notes(conn: &Connection) -> Result<u32, AppError> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
        .map_err(|e| map_sqlite_error(e, "Failed to count notes"))?;
    Ok(count.max(0) as u32)
}

/// Row payload for upserting a note into `notes`, `note_tags`, and `notes_fts`.
#[derive(Debug, Clone)]
pub struct NoteIndexRecord {
    pub key: String,
    pub id: Option<String>,
    pub rel_path: String,
    pub title: String,
    pub doc_type: String,
    pub project: Option<String>,
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
}

/// Lightweight row returned from `notes` queries.
#[derive(Debug, Clone)]
pub struct IndexedNoteRow {
    pub key: String,
    pub rel_path: String,
    pub title: String,
    pub doc_type: String,
    pub project: Option<String>,
    pub excerpt: String,
    pub pinned: bool,
    pub archived: bool,
    pub is_inbox: bool,
    pub mtime_ms: i64,
    pub meta_error: Option<String>,
    pub tags: Vec<String>,
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
            rec.project,
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

    Ok(())
}

/// Loads all indexed notes with their tags, ordered by `pinned DESC, mtime_ms DESC`.
pub fn list_all_indexed_notes(conn: &Connection) -> Result<Vec<IndexedNoteRow>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT key, rel_path, title, doc_type, project, excerpt,
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
                project: row.get(4)?,
                excerpt: row.get(5)?,
                pinned: row.get::<_, i64>(6)? != 0,
                archived: row.get::<_, i64>(7)? != 0,
                is_inbox: row.get::<_, i64>(8)? != 0,
                mtime_ms: row.get(9)?,
                meta_error: row.get(10)?,
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

    let mut tag_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
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

/// FTS5 search hit from `notes_fts` joined with `notes`.
#[derive(Debug, Clone)]
pub struct FtsSearchHit {
    pub key: String,
    pub rel_path: String,
    pub title: String,
    pub rank: f64,
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
