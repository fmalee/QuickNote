use std::path::Path;

use specta::Types;
use specta_typescript::Typescript;

use crate::domain::app::{ActivationPolicy, AppInfo};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::hotkey::HotkeyUpdateArgs;
use crate::domain::settings::{
    EditorSettings, EditorTheme, EditorsSettings, ExternalAppConfig, HotkeySettings, InboxSettings,
    LibraryInfo, MainWindowSettings, PanelId, PanelsSettings, QuickBarSettings, SetRootArgs,
    Settings, SettingsLoadStatus, SettingsSnapshot, TerminalPanelSettings, WindowMode,
    WorkspaceConfig,
};
use crate::domain::ui_state::{
    CaptureMode, FiltersUiState, HotkeySelfCheck, HotkeyStatus, LastActiveDocState,
    MainWindowState, NavTab, PanelGroupUiState, TabItemState, TabsUiState, UiState, UiStateArgs,
    WindowsUiState,
};
use crate::domain::window::{
    HideQuickBarArgs, HideReason, MainWindowArgs, QuickBarArgs, QuickBarMode, WindowState,
};

/// Exact header line required at line 1 of `src/bindings/types.ts` (`C1.4`).
pub const GENERATED_HEADER: &str = "/* GENERATED — DO NOT EDIT. */";

/// Collects all Rust IPC types exported to TypeScript (`C3.1`, `D-0009`).
pub fn collect_types() -> Types {
    Types::default()
        .register::<ActivationPolicy>()
        .register::<AppInfo>()
        .register::<ErrorCode>()
        .register::<AppError>()
        .register::<WindowMode>()
        .register::<EditorTheme>()
        .register::<PanelId>()
        .register::<LibraryInfo>()
        .register::<SetRootArgs>()
        .register::<WorkspaceConfig>()
        .register::<HotkeySettings>()
        .register::<QuickBarSettings>()
        .register::<MainWindowSettings>()
        .register::<EditorSettings>()
        .register::<TerminalPanelSettings>()
        .register::<PanelsSettings>()
        .register::<InboxSettings>()
        .register::<ExternalAppConfig>()
        .register::<EditorsSettings>()
        .register::<Settings>()
        .register::<SettingsLoadStatus>()
        .register::<SettingsSnapshot>()
        .register::<CaptureMode>()
        .register::<NavTab>()
        .register::<HotkeySelfCheck>()
        .register::<HotkeyStatus>()
        .register::<MainWindowState>()
        .register::<WindowsUiState>()
        .register::<TabItemState>()
        .register::<TabsUiState>()
        .register::<PanelGroupUiState>()
        .register::<FiltersUiState>()
        .register::<LastActiveDocState>()
        .register::<UiState>()
        .register::<UiStateArgs>()
        .register::<QuickBarMode>()
        .register::<HideReason>()
        .register::<WindowState>()
        .register::<MainWindowArgs>()
        .register::<QuickBarArgs>()
        .register::<HideQuickBarArgs>()
        .register::<HotkeyUpdateArgs>()
}

/// Renders the full TypeScript bindings content deterministically.
pub fn render_ts_bindings() -> Result<String, AppError> {
    let types = collect_types();
    Typescript::default()
        .header(GENERATED_HEADER)
        .export(&types, specta_serde::Format)
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to render TypeScript IPC bindings",
                err.to_string(),
            )
        })
}

/// Exports the generated TypeScript bindings to `path`.
pub fn export_ts_bindings(path: &Path) -> Result<(), AppError> {
    let rendered = render_ts_bindings()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to create bindings parent directory",
                err.to_string(),
            )
        })?;
    }
    std::fs::write(path, rendered).map_err(|err| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to write TypeScript bindings file",
            err.to_string(),
        )
    })
}
