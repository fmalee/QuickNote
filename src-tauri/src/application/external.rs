//! Application use cases for external filesystem browsing, bookmarks, directory workspace
//! context cascade resolution, path completion, and external OS actions
//! (`v4` §3.5.3, §3.7.1, §5.3, §5.10, `D-0008`, v5 Refactor).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::application::persistence::PersistenceManager;
use crate::application::reconciler::{reconcile_path, write_directory_workspace_config};
use crate::domain::bookmark::{
    normalize_and_sort_bookmarks, normalize_bookmark_alias, normalize_bookmark_path,
    ExternalBookmark, ExternalBookmarkKind,
};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::external::{
    resolve_external_action_states, ExternalActionId, ExternalActionState, ExternalDocTarget,
    ExternalResolveContext, ExternalWorkspaceTarget, PathProbe, ResolveExternalArgs,
    RunExternalArgs, SETTINGS_FILE_SENTINEL,
};
use crate::domain::note::{
    parse_note, PathCompleteArgs, PathEntry, SearchItemKind, SearchResponse, SearchResultItem,
    SearchSource, SpotlightArgs,
};
use crate::domain::settings::{ExternalAppConfig, Settings, SettingsPatch, WorkspaceConfig};
use crate::domain::workspace_context::{
    resolve_effective_workspace, DirectoryWorkspaceConfig, EffectiveWorkspaceContext,
    DIRECTORY_CONFIG_FILENAME,
};
use crate::infra::external_open::ExternalLauncher;
use crate::infra::fs_browser::{
    self, DirectoryEntry, DirectoryListing, FsBrowserError, ListDirectoryOptions, PathEntryKind,
    PathProbeResult, DEFAULT_MAX_DEPTH, DEFAULT_MAX_ENTRIES,
};
use crate::infra::spotlight::{expand_tilde, SpotlightRunner, SPOTLIGHT_HARD_LIMIT};
use crate::infra::sqlite::list_ancestor_directory_workspace_configs;
use crate::infra::trash::{default_file_remover, FileRemover};

const PATH_COMPLETE_LIMIT: usize = 50;

struct FsPathProbe;

impl PathProbe for FsPathProbe {
    fn is_file(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }

    fn is_dir(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }
}

/// Probes an external path (`~`, hidden directories, `/usr/local/etc`, etc.) using pure Rust FS.
pub fn external_probe_path(raw_input: &str) -> PathProbeResult {
    fs_browser::probe_path(raw_input)
}

/// Lists directory entries under an external path with depth/hidden/query/circuit-breaker controls.
pub fn external_list_directory(
    dir_path: &str,
    opts: Option<ListDirectoryOptions>,
) -> Result<DirectoryListing, AppError> {
    let effective = opts.unwrap_or_default();
    fs_browser::list_directory_entries(dir_path, &effective).map_err(map_fs_browser_error)
}

/// Lists editable text files in the parent directory (and subdirectories up to `max_depth`) of `file_path`.
pub fn external_list_sibling_files(
    file_path: &str,
    max_depth: Option<usize>,
) -> Result<DirectoryListing, AppError> {
    let depth = max_depth.unwrap_or(1);
    fs_browser::list_sibling_files(file_path, depth).map_err(map_fs_browser_error)
}

