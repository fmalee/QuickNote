use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::application::capture::CaptureManager;
use crate::application::persistence::PersistenceManager;
use crate::domain::capture::{
    AppendNoteArgs, AppendResult, ResolvedTarget, TargetResolveInput, UndoAppendResult,
};
use crate::domain::error::AppError;

#[tauri::command]
pub fn capture_resolve_target(
    pm: State<'_, Arc<PersistenceManager>>,
    cm: State<'_, Arc<CaptureManager>>,
    args: TargetResolveInput,
) -> Result<ResolvedTarget, AppError> {
    cm.resolve_target(&pm, args)
}

#[tauri::command]
pub fn note_append(
    app: AppHandle,
    pm: State<'_, Arc<PersistenceManager>>,
    cm: State<'_, Arc<CaptureManager>>,
    args: AppendNoteArgs,
) -> Result<AppendResult, AppError> {
    cm.note_append(&pm, args, Some(&app))
}

#[tauri::command]
pub fn note_undo_last_append(
    app: AppHandle,
    pm: State<'_, Arc<PersistenceManager>>,
    cm: State<'_, Arc<CaptureManager>>,
) -> Result<UndoAppendResult, AppError> {
    cm.note_undo_last_append(&pm, Some(&app))
}
