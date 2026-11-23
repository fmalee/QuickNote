use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::application::library::{get_library_info, set_library_root};
use crate::application::persistence::PersistenceManager;
use crate::domain::error::AppError;
use crate::domain::settings::{LibraryInfo, SetRootArgs};
use crate::infra::vault_watch::VaultWatcher;

/// Returns current library root and note count (`C4.2`, `M0-02`).
#[tauri::command]
pub fn library_get_info(
    state: State<'_, Arc<PersistenceManager>>,
) -> Result<LibraryInfo, AppError> {
    get_library_info(&state)
}

/// Configures `library_root` and optionally initializes the skeleton directory tree (`C4.2`, `M0-02`).
#[tauri::command]
pub fn library_set_root(
    app: AppHandle,
    state: State<'_, Arc<PersistenceManager>>,
    args: SetRootArgs,
) -> Result<LibraryInfo, AppError> {
    let info = set_library_root(&state, args)?;
    if let Some(watcher) = app.try_state::<Arc<VaultWatcher>>() {
        if let Err(err) = watcher.rebind(info.root.as_deref().map(Path::new)) {
            eprintln!("[vault-watch] rebind failed: code={:?}", err.code);
        }
    }
    Ok(info)
}
