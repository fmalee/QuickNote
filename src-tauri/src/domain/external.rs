//! Pure external action resolution rules and Spotlight path blacklist (`C1.2`, `C3.4`, `D-0008`).
//!
//! Zero `std::fs`, `std::process`, or `tauri` dependencies.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Sentinel `absPath` allowing `SettingsErrorBanner` to open/reveal local `settings.yaml` safely (`D-0008`).
pub const SETTINGS_FILE_SENTINEL: &str = "$SETTINGS_YAML";

/// Directory/file path segments excluded from Spotlight search results (`v4` §5.3).
pub const SPOTLIGHT_BLACKLIST: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    ".DS_Store",
];

/// Fixed action identifiers for external OS operations (`C3.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub enum ExternalActionId {
    #[serde(rename = "external.reveal")]
    ExternalReveal,
    #[serde(rename = "external.terminal")]
    ExternalTerminal,
    #[serde(rename = "external.vscode")]
    ExternalVscode,
    #[serde(rename = "external.sublime")]
    ExternalSublime,
    #[serde(rename = "external.workspace")]
    ExternalWorkspace,
    #[serde(rename = "external.default")]
    ExternalDefault,
    #[serde(rename = "external.copyPath")]
    ExternalCopyPath,
    #[serde(rename = "external.pin")]
    ExternalPin,
}

/// Resolved availability and target path for one external action (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalActionState {
    pub id: ExternalActionId,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    pub resolved_path: Option<String>,
}

/// Request payload for `external_actions_resolve` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResolveExternalArgs {
    pub abs_path: Option<String>,
    pub workspace_key: Option<String>,
    pub command_id: Option<String>,
}

/// Request payload for `external_action_run` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunExternalArgs {
    pub id: ExternalActionId,
    pub abs_path: Option<String>,
    pub dir_path: Option<String>,
}

/// Pure path-segment blacklist filter for Spotlight results (`v4` §5.3).
pub fn is_spotlight_blacklisted(path: &str) -> bool {
    path.split(['/', '\\'])
        .any(|segment| !segment.is_empty() && SPOTLIGHT_BLACKLIST.contains(&segment))
}

/// Filesystem probe abstraction so `domain/external.rs` stays free of `std::fs` (`C1.2`).
pub trait PathProbe {
    fn is_file(&self, path: &str) -> bool;
    fn is_dir(&self, path: &str) -> bool;
}

/// Workspace directory fields needed for external action resolution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalWorkspaceTarget {
    pub root: Option<String>,
    pub open_dir: Option<String>,
    pub terminal_dir: Option<String>,
    pub code_workspace: Option<String>,
}

/// Document-level directory fields needed for external action resolution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalDocTarget {
    pub open_dir: Option<String>,
    pub terminal_dir: Option<String>,
    pub code_workspace: Option<String>,
}

/// Full pure input context for `resolve_external_action_states` (`v4` §3.7.1, §3.5.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalResolveContext {
    /// Resolved local file path (`$SETTINGS_YAML` already mapped by application layer).
    pub file_path: Option<String>,
    /// Parent directory of `file_path` (used only when `is_library_doc == false`).
    pub file_parent_dir: Option<String>,
    /// True when `file_path` resides inside `library_root`.
    pub is_library_doc: bool,
    /// True when resolving for a command-book entry (`command_id` was provided).
    pub has_command_id: bool,
    /// Active/context document target metadata.
    pub doc: Option<ExternalDocTarget>,
    /// Associated workspace for the active/context document or explicit `workspace_key`.
    pub workspace: Option<ExternalWorkspaceTarget>,
    /// Command-book document's own target metadata (when `has_command_id == true`).
    pub command_doc: Option<ExternalDocTarget>,
    /// Command-book document's associated workspace (when `has_command_id == true`).
    pub command_workspace: Option<ExternalWorkspaceTarget>,
    /// Recent workspace fallback for `command_id` only (`v4` §3.5.3).
    pub recent_workspace: Option<ExternalWorkspaceTarget>,
    /// Whether external terminal app config is non-empty.
    pub terminal_configured: bool,
    /// Whether VS Code app config is non-empty.
    pub vscode_configured: bool,
    /// Whether Sublime Text app config is non-empty.
    pub sublime_configured: bool,
}

