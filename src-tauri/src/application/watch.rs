use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::application::index::index_single_note;
use crate::application::persistence::{canonicalize_watch_path, PersistenceManager};
use crate::application::reconciler::{
    reconcile_vault, reconcile_watched_path, ReconcilePathOutcome,
};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::{compute_body_hash, parse_note};
use crate::domain::watch::{ExternalChangeEvent, ExternalChangeKind};
use crate::domain::workspace_context::DIRECTORY_CONFIG_FILENAME;
use crate::infra::fs::map_io_error;
use crate::infra::sqlite::{delete_note_index_by_rel_path, get_note_key_by_rel_path};

/// Outcome of handling a filesystem watch event under `library_root`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultWatchOutcome {
    Noop,
    ExternalChange(Box<ExternalChangeEvent>),
    ReconciledVault,
}

/// Handles any watched vault path (`.md`, `.quicknote.yaml`, or directory move/delete) and
/// converges disk state into SQLite cache (`C6.1`, `C8.5`).
pub fn handle_vault_watch_path(
    pm: &PersistenceManager,
    path: &Path,
) -> Result<VaultWatchOutcome, AppError> {
    if is_markdown_path(path) {
        return match process_external_fs_path(pm, path)? {
            Some(event) => Ok(VaultWatchOutcome::ExternalChange(Box::new(event))),
            None => Ok(VaultWatchOutcome::Noop),
        };
    }

    let Some(root_str) = pm.current_library_root()? else {
        return Ok(VaultWatchOutcome::Noop);
    };
    let root = PathBuf::from(&root_str);
    if !root.is_dir() {
        return Ok(VaultWatchOutcome::Noop);
    }

    let canonical_root = PathBuf::from(canonicalize_watch_path(&root));
    let canonical_path = PathBuf::from(canonicalize_watch_path(path));
    let Ok(rel) = canonical_path.strip_prefix(&canonical_root) else {
        return Ok(VaultWatchOutcome::Noop);
    };

    if is_directory_config_path(path) {
        let outcome = reconcile_watched_path(pm, &root, path)?;
        return match outcome {
            ReconcilePathOutcome::ConfigUpserted
            | ReconcilePathOutcome::ConfigPruned
            | ReconcilePathOutcome::ConfigParseError(_) => Ok(VaultWatchOutcome::ReconciledVault),
            _ => Ok(VaultWatchOutcome::Noop),
        };
    }

    if is_ignored_rel_path(rel) {
        return Ok(VaultWatchOutcome::Noop);
    }

    if canonical_path.is_dir() || (!canonical_path.exists() && path.extension().is_none()) {
        let report = pm.with_db(|conn| reconcile_vault(&root, conn))?;
        if report.notes_upserted > 0
            || report.notes_pruned > 0
            || report.configs_upserted > 0
            || report.configs_pruned > 0
        {
            return Ok(VaultWatchOutcome::ReconciledVault);
        }
    }

    Ok(VaultWatchOutcome::Noop)
}

fn is_directory_config_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == DIRECTORY_CONFIG_FILENAME)
}

/// Processes a filesystem change path under `library_root` with `SelfWriteGuard` suppression (`C8.5`).
pub fn process_external_fs_path(
    pm: &PersistenceManager,
    path: &Path,
) -> Result<Option<ExternalChangeEvent>, AppError> {
    if !is_markdown_path(path) {
        return Ok(None);
    }

    let Some(root_str) = pm.current_library_root()? else {
        return Ok(None);
    };
    let root = PathBuf::from(&root_str);
    if !root.is_dir() {
        return Ok(None);
    }

    let canonical_root = PathBuf::from(canonicalize_watch_path(&root));
    let canonical_path = PathBuf::from(canonicalize_watch_path(path));

    let Ok(rel) = canonical_path.strip_prefix(&canonical_root) else {
        return Ok(None);
    };

    if is_ignored_rel_path(rel) {
        return Ok(None);
    }

    let rel_path = rel_to_slash_string(rel);
    if rel_path.is_empty() {
        return Ok(None);
    }

    let abs_path_str = root.join(&rel_path).to_string_lossy().to_string();

    if !canonical_path.exists() {
        let was_indexed = pm
            .with_db(|conn| get_note_key_by_rel_path(conn, &rel_path))?
            .is_some();
        pm.clear_self_write(&canonical_path);
        if !was_indexed {
            return Ok(None);
        }
        pm.with_db(|conn| delete_note_index_by_rel_path(conn, &rel_path))?;
        return Ok(Some(ExternalChangeEvent {
            abs_path: abs_path_str,
            kind: ExternalChangeKind::Deleted,
            new_body: None,
            new_body_hash: None,
            new_meta: None,
        }));
    }

    if !canonical_path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&canonical_path)
        .map_err(|e| map_io_error(e, "Failed to read externally modified note"))?;
    let stem = canonical_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");
    let parsed = parse_note(&raw, stem);
    let disk_body_hash = compute_body_hash(&parsed.body);
    let (mtime_ms, size_bytes) = file_mtime_and_size(&canonical_path)?;

    if pm.should_suppress_external_modify(&canonical_path, &disk_body_hash) {
        eprintln!("[vault-watch] event=vault_change_suppressed");
        index_single_note(pm, &rel_path, &raw, mtime_ms, size_bytes as i64)?;
        return Ok(None);
    }

    index_single_note(pm, &rel_path, &raw, mtime_ms, size_bytes as i64)?;

    Ok(Some(ExternalChangeEvent {
        abs_path: abs_path_str,
        kind: ExternalChangeKind::Modified,
        new_body: Some(parsed.body),
        new_body_hash: Some(disk_body_hash),
        new_meta: Some(parsed.meta),
    }))
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

fn is_ignored_rel_path(rel: &Path) -> bool {
    for (idx, comp) in rel.components().enumerate() {
        if let Component::Normal(part) = comp {
            let s = part.to_string_lossy();
            if s.starts_with('.') || s.contains(".tmp.") {
                return true;
            }
            if idx == 0 && s == "assets" {
                return true;
            }
        }
    }
    false
}

fn rel_to_slash_string(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn file_mtime_and_size(path: &Path) -> Result<(i64, u64), AppError> {
    let meta = fs::metadata(path).map_err(|e| map_io_error(e, "Failed to read file metadata"))?;
    let modified = meta
        .modified()
        .map_err(|e| map_io_error(e, "Failed to read file mtime"))?;
    let mtime_ms = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Invalid file modification timestamp",
                err.to_string(),
            )
        })?
        .as_millis() as i64;
    Ok((mtime_ms, meta.len()))
}
