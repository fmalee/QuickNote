use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::error::{AppError, ErrorCode};
use crate::infra::fs::map_io_error;

use super::map_sqlite_error;

/// Initial SQLite schema migration (`C5.2`).
pub const MIGRATION_0001: &str = include_str!("../../../migrations/0001_init.sql");

/// Directory workspace `.quicknote.yaml` cache migration (`C5.2`).
pub const MIGRATION_0002: &str =
    include_str!("../../../migrations/0002_directory_workspace_cache.sql");

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", MIGRATION_0001),
    ("0002_directory_workspace_cache", MIGRATION_0002),
];

const EXPECTED_TABLES: &[&str] = &[
    "schema_migrations",
    "library_meta",
    "notes",
    "note_tags",
    "note_links",
    "commands",
    "notes_fts",
    "directory_workspace_configs",
];

const EXPECTED_NOTES_COLUMNS: &[&str] = &[
    "key",
    "id",
    "rel_path",
    "title",
    "doc_type",
    "project",
    "mtime_ms",
    "size_bytes",
    "body_hash",
    "frontmatter_json",
    "excerpt",
    "meta_error",
    "pinned",
    "archived",
    "is_inbox",
    "updated_at",
];

const EXPECTED_COMMANDS_COLUMNS: &[&str] = &[
    "id",
    "note_rel_path",
    "section",
    "title",
    "description",
    "code",
    "language",
    "keywords",
    "is_danger",
    "line",
];

const EXPECTED_DIRECTORY_WORKSPACE_CONFIGS_COLUMNS: &[&str] =
    &["vault_rel_dir", "config_json", "mtime_ms", "parse_error"];

fn sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut os = db_path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

/// Safely removes a SQLite database file and its `-wal`, `-shm`, and `-journal` sidecars.
pub fn remove_sqlite_files(db_path: &Path) -> Result<(), AppError> {
    let targets = [
        db_path.to_path_buf(),
        sidecar_path(db_path, "-wal"),
        sidecar_path(db_path, "-shm"),
        sidecar_path(db_path, "-journal"),
    ];

    for target in targets {
        match std::fs::remove_file(&target) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(map_io_error(
                    err,
                    "Failed to remove stale SQLite cache file",
                ));
            }
        }
    }

    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, AppError> {
    let found: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1",
            params![table],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "Failed to inspect sqlite_master"))?;
    Ok(found.is_some())
}

fn verify_table_columns(
    conn: &Connection,
    table: &str,
    expected_columns: &[&str],
) -> Result<(), AppError> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut stmt = conn
        .prepare(&pragma)
        .map_err(|e| map_sqlite_error(e, "Failed to inspect SQLite table columns"))?;
    let cols = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| map_sqlite_error(e, "Failed to query SQLite table_info"))?
        .collect::<Result<HashSet<String>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to collect SQLite table columns"))?;

    for expected in expected_columns {
        if !cols.contains(*expected) {
            return Err(AppError::new(
                ErrorCode::InternalError,
                "SQLite schema column mismatch",
            ));
        }
    }
    Ok(())
}

fn validate_sqlite_schema(conn: &Connection) -> Result<(), AppError> {
    let integrity: String = conn
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|e| map_sqlite_error(e, "SQLite integrity check failed"))?;
    if integrity != "ok" {
        return Err(AppError::new(
            ErrorCode::InternalError,
            "SQLite quick_check reported corruption",
        ));
    }

    for table in EXPECTED_TABLES {
        if !table_exists(conn, table)? {
            return Err(AppError::new(
                ErrorCode::InternalError,
                "SQLite schema missing required table",
            ));
        }
    }

    let mut mig_stmt = conn
        .prepare("SELECT id FROM schema_migrations")
        .map_err(|e| map_sqlite_error(e, "Failed to query schema_migrations"))?;
    let applied_ids = mig_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| map_sqlite_error(e, "Failed to read schema_migrations"))?
        .collect::<Result<HashSet<String>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to collect schema_migrations"))?;

    let expected_ids: HashSet<&str> = MIGRATIONS.iter().map(|(id, _)| *id).collect();
    if applied_ids.len() != expected_ids.len()
        || applied_ids
            .iter()
            .any(|id| !expected_ids.contains(id.as_str()))
    {
        return Err(AppError::new(
            ErrorCode::InternalError,
            "Incompatible SQLite migration state",
        ));
    }

    verify_table_columns(conn, "notes", EXPECTED_NOTES_COLUMNS)?;
    verify_table_columns(conn, "commands", EXPECTED_COMMANDS_COLUMNS)?;
    verify_table_columns(
        conn,
        "directory_workspace_configs",
        EXPECTED_DIRECTORY_WORKSPACE_CONFIGS_COLUMNS,
    )?;

    Ok(())
}

