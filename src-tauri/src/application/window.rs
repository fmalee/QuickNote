use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::domain::app::ActivationPolicy;
use crate::domain::capture::AppendResult;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::settings::WindowMode;
use crate::domain::window::{HideReason, QuickBarMode, WindowPolicyState, WindowState};
use crate::infra::macos::{log_native_event, NativeWindowBridge};

/// Coordinates window visibility, dynamic `.accessory` <-> `.regular` activation policy,
/// Quick Bar / Toast `NSPanel` lifecycle, and `window:state` event emission (`D-0001`..`D-0003`).
pub struct WindowManager {
    policy_state: Mutex<WindowPolicyState>,
    bridge: Arc<dyn NativeWindowBridge>,
    toast_generation: AtomicU64,
}

impl WindowManager {
    pub fn new(bridge: Arc<dyn NativeWindowBridge>, main_initially_visible: bool) -> Self {
        Self {
            policy_state: Mutex::new(WindowPolicyState::with_main_visible(main_initially_visible)),
            bridge,
            toast_generation: AtomicU64::new(0),
        }
    }

    pub fn current_activation_policy(&self) -> Result<ActivationPolicy, AppError> {
        let state = self
            .policy_state
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Window policy lock poisoned"))?;
        Ok(state.activation_policy())
    }

    pub fn is_quick_bar_visible(&self) -> Result<bool, AppError> {
        let state = self
            .policy_state
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Window policy lock poisoned"))?;
        Ok(state.is_quick_bar_visible())
    }

