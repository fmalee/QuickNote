use std::sync::Arc;

use tauri::State;

use crate::application::persistence::PersistenceManager;
use crate::domain::error::AppError;
use crate::domain::settings::SettingsSnapshot;
use crate::domain::ui_state::{UiState, UiStateArgs};

/// Returns the current settings snapshot and load status (`C4.2`, `D-0008`).
#[tauri::command]
pub fn settings_get(
    state: State<'_, Arc<PersistenceManager>>,
) -> Result<SettingsSnapshot, AppError> {
    state.settings_get()
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
    state.ui_state_set(&args.key, args.value)
}
