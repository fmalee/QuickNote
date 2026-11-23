use std::sync::Arc;

use tauri::State;

use crate::application::persistence::PersistenceManager;
use crate::domain::app::AppInfo;
use crate::domain::error::AppError;

/// Smoke IPC command returning basic application runtime metadata (`C4.2`, `M0-02`).
#[tauri::command]
pub fn app_get_info(state: State<'_, Arc<PersistenceManager>>) -> Result<AppInfo, AppError> {
    state.app_info(env!("CARGO_PKG_VERSION"), std::env::consts::OS)
}
