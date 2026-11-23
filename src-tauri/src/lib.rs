pub mod application;
pub mod bindings;
pub mod commands;
pub mod domain;
pub mod infra;

/// Boots the Tauri desktop runtime with registered IPC commands.
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app::app_get_info])
        .run(tauri::generate_context!())
}
