use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::error::{AppError, ErrorCode};
use crate::domain::settings::{PanelId, WindowMode};

/// Capture formatting mode (`C3.4`, `C6.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    #[default]
    List,
    Todo,
    Prose,
}

/// Left navigation active tab (`C6.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum NavTab {
    Tags,
    Recent,
    Workspaces,
    #[default]
    #[serde(other)]
    Projects,
}

/// Global hotkey self-check state (`C3.4`, `C6.3`, `D-0004`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum HotkeySelfCheck {
    #[default]
    Pending,
    Ok,
    Conflict,
}

/// Global hotkey registration state (`C3.4`, `C6.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    pub combo: String,
    pub backup_combo: String,
    pub self_check: HotkeySelfCheck,
}

impl Default for HotkeyStatus {
    fn default() -> Self {
        Self {
            combo: "ctrl+alt+space".to_string(),
            backup_combo: "ctrl+alt+cmd+space".to_string(),
            self_check: HotkeySelfCheck::Pending,
        }
    }
}

/// Main window geometry and mode state (`C6.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MainWindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub mode: WindowMode,
    pub is_open: bool,
}

impl Default for MainWindowState {
    fn default() -> Self {
        Self {
            x: 120,
            y: 80,
            width: 1200,
            height: 800,
            mode: WindowMode::Standard,
            is_open: false,
        }
    }
}

/// Windows section of `ui-state.json` (`C6.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowsUiState {
    pub main: MainWindowState,
}

/// Single open editor tab state (`C6.3`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TabItemState {
    pub key: String,
    pub rel_path: Option<String>,
    pub abs_path: String,
    pub is_pinned: bool,
    pub is_preview: bool,
    #[specta(type = specta_typescript::Number)]
    pub scroll_top: f64,
    pub cursor_pos: u32,
}

/// Open tabs section of `ui-state.json` (`C6.3`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct TabsUiState {
    pub items: Vec<TabItemState>,
    pub active_key: Option<String>,
    pub closed_stack: Vec<String>,
}

/// Right panel group state (`C6.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PanelGroupUiState {
    pub visible: bool,
    pub active_id: PanelId,
    pub width: u32,
    pub collapsed: bool,
}

impl Default for PanelGroupUiState {
    fn default() -> Self {
        Self {
            visible: true,
            active_id: PanelId::Outline,
            width: 260,
            collapsed: false,
        }
    }
}

/// Top-level view shown in the left sidebar (`C6.3`, `C7.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum LeftViewId {
    #[default]
    Workspace,
    Related,
    Outline,
}

/// Resizable left sidebar state (`C6.3`, `C7.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LeftSidebarUiState {
    pub width: u32,
    pub active_view: LeftViewId,
    pub collapsed: bool,
}

impl Default for LeftSidebarUiState {
    fn default() -> Self {
        Self {
            width: 300,
            active_view: LeftViewId::Workspace,
            collapsed: false,
        }
    }
}

/// Terminal dock below the editor (`C6.3`, `C7.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalPanelUiState {
    pub visible: bool,
    pub height: u32,
    pub collapsed: bool,
}

impl Default for TerminalPanelUiState {
    fn default() -> Self {
        Self {
            visible: false,
            height: 240,
            collapsed: false,
        }
    }
}

/// Remembered paths for settings backup file dialogs (`C6.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupPathsUiState {
    pub last_export_path: Option<String>,
    pub last_import_path: Option<String>,
}

/// Navigation filter state (`C6.3`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FiltersUiState {
    pub active_tab: NavTab,
    pub selected_dir: Option<String>,
    pub selected_tags: Vec<String>,
    #[specta(type = specta_typescript::Number)]
    pub list_scroll_top: f64,
}

impl Default for FiltersUiState {
    fn default() -> Self {
        Self {
            active_tab: NavTab::Projects,
            selected_dir: None,
            selected_tags: Vec::new(),
            list_scroll_top: 0.0,
        }
    }
}

/// Last active document tracking for Quick Bar target resolution (`C6.3`, `C8.1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LastActiveDocState {
    pub rel_path: String,
    pub workspace_key: Option<String>,
    #[specta(type = specta_typescript::Number)]
    pub touched_at_ms: i64,
}

/// Complete persisted UI state (`C6.3`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub windows: WindowsUiState,
    pub tabs: TabsUiState,
    pub panel_group: PanelGroupUiState,
    #[serde(default)]
    pub left_sidebar: LeftSidebarUiState,
    #[serde(default)]
    pub terminal_panel: TerminalPanelUiState,
    pub filters: FiltersUiState,
    pub selected_workspace_key: Option<String>,
    pub last_active_doc: Option<LastActiveDocState>,
    pub workspace_modes: BTreeMap<String, CaptureMode>,
    pub hotkey: HotkeyStatus,
    #[serde(default)]
    pub backup_paths: BackupPathsUiState,
}

/// Whitelisted patch keys accepted by `ui_state_set` (`C4.2`, `C6.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UiStatePatchKey {
    Windows,
    Tabs,
    PanelGroup,
    LeftSidebar,
    TerminalPanel,
    Filters,
    SelectedWorkspace,
    Hotkey,
    LastActiveDoc,
    WorkspaceModes,
    BackupPaths,
}

impl UiStatePatchKey {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Tabs => "tabs",
            Self::PanelGroup => "panelGroup",
            Self::LeftSidebar => "leftSidebar",
            Self::TerminalPanel => "terminalPanel",
            Self::Filters => "filters",
            Self::SelectedWorkspace => "selectedWorkspace",
            Self::Hotkey => "hotkey",
            Self::LastActiveDoc => "lastActiveDoc",
            Self::WorkspaceModes => "workspaceModes",
            Self::BackupPaths => "backupPaths",
        }
    }
}

/// Arguments for `ui_state_set` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UiStateArgs {
    pub key: UiStatePatchKey,
    #[specta(type = specta_typescript::Unknown)]
    pub value: serde_json::Value,
}

impl UiState {
    /// Applies a single whitelisted key update, returning `ErrorCode::ValidationError`
    /// if the JSON value does not match the target schema (`C4.2`).
    pub fn apply_patch(
        &mut self,
        key: UiStatePatchKey,
        value: serde_json::Value,
    ) -> Result<(), AppError> {
        let key_name = key.as_str();
        match key {
            UiStatePatchKey::Windows => {
                self.windows = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::Tabs => {
                self.tabs = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::PanelGroup => {
                self.panel_group = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::LeftSidebar => {
                self.left_sidebar = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::TerminalPanel => {
                self.terminal_panel = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::Filters => {
                self.filters = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::SelectedWorkspace => {
                self.selected_workspace_key = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::Hotkey => {
                self.hotkey = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::LastActiveDoc => {
                self.last_active_doc = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::WorkspaceModes => {
                self.workspace_modes = parse_patch_value(key_name, value)?;
            }
            UiStatePatchKey::BackupPaths => {
                self.backup_paths = parse_patch_value(key_name, value)?;
            }
        }
        Ok(())
    }
}

fn parse_patch_value<T: for<'de> Deserialize<'de>>(
    key: &str,
    value: serde_json::Value,
) -> Result<T, AppError> {
    serde_json::from_value(value).map_err(|err| {
        AppError::with_detail(
            ErrorCode::ValidationError,
            format!("Invalid value for ui_state_set key '{key}'"),
            err.to_string(),
        )
    })
}
