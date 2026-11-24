use std::sync::{Arc, OnceLock};

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::application::capture::CaptureManager;
use crate::application::hotkey::HotkeyManager;
use crate::application::persistence::PersistenceManager;
use crate::application::window::WindowManager;
use crate::domain::window::HideReason;
use crate::infra::macos::{NativeWindowBridge, NoopNativeBridge};
use crate::infra::settings_watch::SettingsWatcher;
use crate::infra::vault_watch::VaultWatcher;

pub mod application;
pub mod bindings;
pub mod commands;
pub mod domain;
pub mod infra;

static GLOBAL_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// C callback invoked by Carbon `RegisterEventHotKey` when the global hotkey fires.
pub extern "C" fn on_hotkey_pressed() {
    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        if let Some(hm) = app.try_state::<Arc<HotkeyManager>>() {
            if let Err(err) = hm.mark_hotkey_triggered() {
                eprintln!("[hotkey] mark_hotkey_triggered failed: code={:?}", err.code);
            }
        }
        let wm = app.state::<Arc<WindowManager>>();
        if let Err(err) = wm.toggle_quick_bar(app) {
            eprintln!("[hotkey] toggle_quick_bar failed: code={:?}", err.code);
        }
    }
}

extern "C" fn on_quick_bar_esc() {
    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        let wm = app.state::<Arc<WindowManager>>();
        if let Err(err) = wm.hide_quick_bar(app, HideReason::Esc) {
            eprintln!("[quickbar] hide_quick_bar(esc) failed: code={:?}", err.code);
        }
    }
}

extern "C" fn on_space_changed() {
    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        let wm = app.state::<Arc<WindowManager>>();
        if wm.is_quick_bar_visible().unwrap_or(false) {
            if let Err(err) = wm.hide_quick_bar(app, HideReason::SpaceChanged) {
                eprintln!(
                    "[quickbar] hide_quick_bar(space_changed) failed: code={:?}",
                    err.code
                );
            }
        }
    }
}

fn create_native_bridge() -> Arc<dyn NativeWindowBridge> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(crate::infra::macos::darwin::MacosNativeBridge)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(NoopNativeBridge)
    }
}

#[allow(dead_code)]
fn noop_bridge() -> Arc<dyn NativeWindowBridge> {
    Arc::new(NoopNativeBridge)
}