/// Searches configured `search_roots` and bookmarked directories directly via pure Rust FS traversal
/// (replaces Spotlight as the primary external directory search path in v5).
pub fn external_search_roots(
    pm: &PersistenceManager,
    query: &str,
    max_depth: Option<usize>,
    max_entries: Option<usize>,
) -> Result<Vec<DirectoryEntry>, AppError> {
    let settings = pm.settings_get()?.settings;
    let trimmed_query = query.trim();
    let limit = max_entries.unwrap_or(DEFAULT_MAX_ENTRIES);
    let depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);

    let mut roots: Vec<String> = Vec::new();
    let mut seen_roots: HashSet<String> = HashSet::new();

    for root in &settings.search_roots {
        if let Ok(resolved) = fs_browser::resolve_input_path(root) {
            let key = resolved.to_string_lossy().into_owned();
            if seen_roots.insert(key.clone()) {
                roots.push(key);
            }
        }
    }
    for bookmark in &settings.external_bookmarks {
        if bookmark.kind == ExternalBookmarkKind::Directory {
            if let Ok(resolved) = fs_browser::resolve_input_path(&bookmark.path) {
                let key = resolved.to_string_lossy().into_owned();
                if seen_roots.insert(key.clone()) {
                    roots.push(key);
                }
            }
        }
    }

    let opts = ListDirectoryOptions {
        max_depth: depth,
        include_hidden: true,
        query: if trimmed_query.is_empty() {
            None
        } else {
            Some(trimmed_query.to_string())
        },
        max_entries: limit,
    };

    let mut seen_entries: HashSet<String> = HashSet::new();
    let mut results: Vec<DirectoryEntry> = Vec::new();
    for root in roots {
        let Ok(listing) = fs_browser::list_directory_entries(&root, &opts) else {
            continue;
        };
        for entry in listing.entries {
            if seen_entries.insert(entry.path.clone()) {
                results.push(entry);
            }
        }
    }

    if trimmed_query.is_empty() {
        results.sort_by(|a, b| {
            a.depth.cmp(&b.depth).then_with(|| {
                a.path
                    .to_ascii_lowercase()
                    .cmp(&b.path.to_ascii_lowercase())
            })
        });
    } else {
        results.sort_by(|a, b| {
            b.match_score
                .cmp(&a.match_score)
                .then_with(|| a.depth.cmp(&b.depth))
                .then_with(|| {
                    a.path
                        .to_ascii_lowercase()
                        .cmp(&b.path.to_ascii_lowercase())
                })
        });
    }
    results.truncate(limit);
    Ok(results)
}

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
    workspace_key: Option<&str>,
) -> Result<EffectiveWorkspaceContext, AppError> {
    let settings = pm.settings_get()?.settings;
    let library_root = pm.current_library_root()?;
    let resolved_path = resolve_sentinel_path(pm, abs_path);

    let (doc_layer, doc_project) = match resolved_path.as_deref() {
        Some(path) => read_doc_workspace_layer(Path::new(path)),
        None => (None, None),
    };

    let ancestor_layers = match (resolved_path.as_deref(), library_root.as_deref()) {
        (Some(abs), Some(root)) => {
            load_ancestor_workspace_layers(pm, Path::new(abs), Path::new(root))?
        }
        _ => Vec::new(),
    };

    let effective_ws_key = workspace_key
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(doc_project.as_deref());
    let global_layer = effective_ws_key.and_then(|k| lookup_workspace_as_layer(&settings, k));

    Ok(resolve_effective_workspace(
        doc_layer.as_ref(),
        &ancestor_layers,
        global_layer.as_ref(),
    ))
}

/// Runs legacy cancellable Spotlight (`mdfind`) search across configured `search_roots`.
/// Note: in v5, [`external_search_roots`] is the primary pure-Rust external directory search path.
pub fn search_spotlight(
    pm: &PersistenceManager,
    runner: &SpotlightRunner,
    args: SpotlightArgs,
) -> Result<SearchResponse, AppError> {
    let settings = pm.settings_get()?.settings;
    let result = runner.search(&settings.search_roots, &args.query)?;

    let items = result
        .paths
        .into_iter()
        .enumerate()
        .map(|(idx, path)| {
            let p = Path::new(&path);
            let kind = if p.is_dir() {
                SearchItemKind::Dir
            } else {
                SearchItemKind::File
            };
            let title = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path)
                .to_string();
            SearchResultItem {
                kind,
                key: format!("abs:{path}"),
                title,
                subtitle: path.clone(),
                rel_path: None,
                abs_path: path,
                score: (SPOTLIGHT_HARD_LIMIT.saturating_sub(idx)) as f64,
                source: SearchSource::Spotlight,
            }
        })
        .collect();

    Ok(SearchResponse {
        request_id: args.request_id,
        items,
        truncated: result.truncated,
    })
}