fn non_empty(opt: Option<&str>) -> Option<String> {
    opt.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn workspace_terminal_dir(ws: Option<&ExternalWorkspaceTarget>) -> Option<String> {
    let ws = ws?;
    non_empty(ws.terminal_dir.as_deref())
        .or_else(|| non_empty(ws.open_dir.as_deref()))
        .or_else(|| non_empty(ws.root.as_deref()))
}

fn workspace_open_dir(ws: Option<&ExternalWorkspaceTarget>) -> Option<String> {
    let ws = ws?;
    non_empty(ws.open_dir.as_deref()).or_else(|| non_empty(ws.root.as_deref()))
}

fn resolve_terminal_dir(ctx: &ExternalResolveContext) -> Option<String> {
    if ctx.has_command_id {
        return ctx
            .doc
            .as_ref()
            .and_then(|d| {
                non_empty(d.terminal_dir.as_deref()).or_else(|| non_empty(d.open_dir.as_deref()))
            })
            .or_else(|| workspace_terminal_dir(ctx.workspace.as_ref()))
            .or_else(|| {
                ctx.command_doc.as_ref().and_then(|d| {
                    non_empty(d.terminal_dir.as_deref())
                        .or_else(|| non_empty(d.open_dir.as_deref()))
                })
            })
            .or_else(|| workspace_terminal_dir(ctx.command_workspace.as_ref()))
            .or_else(|| workspace_terminal_dir(ctx.recent_workspace.as_ref()));
    }

    ctx.doc
        .as_ref()
        .and_then(|d| {
            non_empty(d.terminal_dir.as_deref()).or_else(|| non_empty(d.open_dir.as_deref()))
        })
        .or_else(|| workspace_terminal_dir(ctx.workspace.as_ref()))
        .or_else(|| {
            if !ctx.is_library_doc {
                non_empty(ctx.file_parent_dir.as_deref())
            } else {
                None
            }
        })
}

fn resolve_editor_dir(ctx: &ExternalResolveContext) -> Option<String> {
    if ctx.has_command_id {
        return ctx
            .doc
            .as_ref()
            .and_then(|d| non_empty(d.open_dir.as_deref()))
            .or_else(|| workspace_open_dir(ctx.workspace.as_ref()))
            .or_else(|| {
                ctx.command_doc
                    .as_ref()
                    .and_then(|d| non_empty(d.open_dir.as_deref()))
            })
            .or_else(|| workspace_open_dir(ctx.command_workspace.as_ref()))
            .or_else(|| workspace_open_dir(ctx.recent_workspace.as_ref()));
    }

    ctx.doc
        .as_ref()
        .and_then(|d| non_empty(d.open_dir.as_deref()))
        .or_else(|| workspace_open_dir(ctx.workspace.as_ref()))
        .or_else(|| {
            if !ctx.is_library_doc {
                non_empty(ctx.file_parent_dir.as_deref())
            } else {
                None
            }
        })
}

fn resolve_code_workspace_file(ctx: &ExternalResolveContext) -> Option<String> {
    if ctx.has_command_id {
        return ctx
            .doc
            .as_ref()
            .and_then(|d| non_empty(d.code_workspace.as_deref()))
            .or_else(|| {
                ctx.workspace
                    .as_ref()
                    .and_then(|w| non_empty(w.code_workspace.as_deref()))
            })
            .or_else(|| {
                ctx.command_doc
                    .as_ref()
                    .and_then(|d| non_empty(d.code_workspace.as_deref()))
            })
            .or_else(|| {
                ctx.command_workspace
                    .as_ref()
                    .and_then(|w| non_empty(w.code_workspace.as_deref()))
            })
            .or_else(|| {
                ctx.recent_workspace
                    .as_ref()
                    .and_then(|w| non_empty(w.code_workspace.as_deref()))
            });
    }

    ctx.doc
        .as_ref()
        .and_then(|d| non_empty(d.code_workspace.as_deref()))
        .or_else(|| {
            ctx.workspace
                .as_ref()
                .and_then(|w| non_empty(w.code_workspace.as_deref()))
        })
}

fn missing_dir_reason(ctx: &ExternalResolveContext) -> String {
    if ctx.has_command_id {
        "未配置项目目录，请手动选择目录".to_string()
    } else if ctx.is_library_doc {
        "库内文档未配置 open_dir 或关联工作区，请先配置".to_string()
    } else {
        "未配置项目目录，请先关联工作区".to_string()
    }
}

fn resolve_file_action(
    id: ExternalActionId,
    file_path: Option<&str>,
    probe: &dyn PathProbe,
) -> ExternalActionState {
    match file_path.map(str::trim).filter(|s| !s.is_empty()) {
        None => ExternalActionState {
            id,
            enabled: false,
            disabled_reason: Some("未选中文件".to_string()),
            resolved_path: None,
        },
        Some(path) if !probe.is_file(path) => ExternalActionState {
            id,
            enabled: false,
            disabled_reason: Some("文件不存在".to_string()),
            resolved_path: Some(path.to_string()),
        },
        Some(path) => ExternalActionState {
            id,
            enabled: true,
            disabled_reason: None,
            resolved_path: Some(path.to_string()),
        },
    }
}

fn resolve_dir_action(
    id: ExternalActionId,
    dir: Option<String>,
    app_configured: bool,
    missing_app_reason: &str,
    ctx: &ExternalResolveContext,
    probe: &dyn PathProbe,
) -> ExternalActionState {
    let Some(dir_path) = dir else {
        return ExternalActionState {
            id,
            enabled: false,
            disabled_reason: Some(missing_dir_reason(ctx)),
            resolved_path: None,
        };
    };

    if !probe.is_dir(&dir_path) {
        return ExternalActionState {
            id,
            enabled: false,
            disabled_reason: Some("目标目录不存在".to_string()),
            resolved_path: Some(dir_path),
        };
    }

    if !app_configured {
        return ExternalActionState {
            id,
            enabled: false,
            disabled_reason: Some(missing_app_reason.to_string()),
            resolved_path: Some(dir_path),
        };
    }

    ExternalActionState {
        id,
        enabled: true,
        disabled_reason: None,
        resolved_path: Some(dir_path),
    }
}

/// Computes availability and resolved paths for all 8 external actions in fixed order (`v4` §3.7.1).
pub fn resolve_external_action_states(
    ctx: &ExternalResolveContext,
    probe: &dyn PathProbe,
) -> Vec<ExternalActionState> {
    let file_path = ctx.file_path.as_deref();

    let reveal = resolve_file_action(ExternalActionId::ExternalReveal, file_path, probe);
    let default_app = resolve_file_action(ExternalActionId::ExternalDefault, file_path, probe);
    let copy_path = resolve_file_action(ExternalActionId::ExternalCopyPath, file_path, probe);

    let pin = match file_path.map(str::trim).filter(|s| !s.is_empty()) {
        None => ExternalActionState {
            id: ExternalActionId::ExternalPin,
            enabled: false,
            disabled_reason: Some("未选中文件".to_string()),
            resolved_path: None,
        },
        Some(path) if ctx.is_library_doc => ExternalActionState {
            id: ExternalActionId::ExternalPin,
            enabled: false,
            disabled_reason: Some("仅支持固定库外常用文件".to_string()),
            resolved_path: Some(path.to_string()),
        },
        Some(path) if !probe.is_file(path) => ExternalActionState {
            id: ExternalActionId::ExternalPin,
            enabled: false,
            disabled_reason: Some("文件不存在".to_string()),
            resolved_path: Some(path.to_string()),
        },
        Some(path) => ExternalActionState {
            id: ExternalActionId::ExternalPin,
            enabled: true,
            disabled_reason: None,
            resolved_path: Some(path.to_string()),
        },
    };

    let terminal = resolve_dir_action(
        ExternalActionId::ExternalTerminal,
        resolve_terminal_dir(ctx),
        ctx.terminal_configured,
        "未配置外部终端应用",
        ctx,
        probe,
    );

    let editor_dir = resolve_editor_dir(ctx);
    let vscode = resolve_dir_action(
        ExternalActionId::ExternalVscode,
        editor_dir.clone(),
        ctx.vscode_configured,
        "未配置 VS Code 应用",
        ctx,
        probe,
    );
    let sublime = resolve_dir_action(
        ExternalActionId::ExternalSublime,
        editor_dir,
        ctx.sublime_configured,
        "未配置 Sublime Text 应用",
        ctx,
        probe,
    );

    let workspace = match resolve_code_workspace_file(ctx) {
        None => ExternalActionState {
            id: ExternalActionId::ExternalWorkspace,
            enabled: false,
            disabled_reason: Some("未配置 .code-workspace 文件".to_string()),
            resolved_path: None,
        },
        Some(ws_file) if !probe.is_file(&ws_file) => ExternalActionState {
            id: ExternalActionId::ExternalWorkspace,
            enabled: false,
            disabled_reason: Some("指定的 .code-workspace 文件不存在".to_string()),
            resolved_path: Some(ws_file),
        },
        Some(ws_file) if !ctx.vscode_configured => ExternalActionState {
            id: ExternalActionId::ExternalWorkspace,
            enabled: false,
            disabled_reason: Some("未配置 VS Code 应用".to_string()),
            resolved_path: Some(ws_file),
        },
        Some(ws_file) => ExternalActionState {
            id: ExternalActionId::ExternalWorkspace,
            enabled: true,
            disabled_reason: None,
            resolved_path: Some(ws_file),
        },
    };

    vec![
        reveal,
        terminal,
        vscode,
        sublime,
        workspace,
        default_app,
        copy_path,
        pin,
    ]
}