fn build_app_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, tauri::Error> {
    let app_submenu = SubmenuBuilder::new(app, "QuickNote")
        .item(&PredefinedMenuItem::about(
            app,
            Some("关于 QuickNote"),
            None,
        )?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("menu_settings", "设置…")
                .accelerator("CmdOrCtrl+,")
                .build(app)?,
        )
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some("隐藏 QuickNote"))?)
        .item(&PredefinedMenuItem::hide_others(app, Some("隐藏其他"))?)
        .item(&PredefinedMenuItem::show_all(app, Some("显示全部"))?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("退出 QuickNote"))?)
        .build()?;

    let file_submenu = SubmenuBuilder::new(app, "文件")
        .item(
            &MenuItemBuilder::with_id("window.newNote", "新建笔记")
                .accelerator("CmdOrCtrl+N")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("menu_quick_bar", "快速捕捉").build(app)?)
        .item(
            &MenuItemBuilder::with_id("menu_quick_open", "快速打开…")
                .accelerator("CmdOrCtrl+K")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("menu_command_book", "命令库…")
                .accelerator("CmdOrCtrl+Shift+K")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("menu_hide_window", "关闭窗口").build(app)?)
        .build()?;

    let edit_submenu = SubmenuBuilder::new(app, "编辑")
        .item(&PredefinedMenuItem::undo(app, Some("撤销"))?)
        .item(&PredefinedMenuItem::redo(app, Some("重做"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some("剪切"))?)
        .item(&PredefinedMenuItem::copy(app, Some("复制"))?)
        .item(&PredefinedMenuItem::paste(app, Some("粘贴"))?)
        .item(&PredefinedMenuItem::select_all(app, Some("全选"))?)
        .build()?;

    let format_submenu = SubmenuBuilder::new(app, "格式")
        .item(
            &MenuItemBuilder::with_id("editor.bold", "加粗")
                .accelerator("CmdOrCtrl+B")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.italic", "斜体")
                .accelerator("CmdOrCtrl+I")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.codeBlock", "代码块")
                .accelerator("Alt+CmdOrCtrl+C")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.table", "表格")
                .accelerator("Alt+CmdOrCtrl+G")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.toggleTodo", "切换待办")
                .accelerator("Alt+CmdOrCtrl+X")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.link", "插入链接")
                .accelerator("Alt+CmdOrCtrl+K")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.image", "插入图片")
                .accelerator("Alt+CmdOrCtrl+I")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.bulletList", "无序列表")
                .accelerator("Alt+CmdOrCtrl+L")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.blockquote", "引用块")
                .accelerator("Alt+CmdOrCtrl+Q")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("editor.heading1", "标题 1")
                .accelerator("Alt+CmdOrCtrl+1")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.heading2", "标题 2")
                .accelerator("Alt+CmdOrCtrl+2")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.heading3", "标题 3")
                .accelerator("Alt+CmdOrCtrl+3")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.heading4", "标题 4")
                .accelerator("Alt+CmdOrCtrl+4")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.heading5", "标题 5")
                .accelerator("Alt+CmdOrCtrl+5")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("editor.heading6", "标题 6")
                .accelerator("Alt+CmdOrCtrl+6")
                .build(app)?,
        )
        .build()?;

    let view_submenu = SubmenuBuilder::new(app, "视图")
        .item(
            &MenuItemBuilder::with_id("panel.toggle.outline", "大纲面板")
                .accelerator("Ctrl+CmdOrCtrl+O")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("menu_view_terminal", "终端面板")
                .accelerator("Ctrl+CmdOrCtrl+T")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("panel.collapseGroup", "折叠/展开右侧面板")
                .accelerator("Ctrl+CmdOrCtrl+0")
                .build(app)?,
        )
        .separator()
        .item(&PredefinedMenuItem::fullscreen(app, Some("进入/退出全屏"))?)
        .build()?;

    let tools_submenu = SubmenuBuilder::new(app, "工具")
        .item(&MenuItemBuilder::with_id("menu_tools_commands", "打开命令库").build(app)?)
        .item(&MenuItemBuilder::with_id("menu_tools_toast", "调试：触发 5 秒 Toast").build(app)?)
        .build()?;

    let window_submenu = SubmenuBuilder::new(app, "窗口")
        .item(&PredefinedMenuItem::minimize(app, Some("最小化"))?)
        .item(&PredefinedMenuItem::maximize(app, Some("缩放"))?)
        .separator()
        .item(&MenuItemBuilder::with_id("menu_show_main", "显示主窗口").build(app)?)
        .build()?;

    let help_submenu = SubmenuBuilder::new(app, "帮助")
        .item(&MenuItemBuilder::with_id("menu_help_about", "QuickNote 快捷键指南").build(app)?)
        .build()?;

    MenuBuilder::new(app)
        .item(&app_submenu)
        .item(&file_submenu)
        .item(&edit_submenu)
        .item(&format_submenu)
        .item(&view_submenu)
        .item(&tools_submenu)
        .item(&window_submenu)
        .item(&help_submenu)
        .build()
}

