use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::application::hotkey::HotkeyManager;
use crate::application::persistence::PersistenceManager;
use crate::domain::error::AppError;
use crate::domain::settings::{ExportPathArgs, ImportPathArgs, SettingsPatch, SettingsSnapshot};
use crate::domain::ui_state::{UiState, UiStateArgs};
use crate::on_hotkey_pressed;

/// Returns the current settings snapshot and load status (`C4.2`, `D-0008`).
#[tauri::command]
pub fn settings_get(
    state: State<'_, Arc<PersistenceManager>>,
) -> Result<SettingsSnapshot, AppError> {
    state.settings_get()
}

/// Updates settings with a partial patch, persists `settings.yaml`, and broadcasts `settings:status` (`C4.2`, `D-0008`).
#[tauri::command]
pub fn settings_update(
    app: AppHandle,
    state: State<'_, Arc<PersistenceManager>>,
    args: SettingsPatch,
) -> Result<SettingsSnapshot, AppError> {
    let snapshot = state.settings_update(args)?;
    let _ = app.emit("settings:status", &snapshot.status);
    Ok(snapshot)
}

/// Returns the persisted UI state snapshot (`C4.2`, `C6.3`).
#[tauri::command]
pub fn ui_state_get(state: State<'_, Arc<PersistenceManager>>) -> Result<UiState, AppError> {
    state.ui_state_get()
}

/// Applies a whitelisted key update to `ui-state.json` (`C4.2`, `C6.3`).
#[tauri::command]
pub fn ui_state_set(
    state: State<'_, Arc<PersistenceManager>>,
    args: UiStateArgs,
) -> Result<UiState, AppError> {
    state.ui_state_set(args.key, args.value)
}

/// Exports current valid settings as canonical YAML to `dest_path` (`C4.2`).
#[tauri::command]
pub fn settings_export(
    state: State<'_, Arc<PersistenceManager>>,
    args: ExportPathArgs,
) -> Result<(), AppError> {
    state.settings_export(&args.dest_path)
}

/// Imports and validates settings YAML from `src_path`, backing up existing `settings.yaml` (`C4.2`, `D-0008`).
#[tauri::command]
pub fn settings_import(
    app: AppHandle,
    state: State<'_, Arc<PersistenceManager>>,
    hm: State<'_, Arc<HotkeyManager>>,
    args: ImportPathArgs,
) -> Result<SettingsSnapshot, AppError> {
    let prev_hotkeys = state.settings_get().ok().map(|s| s.settings.hotkey);
    let snapshot = state.settings_import(&args.src_path)?;
    let _ = app.emit("settings:status", &snapshot.status);
    if prev_hotkeys.as_ref() != Some(&snapshot.settings.hotkey) {
        let _ = hm.update_hotkey(&snapshot.settings.hotkey.quick_bar, on_hotkey_pressed);
    }
    Ok(snapshot)
}
