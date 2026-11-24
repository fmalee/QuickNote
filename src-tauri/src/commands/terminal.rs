use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::application::persistence::PersistenceManager;
use crate::application::terminal::{
    TauriTerminalDataSink, TauriTerminalEventSink, TerminalManager,
};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::terminal::{
    TerminalData, TerminalKillArgs, TerminalResizeArgs, TerminalSessionInfo, TerminalSpawnArgs,
    TerminalVisibilityArgs, TerminalWriteArgs,
};

/// Spawns a workspace-scoped embedded terminal PTY session and streams output via `Channel` (`C4.2`, `C4.3`).
#[tauri::command]
pub fn terminal_spawn(
    app: AppHandle,
    state: State<'_, Arc<TerminalManager>>,
    _pm: State<'_, Arc<PersistenceManager>>,
    args: TerminalSpawnArgs,
    on_data: Channel<TerminalData>,
) -> Result<TerminalSessionInfo, AppError> {
    let _ = state.set_event_sink(Arc::new(TauriTerminalEventSink::new(app)));
    state.spawn_session(args, Arc::new(TauriTerminalDataSink::new(on_data)))
}

/// Writes base64-encoded user input bytes into an active or hidden terminal session (`C4.2`).
#[tauri::command]
pub fn terminal_write(
    state: State<'_, Arc<TerminalManager>>,
    args: TerminalWriteArgs,
) -> Result<(), AppError> {
    state.write_session(args)
}

/// Resizes an embedded terminal PTY session (`C4.2`).
#[tauri::command]
pub fn terminal_resize(
    state: State<'_, Arc<TerminalManager>>,
    args: TerminalResizeArgs,
) -> Result<(), AppError> {
    state.resize_session(args)
}

/// Toggles terminal visibility (fold-keepalive / resume & flush 1MB ring buffer) (`C4.2`, `C8.7`).
#[tauri::command]
pub fn terminal_set_visibility(
    app: AppHandle,
    state: State<'_, Arc<TerminalManager>>,
    args: TerminalVisibilityArgs,
) -> Result<(), AppError> {
    let _ = state.set_event_sink(Arc::new(TauriTerminalEventSink::new(app)));
    state.set_visibility(args)
}

/// Manually kills an embedded terminal session and reaps its process group (`C4.2`, `C8.7`).
#[tauri::command]
pub async fn terminal_kill(
    state: State<'_, Arc<TerminalManager>>,
    args: TerminalKillArgs,
) -> Result<(), AppError> {
    let manager = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || manager.kill_session(args))
        .await
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Terminal kill background task failed",
                err.to_string(),
            )
        })?
}

/// Lists all alive embedded terminal sessions (`C4.2`).
#[tauri::command]
pub fn terminal_list_sessions(
    state: State<'_, Arc<TerminalManager>>,
) -> Result<Vec<TerminalSessionInfo>, AppError> {
    state.list_sessions()
}
