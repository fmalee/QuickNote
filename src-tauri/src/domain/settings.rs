use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::error::{AppError, ErrorCode};

/// Main window presentation mode (`C3.4`, `C6.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum WindowMode {
    #[default]
    Standard,
    Fullscreen,
    Auxiliary,
}

/// Editor color theme preference (`C6.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum EditorTheme {
    #[default]
    System,
    Light,
    Dark,
}

/// Right-side panel identifier (`C6.2`, `C6.3`, `C7.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum PanelId {
    #[default]
    Outline,
    Terminal,
    Related,
    Recent,
}

/// Summary of the configured note library (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub root: Option<String>,
    pub note_count: u32,
    #[specta(type = Option<specta_typescript::Number>)]
    pub last_indexed_at: Option<i64>,
}

/// Arguments for `library_set_root` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetRootArgs {
    pub root: String,
    pub create_if_missing: bool,
}

/// Workspace configuration entry (`C3.4`, `C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub name: String,
    pub key: String,
    pub root: String,
    pub open_dir: String,
    pub code_workspace: Option<String>,
    pub terminal_dir: Option<String>,
    pub default_editor: String,
    pub default_entry_doc: Option<String>,
}

/// Hotkey configuration section (`C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySettings {
    pub quick_bar: String,
    pub quick_bar_backup: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            quick_bar: "ctrl+alt+space".to_string(),
            quick_bar_backup: "ctrl+alt+cmd+space".to_string(),
        }
    }
}

/// Quick Bar configuration section (`C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QuickBarSettings {
    pub default_target: String,
    pub last_active_ttl_hours: u32,
    pub destroy_after_hidden_minutes: u32,
    pub auto_create_missing_section: bool,
}

impl Default for QuickBarSettings {
    fn default() -> Self {
        Self {
            default_target: "workspace_last_active_or_inbox".to_string(),
            last_active_ttl_hours: 24,
            destroy_after_hidden_minutes: 10,
            auto_create_missing_section: false,
        }
    }
}

/// Main window configuration section (`C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MainWindowSettings {
    pub mode: WindowMode,
    pub auxiliary_width_percent: u8,
}

impl Default for MainWindowSettings {
    fn default() -> Self {
        Self {
            mode: WindowMode::Standard,
            auxiliary_width_percent: 45,
        }
    }
}

/// Editor configuration section (`C6.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    pub theme: EditorTheme,
    pub font_size: u16,
    #[specta(type = specta_typescript::Number)]
    pub line_height: f64,
    pub max_width: u32,
    pub tab_size_prose: u8,
    pub tab_size_code: u8,
    pub auto_save_debounce_ms: u32,
    pub auto_save_max_wait_ms: u32,
    pub single_click_link: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            theme: EditorTheme::System,
            font_size: 14,
            line_height: 1.6,
            max_width: 900,
            tab_size_prose: 2,
            tab_size_code: 4,
            auto_save_debounce_ms: 600,
            auto_save_max_wait_ms: 2000,
            single_click_link: false,
        }
    }
}

/// Embedded terminal panel configuration (`C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalPanelSettings {
    pub visible_by_default: bool,
    pub width: u32,
    pub shell: String,
    pub idle_recycle_minutes: u32,
    pub max_concurrent_sessions: u32,
}

impl Default for TerminalPanelSettings {
    fn default() -> Self {
        Self {
            visible_by_default: false,
            width: 360,
            shell: "/bin/zsh".to_string(),
            idle_recycle_minutes: 30,
            max_concurrent_sessions: 3,
        }
    }
}

/// Right-side panel configuration (`C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PanelsSettings {
    pub default: PanelId,
    pub width: u32,
    pub terminal: TerminalPanelSettings,
}

impl Default for PanelsSettings {
    fn default() -> Self {
        Self {
            default: PanelId::Outline,
            width: 260,
            terminal: TerminalPanelSettings::default(),
        }
    }
}

/// Inbox configuration section (`C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InboxSettings {
    pub bloat_threshold_lines: u32,
}

