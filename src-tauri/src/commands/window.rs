use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::application::hotkey::HotkeyManager;
use crate::application::window::WindowManager;
use crate::domain::error::AppError;
use crate::domain::hotkey::HotkeyUpdateArgs;
use crate::domain::ui_state::HotkeyStatus;
use crate::domain::window::{HideQuickBarArgs, MainWindowArgs, QuickBarArgs, WindowState};
use crate::on_hotkey_pressed;

/// Shows the main workspace window and switches activation policy to `.regular` (`C4.2`, `D-0003`).
#[tauri::command]
pub fn window_show_main(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
    args: Option<MainWindowArgs>,
) -> Result<WindowState, AppError> {
    wm.show_main(&app, args.and_then(|a| a.mode))
}

/// Hides the main workspace window and switches activation policy to `.accessory` if settings is also closed (`C4.2`, `D-0003`).
#[tauri::command]
pub fn window_hide_main(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
) -> Result<WindowState, AppError> {
    wm.hide_main(&app)
}

/// Shows the Quick Bar native non-activating `NSPanel` (`C4.2`, `D-0001`).
#[tauri::command]
pub fn window_show_quick_bar(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
    args: Option<QuickBarArgs>,
) -> Result<WindowState, AppError> {
    wm.show_quick_bar(&app, args.and_then(|a| a.mode))
}

/// Hides the Quick Bar native `NSPanel` with an explicit `HideReason` (`C4.2`, `D-0002`).
#[tauri::command]
pub fn window_hide_quick_bar(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
    args: HideQuickBarArgs,
) -> Result<WindowState, AppError> {
    wm.hide_quick_bar(&app, args.reason)
}

/// Opens the settings window and switches activation policy to `.regular` (`C4.2`, `D-0003`).
#[tauri::command]
pub fn window_open_settings(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
) -> Result<WindowState, AppError> {
    wm.open_settings(&app)
}

/// Validates and hot re-registers the Carbon global hotkey, persisting status to `ui-state.json` (`C4.2`, `D-0004`).
#[tauri::command]
pub fn hotkey_update(
    hm: State<'_, Arc<HotkeyManager>>,
    args: HotkeyUpdateArgs,
) -> Result<HotkeyStatus, AppError> {
    hm.update_hotkey(&args.combo, on_hotkey_pressed)
}
