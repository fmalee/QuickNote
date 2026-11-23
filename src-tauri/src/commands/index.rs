use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::application::index::{index_get_status as get_status_uc, index_rebuild as rebuild_uc};
use crate::application::persistence::PersistenceManager;
use crate::domain::error::AppError;
use crate::domain::note::IndexStatus;

#[tauri::command]
pub fn index_get_status(pm: State<'_, Arc<PersistenceManager>>) -> Result<IndexStatus, AppError> {
    get_status_uc(&pm)
}

#[tauri::command]
pub fn index_rebuild(
    app: AppHandle,
    pm: State<'_, Arc<PersistenceManager>>,
) -> Result<IndexStatus, AppError> {
    rebuild_uc(&pm, Some(&app))
}