impl Default for InboxSettings {
    fn default() -> Self {
        Self {
            bloat_threshold_lines: 500,
        }
    }
}

/// External macOS app descriptor (`C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAppConfig {
    pub name: String,
    pub bundle_id: String,
}

/// External editors configuration (`C6.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorsSettings {
    pub vscode: ExternalAppConfig,
    pub sublime: ExternalAppConfig,
}

impl Default for EditorsSettings {
    fn default() -> Self {
        Self {
            vscode: ExternalAppConfig {
                name: "Visual Studio Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
            },
            sublime: ExternalAppConfig {
                name: "Sublime Text".to_string(),
                bundle_id: "com.sublimetext.4".to_string(),
            },
        }
    }
}

/// Application settings exposed over IPC (`C3.4`, `C6.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub library_root: Option<String>,
    pub hotkey: HotkeySettings,
    pub quick_bar: QuickBarSettings,
    pub main_window: MainWindowSettings,
    pub editor: EditorSettings,
    pub panels: PanelsSettings,
    pub inbox: InboxSettings,
    pub terminal: ExternalAppConfig,
    pub editors: EditorsSettings,
    pub search_roots: Vec<String>,
    pub workspaces: BTreeMap<String, WorkspaceConfig>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            library_root: None,
            hotkey: HotkeySettings::default(),
            quick_bar: QuickBarSettings::default(),
            main_window: MainWindowSettings::default(),
            editor: EditorSettings::default(),
            panels: PanelsSettings::default(),
            inbox: InboxSettings::default(),
            terminal: ExternalAppConfig {
                name: "iTerm".to_string(),
                bundle_id: "com.googlecode.iterm2".to_string(),
            },
            editors: EditorsSettings::default(),
            search_roots: vec![
                "~/Sites/Projects".to_string(),
                "~/.config".to_string(),
                "/opt/homebrew/etc".to_string(),
            ],
            workspaces: BTreeMap::new(),
        }
    }
}

/// Diagnostic status of loading `settings.yaml` (`C3.4`, `D-0008`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsLoadStatus {
    pub valid: bool,
    pub active_source: String,
    pub error_line: Option<u32>,
    pub error_column: Option<u32>,
    pub error_message: Option<String>,
}

/// Combined settings and load-status payload returned by `settings_get` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub settings: Settings,
    pub status: SettingsLoadStatus,
}

// --- YAML disk representation (`snake_case`, `deny_unknown_fields`, `C6.2`) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WorkspaceConfigYaml {
    name: String,
    key: String,
    root: String,
    open_dir: String,
    code_workspace: Option<String>,
    terminal_dir: Option<String>,
    default_editor: String,
    default_entry_doc: Option<String>,
}

