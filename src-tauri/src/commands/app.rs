use std::sync::Arc;

use tauri::State;

use crate::application::persistence::PersistenceManager;
use crate::application::window::WindowManager;
use crate::domain::app::AppInfo;
use crate::domain::error::AppError;

/// Smoke IPC command returning basic application runtime metadata including real `activationPolicy` (`C4.2`, `M0-03`).
#[tauri::command]
pub fn app_get_info(
    pm: State<'_, Arc<PersistenceManager>>,
    wm: State<'_, Arc<WindowManager>>,
) -> Result<AppInfo, AppError> {
    let mut info = pm.app_info(env!("CARGO_PKG_VERSION"), std::env::consts::OS)?;
    info.activation_policy = wm.current_activation_policy()?;
    Ok(info)
}
