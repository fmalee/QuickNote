use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::bookmark::{
    normalize_and_sort_bookmarks, ExternalBookmark, ExternalBookmarkKind,
};
use crate::domain::error::{AppError, ErrorCode};

/// Default upper bound for the CM6 EditorState LRU pool (v5 Refactor).
pub const DEFAULT_MAX_ACTIVE_EDITOR_RENDERS: u32 = 5;

fn default_max_active_editor_renders() -> u32 {
    DEFAULT_MAX_ACTIVE_EDITOR_RENDERS
}

fn default_scan_hidden_files() -> bool {
    true
}

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

/// Arguments for `settings_export` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportPathArgs {
    pub dest_path: String,
}

/// Arguments for `settings_import` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPathArgs {
    pub src_path: String,
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
    pub pinned: bool,
}

impl Default for QuickBarSettings {
    fn default() -> Self {
        Self {
            default_target: "workspace_last_active_or_inbox".to_string(),
            last_active_ttl_hours: 24,
            destroy_after_hidden_minutes: 10,
            auto_create_missing_section: false,
            pinned: false,
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
    pub show_tab_close_button: bool,
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
            show_tab_close_button: false,
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
    #[serde(default = "default_max_active_editor_renders")]
    pub max_active_editor_renders: u32,
    #[serde(default)]
    pub external_bookmarks: Vec<ExternalBookmark>,
    #[serde(default = "default_scan_hidden_files")]
    pub scan_hidden_files: bool,
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
            max_active_editor_renders: DEFAULT_MAX_ACTIVE_EDITOR_RENDERS,
            external_bookmarks: Vec::new(),
            scan_hidden_files: true,
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

/// Partial patch for `HotkeySettings` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySettingsPatch {
    #[serde(default)]
    pub quick_bar: Option<String>,
    #[serde(default)]
    pub quick_bar_backup: Option<String>,
}

/// Partial patch for `QuickBarSettings` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuickBarSettingsPatch {
    #[serde(default)]
    pub default_target: Option<String>,
    #[serde(default)]
    pub last_active_ttl_hours: Option<u32>,
    #[serde(default)]
    pub destroy_after_hidden_minutes: Option<u32>,
    #[serde(default)]
    pub auto_create_missing_section: Option<bool>,
    #[serde(default)]
    pub pinned: Option<bool>,
}

/// Partial patch for `MainWindowSettings` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct MainWindowSettingsPatch {
    #[serde(default)]
    pub mode: Option<WindowMode>,
    #[serde(default)]
    pub auxiliary_width_percent: Option<u8>,
}

/// Partial patch for `EditorSettings` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettingsPatch {
    #[serde(default)]
    pub theme: Option<EditorTheme>,
    #[serde(default)]
    pub font_size: Option<u16>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub line_height: Option<f64>,
    #[serde(default)]
    pub max_width: Option<u32>,
    #[serde(default)]
    pub tab_size_prose: Option<u8>,
    #[serde(default)]
    pub tab_size_code: Option<u8>,
    #[serde(default)]
    pub auto_save_debounce_ms: Option<u32>,
    #[serde(default)]
    pub auto_save_max_wait_ms: Option<u32>,
    #[serde(default)]
    pub single_click_link: Option<bool>,
    #[serde(default)]
    pub show_tab_close_button: Option<bool>,
}

/// Partial patch for `TerminalPanelSettings` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct TerminalPanelSettingsPatch {
    #[serde(default)]
    pub visible_by_default: Option<bool>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub idle_recycle_minutes: Option<u32>,
    #[serde(default)]
    pub max_concurrent_sessions: Option<u32>,
}

/// Partial patch for `PanelsSettings` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct PanelsSettingsPatch {
    #[serde(default)]
    pub default: Option<PanelId>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub terminal: Option<TerminalPanelSettingsPatch>,
}

/// Partial patch for `InboxSettings` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct InboxSettingsPatch {
    #[serde(default)]
    pub bloat_threshold_lines: Option<u32>,
}