    /// Applies the Tauri/macOS activation policy and emits `C2.4` diagnostic log.
    pub fn apply_activation_policy(&self, app: &AppHandle, policy: ActivationPolicy) {
        #[cfg(target_os = "macos")]
        {
            let tauri_policy = match policy {
                ActivationPolicy::Accessory => tauri::ActivationPolicy::Accessory,
                ActivationPolicy::Regular => tauri::ActivationPolicy::Regular,
            };
            let _ = app.set_activation_policy(tauri_policy);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = app;

        log_native_event("policy_switch", policy, 0, false, None);
    }

    pub fn show_main(
        &self,
        app: &AppHandle,
        mode: Option<WindowMode>,
    ) -> Result<WindowState, AppError> {
        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.show_main(mode)
        };

        self.apply_activation_policy(app, policy);

        if let Some(win) = app.get_webview_window("main") {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
            if mode == Some(WindowMode::Fullscreen) {
                let _ = win.set_fullscreen(true);
            } else if mode == Some(WindowMode::Standard) {
                let _ = win.set_fullscreen(false);
            }
        }

        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    pub fn hide_main(&self, app: &AppHandle) -> Result<WindowState, AppError> {
        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.hide_main()
        };

        if let Some(win) = app.get_webview_window("main") {
            let _ = win.hide();
        }

        self.apply_activation_policy(app, policy);
        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    pub fn open_settings(&self, app: &AppHandle) -> Result<WindowState, AppError> {
        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.open_settings()
        };

        self.apply_activation_policy(app, policy);

        if let Some(win) = app.get_webview_window("settings") {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }

        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    pub fn close_settings(&self, app: &AppHandle) -> Result<WindowState, AppError> {
        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.close_settings()
        };

        if let Some(win) = app.get_webview_window("settings") {
            let _ = win.hide();
        }

        self.apply_activation_policy(app, policy);
        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    /// Hides whichever standard window (`settings` or `main`) currently has focus (`⌘W`).
    pub fn hide_focused_window(&self, app: &AppHandle) -> Result<Option<WindowState>, AppError> {
        let settings_focused = app
            .get_webview_window("settings")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if settings_focused {
            return self.close_settings(app).map(Some);
        }

        let main_focused = app
            .get_webview_window("main")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if main_focused {
            return self.hide_main(app).map(Some);
        }

        Ok(None)
    }

    pub fn show_quick_bar(
        &self,
        app: &AppHandle,
        mode: Option<QuickBarMode>,
    ) -> Result<WindowState, AppError> {
        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            let ws = guard.show_quick_bar(mode);
            let pol = guard.activation_policy();
            (ws, pol)
        };

        let ptr = window_ns_ptr(app, "quickbar")?;
        let diag = self.bridge.show_quick_bar_panel(ptr)?;
        log_native_event(
            "panel_show",
            policy,
            diag.collection_behavior,
            diag.is_key_window,
            None,
        );

        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    pub fn hide_quick_bar(
        &self,
        app: &AppHandle,
        reason: HideReason,
    ) -> Result<WindowState, AppError> {
        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            let ws = guard.hide_quick_bar(reason);
            let pol = guard.activation_policy();
            (ws, pol)
        };

        let ptr = window_ns_ptr(app, "quickbar")?;
        let diag = self.bridge.hide_quick_bar_panel(ptr)?;
        let event_name = match reason {
            HideReason::SpaceChanged => "space_changed",
            HideReason::Esc | HideReason::Hotkey => "panel_hide",
        };
        log_native_event(
            event_name,
            policy,
            diag.collection_behavior,
            diag.is_key_window,
            Some(reason),
        );

        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    pub fn toggle_quick_bar(&self, app: &AppHandle) -> Result<WindowState, AppError> {
        let (visible, policy) = {
            let guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            (guard.is_quick_bar_visible(), guard.activation_policy())
        };
        log_native_event("hotkey_fired", policy, 256, false, None);

        if visible {
            self.hide_quick_bar(app, HideReason::Hotkey)
        } else {
            self.show_quick_bar(app, None)
        }
    }

    /// Triggers the non-activating bottom Toast panel for 5 seconds and emits `toast:append-undo` (`M0-03`).
    pub fn show_debug_toast(&self, app: &AppHandle) -> Result<(), AppError> {
        let debug_payload = AppendResult {
            rel_path: "inbox.md".to_string(),
            title: "收件箱".to_string(),
            added_lines: 1,
            section_heading: Some("2026-11-23".to_string()),
            body_hash: "debug-toast".to_string(),
            mtime_ms: 0,
            undo_available: true,
        };
        self.show_append_toast(app, &debug_payload)
    }

    /// Shows the 5-second non-activating Toast panel and emits `toast:append-undo` with `AppendResult` (`C8.3`).
    pub fn show_append_toast(
        &self,
        app: &AppHandle,
        payload: &AppendResult,
    ) -> Result<(), AppError> {
        let policy = self.current_activation_policy()?;
        let ptr = window_ns_ptr(app, "toast")?;
        let diag = self.bridge.show_toast_panel(ptr)?;
        log_native_event(
            "toast_show",
            policy,
            diag.collection_behavior,
            diag.is_key_window,
            None,
        );

        let _ = app.emit("toast:append-undo", payload);

        let gen = self.toast_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let app_handle = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            let app_for_main = app_handle.clone();
            let _ = app_handle.run_on_main_thread(move || {
                let wm = app_for_main.state::<Arc<WindowManager>>();
                if wm.toast_generation.load(Ordering::SeqCst) == gen {
                    if let Ok(toast_ptr) = window_ns_ptr(&app_for_main, "toast") {
                        if let Ok(diag) = wm.bridge.hide_toast_panel(toast_ptr) {
                            if let Ok(policy) = wm.current_activation_policy() {
                                log_native_event(
                                    "toast_hide",
                                    policy,
                                    diag.collection_behavior,
                                    diag.is_key_window,
                                    None,
                                );
                            }
                        }
                    }
                }
            });
        });

        Ok(())
    }

    /// Configures `quickbar` and `toast` windows as native non-activating `NSPanel`s during app setup.
    pub fn configure_native_panels(&self, app: &AppHandle) -> Result<(), AppError> {
        let qb_ptr = window_ns_ptr(app, "quickbar")?;
        self.bridge.configure_quick_bar_panel(qb_ptr)?;
        let toast_ptr = window_ns_ptr(app, "toast")?;
        self.bridge.configure_toast_panel(toast_ptr)?;
        Ok(())
    }
}

fn window_ns_ptr(app: &AppHandle, label: &str) -> Result<usize, AppError> {
    let win: WebviewWindow = app.get_webview_window(label).ok_or_else(|| {
        AppError::with_detail(
            ErrorCode::NotFound,
            "Target window is not initialized",
            label,
        )
    })?;

    #[cfg(target_os = "macos")]
    {
        let ptr = win.ns_window().map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to acquire NSWindow pointer",
                err.to_string(),
            )
        })?;
        Ok(ptr as usize)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = win;
        Ok(0)
    }
}
