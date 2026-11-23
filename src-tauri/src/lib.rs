use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::application::persistence::PersistenceManager;
use crate::infra::settings_watch::SettingsWatcher;

pub mod application;
pub mod bindings;
pub mod commands;
pub mod domain;
pub mod infra;

/// Boots the Tauri desktop runtime with registered IPC commands and persistence manager.
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let pm = Arc::new(
                PersistenceManager::open(app_data_dir)
                    .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?,
            );
            app.manage(Arc::clone(&pm));

            let handle = app.handle().clone();
            let pm_for_watch = Arc::clone(&pm);
            let watcher = SettingsWatcher::start(pm.app_data_dir(), move || {
                if let Ok(snapshot) = pm_for_watch.reload_settings() {
                    let _ = handle.emit("settings:status", &snapshot.status);
                }
            })
            .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;
            app.manage(watcher);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_get_info,
            commands::settings::settings_get,
            commands::settings::ui_state_get,
            commands::settings::ui_state_set,
            commands::library::library_get_info,
            commands::library::library_set_root,
        ])
        .run(tauri::generate_context!())
}