impl Default for WorkspaceConfigYaml {
    fn default() -> Self {
        Self {
            name: String::new(),
            key: String::new(),
            root: String::new(),
            open_dir: String::new(),
            code_workspace: None,
            terminal_dir: None,
            default_editor: "vscode".to_string(),
            default_entry_doc: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct HotkeySettingsYaml {
    quick_bar: String,
    quick_bar_backup: String,
}

impl Default for HotkeySettingsYaml {
    fn default() -> Self {
        let d = HotkeySettings::default();
        Self {
            quick_bar: d.quick_bar,
            quick_bar_backup: d.quick_bar_backup,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct QuickBarSettingsYaml {
    default_target: String,
    last_active_ttl_hours: u32,
    destroy_after_hidden_minutes: u32,
    auto_create_missing_section: bool,
}

impl Default for QuickBarSettingsYaml {
    fn default() -> Self {
        let d = QuickBarSettings::default();
        Self {
            default_target: d.default_target,
            last_active_ttl_hours: d.last_active_ttl_hours,
            destroy_after_hidden_minutes: d.destroy_after_hidden_minutes,
            auto_create_missing_section: d.auto_create_missing_section,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct MainWindowSettingsYaml {
    mode: WindowMode,
    auxiliary_width_percent: u8,
}

impl Default for MainWindowSettingsYaml {
    fn default() -> Self {
        let d = MainWindowSettings::default();
        Self {
            mode: d.mode,
            auxiliary_width_percent: d.auxiliary_width_percent,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct EditorSettingsYaml {
    theme: EditorTheme,
    font_size: u16,
    line_height: f64,
    max_width: u32,
    tab_size_prose: u8,
    tab_size_code: u8,
    auto_save_debounce_ms: u32,
    auto_save_max_wait_ms: u32,
    single_click_link: bool,
}

impl Default for EditorSettingsYaml {
    fn default() -> Self {
        let d = EditorSettings::default();
        Self {
            theme: d.theme,
            font_size: d.font_size,
            line_height: d.line_height,
            max_width: d.max_width,
            tab_size_prose: d.tab_size_prose,
            tab_size_code: d.tab_size_code,
            auto_save_debounce_ms: d.auto_save_debounce_ms,
            auto_save_max_wait_ms: d.auto_save_max_wait_ms,
            single_click_link: d.single_click_link,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TerminalPanelSettingsYaml {
    visible_by_default: bool,
    width: u32,
    shell: String,
    idle_recycle_minutes: u32,
    max_concurrent_sessions: u32,
}

impl Default for TerminalPanelSettingsYaml {
    fn default() -> Self {
        let d = TerminalPanelSettings::default();
        Self {
            visible_by_default: d.visible_by_default,
            width: d.width,
            shell: d.shell,
            idle_recycle_minutes: d.idle_recycle_minutes,
            max_concurrent_sessions: d.max_concurrent_sessions,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PanelsSettingsYaml {
    default: PanelId,
    width: u32,
    terminal: TerminalPanelSettingsYaml,
}

impl Default for PanelsSettingsYaml {
    fn default() -> Self {
        let d = PanelsSettings::default();
        Self {
            default: d.default,
            width: d.width,
            terminal: TerminalPanelSettingsYaml::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct InboxSettingsYaml {
    bloat_threshold_lines: u32,
}

impl Default for InboxSettingsYaml {
    fn default() -> Self {
        Self {
            bloat_threshold_lines: InboxSettings::default().bloat_threshold_lines,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ExternalAppConfigYaml {
    name: String,
    bundle_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct EditorsSettingsYaml {
    vscode: ExternalAppConfigYaml,
    sublime: ExternalAppConfigYaml,
}

impl Default for EditorsSettingsYaml {
    fn default() -> Self {
        let d = EditorsSettings::default();
        Self {
            vscode: ExternalAppConfigYaml {
                name: d.vscode.name,
                bundle_id: d.vscode.bundle_id,
            },
            sublime: ExternalAppConfigYaml {
                name: d.sublime.name,
                bundle_id: d.sublime.bundle_id,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SettingsYaml {
    library_root: Option<String>,
    hotkey: HotkeySettingsYaml,
    quick_bar: QuickBarSettingsYaml,
    main_window: MainWindowSettingsYaml,
    editor: EditorSettingsYaml,
    panels: PanelsSettingsYaml,
    inbox: InboxSettingsYaml,
    terminal: ExternalAppConfigYaml,
    editors: EditorsSettingsYaml,
    search_roots: Vec<String>,
    workspaces: BTreeMap<String, WorkspaceConfigYaml>,
}

impl Default for SettingsYaml {
    fn default() -> Self {
        let d = Settings::default();
        Self {
            library_root: d.library_root,
            hotkey: HotkeySettingsYaml::default(),
            quick_bar: QuickBarSettingsYaml::default(),
            main_window: MainWindowSettingsYaml::default(),
            editor: EditorSettingsYaml::default(),
            panels: PanelsSettingsYaml::default(),
            inbox: InboxSettingsYaml::default(),
            terminal: ExternalAppConfigYaml {
                name: d.terminal.name,
                bundle_id: d.terminal.bundle_id,
            },
            editors: EditorsSettingsYaml::default(),
            search_roots: d.search_roots,
            workspaces: BTreeMap::new(),
        }
    }
}

impl From<SettingsYaml> for Settings {
    fn from(y: SettingsYaml) -> Self {
        let library_root = y
            .library_root
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let workspaces = y
            .workspaces
            .into_iter()
            .map(|(k, w)| {
                let key = if w.key.is_empty() { k.clone() } else { w.key };
                (
                    k,
                    WorkspaceConfig {
                        name: w.name,
                        key,
                        root: w.root,
                        open_dir: w.open_dir,
                        code_workspace: w.code_workspace,
                        terminal_dir: w.terminal_dir,
                        default_editor: w.default_editor,
                        default_entry_doc: w.default_entry_doc,
                    },
                )
            })
            .collect();

        Self {
            library_root,
            hotkey: HotkeySettings {
                quick_bar: y.hotkey.quick_bar,
                quick_bar_backup: y.hotkey.quick_bar_backup,
            },
            quick_bar: QuickBarSettings {
                default_target: y.quick_bar.default_target,
                last_active_ttl_hours: y.quick_bar.last_active_ttl_hours,
                destroy_after_hidden_minutes: y.quick_bar.destroy_after_hidden_minutes,
                auto_create_missing_section: y.quick_bar.auto_create_missing_section,
            },
            main_window: MainWindowSettings {
                mode: y.main_window.mode,
                auxiliary_width_percent: y.main_window.auxiliary_width_percent,
            },
            editor: EditorSettings {
                theme: y.editor.theme,
                font_size: y.editor.font_size,
                line_height: y.editor.line_height,
                max_width: y.editor.max_width,
                tab_size_prose: y.editor.tab_size_prose,
                tab_size_code: y.editor.tab_size_code,
                auto_save_debounce_ms: y.editor.auto_save_debounce_ms,
                auto_save_max_wait_ms: y.editor.auto_save_max_wait_ms,
                single_click_link: y.editor.single_click_link,
            },
            panels: PanelsSettings {
                default: y.panels.default,
                width: y.panels.width,
                terminal: TerminalPanelSettings {
                    visible_by_default: y.panels.terminal.visible_by_default,
                    width: y.panels.terminal.width,
                    shell: y.panels.terminal.shell,
                    idle_recycle_minutes: y.panels.terminal.idle_recycle_minutes,
                    max_concurrent_sessions: y.panels.terminal.max_concurrent_sessions,
                },
            },
            inbox: InboxSettings {
                bloat_threshold_lines: y.inbox.bloat_threshold_lines,
            },
            terminal: ExternalAppConfig {
                name: y.terminal.name,
                bundle_id: y.terminal.bundle_id,
            },
            editors: EditorsSettings {
                vscode: ExternalAppConfig {
                    name: y.editors.vscode.name,
                    bundle_id: y.editors.vscode.bundle_id,
                },
                sublime: ExternalAppConfig {
                    name: y.editors.sublime.name,
                    bundle_id: y.editors.sublime.bundle_id,
                },
            },
            search_roots: y.search_roots,
            workspaces,
        }
    }
}

impl From<&Settings> for SettingsYaml {
    fn from(s: &Settings) -> Self {
        let workspaces = s
            .workspaces
            .iter()
            .map(|(k, w)| {
                (
                    k.clone(),
                    WorkspaceConfigYaml {
                        name: w.name.clone(),
                        key: w.key.clone(),
                        root: w.root.clone(),
                        open_dir: w.open_dir.clone(),
                        code_workspace: w.code_workspace.clone(),
                        terminal_dir: w.terminal_dir.clone(),
                        default_editor: w.default_editor.clone(),
                        default_entry_doc: w.default_entry_doc.clone(),
                    },
                )
            })
            .collect();

        Self {
            library_root: s.library_root.clone(),
            hotkey: HotkeySettingsYaml {
                quick_bar: s.hotkey.quick_bar.clone(),
                quick_bar_backup: s.hotkey.quick_bar_backup.clone(),
            },
            quick_bar: QuickBarSettingsYaml {
                default_target: s.quick_bar.default_target.clone(),
                last_active_ttl_hours: s.quick_bar.last_active_ttl_hours,
                destroy_after_hidden_minutes: s.quick_bar.destroy_after_hidden_minutes,
                auto_create_missing_section: s.quick_bar.auto_create_missing_section,
            },
            main_window: MainWindowSettingsYaml {
                mode: s.main_window.mode,
                auxiliary_width_percent: s.main_window.auxiliary_width_percent,
            },
            editor: EditorSettingsYaml {
                theme: s.editor.theme,
                font_size: s.editor.font_size,
                line_height: s.editor.line_height,
                max_width: s.editor.max_width,
                tab_size_prose: s.editor.tab_size_prose,
                tab_size_code: s.editor.tab_size_code,
                auto_save_debounce_ms: s.editor.auto_save_debounce_ms,
                auto_save_max_wait_ms: s.editor.auto_save_max_wait_ms,
                single_click_link: s.editor.single_click_link,
            },
            panels: PanelsSettingsYaml {
                default: s.panels.default,
                width: s.panels.width,
                terminal: TerminalPanelSettingsYaml {
                    visible_by_default: s.panels.terminal.visible_by_default,
                    width: s.panels.terminal.width,
                    shell: s.panels.terminal.shell.clone(),
                    idle_recycle_minutes: s.panels.terminal.idle_recycle_minutes,
                    max_concurrent_sessions: s.panels.terminal.max_concurrent_sessions,
                },
            },
            inbox: InboxSettingsYaml {
                bloat_threshold_lines: s.inbox.bloat_threshold_lines,
            },
            terminal: ExternalAppConfigYaml {
                name: s.terminal.name.clone(),
                bundle_id: s.terminal.bundle_id.clone(),
            },
            editors: EditorsSettingsYaml {
                vscode: ExternalAppConfigYaml {
                    name: s.editors.vscode.name.clone(),
                    bundle_id: s.editors.vscode.bundle_id.clone(),
                },
                sublime: ExternalAppConfigYaml {
                    name: s.editors.sublime.name.clone(),
                    bundle_id: s.editors.sublime.bundle_id.clone(),
                },
            },
            search_roots: s.search_roots.clone(),
            workspaces,
        }
    }
}

/// Parses raw `settings.yaml` with `deny_unknown_fields` and fault-tolerant fallback (`D-0008`, `C6.2`).
pub fn parse_settings_yaml(
    raw: &str,
    previous_valid: Option<&Settings>,
) -> (Settings, SettingsLoadStatus) {
    match serde_yaml::from_str::<SettingsYaml>(raw) {
        Ok(parsed) => (
            Settings::from(parsed),
            SettingsLoadStatus {
                valid: true,
                active_source: "file".to_string(),
                error_line: None,
                error_column: None,
                error_message: None,
            },
        ),
        Err(err) => {
            let location = err.location();
            let error_line = location.as_ref().map(|loc| loc.line() as u32);
            let error_column = location.as_ref().map(|loc| loc.column() as u32);
            let error_message = Some(err.to_string());

            let (fallback, active_source) = match previous_valid {
                Some(prev) => (prev.clone(), "last_valid".to_string()),
                None => (Settings::default(), "default".to_string()),
            };

            (
                fallback,
                SettingsLoadStatus {
                    valid: false,
                    active_source,
                    error_line,
                    error_column,
                    error_message,
                },
            )
        }
    }
}

/// Serializes `Settings` into canonical `settings.yaml` format (`C6.2`).
pub fn serialize_settings_yaml(settings: &Settings) -> Result<String, AppError> {
    let yaml_struct = SettingsYaml::from(settings);
    serde_yaml::to_string(&yaml_struct).map_err(|err| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to serialize settings.yaml",
            err.to_string(),
        )
    })
}