/// Completes local filesystem paths for Quick Bar `/path` mode (`v4` §5.3).
pub fn path_complete(args: PathCompleteArgs) -> Result<Vec<PathEntry>, AppError> {
    if args.prefix.is_empty() {
        return Ok(Vec::new());
    }

    let expanded = if args.prefix == "~" {
        let Some(home) = std::env::var_os("HOME") else {
            return Ok(Vec::new());
        };
        format!("{}/", home.to_string_lossy())
    } else if let Some(rest) = args.prefix.strip_prefix("~/") {
        let Some(home) = std::env::var_os("HOME") else {
            return Ok(Vec::new());
        };
        let home_str = home.to_string_lossy();
        if home_str.ends_with('/') {
            format!("{home_str}{rest}")
        } else {
            format!("{home_str}/{rest}")
        }
    } else {
        args.prefix.clone()
    };

    if !expanded.starts_with('/') {
        return Ok(Vec::new());
    }

    let (parent_dir, name_prefix) = if expanded.ends_with('/') {
        (PathBuf::from(&expanded), "")
    } else if let Some(pos) = expanded.rfind('/') {
        if pos == 0 {
            (PathBuf::from("/"), &expanded[1..])
        } else {
            (PathBuf::from(&expanded[..pos]), &expanded[pos + 1..])
        }
    } else {
        return Ok(Vec::new());
    };

    if !parent_dir.is_dir() {
        return Ok(Vec::new());
    }

    let Ok(read_dir) = fs::read_dir(&parent_dir) else {
        return Ok(Vec::new());
    };

    let name_prefix_lower = name_prefix.to_lowercase();
    let show_hidden = name_prefix.starts_with('.');

    let mut entries = Vec::new();
    for item in read_dir {
        let Ok(entry) = item else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" {
            continue;
        }
        if name.starts_with('.') && !show_hidden {
            continue;
        }
        if !name_prefix_lower.is_empty() && !name.to_lowercase().starts_with(&name_prefix_lower) {
            continue;
        }

        let path = entry.path();
        let is_dir = match entry.file_type() {
            Ok(ft) if ft.is_symlink() => path.is_dir(),
            Ok(ft) => ft.is_dir(),
            Err(_) => path.is_dir(),
        };

        entries.push(PathEntry {
            name,
            abs_path: path.to_string_lossy().to_string(),
            is_dir,
        });
    }

    entries.sort_by(|a, b| {
        (!a.is_dir, a.name.to_lowercase()).cmp(&(!b.is_dir, b.name.to_lowercase()))
    });
    entries.truncate(PATH_COMPLETE_LIMIT);
    Ok(entries)
}

