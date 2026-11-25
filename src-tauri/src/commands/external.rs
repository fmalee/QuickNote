use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use crate::application::external::{
    directory_workspace_config_write as directory_workspace_config_write_uc,
    external_action_run as external_action_run_uc,
    external_actions_resolve as external_actions_resolve_uc,
    external_bookmark_add as external_bookmark_add_uc,
    external_bookmark_remove as external_bookmark_remove_uc,
    external_list_directory as external_list_directory_uc,
    external_list_sibling_files as external_list_sibling_files_uc,
    external_probe_path as external_probe_path_uc,
};
use crate::application::persistence::PersistenceManager;
use crate::application::reconciler::reconcile_path;
use crate::domain::bookmark::ExternalBookmark;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::external::{ExternalActionState, ResolveExternalArgs, RunExternalArgs};
use crate::domain::workspace_context::{DirectoryWorkspaceConfig, DIRECTORY_CONFIG_FILENAME};
use crate::infra::external_open::ExternalLauncher;
use crate::infra::fs_browser::{
    DirectoryEntry, DirectoryListing, ListDirectoryOptions, PathEntryKind, PathProbeResult,
};
use crate::infra::sqlite::list_directory_workspace_configs as list_directory_workspace_configs_db;

/// Serializable IPC entry kind mirroring `infra::fs_browser::PathEntryKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPathEntryKind {
    File,
    Directory,
    Other,
    NotFound,
}

impl From<PathEntryKind> for ExternalPathEntryKind {
    fn from(kind: PathEntryKind) -> Self {
        match kind {
            PathEntryKind::File => Self::File,
            PathEntryKind::Directory => Self::Directory,
            PathEntryKind::Other => Self::Other,
            PathEntryKind::NotFound => Self::NotFound,
        }
    }
}

/// Serializable IPC result for `probe_external_path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalPathProbeResult {
    pub raw_input: String,
    pub resolved_path: String,
    pub exists: bool,
    pub kind: ExternalPathEntryKind,
    pub readable: bool,
    #[specta(type = Option<specta_typescript::Number>)]
    pub size: Option<u64>,
    pub is_text: bool,
    pub is_utf8: bool,
    pub is_hidden: bool,
}

impl From<PathProbeResult> for ExternalPathProbeResult {
    fn from(result: PathProbeResult) -> Self {
        Self {
            raw_input: result.raw_input,
            resolved_path: result.resolved_path,
            exists: result.exists,
            kind: result.kind.into(),
            readable: result.readable,
            size: result.size,
            is_text: result.is_text,
            is_utf8: result.is_utf8,
            is_hidden: result.is_hidden,
        }
    }
}

/// Serializable IPC entry for external directory listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalDirectoryEntry {
    pub name: String,
    pub path: String,
    pub rel_path: String,
    pub kind: ExternalPathEntryKind,
    #[specta(type = specta_typescript::Number)]
    pub depth: usize,
    #[specta(type = Option<specta_typescript::Number>)]
    pub size: Option<u64>,
    pub is_hidden: bool,
    pub match_score: u32,
}

impl From<DirectoryEntry> for ExternalDirectoryEntry {
    fn from(entry: DirectoryEntry) -> Self {
        Self {
            name: entry.name,
            path: entry.path,
            rel_path: entry.rel_path,
            kind: entry.kind.into(),
            depth: entry.depth,
            size: entry.size,
            is_hidden: entry.is_hidden,
            match_score: entry.match_score,
        }
    }
}

/// Serializable IPC listing payload for `list_external_directory` and `list_current_doc_siblings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalDirectoryListing {
    pub root_path: String,
    pub entries: Vec<ExternalDirectoryEntry>,
    pub truncated: bool,
    #[specta(type = specta_typescript::Number)]
    pub visited_count: usize,
}

impl From<DirectoryListing> for ExternalDirectoryListing {
    fn from(listing: DirectoryListing) -> Self {
        Self {
            root_path: listing.root_path,
            entries: listing
                .entries
                .into_iter()
                .map(ExternalDirectoryEntry::from)
                .collect(),
            truncated: listing.truncated,
            visited_count: listing.visited_count,
        }
    }
}

/// Serializable IPC entry for `list_directory_workspace_configs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryWorkspaceConfigEntry {
    pub vault_rel_dir: String,
    pub config: Option<DirectoryWorkspaceConfig>,
    #[specta(type = specta_typescript::Number)]
    pub mtime_ms: i64,
    pub parse_error: Option<String>,
}

#[tauri::command]
pub fn external_actions_resolve(
    pm: State<'_, Arc<PersistenceManager>>,
    args: ResolveExternalArgs,
) -> Result<Vec<ExternalActionState>, AppError> {
    external_actions_resolve_uc(&pm, args)
}

