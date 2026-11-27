mod batch;
mod crud;
mod media;
mod nav;
mod search;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;

use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::format_ulid;
use crate::infra::fs::map_io_error;

pub use batch::{notes_backfill_missing_ids, tags_batch_rename};
pub use crud::{
    note_create, note_delete, note_delete_with, note_read, note_rename, note_update_meta,
    note_write, MAX_EDITABLE_FILE_BYTES,
};
pub use media::{note_save_image, MAX_IMAGE_BYTES};
pub use nav::{notes_get_tree_and_tags, notes_list};
pub use search::{commands_search, notes_get_related, search_notes_and_files};

static ULID_SEQ: AtomicU64 = AtomicU64::new(1);

pub(super) fn generate_ulid() -> String {
    let ts_ms = Utc::now().timestamp_millis().max(0) as u64;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = ULID_SEQ.fetch_add(1, Ordering::Relaxed) as u128;
    let random = (nanos << 32) ^ (seq.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    format_ulid(ts_ms, random)
}

pub(super) fn file_mtime_and_size(path: &Path) -> Result<(i64, u64), AppError> {
    let meta = fs::metadata(path).map_err(|e| map_io_error(e, "Failed to stat note file"))?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    Ok((mtime_ms, meta.len()))
}

pub(super) fn require_library_root(pm: &PersistenceManager) -> Result<PathBuf, AppError> {
    let root = pm.current_library_root()?.ok_or_else(|| {
        AppError::new(
            ErrorCode::LibraryNotConfigured,
            "Library root is not configured",
        )
    })?;
    Ok(PathBuf::from(root))
}

pub(super) fn resolve_rel_path(root: &Path, abs_path: &Path) -> Option<String> {
    if let Ok(rel) = abs_path.strip_prefix(root) {
        return Some(rel.to_string_lossy().replace('\\', "/"));
    }
    let canon_root = fs::canonicalize(root).ok()?;
    let canon_abs = fs::canonicalize(abs_path).ok()?;
    canon_abs
        .strip_prefix(&canon_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

pub(super) fn sanitize_filename(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for c in title.trim().chars() {
        if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('-');
        } else {
            out.push(c);
        }
    }
    let trimmed = out.trim().trim_matches('.');
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn normalize_rel_path(rel: &str) -> Result<String, AppError> {
    let cleaned = rel.trim().replace('\\', "/");
    let trimmed = cleaned.trim_start_matches('/');
    if trimmed.is_empty() || trimmed.contains("..") {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Invalid relative path",
        ));
    }
    Ok(trimmed.to_string())
}

pub(super) fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

pub(super) fn expand_input_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn is_external_file_pinned(pm: &PersistenceManager, abs_path: &Path) -> bool {
    let Ok(pinned_list) = pm.list_pinned_files() else {
        return false;
    };
    let target = normalize_path_for_compare(abs_path);
    pinned_list.into_iter().any(|item| {
        let candidate = expand_input_path(&item);
        normalize_path_for_compare(&candidate) == target
    })
}
