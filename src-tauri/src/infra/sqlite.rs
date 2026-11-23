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
