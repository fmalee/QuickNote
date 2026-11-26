use rusqlite::{params, params_from_iter, Connection};

use crate::domain::error::AppError;

use super::map_sqlite_error;

/// Cached row from `directory_workspace_configs` representing a `.quicknote.yaml` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryWorkspaceConfigRecord {
    pub vault_rel_dir: String,
    pub config_json: String,
    pub mtime_ms: i64,
    pub parse_error: Option<String>,
}

fn normalize_vault_rel_dir(raw: &str) -> String {
    raw.replace('\\', "/")
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn collect_ancestor_dirs(vault_rel_dir: &str) -> Vec<String> {
    let normalized = normalize_vault_rel_dir(vault_rel_dir);
    let mut ancestors = Vec::new();
    ancestors.push(String::new());
    if !normalized.is_empty() {
        let mut current = String::new();
        for segment in normalized.split('/') {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(segment);
            ancestors.push(current.clone());
        }
    }
    ancestors
}

/// Upserts a cached directory workspace config (`.quicknote.yaml`) by `vault_rel_dir`.
pub fn upsert_directory_workspace_config(
    conn: &Connection,
    rec: &DirectoryWorkspaceConfigRecord,
) -> Result<(), AppError> {
    let normalized_dir = normalize_vault_rel_dir(&rec.vault_rel_dir);
    conn.execute(
        "INSERT INTO directory_workspace_configs (
            vault_rel_dir, config_json, mtime_ms, parse_error
         ) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(vault_rel_dir) DO UPDATE SET
            config_json = excluded.config_json,
            mtime_ms = excluded.mtime_ms,
            parse_error = excluded.parse_error",
        params![
            normalized_dir,
            rec.config_json,
            rec.mtime_ms,
            rec.parse_error
        ],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to upsert directory_workspace_configs"))?;
    Ok(())
}

/// Deletes a cached directory workspace config by `vault_rel_dir`.
pub fn delete_directory_workspace_config(
    conn: &Connection,
    vault_rel_dir: &str,
) -> Result<(), AppError> {
    let normalized_dir = normalize_vault_rel_dir(vault_rel_dir);
    conn.execute(
        "DELETE FROM directory_workspace_configs WHERE vault_rel_dir = ?1",
        params![normalized_dir],
    )
    .map_err(|e| map_sqlite_error(e, "Failed to delete directory_workspace_configs"))?;
    Ok(())
}

/// Lists all cached directory workspace configs ordered by `vault_rel_dir ASC`.
pub fn list_directory_workspace_configs(
    conn: &Connection,
) -> Result<Vec<DirectoryWorkspaceConfigRecord>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT vault_rel_dir, config_json, mtime_ms, parse_error
             FROM directory_workspace_configs
             ORDER BY vault_rel_dir ASC",
        )
        .map_err(|e| map_sqlite_error(e, "Failed to prepare list_directory_workspace_configs"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DirectoryWorkspaceConfigRecord {
                vault_rel_dir: row.get(0)?,
                config_json: row.get(1)?,
                mtime_ms: row.get(2)?,
                parse_error: row.get(3)?,
            })
        })
        .map_err(|e| map_sqlite_error(e, "Failed to query directory_workspace_configs"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "Failed to read directory_workspace_configs rows"))?;

    Ok(rows)
}

/// Batch-queries cached directory workspace configs for `vault_rel_dir` and all of its ancestors
/// (including vault root `""`), ordered from nearest ancestor to vault root.
pub fn list_ancestor_directory_workspace_configs(
    conn: &Connection,
    vault_rel_dir: &str,
) -> Result<Vec<DirectoryWorkspaceConfigRecord>, AppError> {
    let ancestors = collect_ancestor_dirs(vault_rel_dir);
    let placeholders = (1..=ancestors.len())
        .map(|idx| format!("?{idx}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT vault_rel_dir, config_json, mtime_ms, parse_error
         FROM directory_workspace_configs
         WHERE vault_rel_dir IN ({placeholders})
         ORDER BY LENGTH(vault_rel_dir) DESC, vault_rel_dir ASC"
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| {
        map_sqlite_error(
            e,
            "Failed to prepare list_ancestor_directory_workspace_configs",
        )
    })?;

    let rows = stmt
        .query_map(params_from_iter(ancestors.iter()), |row| {
            Ok(DirectoryWorkspaceConfigRecord {
                vault_rel_dir: row.get(0)?,
                config_json: row.get(1)?,
                mtime_ms: row.get(2)?,
                parse_error: row.get(3)?,
            })
        })
        .map_err(|e| map_sqlite_error(e, "Failed to query ancestor directory_workspace_configs"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            map_sqlite_error(
                e,
                "Failed to read ancestor directory_workspace_configs rows",
            )
        })?;

    Ok(rows)
}
