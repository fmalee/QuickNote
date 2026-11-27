use std::fs;
use std::path::{Path, PathBuf};

use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::external::{
    resolve_external_action_states, ExternalActionId, ExternalActionState, ExternalDocTarget,
    ExternalResolveContext, ExternalWorkspaceTarget, PathProbe, ResolveExternalArgs,
    RunExternalArgs,
};
use crate::domain::note::parse_note;
use crate::domain::settings::ExternalAppConfig;
use crate::domain::workspace_context::{resolve_effective_workspace, EffectiveWorkspaceContext};
use crate::infra::external_open::ExternalLauncher;

use super::workspace::{load_ancestor_workspace_layers, resolve_effective_workspace_for_path};
use super::{expand_optional_path, is_path_inside_root, resolve_sentinel_path};

struct FsPathProbe;

impl PathProbe for FsPathProbe {
    fn is_file(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }

    fn is_dir(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }
}

/// Resolves availability and target paths for all 8 external actions (`v4` §3.7.1, v5 3-tier cascade).
pub fn external_actions_resolve(
    pm: &PersistenceManager,
    args: ResolveExternalArgs,
) -> Result<Vec<ExternalActionState>, AppError> {
    let settings = pm.settings_get()?.settings;
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

    let doc = if let Some(ref abs) = file_path {
        read_doc_target(Path::new(abs))
    } else {
        None
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

    let inherited_ctx = resolve_effective_workspace(None, &ancestor_layers, None);
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
        let cmd_doc = match cmd_abs.as_deref() {
            Some(path) => read_doc_target(path),
            None => None,
        };
        let cmd_ancestors = match (cmd_abs.as_deref(), library_root.as_deref()) {
            (Some(abs), Some(root)) => load_ancestor_workspace_layers(pm, abs, Path::new(root))?,
            _ => Vec::new(),
        };
        let cmd_inherited = resolve_effective_workspace(None, &cmd_ancestors, None);
        let cmd_ws = effective_context_to_workspace_target(&cmd_inherited);

        (cmd_doc, cmd_ws, None)
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

    let effective = resolve_effective_workspace_for_path(pm, args.abs_path.as_deref())?;
    let is_library_doc = match (abs_path.as_deref(), library_root.as_deref()) {
        (Some(abs), Some(root)) => is_path_inside_root(Path::new(abs), Path::new(root)),
        _ => false,
    };
    let file_parent_dir = abs_path
        .as_deref()
        .and_then(|p| Path::new(p).parent())
        .map(|p| p.to_string_lossy().to_string());
    let external_parent_dir = if !is_library_doc {
        file_parent_dir.clone()
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
            let target = if let Some(explicit) = dir_path.as_deref() {
                require_existing_file_or_dir(Some(explicit))?
            } else if let Some(ws_file) = effective.vscode_workspace_path.as_deref() {
                require_existing_file(Some(ws_file))?
            } else {
                let candidate = effective.project_root.clone().or(file_parent_dir);
                require_existing_dir(candidate.as_deref())?
            };
            let app_cfg = apply_app_override(&settings.editors.vscode, &effective, "vscode");
            launcher.open_with_app(&app_cfg, &target)
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

fn require_existing_file_or_dir(path: Option<&str>) -> Result<PathBuf, AppError> {
    let Some(raw) = path.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err(AppError::new(
            ErrorCode::WorkspaceDirMissing,
            "未配置或未提供目标路径",
        ));
    };
    let pb = PathBuf::from(raw);
    if pb.is_file() || pb.is_dir() {
        return Ok(pb);
    }
    if raw.ends_with(".code-workspace") {
        return Err(AppError::new(
            ErrorCode::NotFound,
            "指定的 .code-workspace 文件不存在",
        ));
    }
    Err(AppError::new(
        ErrorCode::WorkspaceDirMissing,
        "目标目录不存在",
    ))
}

fn read_doc_target(path: &Path) -> Option<ExternalDocTarget> {
    if !path.is_file() {
        return None;
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return None;
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
    let parsed = parse_note(&raw, stem);
    Some(ExternalDocTarget {
        open_dir: parsed.meta.open_dir.map(|s| expand_optional_path(&s)),
        terminal_dir: parsed.meta.terminal_dir.map(|s| expand_optional_path(&s)),
        code_workspace: parsed.meta.code_workspace.map(|s| expand_optional_path(&s)),
    })
}

fn extract_command_rel_path(command_id: &str) -> Option<&str> {
    command_id.rsplitn(3, ':').nth(2).filter(|s| !s.is_empty())
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
