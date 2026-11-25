//! Directory-bound workspace context model and 3-tier cascade resolution (`C1.2`, v5 Refactor).
//!
//! Pure in-memory functions only: zero filesystem, SQLite, or Tauri runtime dependencies.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::note::NoteMeta;

/// Filename of the per-directory workspace configuration inside a note vault.
///
/// Files with this name must be excluded from note lists and FTS indexing.
pub const DIRECTORY_CONFIG_FILENAME: &str = ".quicknote.yaml";

/// Returns `true` if the path's final segment equals [`DIRECTORY_CONFIG_FILENAME`].
pub fn is_directory_config_file(path: &str) -> bool {
    path.rsplit(['/', '\\'])
        .next()
        .is_some_and(|name| name == DIRECTORY_CONFIG_FILENAME)
}

/// Optional workspace overrides declared in a `.quicknote.yaml`, note frontmatter, or global defaults.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryWorkspaceConfig {
    #[serde(
        default,
        alias = "vscode_workspace_path",
        skip_serializing_if = "Option::is_none"
    )]
    pub vscode_workspace_path: Option<String>,
    #[serde(
        default,
        alias = "project_root",
        skip_serializing_if = "Option::is_none"
    )]
    pub project_root: Option<String>,
    #[serde(
        default,
        alias = "default_terminal_cwd",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_terminal_cwd: Option<String>,
    #[serde(
        default,
        alias = "external_app_overrides",
        skip_serializing_if = "Option::is_none"
    )]
    pub external_app_overrides: Option<BTreeMap<String, String>>,
}

impl DirectoryWorkspaceConfig {
    /// Projects legacy/standard [`NoteMeta`] workspace fields into a [`DirectoryWorkspaceConfig`] layer.
    pub fn from_note_meta(meta: &NoteMeta) -> Self {
        Self {
            vscode_workspace_path: clean_non_empty(meta.code_workspace.as_deref()),
            project_root: clean_non_empty(meta.open_dir.as_deref()),
            default_terminal_cwd: clean_non_empty(meta.terminal_dir.as_deref()),
            external_app_overrides: None,
        }
    }
}

/// Resolved effective workspace context after 3-tier cascade merging.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveWorkspaceContext {
    pub vscode_workspace_path: Option<String>,
    pub project_root: Option<String>,
    pub default_terminal_cwd: Option<String>,
    pub external_app_overrides: BTreeMap<String, String>,
}

fn clean_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn apply_layer(target: &mut EffectiveWorkspaceContext, layer: &DirectoryWorkspaceConfig) {
    if target.vscode_workspace_path.is_none() {
        target.vscode_workspace_path = clean_non_empty(layer.vscode_workspace_path.as_deref());
    }
    if target.project_root.is_none() {
        target.project_root = clean_non_empty(layer.project_root.as_deref());
    }
    if target.default_terminal_cwd.is_none() {
        target.default_terminal_cwd = clean_non_empty(layer.default_terminal_cwd.as_deref());
    }
    if let Some(overrides) = &layer.external_app_overrides {
        for (app_key, app_value) in overrides {
            let trimmed_key = app_key.trim();
            let trimmed_value = app_value.trim();
            if trimmed_key.is_empty() || trimmed_value.is_empty() {
                continue;
            }
            target
                .external_app_overrides
                .entry(trimmed_key.to_string())
                .or_insert_with(|| trimmed_value.to_string());
        }
    }
}

