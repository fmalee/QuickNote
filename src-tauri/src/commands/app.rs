use std::collections::BTreeMap;
use std::sync::Arc;

use tauri::menu::MenuItemKind;
use tauri::{AppHandle, State};

use crate::application::open_files::PendingOpenFilesQueue;
use crate::application::persistence::PersistenceManager;
use crate::application::window::WindowManager;
use crate::domain::app::{
    normalize_combo_to_tauri_accelerator, validate_external_url, AppInfo, OpenExternalUrlArgs,
    SetMenuAcceleratorsArgs,
};
use crate::domain::error::{AppError, ErrorCode};

/// Smoke IPC command returning basic application runtime metadata including real `activationPolicy` (`C4.2`, `M0-03`).
#[tauri::command]
pub fn app_get_info(
    pm: State<'_, Arc<PersistenceManager>>,
    wm: State<'_, Arc<WindowManager>>,
) -> Result<AppInfo, AppError> {
    let mut info = pm.app_info(env!("CARGO_PKG_VERSION"), std::env::consts::OS)?;
    info.activation_policy = wm.current_activation_policy()?;
    Ok(info)
}

/// Validates and launches an external `http://`, `https://`, or `mailto:` URL in the OS default handler (`C2.1`).
#[tauri::command]
pub fn app_open_external_url(args: OpenExternalUrlArgs) -> Result<(), AppError> {
    let url = validate_external_url(&args.url)?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|err| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to launch default browser",
                    err.to_string(),
                )
            })?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = url;
        Err(AppError::new(
            ErrorCode::ExternalAppMissing,
            "External URL launch is only supported on macOS",
        ))
    }
}

fn update_menu_items_accelerators(
    items: &[MenuItemKind<tauri::Wry>],
    entries: &BTreeMap<String, Option<String>>,
) -> Result<(), AppError> {
    for kind in items {
        match kind {
            MenuItemKind::MenuItem(item) => {
                let id = item.id().as_ref();
                if let Some(combo_opt) = entries.get(id) {
                    let accel = combo_opt
                        .as_deref()
                        .and_then(normalize_combo_to_tauri_accelerator);
                    item.set_accelerator(accel.as_deref()).map_err(|err| {
                        AppError::with_detail(
                            ErrorCode::InternalError,
                            "Failed to update native menu accelerator",
                            err.to_string(),
                        )
                    })?;
                }
            }
            MenuItemKind::Submenu(submenu) => {
                let sub_items = submenu.items().map_err(|err| {
                    AppError::with_detail(
                        ErrorCode::InternalError,
                        "Failed to inspect native submenu items",
                        err.to_string(),
                    )
                })?;
                update_menu_items_accelerators(&sub_items, entries)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Dynamically updates or clears native application menu accelerators (`M2-QA`).
#[tauri::command]
pub fn app_set_menu_accelerators(
    app: AppHandle,
    args: SetMenuAcceleratorsArgs,
) -> Result<(), AppError> {
    let Some(menu) = app.menu() else {
        return Ok(());
    };
    let items = menu.items().map_err(|err| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to inspect native application menu",
            err.to_string(),
        )
    })?;
    update_menu_items_accelerators(&items, &args.entries)
}

/// Atomically drains pending local file paths delivered by macOS (Dock drop, Finder Open With, or CLI args).
#[tauri::command]
pub fn app_take_pending_open_files(
    queue: State<'_, Arc<PendingOpenFilesQueue>>,
) -> Result<Vec<String>, AppError> {
    queue.take_all()
}