fn setup_tray_and_menu(app: &AppHandle) -> Result<(), tauri::Error> {
    let menu = build_app_menu(app)?;
    app.set_menu(menu)?;

    let tray_menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("tray_quick_bar", "快速捕捉").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("tray_show_main", "打开主窗口").build(app)?)
        .item(&MenuItemBuilder::with_id("tray_hide_main", "隐藏主窗口").build(app)?)
        .item(&MenuItemBuilder::with_id("tray_settings", "设置…").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("tray_debug_toast", "调试：触发 5 秒 Toast 浮窗")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("tray_quit", "退出 QuickNote").build(app)?)
        .build()?;

    let mut tray_builder = TrayIconBuilder::with_id("quicknote-tray")
        .menu(&tray_menu)
        .tooltip("QuickNote")
        .show_menu_on_left_click(true);

    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    tray_builder.build(app)?;

    app.on_menu_event(|app_handle, event| {
        let wm = app_handle.state::<Arc<WindowManager>>();
        let menu_id = event.id().as_ref();
        match menu_id {
            "tray_quick_bar" | "menu_quick_bar" => {
                if let Err(err) = wm.show_quick_bar(app_handle, None) {
                    eprintln!("[menu] {menu_id} failed: code={:?}", err.code);
                }
            }
            "tray_show_main" | "menu_show_main" => {
                if let Err(err) = wm.show_main(app_handle, None) {
                    eprintln!("[menu] {menu_id} failed: code={:?}", err.code);
                }
            }
            "tray_hide_main" => {
                if let Err(err) = wm.hide_main(app_handle) {
                    eprintln!("[menu] {menu_id} failed: code={:?}", err.code);
                }
            }
            "menu_hide_window" => {
                if let Err(err) = wm.hide_focused_window(app_handle) {
                    eprintln!("[menu] {menu_id} failed: code={:?}", err.code);
                }
            }
            "tray_settings" | "menu_settings" => {
                if let Err(err) = wm.open_settings(app_handle) {
                    eprintln!("[menu] {menu_id} failed: code={:?}", err.code);
                }
            }
            "tray_debug_toast" | "menu_tools_toast" => {
                if let Err(err) = wm.show_debug_toast(app_handle) {
                    eprintln!("[menu] {menu_id} failed: code={:?}", err.code);
                }
            }
            "tray_quit" => {
                app_handle.exit(0);
            }
            "window.newNote"
            | "editor.bold"
            | "editor.italic"
            | "editor.codeBlock"
            | "editor.table"
            | "editor.toggleTodo"
            | "editor.link"
            | "editor.image"
            | "editor.bulletList"
            | "editor.blockquote"
            | "editor.heading1"
            | "editor.heading2"
            | "editor.heading3"
            | "editor.heading4"
            | "editor.heading5"
            | "editor.heading6"
            | "panel.toggle.outline"
            | "panel.collapseGroup" => {
                let main_focused = app_handle
                    .get_webview_window("main")
                    .and_then(|win| win.is_focused().ok())
                    .unwrap_or(false);
                if main_focused {
                    if let Err(err) = app_handle.emit("menu:action", menu_id) {
                        eprintln!("[menu] emit menu:action failed: {err}");
                    }
                }
            }
            _ => {}
        }
    });

    Ok(())
}

fn create_aux_windows(app: &AppHandle) -> Result<(), tauri::Error> {
    if app.get_webview_window("quickbar").is_none() {
        WebviewWindowBuilder::new(
            app,
            "quickbar",
            WebviewUrl::App("index.html?window=quickbar".into()),
        )
        .title("QuickNote Quick Bar")
        .inner_size(640.0, 420.0)
        .decorations(false)
        .resizable(false)
        .visible(false)
        .skip_taskbar(true)
        .build()?;
    }

    if app.get_webview_window("toast").is_none() {
        WebviewWindowBuilder::new(
            app,
            "toast",
            WebviewUrl::App("index.html?window=toast".into()),
        )
        .title("QuickNote Toast")
        .inner_size(380.0, 44.0)
        .decorations(false)
        .resizable(false)
        .visible(false)
        .skip_taskbar(true)
        .build()?;
    }

    if app.get_webview_window("settings").is_none() {
        WebviewWindowBuilder::new(
            app,
            "settings",
            WebviewUrl::App("index.html?window=settings".into()),
        )
        .title("QuickNote 设置")
        .inner_size(760.0, 560.0)
        .visible(false)
        .build()?;
    }

    Ok(())
}

