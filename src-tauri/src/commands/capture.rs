use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::application::capture::{inbox_extract_to_note as extract_uc, CaptureManager};
use crate::application::persistence::PersistenceManager;
use crate::domain::capture::{
    AppendNoteArgs, AppendResult, InboxExtractArgs, ResolvedTarget, TargetResolveInput,
    UndoAppendResult,
};
use crate::domain::error::AppError;
use crate::domain::note::NoteDoc;

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

#[tauri::command]
pub fn inbox_extract_to_note(
    pm: State<'_, Arc<PersistenceManager>>,
    args: InboxExtractArgs,
) -> Result<NoteDoc, AppError> {
    extract_uc(&pm, args)
}
