use std::sync::Arc;

use tauri::State;

use crate::application::external::{
    external_action_run as external_action_run_uc,
    external_actions_resolve as external_actions_resolve_uc,
};
use crate::application::persistence::PersistenceManager;
use crate::domain::error::AppError;
use crate::domain::external::{ExternalActionState, ResolveExternalArgs, RunExternalArgs};
use crate::infra::external_open::ExternalLauncher;

#[tauri::command]
pub fn external_actions_resolve(
    pm: State<'_, Arc<PersistenceManager>>,
    args: ResolveExternalArgs,
) -> Result<Vec<ExternalActionState>, AppError> {
    external_actions_resolve_uc(&pm, args)
}

#[tauri::command]
pub fn external_action_run(
    pm: State<'_, Arc<PersistenceManager>>,
    launcher: State<'_, Arc<dyn ExternalLauncher>>,
    args: RunExternalArgs,
) -> Result<(), AppError> {
    external_action_run_uc(&pm, launcher.inner().as_ref(), args)
}
