use std::sync::Arc;

use tauri::State;

use crate::application::notes::{
    note_create as create_uc, note_delete as delete_uc, note_read as read_uc,
    note_rename as rename_uc, note_update_meta as update_meta_uc, note_write as write_uc,
    notes_get_tree_and_tags as tree_uc, notes_list as list_uc,
};
use crate::application::persistence::PersistenceManager;
use crate::domain::error::AppError;
use crate::domain::note::{
    CreateNoteArgs, NavTreeSnapshot, NoteDoc, NoteSummary, NoteUpdateMetaArgs, NotesFilterArgs,
    ReadNoteArgs, RelPathArgs, RenameNoteArgs, WriteNoteArgs, WriteResult,
};

#[tauri::command]
pub fn notes_list(
    pm: State<'_, Arc<PersistenceManager>>,
    args: NotesFilterArgs,
) -> Result<Vec<NoteSummary>, AppError> {
    list_uc(&pm, args)
}

#[tauri::command]
pub fn notes_get_tree_and_tags(
    pm: State<'_, Arc<PersistenceManager>>,
) -> Result<NavTreeSnapshot, AppError> {
    tree_uc(&pm)
}

#[tauri::command]
pub fn note_read(
    pm: State<'_, Arc<PersistenceManager>>,
    args: ReadNoteArgs,
) -> Result<NoteDoc, AppError> {
    read_uc(&pm, args)
}

#[tauri::command]
pub fn note_create(
    pm: State<'_, Arc<PersistenceManager>>,
    args: CreateNoteArgs,
) -> Result<NoteDoc, AppError> {
    create_uc(&pm, args)
}

#[tauri::command]
pub fn note_write(
    pm: State<'_, Arc<PersistenceManager>>,
    args: WriteNoteArgs,
) -> Result<WriteResult, AppError> {
    write_uc(&pm, args)
}

#[tauri::command]
pub fn note_update_meta(
    pm: State<'_, Arc<PersistenceManager>>,
    args: NoteUpdateMetaArgs,
) -> Result<NoteDoc, AppError> {
    update_meta_uc(&pm, args)
}

#[tauri::command]
pub fn note_rename(
    pm: State<'_, Arc<PersistenceManager>>,
    args: RenameNoteArgs,
) -> Result<NoteDoc, AppError> {
    rename_uc(&pm, args)
}

#[tauri::command]
pub fn note_delete(
    pm: State<'_, Arc<PersistenceManager>>,
    args: RelPathArgs,
) -> Result<(), AppError> {
    delete_uc(&pm, args)
}