#[tauri::command]
pub fn external_action_run(
    pm: State<'_, Arc<PersistenceManager>>,
    launcher: State<'_, Arc<dyn ExternalLauncher>>,
    args: RunExternalArgs,
) -> Result<(), AppError> {
    external_action_run_uc(&pm, launcher.inner().as_ref(), args)
}

#[tauri::command]
pub fn probe_external_path(raw_path: String) -> ExternalPathProbeResult {
    external_probe_path_uc(&raw_path).into()
}

#[tauri::command]
pub fn list_external_directory(
    dir_path: String,
    query: Option<String>,
    include_hidden: bool,
) -> Result<ExternalDirectoryListing, AppError> {
    let opts = ListDirectoryOptions {
        include_hidden,
        query,
        ..ListDirectoryOptions::default()
    };
    external_list_directory_uc(&dir_path, Some(opts)).map(ExternalDirectoryListing::from)
}

#[tauri::command]
pub fn list_current_doc_siblings(doc_path: String) -> Result<ExternalDirectoryListing, AppError> {
    external_list_sibling_files_uc(&doc_path, None).map(ExternalDirectoryListing::from)
}

#[tauri::command]
pub fn upsert_external_bookmark(
    pm: State<'_, Arc<PersistenceManager>>,
    bookmark: ExternalBookmark,
) -> Result<Vec<ExternalBookmark>, AppError> {
    external_bookmark_add_uc(&pm, &bookmark.path, bookmark.alias, bookmark.pinned)
}

#[tauri::command]
pub fn remove_external_bookmark(
    pm: State<'_, Arc<PersistenceManager>>,
    path: String,
) -> Result<Vec<ExternalBookmark>, AppError> {
    external_bookmark_remove_uc(&pm, &path)
}

#[tauri::command]
pub fn list_directory_workspace_configs(
    pm: State<'_, Arc<PersistenceManager>>,
) -> Result<Vec<DirectoryWorkspaceConfigEntry>, AppError> {
    let records = pm.with_db(list_directory_workspace_configs_db)?;
    Ok(records
        .into_iter()
        .map(|rec| {
            let (config, parse_error) = if let Some(err) = rec.parse_error {
                (None, Some(err))
            } else {
                match serde_json::from_str::<DirectoryWorkspaceConfig>(&rec.config_json) {
                    Ok(cfg) => (Some(cfg), None),
                    Err(err) => (None, Some(err.to_string())),
                }
            };
            DirectoryWorkspaceConfigEntry {
                vault_rel_dir: rec.vault_rel_dir,
                config,
                mtime_ms: rec.mtime_ms,
                parse_error,
            }
        })
        .collect())
}

fn normalize_vault_rel_dir_checked(raw: &str) -> Option<String> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.starts_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for segment in trimmed.split('/') {
        match segment {
            "" | "." => continue,
            ".." => return None,
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

#[tauri::command]
pub fn set_directory_workspace_config(
    pm: State<'_, Arc<PersistenceManager>>,
    vault_rel_dir: String,
    config: Option<DirectoryWorkspaceConfig>,
) -> Result<(), AppError> {
    match config {
        Some(cfg) => {
            directory_workspace_config_write_uc(&pm, &vault_rel_dir, &cfg)?;
            Ok(())
        }
        None => {
            let root_str = pm.current_library_root()?.ok_or_else(|| {
                AppError::new(
                    ErrorCode::LibraryNotConfigured,
                    "Library root is not configured",
                )
            })?;
            let normalized_dir =
                normalize_vault_rel_dir_checked(&vault_rel_dir).ok_or_else(|| {
                    AppError::with_detail(
                        ErrorCode::ValidationError,
                        "Invalid vault relative directory for .quicknote.yaml",
                        vault_rel_dir.clone(),
                    )
                })?;
            let rel_path = if normalized_dir.is_empty() {
                DIRECTORY_CONFIG_FILENAME.to_string()
            } else {
                format!("{normalized_dir}/{DIRECTORY_CONFIG_FILENAME}")
            };
            let target_path = Path::new(&root_str).join(&rel_path);
            if target_path.exists() {
                pm.record_self_write(&target_path, "");
                if let Err(err) = std::fs::remove_file(&target_path) {
                    if err.kind() != std::io::ErrorKind::NotFound {
                        return Err(AppError::with_detail(
                            ErrorCode::InternalError,
                            "Failed to remove directory .quicknote.yaml",
                            err.to_string(),
                        ));
                    }
                }
                pm.record_self_write(&target_path, "");
            }
            pm.with_db(|conn| reconcile_path(Path::new(&root_str), &rel_path, conn))?;
            Ok(())
        }
    }
}
