use std::fs;
use std::path::{Path, PathBuf};

use crate::application::persistence::PersistenceManager;
use crate::application::reconciler::{reconcile_path, write_directory_workspace_config};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::parse_note;
use crate::domain::workspace_context::{
    resolve_effective_workspace, DirectoryWorkspaceConfig, EffectiveWorkspaceContext,
    DIRECTORY_CONFIG_FILENAME,
};
use crate::infra::sqlite::list_ancestor_directory_workspace_configs;
use crate::infra::trash::{default_file_remover, FileRemover};

use super::{expand_optional_path, resolve_sentinel_path};

/// Reads the directory-level `.quicknote.yaml` workspace config for `vault_rel_dir`
/// (from SQLite cache with direct disk fallback if not yet cached).
pub fn directory_workspace_config_get(
    pm: &PersistenceManager,
    vault_rel_dir: &str,
) -> Result<Option<DirectoryWorkspaceConfig>, AppError> {
    let normalized_dir = normalize_vault_rel_dir(vault_rel_dir);

    let cached = pm.with_db(|conn| {
        let records = list_ancestor_directory_workspace_configs(conn, &normalized_dir)?;
        Ok(records
            .into_iter()
            .find(|r| r.vault_rel_dir == normalized_dir))
    })?;

    if let Some(rec) = cached {
        if rec.parse_error.is_some() {
            return Ok(None);
        }
        let parsed =
            serde_json::from_str::<DirectoryWorkspaceConfig>(&rec.config_json).map_err(|err| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to decode cached directory workspace config",
                    err.to_string(),
                )
            })?;
        return Ok(Some(parsed));
    }

    let Some(root_str) = pm.current_library_root()? else {
        return Ok(None);
    };
    let config_path = if normalized_dir.is_empty() {
        Path::new(&root_str).join(DIRECTORY_CONFIG_FILENAME)
    } else {
        Path::new(&root_str)
            .join(&normalized_dir)
            .join(DIRECTORY_CONFIG_FILENAME)
    };
    if !config_path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&config_path).map_err(|err| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to read directory .quicknote.yaml",
            err.to_string(),
        )
    })?;
    if raw.trim().is_empty() {
        return Ok(Some(DirectoryWorkspaceConfig::default()));
    }
    let parsed = serde_yaml::from_str::<DirectoryWorkspaceConfig>(&raw).map_err(|err| {
        AppError::with_detail(
            ErrorCode::ValidationError,
            "Invalid directory .quicknote.yaml syntax",
            err.to_string(),
        )
    })?;
    Ok(Some(parsed))
}

/// Atomically writes a directory-level `.quicknote.yaml` via `reconciler::write_directory_workspace_config`
/// (with SelfWriteGuard protection and immediate SQLite convergence).
pub fn directory_workspace_config_write(
    pm: &PersistenceManager,
    vault_rel_dir: &str,
    config: &DirectoryWorkspaceConfig,
) -> Result<PathBuf, AppError> {
    let root_str = pm.current_library_root()?.ok_or_else(|| {
        AppError::new(
            ErrorCode::LibraryNotConfigured,
            "Library root is not configured",
        )
    })?;
    write_directory_workspace_config(pm, Path::new(&root_str), vault_rel_dir, config)
}