/// Partial patch for `ExternalAppConfig` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAppConfigPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub bundle_id: Option<String>,
}

/// Partial patch for `EditorsSettings` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditorsSettingsPatch {
    #[serde(default)]
    pub vscode: Option<ExternalAppConfigPatch>,
    #[serde(default)]
    pub sublime: Option<ExternalAppConfigPatch>,
}

/// Partial settings update payload for `settings_update` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    #[serde(default)]
    pub library_root: Option<String>,
    #[serde(default)]
    pub hotkey: Option<HotkeySettingsPatch>,
    #[serde(default)]
    pub quick_bar: Option<QuickBarSettingsPatch>,
    #[serde(default)]
    pub main_window: Option<MainWindowSettingsPatch>,
    #[serde(default)]
    pub editor: Option<EditorSettingsPatch>,
    #[serde(default)]
    pub panels: Option<PanelsSettingsPatch>,
    #[serde(default)]
    pub inbox: Option<InboxSettingsPatch>,
    #[serde(default)]
    pub terminal: Option<ExternalAppConfigPatch>,
    #[serde(default)]
    pub editors: Option<EditorsSettingsPatch>,
    #[serde(default)]
    pub search_roots: Option<Vec<String>>,
    #[serde(default)]
    pub max_active_editor_renders: Option<u32>,
    #[serde(default)]
    pub external_bookmarks: Option<Vec<ExternalBookmark>>,
    #[serde(default)]
    pub scan_hidden_files: Option<bool>,
}

fn apply_external_app_patch(target: &mut ExternalAppConfig, patch: &ExternalAppConfigPatch) {
    if let Some(ref name) = patch.name {
        target.name = name.clone();
    }
    if let Some(ref bundle_id) = patch.bundle_id {
        target.bundle_id = bundle_id.clone();
    }
}

