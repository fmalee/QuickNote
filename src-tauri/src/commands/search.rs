use std::sync::Arc;

use tauri::State;

use crate::application::notes::{
    commands_search as commands_search_uc, search_notes_and_files as search_uc,
};
use crate::application::persistence::PersistenceManager;
use crate::domain::command_book::{CommandEntry, CommandSearchArgs};
use crate::domain::error::AppError;
use crate::domain::note::{SearchQueryArgs, SearchResponse};

#[tauri::command]
pub fn search_notes_and_files(
    pm: State<'_, Arc<PersistenceManager>>,
    args: SearchQueryArgs,
) -> Result<SearchResponse, AppError> {
    search_uc(&pm, args)
}

#[tauri::command]
pub fn commands_search(
    pm: State<'_, Arc<PersistenceManager>>,
    args: CommandSearchArgs,
) -> Result<Vec<CommandEntry>, AppError> {
    commands_search_uc(&pm, args)
}