fn try_init_sqlite(db_path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| map_io_error(e, "Failed to create SQLite parent directory"))?;
    }

    let mut conn =
        Connection::open(db_path).map_err(|e| map_sqlite_error(e, "Failed to open SQLite"))?;

    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
        .map_err(|e| map_sqlite_error(e, "Failed to configure SQLite pragmas"))?;

    let integrity: String = conn
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|e| map_sqlite_error(e, "SQLite quick_check failed"))?;
    if integrity != "ok" {
        return Err(AppError::new(
            ErrorCode::InternalError,
            "SQLite quick_check reported corruption",
        ));
    }

    let has_migrations_table = table_exists(&conn, "schema_migrations")?;
    let applied_ids: HashSet<String> = if has_migrations_table {
        let mut mig_stmt = conn
            .prepare("SELECT id FROM schema_migrations")
            .map_err(|e| map_sqlite_error(e, "Failed to query schema_migrations"))?;
        let ids = mig_stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| map_sqlite_error(e, "Failed to read schema_migrations"))?
            .collect::<Result<HashSet<String>, _>>()
            .map_err(|e| map_sqlite_error(e, "Failed to collect schema_migrations"))?;
        ids
    } else {
        HashSet::new()
    };

    let known_ids: HashSet<&str> = MIGRATIONS.iter().map(|(id, _)| *id).collect();
    if applied_ids
        .iter()
        .any(|id| !known_ids.contains(id.as_str()))
    {
        return Err(AppError::new(
            ErrorCode::InternalError,
            "Incompatible SQLite migration state",
        ));
    }

    if applied_ids.is_empty() {
        for table in EXPECTED_TABLES {
            if *table != "schema_migrations" && table_exists(&conn, table)? {
                return Err(AppError::new(
                    ErrorCode::InternalError,
                    "Incompatible SQLite schema without 0001_init migration",
                ));
            }
        }
    } else if !applied_ids.contains("0001_init") {
        return Err(AppError::new(
            ErrorCode::InternalError,
            "Incompatible SQLite migration state missing 0001_init",
        ));
    }

    if !has_migrations_table {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
               id TEXT PRIMARY KEY,
               applied_at INTEGER NOT NULL
             );",
        )
        .map_err(|e| map_sqlite_error(e, "Failed to ensure schema_migrations table"))?;
    }

    for (id, sql) in MIGRATIONS {
        if applied_ids.contains(*id) {
            continue;
        }
        let tx = conn
            .transaction()
            .map_err(|e| map_sqlite_error(e, "Failed to begin migration transaction"))?;
        tx.execute_batch(sql)
            .map_err(|e| map_sqlite_error(e, &format!("Failed to execute {id} migration")))?;
        tx.execute(
            "INSERT INTO schema_migrations (id, applied_at) VALUES (?1, ?2)",
            params![id, Utc::now().timestamp_millis()],
        )
        .map_err(|e| map_sqlite_error(e, &format!("Failed to record {id} migration")))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, &format!("Failed to commit {id} migration")))?;
    }

    validate_sqlite_schema(&conn)?;
    Ok(conn)
}

/// Opens the SQLite cache DB, enables `WAL` and `foreign_keys = ON`, and runs pending migrations (`C5.1`, `C5.2`).
/// When the database file is missing, corrupted, or schema-incompatible, safely removes stale cache files and rebuilds.
pub fn init_sqlite(db_path: &Path) -> Result<Connection, AppError> {
    match try_init_sqlite(db_path) {
        Ok(conn) => Ok(conn),
        Err(err) => {
            eprintln!(
                "[sqlite] cache init/verify failed ({}); removing and rebuilding cache",
                err.message
            );
            remove_sqlite_files(db_path)?;
            try_init_sqlite(db_path)
        }
    }
}