/// Pure merge and validation of `SettingsPatch` onto `base` (`C6.2`, `C4.2`).
pub fn apply_settings_patch(base: &Settings, patch: &SettingsPatch) -> Result<Settings, AppError> {
    let mut next = base.clone();

    if let Some(ref root) = patch.library_root {
        next.library_root = Some(root.clone());
    }

    if let Some(ref hk) = patch.hotkey {
        if let Some(ref qb) = hk.quick_bar {
            next.hotkey.quick_bar = qb.clone();
        }
        if let Some(ref qbb) = hk.quick_bar_backup {
            next.hotkey.quick_bar_backup = qbb.clone();
        }
    }

    if let Some(ref qb) = patch.quick_bar {
        if let Some(ref dt) = qb.default_target {
            next.quick_bar.default_target = dt.clone();
        }
        if let Some(ttl) = qb.last_active_ttl_hours {
            next.quick_bar.last_active_ttl_hours = ttl;
        }
        if let Some(d) = qb.destroy_after_hidden_minutes {
            next.quick_bar.destroy_after_hidden_minutes = d;
        }
        if let Some(auto) = qb.auto_create_missing_section {
            next.quick_bar.auto_create_missing_section = auto;
        }
        if let Some(pinned) = qb.pinned {
            next.quick_bar.pinned = pinned;
        }
    }

    if let Some(ref mw) = patch.main_window {
        if let Some(mode) = mw.mode {
            next.main_window.mode = mode;
        }
        if let Some(pct) = mw.auxiliary_width_percent {
            next.main_window.auxiliary_width_percent = pct;
        }
    }

    if let Some(ref ed) = patch.editor {
        if let Some(theme) = ed.theme {
            next.editor.theme = theme;
        }
        if let Some(fs) = ed.font_size {
            next.editor.font_size = fs;
        }
        if let Some(lh) = ed.line_height {
            next.editor.line_height = lh;
        }
        if let Some(mw) = ed.max_width {
            next.editor.max_width = mw;
        }
        if let Some(tp) = ed.tab_size_prose {
            next.editor.tab_size_prose = tp;
        }
        if let Some(tc) = ed.tab_size_code {
            next.editor.tab_size_code = tc;
        }
        if let Some(deb) = ed.auto_save_debounce_ms {
            next.editor.auto_save_debounce_ms = deb;
        }
        if let Some(max_w) = ed.auto_save_max_wait_ms {
            next.editor.auto_save_max_wait_ms = max_w;
        }
        if let Some(scl) = ed.single_click_link {
            next.editor.single_click_link = scl;
        }
        if let Some(stcb) = ed.show_tab_close_button {
            next.editor.show_tab_close_button = stcb;
        }
    }

    if let Some(ref panels) = patch.panels {
        if let Some(def) = panels.default {
            next.panels.default = def;
        }
        if let Some(w) = panels.width {
            next.panels.width = w;
        }
        if let Some(ref term) = panels.terminal {
            if let Some(v) = term.visible_by_default {
                next.panels.terminal.visible_by_default = v;
            }
            if let Some(w) = term.width {
                next.panels.terminal.width = w;
            }
            if let Some(ref sh) = term.shell {
                next.panels.terminal.shell = sh.clone();
            }
            if let Some(idle) = term.idle_recycle_minutes {
                next.panels.terminal.idle_recycle_minutes = idle;
            }
            if let Some(max_s) = term.max_concurrent_sessions {
                next.panels.terminal.max_concurrent_sessions = max_s;
            }
        }
    }

    if let Some(ref inbox) = patch.inbox {
        if let Some(th) = inbox.bloat_threshold_lines {
            next.inbox.bloat_threshold_lines = th;
        }
    }

    if let Some(ref term) = patch.terminal {
        apply_external_app_patch(&mut next.terminal, term);
    }

    if let Some(ref eds) = patch.editors {
        if let Some(ref vsc) = eds.vscode {
            apply_external_app_patch(&mut next.editors.vscode, vsc);
        }
        if let Some(ref sub) = eds.sublime {
            apply_external_app_patch(&mut next.editors.sublime, sub);
        }
    }

    if let Some(ref roots) = patch.search_roots {
        next.search_roots = roots.clone();
    }

    if let Some(max_renders) = patch.max_active_editor_renders {
        next.max_active_editor_renders = max_renders;
    }

    if let Some(ref bookmarks) = patch.external_bookmarks {
        next.external_bookmarks = normalize_and_sort_bookmarks(bookmarks.clone());
    }

    if let Some(scan_hidden) = patch.scan_hidden_files {
        next.scan_hidden_files = scan_hidden;
    }

    validate_settings(&next)?;
    Ok(next)
}

fn validate_settings(s: &Settings) -> Result<(), AppError> {
    if !(10..=32).contains(&s.editor.font_size) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "editor.fontSize must be between 10 and 32",
        ));
    }
    if !s.editor.line_height.is_finite() || !(1.0..=3.0).contains(&s.editor.line_height) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "editor.lineHeight must be between 1.0 and 3.0",
        ));
    }
    if !(1..=8).contains(&s.editor.tab_size_prose) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "editor.tabSizeProse must be between 1 and 8",
        ));
    }
    if !(1..=8).contains(&s.editor.tab_size_code) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "editor.tabSizeCode must be between 1 and 8",
        ));
    }
    if !(100..=60_000).contains(&s.editor.auto_save_debounce_ms) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "editor.autoSaveDebounceMs must be between 100 and 60000",
        ));
    }
    if s.editor.auto_save_max_wait_ms < s.editor.auto_save_debounce_ms
        || s.editor.auto_save_max_wait_ms > 120_000
    {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "editor.autoSaveMaxWaitMs must be >= autoSaveDebounceMs and <= 120000",
        ));
    }
    if s.inbox.bloat_threshold_lines < 10 {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "inbox.bloatThresholdLines must be >= 10",
        ));
    }
    if !(20..=80).contains(&s.main_window.auxiliary_width_percent) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "mainWindow.auxiliaryWidthPercent must be between 20 and 80",
        ));
    }
    if s.panels.width < 220 {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "panels.width must be >= 220",
        ));
    }
    if s.panels.terminal.width < 220 {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "panels.terminal.width must be >= 220",
        ));
    }
    if !(1..=10).contains(&s.panels.terminal.max_concurrent_sessions) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "panels.terminal.maxConcurrentSessions must be between 1 and 10",
        ));
    }
    if s.quick_bar.last_active_ttl_hours < 1 {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "quickBar.lastActiveTtlHours must be >= 1",
        ));
    }
    if !(2..=20).contains(&s.max_active_editor_renders) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "maxActiveEditorRenders must be between 2 and 20",
        ));
    }
    Ok(())
}