/// Normalizes and validates a vault-relative directory path (rejecting absolute paths and `..`).
pub fn normalize_vault_rel_dir_checked(raw: &str) -> Option<String> {
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

/// Removes a directory-level `.quicknote.yaml` by moving it to the system Trash
/// (with SelfWriteGuard protection and immediate SQLite convergence).
pub fn directory_workspace_config_delete_uc(
    pm: &PersistenceManager,
    vault_rel_dir: &str,
) -> Result<(), AppError> {
    directory_workspace_config_delete_uc_with(pm, vault_rel_dir, default_file_remover().as_ref())
}

/// Injectable variant of [`directory_workspace_config_delete_uc`] for tests.
pub fn directory_workspace_config_delete_uc_with(
    pm: &PersistenceManager,
    vault_rel_dir: &str,
    remover: &dyn FileRemover,
) -> Result<(), AppError> {
    let root_str = pm.current_library_root()?.ok_or_else(|| {
        AppError::new(
            ErrorCode::LibraryNotConfigured,
            "Library root is not configured",
        )
    })?;
    let normalized_dir = normalize_vault_rel_dir_checked(vault_rel_dir).ok_or_else(|| {
        AppError::with_detail(
            ErrorCode::ValidationError,
            "Invalid vault relative directory for .quicknote.yaml",
            vault_rel_dir.to_string(),
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
        remover.remove(&target_path)?;
        pm.record_self_write(&target_path, "");
    }
    pm.with_db(|conn| reconcile_path(Path::new(&root_str), &rel_path, conn))?;
    Ok(())
}

/// Resolves the effective workspace context for `abs_path` using the 3-tier cascade:
/// **Note Frontmatter > Nearest Ancestor `.quicknote.yaml` > Global Default**.
pub fn resolve_effective_workspace_for_path(
    pm: &PersistenceManager,
    abs_path: Option<&str>,
) -> Result<EffectiveWorkspaceContext, AppError> {
    let library_root = pm.current_library_root()?;
    let resolved_path = resolve_sentinel_path(pm, abs_path);

    let doc_layer = match resolved_path.as_deref() {
        Some(path) => read_doc_workspace_layer(Path::new(path)),
        None => None,
    };

    let ancestor_layers = match (resolved_path.as_deref(), library_root.as_deref()) {
        (Some(abs), Some(root)) => {
            load_ancestor_workspace_layers(pm, Path::new(abs), Path::new(root))?
        }
        _ => Vec::new(),
    };

    Ok(resolve_effective_workspace(
        doc_layer.as_ref(),
        &ancestor_layers,
        None,
    ))
}

pub(super) fn load_ancestor_workspace_layers(
    pm: &PersistenceManager,
    abs_path: &Path,
    library_root: &Path,
) -> Result<Vec<DirectoryWorkspaceConfig>, AppError> {
    let Some(vault_rel_dir) = vault_rel_dir_for_abs_path(abs_path, library_root) else {
        return Ok(Vec::new());
    };
    let records =
        pm.with_db(|conn| list_ancestor_directory_workspace_configs(conn, &vault_rel_dir))?;
    let mut layers = Vec::with_capacity(records.len());
    for rec in records {
        if rec.parse_error.is_some() {
            continue;
        }
        if let Ok(cfg) = serde_json::from_str::<DirectoryWorkspaceConfig>(&rec.config_json) {
            layers.push(expand_workspace_layer(cfg));
        }
    }
    Ok(layers)
}

fn normalize_vault_rel_dir(raw: &str) -> String {
    raw.replace('\\', "/")
        .split('/')
        .filter(|seg| !seg.is_empty() && *seg != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn vault_rel_dir_for_abs_path(abs_path: &Path, library_root: &Path) -> Option<String> {
    let rel = if let Ok(r) = abs_path.strip_prefix(library_root) {
        r.to_path_buf()
    } else {
        let canonical_abs = fs::canonicalize(abs_path).ok()?;
        let canonical_root = fs::canonicalize(library_root).ok()?;
        canonical_abs
            .strip_prefix(&canonical_root)
            .ok()?
            .to_path_buf()
    };

    let rel_dir = if abs_path.is_dir() {
        rel.as_path()
    } else {
        rel.parent().unwrap_or_else(|| Path::new(""))
    };
    Some(normalize_vault_rel_dir(&rel_dir.to_string_lossy()))
}

fn expand_workspace_layer(mut cfg: DirectoryWorkspaceConfig) -> DirectoryWorkspaceConfig {
    cfg.vscode_workspace_path = cfg.vscode_workspace_path.map(|s| expand_optional_path(&s));
    cfg.project_root = cfg.project_root.map(|s| expand_optional_path(&s));
    cfg.default_terminal_cwd = cfg.default_terminal_cwd.map(|s| expand_optional_path(&s));
    cfg
}

fn read_doc_workspace_layer(path: &Path) -> Option<DirectoryWorkspaceConfig> {
    if !path.is_file() {
        return None;
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return None;
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
    let parsed = parse_note(&raw, stem);
    Some(expand_workspace_layer(
        DirectoryWorkspaceConfig::from_note_meta(&parsed.meta),
    ))
}