/// Resolves availability and target paths for all 8 external actions (`v4` §3.7.1, v5 3-tier cascade).
pub fn external_actions_resolve(
    pm: &PersistenceManager,
    args: ResolveExternalArgs,
) -> Result<Vec<ExternalActionState>, AppError> {
    let settings = pm.settings_get()?.settings;
    let ui_state = pm.ui_state_get()?;
    let library_root = pm.current_library_root()?;

    let file_path = resolve_sentinel_path(pm, args.abs_path.as_deref());
    let file_parent_dir = file_path
        .as_deref()
        .and_then(|p| Path::new(p).parent())
        .map(|p| p.to_string_lossy().to_string());

    let is_library_doc = match (file_path.as_deref(), library_root.as_deref()) {
        (Some(abs), Some(root)) => is_path_inside_root(Path::new(abs), Path::new(root)),
        _ => false,
    };

    let (doc, doc_project) = if let Some(ref abs) = file_path {
        read_doc_target(Path::new(abs))
    } else {
        (None, None)
    };

    let ancestor_layers = match (
        is_library_doc,
        file_path.as_deref(),
        library_root.as_deref(),
    ) {
        (true, Some(abs), Some(root)) => {
            load_ancestor_workspace_layers(pm, Path::new(abs), Path::new(root))?
        }
        _ => Vec::new(),
    };

    let effective_ws_key = args
        .workspace_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(doc_project.as_deref());
    let global_layer = effective_ws_key.and_then(|k| lookup_workspace_as_layer(&settings, k));

    let inherited_ctx = resolve_effective_workspace(None, &ancestor_layers, global_layer.as_ref());
    let workspace = effective_context_to_workspace_target(&inherited_ctx);

    let has_command_id = args
        .command_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());

    let (command_doc, command_workspace, recent_workspace) = if has_command_id {
        let cmd_id = args.command_id.as_deref().unwrap_or_default().trim();
        let cmd_abs = match (extract_command_rel_path(cmd_id), library_root.as_deref()) {
            (Some(rel), Some(root)) => Some(Path::new(root).join(rel)),
            _ => None,
        };
        let (cmd_doc, cmd_proj) = match cmd_abs.as_deref() {
            Some(path) => read_doc_target(path),
            None => (None, None),
        };
        let cmd_ancestors = match (cmd_abs.as_deref(), library_root.as_deref()) {
            (Some(abs), Some(root)) => load_ancestor_workspace_layers(pm, abs, Path::new(root))?,
            _ => Vec::new(),
        };
        let cmd_global = cmd_proj
            .as_deref()
            .and_then(|k| lookup_workspace_as_layer(&settings, k));
        let cmd_inherited = resolve_effective_workspace(None, &cmd_ancestors, cmd_global.as_ref());
        let cmd_ws = effective_context_to_workspace_target(&cmd_inherited);

        let recent_ws_key = ui_state
            .last_active_doc
            .as_ref()
            .and_then(|d| d.workspace_key.as_deref())
            .or(ui_state.selected_workspace_key.as_deref());
        let recent_ws = recent_ws_key.and_then(|k| lookup_workspace(&settings, k));
        (cmd_doc, cmd_ws, recent_ws)
    } else {
        (None, None, None)
    };

    let ctx = ExternalResolveContext {
        file_path,
        file_parent_dir,
        is_library_doc,
        has_command_id,
        doc,
        workspace,
        command_doc,
        command_workspace,
        recent_workspace,
        terminal_configured: !settings.terminal.bundle_id.trim().is_empty()
            || !settings.terminal.name.trim().is_empty(),
        vscode_configured: !settings.editors.vscode.bundle_id.trim().is_empty()
            || !settings.editors.vscode.name.trim().is_empty(),
        sublime_configured: !settings.editors.sublime.bundle_id.trim().is_empty()
            || !settings.editors.sublime.name.trim().is_empty(),
    };

    Ok(resolve_external_action_states(&ctx, &FsPathProbe))
}

/// Executes an external OS action safely via `ExternalLauncher` (`C2.1`, `v4` §5.10, v5 3-tier cascade).
///
/// Before launching directory/workspace actions, resolves the target path strictly via
/// **Note Frontmatter > Nearest Ancestor `.quicknote.yaml` > Global Default** when `dir_path`
/// is not explicitly provided.
pub fn external_action_run(
    pm: &PersistenceManager,
    launcher: &dyn ExternalLauncher,
    args: RunExternalArgs,
) -> Result<(), AppError> {
    let settings = pm.settings_get()?.settings;
    let library_root = pm.current_library_root()?;
    let abs_path = resolve_sentinel_path(pm, args.abs_path.as_deref());
    let dir_path = args
        .dir_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(expand_optional_path);

    let effective = resolve_effective_workspace_for_path(pm, args.abs_path.as_deref(), None)?;
    let is_library_doc = match (abs_path.as_deref(), library_root.as_deref()) {
        (Some(abs), Some(root)) => is_path_inside_root(Path::new(abs), Path::new(root)),
        _ => false,
    };
    let external_parent_dir = if !is_library_doc {
        abs_path
            .as_deref()
            .and_then(|p| Path::new(p).parent())
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };

    match args.id {
        ExternalActionId::ExternalReveal => {
            let target = require_existing_file(abs_path.as_deref())?;
            launcher.reveal_in_finder(&target)
        }
        ExternalActionId::ExternalDefault => {
            let target = require_existing_file(abs_path.as_deref())?;
            launcher.open_default(&target)
        }
        ExternalActionId::ExternalCopyPath => {
            let Some(path) = abs_path else {
                return Err(AppError::new(ErrorCode::ValidationError, "未提供文件路径"));
            };
            launcher.copy_to_clipboard(&path)
        }
        ExternalActionId::ExternalPin => {
            let target = require_existing_file(abs_path.as_deref())?;
            if let Some(root) = library_root {
                if is_path_inside_root(&target, Path::new(&root)) {
                    return Err(AppError::new(
                        ErrorCode::ValidationError,
                        "仅支持固定库外常用文件",
                    ));
                }
            }
            let target_str = target.to_string_lossy().to_string();
            pm.toggle_pinned_file(&target_str)?;
            Ok(())
        }
        ExternalActionId::ExternalTerminal => {
            let candidate = dir_path
                .or_else(|| effective.default_terminal_cwd.clone())
                .or_else(|| effective.project_root.clone())
                .or(external_parent_dir);
            let dir = require_existing_dir(candidate.as_deref())?;
            let app_cfg = apply_app_override(&settings.terminal, &effective, "terminal");
            launcher.open_terminal(&app_cfg, &dir)
        }
        ExternalActionId::ExternalVscode => {
            let candidate = dir_path
                .or_else(|| effective.project_root.clone())
                .or(external_parent_dir);
            let dir = require_existing_dir(candidate.as_deref())?;
            let app_cfg = apply_app_override(&settings.editors.vscode, &effective, "vscode");
            launcher.open_with_app(&app_cfg, &dir)
        }
        ExternalActionId::ExternalSublime => {
            let candidate = dir_path
                .or_else(|| effective.project_root.clone())
                .or(external_parent_dir);
            let dir = require_existing_dir(candidate.as_deref())?;
            let app_cfg = apply_app_override(&settings.editors.sublime, &effective, "sublime");
            launcher.open_with_app(&app_cfg, &dir)
        }
        ExternalActionId::ExternalWorkspace => {
            let candidate = if abs_path
                .as_deref()
                .is_some_and(|p| p.ends_with(".code-workspace") && Path::new(p).is_file())
            {
                abs_path
            } else {
                effective
                    .vscode_workspace_path
                    .clone()
                    .or(dir_path)
                    .or(abs_path)
            };
            let ws_file = require_existing_file(candidate.as_deref())?;
            let app_cfg = apply_app_override(&settings.editors.vscode, &effective, "vscode");
            launcher.open_with_app(&app_cfg, &ws_file)
        }
    }
}

