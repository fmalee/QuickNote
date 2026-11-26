mod notes;
mod schema;
mod workspace_configs;

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::error::{AppError, ErrorCode};

pub use notes::{
    delete_note_index_by_rel_path, get_note_key_by_rel_path, list_all_commands,
    list_all_indexed_notes, list_backlink_sources, replace_all_notes_index, search_notes_fts,
    upsert_note_index, FtsSearchHit, IndexedNoteRow, NoteIndexRecord,
};
pub use schema::{init_sqlite, remove_sqlite_files, MIGRATION_0001, MIGRATION_0002};
pub use workspace_configs::{
    delete_directory_workspace_config, list_ancestor_directory_workspace_configs,
    list_directory_workspace_configs, upsert_directory_workspace_config,
    DirectoryWorkspaceConfigRecord,
};

pub(super) fn map_sqlite_error(err: rusqlite::Error, context: &str) -> AppError {
    AppError::with_detail(ErrorCode::InternalError, context, err.to_string())
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
