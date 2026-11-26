//! Application use cases for external filesystem browsing, bookmarks, directory workspace
//! context cascade resolution, path completion, and external OS actions
//! (`v4` §3.5.3, §3.7.1, §5.3, §5.10, `D-0008`, v5 Refactor).

mod actions;
mod bookmarks;
mod browse;
mod workspace;

use std::fs;
use std::path::Path;

use crate::application::persistence::PersistenceManager;
use crate::domain::external::SETTINGS_FILE_SENTINEL;
use crate::infra::fs::expand_tilde;

pub use actions::{external_action_run, external_actions_resolve};
pub use bookmarks::{external_bookmark_add, external_bookmark_remove, external_bookmarks_list};
pub use browse::{
    external_list_directory, external_list_sibling_files, external_probe_path,
    external_search_roots, path_complete,
};
pub use workspace::{
    directory_workspace_config_delete_uc, directory_workspace_config_delete_uc_with,
    directory_workspace_config_get, directory_workspace_config_write,
    normalize_vault_rel_dir_checked, resolve_effective_workspace_for_path,
};

pub(super) fn resolve_sentinel_path(pm: &PersistenceManager, raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == SETTINGS_FILE_SENTINEL {
        return Some(pm.settings_path().to_string_lossy().to_string());
    }
    Some(expand_optional_path(trimmed))
}

pub(super) fn expand_optional_path(raw: &str) -> String {
    expand_tilde(raw).to_string_lossy().to_string()
}

pub(super) fn is_path_inside_root(path: &Path, root: &Path) -> bool {
    if path.starts_with(root) {
        return true;
    }
    match (fs::canonicalize(path), fs::canonicalize(root)) {
        (Ok(cp), Ok(cr)) => cp.starts_with(cr),
        _ => false,
    }
}
