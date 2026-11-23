use std::sync::{Arc, OnceLock};

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::application::hotkey::HotkeyManager;
use crate::application::persistence::PersistenceManager;
use crate::application::window::WindowManager;
use crate::domain::window::HideReason;
use crate::infra::macos::{NativeWindowBridge, NoopNativeBridge};
use crate::infra::settings_watch::SettingsWatcher;

pub mod application;
pub mod bindings;
pub mod commands;
pub mod domain;
pub mod infra;

static GLOBAL_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// C callback invoked by Carbon `RegisterEventHotKey` when the global hotkey fires.
pub extern "C" fn on_hotkey_pressed() {
    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        let wm = app.state::<Arc<WindowManager>>();
        let _ = wm.toggle_quick_bar(app);
    }
}

extern "C" fn on_quick_bar_esc() {
    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        let wm = app.state::<Arc<WindowManager>>();
        let _ = wm.hide_quick_bar(app, HideReason::Esc);
    }
}

extern "C" fn on_space_changed() {
    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        let wm = app.state::<Arc<WindowManager>>();
        if wm.is_quick_bar_visible().unwrap_or(false) {
            let _ = wm.hide_quick_bar(app, HideReason::SpaceChanged);
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
            &MenuItemBuilder::with_id("menu_quick_bar", "快速捕捉")
                .accelerator("Ctrl+Alt+Space")
                .build(app)?,
        )
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
        .item(
            &MenuItemBuilder::with_id("menu_hide_window", "关闭窗口")
                .accelerator("CmdOrCtrl+W")
                .build(app)?,
        )
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
            &MenuItemBuilder::with_id("menu_fmt_bold", "加粗")
                .accelerator("CmdOrCtrl+B")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("menu_fmt_italic", "斜体")
                .accelerator("CmdOrCtrl+I")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("menu_fmt_code", "代码块")
                .accelerator("Alt+CmdOrCtrl+C")
                .build(app)?,
        )
        .build()?;

    let view_submenu = SubmenuBuilder::new(app, "视图")
        .item(
            &MenuItemBuilder::with_id("menu_view_outline", "大纲面板")
                .accelerator("Ctrl+CmdOrCtrl+O")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("menu_view_terminal", "终端面板")
                .accelerator("Ctrl+CmdOrCtrl+T")
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
        .item(&MenuItemBuilder::with_id("tray_quick_bar", "快速捕捉 (⌃⌥Space)").build(app)?)
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
        match event.id().as_ref() {
            "tray_quick_bar" | "menu_quick_bar" => {
                let _ = wm.show_quick_bar(app_handle, None);
            }
            "tray_show_main" | "menu_show_main" => {
                let _ = wm.show_main(app_handle, None);
            }
            "tray_hide_main" | "menu_hide_window" => {
                let _ = wm.hide_main(app_handle);
            }
            "tray_settings" | "menu_settings" => {
                let _ = wm.open_settings(app_handle);
            }
            "tray_debug_toast" | "menu_tools_toast" => {
                let _ = wm.show_debug_toast(app_handle);
            }
            "tray_quit" => {
                app_handle.exit(0);
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
        .inner_size(420.0, 56.0)
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
                let _ = wm.show_main(app, None);
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

            create_aux_windows(app.handle())?;
            wm.configure_native_panels(app.handle())
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;

            bridge.register_quick_bar_esc_handler(on_quick_bar_esc);
            bridge
                .register_space_observer(on_space_changed)
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;

            let _ = hm.init_and_self_check(on_hotkey_pressed);
            setup_tray_and_menu(app.handle())?;

            let policy = wm
                .current_activation_policy()
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;
            wm.apply_activation_policy(app.handle(), policy);

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
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == "main" {
                    api.prevent_close();
                    if let Some(wm) = window.app_handle().try_state::<Arc<WindowManager>>() {
                        let _ = wm.hide_main(window.app_handle());
                    }
                } else if label == "settings" {
                    api.prevent_close();
                    if let Some(wm) = window.app_handle().try_state::<Arc<WindowManager>>() {
                        let _ = wm.close_settings(window.app_handle());
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_get_info,
            commands::settings::settings_get,
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
        ])
        .run(tauri::generate_context!())
}
