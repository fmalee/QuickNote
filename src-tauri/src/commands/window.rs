use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::application::hotkey::HotkeyManager;
use crate::application::persistence::PersistenceManager;
use crate::application::window::WindowManager;
use crate::domain::error::AppError;
use crate::domain::hotkey::HotkeyUpdateArgs;
use crate::domain::ui_state::HotkeyStatus;
use crate::domain::window::{
    clamp_auxiliary_width_percent, HideQuickBarArgs, HideReason, MainWindowArgs, QuickBarArgs,
    SetQuickBarPinnedArgs, WindowState, DEFAULT_AUXILIARY_WIDTH_PERCENT,
};
use crate::on_hotkey_pressed;

fn resolve_auxiliary_width_percent(pm: &PersistenceManager) -> u8 {
    let raw = pm
        .settings_get()
        .map(|s| s.settings.main_window.auxiliary_width_percent)
        .unwrap_or(DEFAULT_AUXILIARY_WIDTH_PERCENT);
    clamp_auxiliary_width_percent(raw)
}

/// Shows the main workspace window and switches activation policy to `.regular` (`C4.2`, `D-0003`).
#[tauri::command]
pub fn window_show_main(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
    pm: State<'_, Arc<PersistenceManager>>,
    args: Option<MainWindowArgs>,
) -> Result<WindowState, AppError> {
    let aux_pct = resolve_auxiliary_width_percent(&pm);
    wm.show_main(&app, args.and_then(|a| a.mode), aux_pct)
}

/// Toggles the resident main workspace window between regular mode and 45% auxiliary sidebar mode (`v5`).
#[tauri::command]
pub fn window_toggle_auxiliary_sidebar(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
    pm: State<'_, Arc<PersistenceManager>>,
) -> Result<WindowState, AppError> {
    let aux_pct = resolve_auxiliary_width_percent(&pm);
    wm.toggle_auxiliary_sidebar_mode(&app, aux_pct)
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
/// Safely falls back to `HideReason::Esc` if `args` is omitted.
#[tauri::command]
pub fn window_hide_quick_bar(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
    args: Option<HideQuickBarArgs>,
) -> Result<WindowState, AppError> {
    let reason = args.map(|a| a.reason).unwrap_or(HideReason::Esc);
    wm.hide_quick_bar(&app, reason)
}

/// Opens (creating on demand if previously destroyed) the settings window and switches activation policy to `.regular` (`C4.2`, `D-0003`).
#[tauri::command]
pub fn window_open_settings(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
) -> Result<WindowState, AppError> {
    wm.open_settings(&app)
}

/// Closes and destroys the on-demand settings window, safely succeeding if it is already destroyed (`v5`).
#[tauri::command]
pub fn window_close_settings(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
) -> Result<WindowState, AppError> {
    wm.close_settings(&app)
}

/// Hides and destroys the on-demand Toast window if alive, safely no-oping if already destroyed (`v5`).
#[tauri::command]
pub fn window_dismiss_toast(
    app: AppHandle,
    wm: State<'_, Arc<WindowManager>>,
) -> Result<(), AppError> {
    wm.dismiss_toast(&app)
}

/// Validates and hot re-registers the Carbon global hotkey, persisting status to `ui-state.json` (`C4.2`, `D-0004`).
#[tauri::command]
pub fn hotkey_update(
    hm: State<'_, Arc<HotkeyManager>>,
    args: HotkeyUpdateArgs,
) -> Result<HotkeyStatus, AppError> {
    hm.update_hotkey(&args.combo, on_hotkey_pressed)
}

/// Persists `quick_bar.pinned` and syncs the native menu checkmark (`C4.2`, `D-0013`).
#[tauri::command]
pub fn window_set_quick_bar_pinned(
    pm: State<'_, Arc<PersistenceManager>>,
    app: AppHandle,
    args: SetQuickBarPinnedArgs,
) -> Result<bool, AppError> {
    app.state::<Arc<WindowManager>>()
        .set_quick_bar_pinned(&app, &pm, args.pinned)
}