fn resolve_sentinel_path(pm: &PersistenceManager, raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == SETTINGS_FILE_SENTINEL {
        return Some(pm.settings_path().to_string_lossy().to_string());
    }
    Some(expand_optional_path(trimmed))
}

fn expand_optional_path(raw: &str) -> String {
    expand_tilde(raw)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| raw.to_string())
}

fn require_existing_file(path: Option<&str>) -> Result<PathBuf, AppError> {
    let Some(raw) = path.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err(AppError::new(ErrorCode::NotFound, "未提供有效文件路径"));
    };
    let pb = PathBuf::from(raw);
    if !pb.is_file() {
        return Err(AppError::new(ErrorCode::NotFound, "目标文件不存在"));
    }
    Ok(pb)
}

fn require_existing_dir(path: Option<&str>) -> Result<PathBuf, AppError> {
    let Some(raw) = path.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err(AppError::new(
            ErrorCode::WorkspaceDirMissing,
            "未配置或未提供目标目录",
        ));
    };
    let pb = PathBuf::from(raw);
    if !pb.is_dir() {
        return Err(AppError::new(
            ErrorCode::WorkspaceDirMissing,
            "目标目录不存在",
        ));
    }
    Ok(pb)
}

fn is_path_inside_root(path: &Path, root: &Path) -> bool {
    if path.starts_with(root) {
        return true;
    }
    match (fs::canonicalize(path), fs::canonicalize(root)) {
        (Ok(cp), Ok(cr)) => cp.starts_with(cr),
        _ => false,
    }
}

