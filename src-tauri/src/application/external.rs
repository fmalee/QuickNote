//! Application use cases for Spotlight search, path completion, and external actions
//! (`v4` §3.5.3, §3.7.1, §5.3, §5.10, `D-0008`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::application::persistence::PersistenceManager;
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
use crate::domain::settings::{Settings, WorkspaceConfig};
use crate::infra::external_open::ExternalLauncher;
use crate::infra::spotlight::{expand_tilde, SpotlightRunner, SPOTLIGHT_HARD_LIMIT};

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

/// Runs cancellable Spotlight (`mdfind`) search across configured `search_roots`.
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

/// Resolves availability and target paths for all 8 external actions (`v4` §3.7.1).
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

    let effective_ws_key = args
        .workspace_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(doc_project.as_deref());
    let workspace = effective_ws_key.and_then(|k| lookup_workspace(&settings, k));

    let has_command_id = args
        .command_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());

    let (command_doc, command_workspace, recent_workspace) = if has_command_id {
        let cmd_id = args.command_id.as_deref().unwrap_or_default().trim();
        let (cmd_doc, cmd_proj) = match (extract_command_rel_path(cmd_id), library_root.as_deref())
        {
            (Some(rel), Some(root)) => read_doc_target(&Path::new(root).join(rel)),
            _ => (None, None),
        };
        let cmd_ws = cmd_proj
            .as_deref()
            .and_then(|k| lookup_workspace(&settings, k));
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

/// Executes an external OS action safely via `ExternalLauncher` (`C2.1`, `v4` §5.10).
pub fn external_action_run(
    pm: &PersistenceManager,
    launcher: &dyn ExternalLauncher,
    args: RunExternalArgs,
) -> Result<(), AppError> {
    let settings = pm.settings_get()?.settings;
    let abs_path = resolve_sentinel_path(pm, args.abs_path.as_deref());
    let dir_path = args
        .dir_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(expand_optional_path);

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
            if let Some(root) = pm.current_library_root()? {
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
            let dir = require_existing_dir(dir_path.as_deref())?;
            launcher.open_terminal(&settings.terminal, &dir)
        }
        ExternalActionId::ExternalVscode => {
            let dir = require_existing_dir(dir_path.as_deref())?;
            launcher.open_with_app(&settings.editors.vscode, &dir)
        }
        ExternalActionId::ExternalSublime => {
            let dir = require_existing_dir(dir_path.as_deref())?;
            launcher.open_with_app(&settings.editors.sublime, &dir)
        }
        ExternalActionId::ExternalWorkspace => {
            let candidate = abs_path.or(dir_path);
            let ws_file = require_existing_file(candidate.as_deref())?;
            launcher.open_with_app(&settings.editors.vscode, &ws_file)
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
