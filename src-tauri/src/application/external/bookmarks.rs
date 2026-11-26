use std::path::Path;

use chrono::Utc;

use crate::application::persistence::PersistenceManager;
use crate::domain::bookmark::{
    normalize_and_sort_bookmarks, normalize_bookmark_alias, normalize_bookmark_path,
    ExternalBookmark, ExternalBookmarkKind,
};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::settings::SettingsPatch;
use crate::infra::fs_browser::{self, PathEntryKind};

/// Returns all persisted external bookmarks from `settings.yaml` (normalized and sorted).
pub fn external_bookmarks_list(pm: &PersistenceManager) -> Result<Vec<ExternalBookmark>, AppError> {
    let settings = pm.settings_get()?.settings;
    Ok(normalize_and_sort_bookmarks(settings.external_bookmarks))
}

/// Adds or updates an external file/directory bookmark and persists it to `settings.yaml`.
pub fn external_bookmark_add(
    pm: &PersistenceManager,
    path: &str,
    alias: Option<String>,
    pinned: bool,
) -> Result<Vec<ExternalBookmark>, AppError> {
    let probe = fs_browser::probe_path(path);
    let normalized_path = normalize_bookmark_path(&probe.resolved_path).ok_or_else(|| {
        AppError::with_detail(
            ErrorCode::ValidationError,
            "Invalid external bookmark path",
            path.to_string(),
        )
    })?;

    let kind = match probe.kind {
        PathEntryKind::Directory => ExternalBookmarkKind::Directory,
        PathEntryKind::File => ExternalBookmarkKind::File,
        PathEntryKind::Other | PathEntryKind::NotFound => {
            if Path::new(&normalized_path).is_dir() {
                ExternalBookmarkKind::Directory
            } else {
                ExternalBookmarkKind::File
            }
        }
    };

    let mut bookmarks = pm.settings_get()?.settings.external_bookmarks;
    bookmarks.push(ExternalBookmark {
        path: normalized_path,
        kind,
        alias: normalize_bookmark_alias(alias.as_deref()),
        pinned,
        last_opened_at: Some(Utc::now().timestamp_millis()),
    });
    let normalized = normalize_and_sort_bookmarks(bookmarks);

    let snapshot = pm.settings_update(SettingsPatch {
        external_bookmarks: Some(normalized),
        ..SettingsPatch::default()
    })?;
    Ok(snapshot.settings.external_bookmarks)
}

/// Removes an external bookmark by path and persists the updated list to `settings.yaml`.
pub fn external_bookmark_remove(
    pm: &PersistenceManager,
    path: &str,
) -> Result<Vec<ExternalBookmark>, AppError> {
    let resolved = fs_browser::resolve_input_path(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());
    let target = normalize_bookmark_path(&resolved).ok_or_else(|| {
        AppError::with_detail(
            ErrorCode::ValidationError,
            "Invalid external bookmark path",
            path.to_string(),
        )
    })?;

    let bookmarks = pm.settings_get()?.settings.external_bookmarks;
    let remaining: Vec<ExternalBookmark> = bookmarks
        .into_iter()
        .filter(|b| normalize_bookmark_path(&b.path).as_deref() != Some(target.as_str()))
        .collect();

    let snapshot = pm.settings_update(SettingsPatch {
        external_bookmarks: Some(remaining),
        ..SettingsPatch::default()
    })?;
    Ok(snapshot.settings.external_bookmarks)
}
