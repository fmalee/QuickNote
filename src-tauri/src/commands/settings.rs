use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::application::hotkey::HotkeyManager;
use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::settings::{
    ExportPathArgs, HotkeySettings, ImportPathArgs, SettingsPatch, SettingsSnapshot,
};
use crate::domain::ui_state::{UiState, UiStateArgs, UiStatePatchKey};
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

/// Synchronizes hotkey registration and persisted `ui_state.hotkey` after `settings_import` (`M4-audit`).
pub fn sync_hotkeys_after_import(
    pm: &PersistenceManager,
    hm: &HotkeyManager,
    prev_hotkeys: Option<&HotkeySettings>,
    snapshot: &SettingsSnapshot,
    on_hotkey: extern "C" fn(),
) -> Result<(), AppError> {
    let imported = &snapshot.settings.hotkey;
    let primary_changed =
        prev_hotkeys.map(|h| h.quick_bar.as_str()) != Some(imported.quick_bar.as_str());
    let backup_changed = prev_hotkeys.map(|h| h.quick_bar_backup.as_str())
        != Some(imported.quick_bar_backup.as_str());

    if primary_changed {
        let _ = hm.update_hotkey(&imported.quick_bar, on_hotkey);
    }

    if primary_changed || backup_changed {
        let mut hk_status = pm.ui_state_get()?.hotkey;
        if primary_changed && hk_status.combo.trim().is_empty() {
            hk_status.combo = imported.quick_bar.clone();
        }
        hk_status.backup_combo = imported.quick_bar_backup.clone();
        let value = serde_json::to_value(hk_status).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize hotkey status",
                err.to_string(),
            )
        })?;
        pm.ui_state_set(UiStatePatchKey::Hotkey, value)?;
    }

    Ok(())
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
    let _ = sync_hotkeys_after_import(
        &state,
        &hm,
        prev_hotkeys.as_ref(),
        &snapshot,
        on_hotkey_pressed,
    );
    Ok(snapshot)
}