// --- YAML disk representation (`snake_case`, `deny_unknown_fields`, `C6.2`) ---

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
    pinned: bool,
}

impl Default for QuickBarSettingsYaml {
    fn default() -> Self {
        let d = QuickBarSettings::default();
        Self {
            default_target: d.default_target,
            last_active_ttl_hours: d.last_active_ttl_hours,
            destroy_after_hidden_minutes: d.destroy_after_hidden_minutes,
            auto_create_missing_section: d.auto_create_missing_section,
            pinned: d.pinned,
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
    show_tab_close_button: bool,
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
            show_tab_close_button: d.show_tab_close_button,
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
struct ExternalBookmarkYaml {
    path: String,
    kind: ExternalBookmarkKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    alias: Option<String>,
    pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_opened_at: Option<i64>,
}

impl Default for ExternalBookmarkYaml {
    fn default() -> Self {
        Self {
            path: String::new(),
            kind: ExternalBookmarkKind::File,
            alias: None,
            pinned: false,
            last_opened_at: None,
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
    max_active_editor_renders: u32,
    external_bookmarks: Vec<ExternalBookmarkYaml>,
    scan_hidden_files: bool,
    /// Legacy compatibility placeholder for `settings.yaml` files written before M5-06f.
    /// Accepted on read (due to `deny_unknown_fields`), ignored at runtime, never serialized back.
    #[allow(dead_code)]
    #[serde(default, skip_serializing)]
    workspaces: Option<serde_yaml::Value>,
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
            max_active_editor_renders: d.max_active_editor_renders,
            external_bookmarks: Vec::new(),
            scan_hidden_files: d.scan_hidden_files,
            workspaces: None,
        }
    }
}

impl From<SettingsYaml> for Settings {
    fn from(y: SettingsYaml) -> Self {
        let library_root = y
            .library_root
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let external_bookmarks = normalize_and_sort_bookmarks(
            y.external_bookmarks
                .into_iter()
                .map(|b| ExternalBookmark {
                    path: b.path,
                    kind: b.kind,
                    alias: b.alias,
                    pinned: b.pinned,
                    last_opened_at: b.last_opened_at,
                })
                .collect(),
        );

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
                pinned: y.quick_bar.pinned,
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
                show_tab_close_button: y.editor.show_tab_close_button,
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
            max_active_editor_renders: y.max_active_editor_renders,
            external_bookmarks,
            scan_hidden_files: y.scan_hidden_files,
        }
    }
}

impl From<&Settings> for SettingsYaml {
    fn from(s: &Settings) -> Self {
        let external_bookmarks = s
            .external_bookmarks
            .iter()
            .map(|b| ExternalBookmarkYaml {
                path: b.path.clone(),
                kind: b.kind,
                alias: b.alias.clone(),
                pinned: b.pinned,
                last_opened_at: b.last_opened_at,
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
                pinned: s.quick_bar.pinned,
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
                show_tab_close_button: s.editor.show_tab_close_button,
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
            max_active_editor_renders: s.max_active_editor_renders,
            external_bookmarks,
            scan_hidden_files: s.scan_hidden_files,
            workspaces: None,
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