/// Resolves the effective workspace context by merging fields strictly in priority order:
///
/// 1. `note_frontmatter` (highest priority)
/// 2. `ancestor_configs_nearest_first` (from closest ancestor directory up to vault root)
/// 3. `global_defaults` (lowest priority fallback)
///
/// Scalar string fields take the first non-empty value in priority order.
/// `external_app_overrides` is merged per key so higher-priority layers only override matching keys.
pub fn resolve_effective_workspace(
    note_frontmatter: Option<&DirectoryWorkspaceConfig>,
    ancestor_configs_nearest_first: &[DirectoryWorkspaceConfig],
    global_defaults: Option<&DirectoryWorkspaceConfig>,
) -> EffectiveWorkspaceContext {
    let mut resolved = EffectiveWorkspaceContext::default();

    if let Some(frontmatter) = note_frontmatter {
        apply_layer(&mut resolved, frontmatter);
    }

    for ancestor in ancestor_configs_nearest_first {
        apply_layer(&mut resolved, ancestor);
    }

    if let Some(defaults) = global_defaults {
        apply_layer(&mut resolved, defaults);
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_of(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn directory_config_filename_detection_matches_only_exact_basename() {
        assert!(is_directory_config_file(".quicknote.yaml"));
        assert!(is_directory_config_file("projects/app/.quicknote.yaml"));
        assert!(is_directory_config_file("/vault/sub/.quicknote.yaml"));
        assert!(!is_directory_config_file("/vault/sub/note.md"));
        assert!(!is_directory_config_file("/vault/.quicknote.yaml.bak"));
    }

    #[test]
    fn resolve_effective_workspace_applies_three_tier_cascade_per_field() {
        let frontmatter = DirectoryWorkspaceConfig {
            vscode_workspace_path: Some("  /repo/app/frontend.code-workspace ".to_string()),
            project_root: Some("   ".to_string()), // blank must not shadow ancestor
            default_terminal_cwd: None,
            external_app_overrides: Some(map_of(&[("editor", "Cursor"), ("ignored", "  ")])),
        };

        let nearest_ancestor = DirectoryWorkspaceConfig {
            vscode_workspace_path: Some("/repo/nearest.code-workspace".to_string()),
            project_root: Some("/repo/app".to_string()),
            default_terminal_cwd: None,
            external_app_overrides: Some(map_of(&[("editor", "VSCode"), ("terminal", "Ghostty")])),
        };

        let farther_ancestor = DirectoryWorkspaceConfig {
            vscode_workspace_path: Some("/repo/root.code-workspace".to_string()),
            project_root: Some("/repo".to_string()),
            default_terminal_cwd: Some("/repo/scripts".to_string()),
            external_app_overrides: Some(map_of(&[
                ("terminal", "iTerm2"),
                ("diff", "Kaleidoscope"),
            ])),
        };

        let global_defaults = DirectoryWorkspaceConfig {
            vscode_workspace_path: Some("/global/default.code-workspace".to_string()),
            project_root: Some("/global/root".to_string()),
            default_terminal_cwd: Some("/global/cwd".to_string()),
            external_app_overrides: Some(map_of(&[
                ("editor", "Sublime"),
                ("diff", "FileMerge"),
                ("browser", "Arc"),
            ])),
        };

        let effective = resolve_effective_workspace(
            Some(&frontmatter),
            &[nearest_ancestor, farther_ancestor],
            Some(&global_defaults),
        );

        assert_eq!(
            effective.vscode_workspace_path.as_deref(),
            Some("/repo/app/frontend.code-workspace")
        );
        assert_eq!(effective.project_root.as_deref(), Some("/repo/app"));
        assert_eq!(
            effective.default_terminal_cwd.as_deref(),
            Some("/repo/scripts")
        );
        assert_eq!(
            effective.external_app_overrides,
            map_of(&[
                ("browser", "Arc"),
                ("diff", "Kaleidoscope"),
                ("editor", "Cursor"),
                ("terminal", "Ghostty"),
            ])
        );
    }

    #[test]
    fn resolve_effective_workspace_falls_back_to_global_defaults_when_upper_tiers_missing() {
        let global_defaults = DirectoryWorkspaceConfig {
            vscode_workspace_path: None,
            project_root: Some("/Users/dev/default-workspace".to_string()),
            default_terminal_cwd: Some("/Users/dev".to_string()),
            external_app_overrides: Some(map_of(&[("terminal", "Terminal.app")])),
        };

        let effective = resolve_effective_workspace(None, &[], Some(&global_defaults));
        assert_eq!(effective.vscode_workspace_path, None);
        assert_eq!(
            effective.project_root.as_deref(),
            Some("/Users/dev/default-workspace")
        );
        assert_eq!(
            effective.default_terminal_cwd.as_deref(),
            Some("/Users/dev")
        );
        assert_eq!(
            effective.external_app_overrides,
            map_of(&[("terminal", "Terminal.app")])
        );
    }

    #[test]
    fn directory_workspace_config_deserializes_both_snake_case_yaml_and_camel_case_json() {
        let yaml = r#"
vscode_workspace_path: /repo/app.code-workspace
project_root: /repo
default_terminal_cwd: /repo/backend
external_app_overrides:
  terminal: iTerm
"#;
        let from_yaml: Result<DirectoryWorkspaceConfig, _> = serde_yaml::from_str(yaml);
        assert!(from_yaml.is_ok());

        let json = r#"{
            "vscodeWorkspacePath": "/repo/app.code-workspace",
            "projectRoot": "/repo",
            "defaultTerminalCwd": "/repo/backend",
            "externalAppOverrides": { "terminal": "iTerm" }
        }"#;
        let from_json: Result<DirectoryWorkspaceConfig, _> = serde_json::from_str(json);
        assert_eq!(from_yaml.ok(), from_json.ok());
    }
}