/// Boots the Tauri desktop runtime with registered IPC commands, native macOS bridge, tray, and hotkey.
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(wm) = app.try_state::<Arc<WindowManager>>() {
                if let Err(err) = wm.show_main(app, None) {
                    eprintln!("[single-instance] show_main failed: code={:?}", err.code);
                }
            }
        }))
        .setup(|app| {
            let _ = GLOBAL_APP_HANDLE.set(app.handle().clone());

            let app_data_dir = app.path().app_data_dir()?;
            let pm = Arc::new(
                PersistenceManager::open(app_data_dir)
                    .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?,
            );
            app.manage(Arc::clone(&pm));

            let bridge = create_native_bridge();
            let main_initially_visible = app
                .get_webview_window("main")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(true);

            let wm = Arc::new(WindowManager::new(
                Arc::clone(&bridge),
                main_initially_visible,
            ));
            app.manage(Arc::clone(&wm));

            let hm = Arc::new(HotkeyManager::new(Arc::clone(&bridge), Arc::clone(&pm)));
            app.manage(Arc::clone(&hm));

            let cm = Arc::new(CaptureManager::new());
            app.manage(Arc::clone(&cm));

            create_aux_windows(app.handle())?;
            wm.configure_native_panels(app.handle())
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;

            bridge.register_quick_bar_esc_handler(on_quick_bar_esc);
            bridge
                .register_space_observer(on_space_changed)
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;

            if let Err(err) = hm.init_and_self_check(on_hotkey_pressed) {
                eprintln!("[hotkey] init_and_self_check failed: code={:?}", err.code);
            }
            setup_tray_and_menu(app.handle())?;

            let policy = wm
                .current_activation_policy()
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;
            wm.apply_activation_policy(app.handle(), policy);

            let handle = app.handle().clone();
            let pm_for_watch = Arc::clone(&pm);
            let watcher = SettingsWatcher::start(pm.app_data_dir(), move || {
                match pm_for_watch.reload_settings() {
                    Ok(snapshot) => {
                        if let Err(err) = handle.emit("settings:status", &snapshot.status) {
                            eprintln!("[settings-watch] emit settings:status failed: {err}");
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "[settings-watch] reload_settings failed: code={:?}",
                            err.code
                        );
                    }
                }
            })
            .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;
            app.manage(watcher);

            let initial_root = pm
                .current_library_root()
                .ok()
                .flatten()
                .map(std::path::PathBuf::from);
            let handle_for_vault = app.handle().clone();
            let pm_for_vault = Arc::clone(&pm);
            let vault_watcher = Arc::new(
                VaultWatcher::start(initial_root.as_deref(), move |path| {
                    match crate::application::watch::process_external_fs_path(&pm_for_vault, path) {
                        Ok(Some(event)) => {
                            if let Err(err) = handle_for_vault.emit("fs:external-change", &event) {
                                eprintln!("[vault-watch] emit fs:external-change failed: {err}");
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            eprintln!(
                                "[vault-watch] process_external_fs_path failed: code={:?}",
                                err.code
                            );
                        }
                    }
                })
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?,
            );
            app.manage(vault_watcher);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == "main" {
                    api.prevent_close();
                    if let Some(wm) = window.app_handle().try_state::<Arc<WindowManager>>() {
                        if let Err(err) = wm.hide_main(window.app_handle()) {
                            eprintln!("[window] hide_main failed: code={:?}", err.code);
                        }
                    }
                } else if label == "settings" {
                    api.prevent_close();
                    if let Some(wm) = window.app_handle().try_state::<Arc<WindowManager>>() {
                        if let Err(err) = wm.close_settings(window.app_handle()) {
                            eprintln!("[window] close_settings failed: code={:?}", err.code);
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_get_info,
            commands::app::app_open_external_url,
            commands::app::app_set_menu_accelerators,
            commands::settings::settings_get,
            commands::settings::settings_update,
            commands::settings::ui_state_get,
            commands::settings::ui_state_set,
            commands::library::library_get_info,
            commands::library::library_set_root,
            commands::window::window_show_main,
            commands::window::window_hide_main,
            commands::window::window_show_quick_bar,
            commands::window::window_hide_quick_bar,
            commands::window::window_open_settings,
            commands::window::hotkey_update,
            commands::index::index_get_status,
            commands::index::index_rebuild,
            commands::notes::notes_list,
            commands::notes::notes_get_tree_and_tags,
            commands::notes::note_read,
            commands::notes::note_create,
            commands::notes::note_write,
            commands::notes::note_update_meta,
            commands::notes::note_rename,
            commands::notes::note_delete,
            commands::notes::note_save_image,
            commands::notes::notes_get_related,
            commands::notes::tags_batch_rename,
            commands::search::search_notes_and_files,
            commands::search::commands_search,
            commands::capture::capture_resolve_target,
            commands::capture::note_append,
            commands::capture::note_undo_last_append,
            commands::capture::inbox_extract_to_note,
        ])
        .run(tauri::generate_context!())
}
