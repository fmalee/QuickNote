use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::settings::{LibraryInfo, SetRootArgs};
use crate::infra::fs::{ensure_library_skeleton, expand_tilde};
use crate::infra::sqlite::{count_notes, get_library_meta, set_library_meta};

/// Returns the current configured library summary (`C4.2`, `M0-02`).
pub fn get_library_info(pm: &PersistenceManager) -> Result<LibraryInfo, AppError> {
    let root = pm.current_library_root()?;
    let (note_count, last_indexed_at) = pm.with_db(|conn| {
        let count = count_notes(conn)?;
        let indexed =
            get_library_meta(conn, "last_indexed_at")?.and_then(|s| s.parse::<i64>().ok());
        Ok((count, indexed))
    })?;

    Ok(LibraryInfo {
        root,
        note_count,
        last_indexed_at,
    })
}

/// Configures `library_root`, optionally creating the directory and skeleton files (`C4.2`, `M0-02`).
pub fn set_library_root(
    pm: &PersistenceManager,
    args: SetRootArgs,
) -> Result<LibraryInfo, AppError> {
    let trimmed = args.root.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Library root path cannot be empty",
        ));
    }

    let expanded = expand_tilde(trimmed);
    if !expanded.exists() {
        if !args.create_if_missing {
            return Err(AppError::with_detail(
                ErrorCode::NotFound,
                "Library root directory does not exist",
                expanded.to_string_lossy().to_string(),
            ));
        }
        ensure_library_skeleton(&expanded)?;
    } else if !expanded.is_dir() {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Library root path must be a directory",
            expanded.to_string_lossy().to_string(),
        ));
    } else if args.create_if_missing {
        ensure_library_skeleton(&expanded)?;
    }

    let resolved = expanded.to_string_lossy().to_string();
    pm.with_db(|conn| set_library_meta(conn, "library_root", &resolved))?;
    pm.update_library_root_in_settings(&resolved)?;

    get_library_info(pm)
}