fn read_doc_target(path: &Path) -> (Option<ExternalDocTarget>, Option<String>) {
    if !path.is_file() {
        return (None, None);
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return (None, None);
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
    let parsed = parse_note(&raw, stem);
    let target = ExternalDocTarget {
        open_dir: parsed.meta.open_dir.map(|s| expand_optional_path(&s)),
        terminal_dir: parsed.meta.terminal_dir.map(|s| expand_optional_path(&s)),
        code_workspace: parsed.meta.code_workspace.map(|s| expand_optional_path(&s)),
    };
    (Some(target), parsed.meta.project)
}

fn lookup_workspace(settings: &Settings, key: &str) -> Option<ExternalWorkspaceTarget> {
    let ws: &WorkspaceConfig = settings.workspaces.get(key).or_else(|| {
        settings
            .workspaces
            .values()
            .find(|w| w.key == key || w.name == key)
    })?;
    Some(ExternalWorkspaceTarget {
        root: Some(expand_optional_path(&ws.root)),
        open_dir: Some(expand_optional_path(&ws.open_dir)),
        terminal_dir: ws.terminal_dir.as_deref().map(expand_optional_path),
        code_workspace: ws.code_workspace.as_deref().map(expand_optional_path),
    })
}

fn extract_command_rel_path(command_id: &str) -> Option<&str> {
    command_id.rsplitn(3, ':').nth(2).filter(|s| !s.is_empty())
}

fn map_fs_browser_error(err: FsBrowserError) -> AppError {
    match err {
        FsBrowserError::EmptyPath => {
            AppError::new(ErrorCode::ValidationError, "Path input cannot be empty")
        }
        FsBrowserError::NoParentDirectory(p) => AppError::with_detail(
            ErrorCode::ValidationError,
            "Path has no parent directory",
            p,
        ),
        FsBrowserError::NotFound(p) => {
            AppError::with_detail(ErrorCode::NotFound, "Target path does not exist", p)
        }
        FsBrowserError::NotADirectory(p) => {
            AppError::with_detail(ErrorCode::ValidationError, "Path is not a directory", p)
        }
        FsBrowserError::PermissionDenied(p) => AppError::with_detail(
            ErrorCode::PermissionDenied,
            "Permission denied accessing path",
            p,
        ),
        FsBrowserError::Io { path, source } => AppError::with_detail(
            ErrorCode::InternalError,
            format!("Failed to access path: {path}"),
            source.to_string(),
        ),
    }
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

fn read_doc_workspace_layer(path: &Path) -> (Option<DirectoryWorkspaceConfig>, Option<String>) {
    if !path.is_file() {
        return (None, None);
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return (None, None);
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
    let parsed = parse_note(&raw, stem);
    let layer = expand_workspace_layer(DirectoryWorkspaceConfig::from_note_meta(&parsed.meta));
    (Some(layer), parsed.meta.project)
}

fn load_ancestor_workspace_layers(
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

fn lookup_workspace_as_layer(settings: &Settings, key: &str) -> Option<DirectoryWorkspaceConfig> {
    let ws = lookup_workspace(settings, key)?;
    let project_root = ws
        .open_dir
        .filter(|s| !s.trim().is_empty())
        .or_else(|| ws.root.filter(|s| !s.trim().is_empty()));
    let default_terminal_cwd = ws
        .terminal_dir
        .filter(|s| !s.trim().is_empty())
        .or_else(|| project_root.clone());
    Some(DirectoryWorkspaceConfig {
        vscode_workspace_path: ws.code_workspace.filter(|s| !s.trim().is_empty()),
        project_root,
        default_terminal_cwd,
        external_app_overrides: None,
    })
}

fn effective_context_to_workspace_target(
    ctx: &EffectiveWorkspaceContext,
) -> Option<ExternalWorkspaceTarget> {
    if ctx.vscode_workspace_path.is_none()
        && ctx.project_root.is_none()
        && ctx.default_terminal_cwd.is_none()
    {
        return None;
    }
    Some(ExternalWorkspaceTarget {
        root: ctx.project_root.clone(),
        open_dir: ctx.project_root.clone(),
        terminal_dir: ctx.default_terminal_cwd.clone(),
        code_workspace: ctx.vscode_workspace_path.clone(),
    })
}

fn apply_app_override(
    base: &ExternalAppConfig,
    effective: &EffectiveWorkspaceContext,
    key: &str,
) -> ExternalAppConfig {
    let Some(raw) = effective
        .external_app_overrides
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return base.clone();
    };
    if raw.contains('.') && !raw.ends_with(".app") {
        ExternalAppConfig {
            name: base.name.clone(),
            bundle_id: raw.to_string(),
        }
    } else {
        ExternalAppConfig {
            name: raw.to_string(),
            bundle_id: String::new(),
        }
    }
}
