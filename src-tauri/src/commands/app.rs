use crate::application::app::build_app_info;
use crate::domain::app::AppInfo;
use crate::domain::error::AppError;

/// Smoke IPC command returning basic application runtime metadata (`C4.2`).
#[tauri::command]
pub fn app_get_info() -> Result<AppInfo, AppError> {
    Ok(build_app_info(
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
    ))
}
