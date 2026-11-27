use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::menu::CheckMenuItem;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
#[cfg(not(target_os = "macos"))]
use tauri::{PhysicalPosition, PhysicalSize, Position, Size};

use crate::application::persistence::PersistenceManager;
use crate::domain::app::ActivationPolicy;
use crate::domain::capture::AppendResult;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::settings::{QuickBarSettingsPatch, SettingsPatch, WindowMode};
use crate::domain::window::{
    clamp_auxiliary_width_percent, HideReason, QuickBarMode, WindowPolicyState, WindowState,
    DEFAULT_AUXILIARY_WIDTH_PERCENT,
};
use crate::infra::macos::{log_native_event, NativeWindowBridge};

const MAIN_WINDOW_LABEL: &str = "main";
const QUICKBAR_WINDOW_LABEL: &str = "quickbar";
const SETTINGS_WINDOW_LABEL: &str = "settings";
const TOAST_WINDOW_LABEL: &str = "toast";

/// Physical frame snapshot of the main window before entering `AuxiliarySidebar45` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainWindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Layout mode for the resident main workbench window.
/// Reuses the single main WebView without spawning a separate editor WebView process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainWindowLayoutMode {
    Regular,
    AuxiliarySidebar45,
}

impl From<WindowMode> for MainWindowLayoutMode {
    fn from(value: WindowMode) -> Self {
        match value {
            WindowMode::Auxiliary => Self::AuxiliarySidebar45,
            WindowMode::Standard | WindowMode::Fullscreen => Self::Regular,
        }
    }
}

impl MainWindowLayoutMode {
    pub fn into_window_mode(self) -> WindowMode {
        match self {
            Self::Regular => WindowMode::Standard,
            Self::AuxiliarySidebar45 => WindowMode::Auxiliary,
        }
    }
}

/// Transition outcome produced by `MainWindowModeStateMachine`, providing a unified
/// invocation point for Tauri geometry restoration and macOS native bridge calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainWindowModeTransition {
    pub previous_mode: MainWindowLayoutMode,
    pub target_mode: MainWindowLayoutMode,
    pub auxiliary_width_percent: u8,
    pub restore_bounds: Option<MainWindowBounds>,
}

/// Pure state machine tracking `Regular` <-> `AuxiliarySidebar45` transitions
/// and remembering the pre-auxiliary regular window position and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainWindowModeStateMachine {
    current_mode: MainWindowLayoutMode,
    saved_regular_bounds: Option<MainWindowBounds>,
    auxiliary_width_percent: u8,
}

impl MainWindowModeStateMachine {
    pub fn new(initial_mode: WindowMode, auxiliary_width_percent: u8) -> Self {
        Self {
            current_mode: MainWindowLayoutMode::from(initial_mode),
            saved_regular_bounds: None,
            auxiliary_width_percent: clamp_auxiliary_width_percent(auxiliary_width_percent),
        }
    }

    pub fn current_mode(&self) -> MainWindowLayoutMode {
        self.current_mode
    }

    pub fn saved_regular_bounds(&self) -> Option<MainWindowBounds> {
        self.saved_regular_bounds
    }

    pub fn auxiliary_width_percent(&self) -> u8 {
        self.auxiliary_width_percent
    }

    /// Transitions between `Regular` and `AuxiliarySidebar45`, capturing regular bounds on entry
    /// and returning them for restoration on exit.
    pub fn transition_to(
        &mut self,
        target: MainWindowLayoutMode,
        current_bounds: Option<MainWindowBounds>,
        auxiliary_width_percent: Option<u8>,
    ) -> MainWindowModeTransition {
        if let Some(pct) = auxiliary_width_percent {
            self.auxiliary_width_percent = clamp_auxiliary_width_percent(pct);
        }

        let previous_mode = self.current_mode;
        let restore_bounds = match (previous_mode, target) {
            (MainWindowLayoutMode::Regular, MainWindowLayoutMode::AuxiliarySidebar45) => {
                if let Some(bounds) = current_bounds {
                    self.saved_regular_bounds = Some(bounds);
                }
                self.current_mode = MainWindowLayoutMode::AuxiliarySidebar45;
                None
            }
            (MainWindowLayoutMode::AuxiliarySidebar45, MainWindowLayoutMode::Regular) => {
                self.current_mode = MainWindowLayoutMode::Regular;
                self.saved_regular_bounds.take()
            }
            (MainWindowLayoutMode::Regular, MainWindowLayoutMode::Regular) => None,
            (
                MainWindowLayoutMode::AuxiliarySidebar45,
                MainWindowLayoutMode::AuxiliarySidebar45,
            ) => None,
        };

        MainWindowModeTransition {
            previous_mode,
            target_mode: target,
            auxiliary_width_percent: self.auxiliary_width_percent,
            restore_bounds,
        }
    }

    /// Toggles between `Regular` and `AuxiliarySidebar45`.
    pub fn toggle(
        &mut self,
        current_bounds: Option<MainWindowBounds>,
        auxiliary_width_percent: Option<u8>,
    ) -> MainWindowModeTransition {
        let target = match self.current_mode {
            MainWindowLayoutMode::Regular => MainWindowLayoutMode::AuxiliarySidebar45,
            MainWindowLayoutMode::AuxiliarySidebar45 => MainWindowLayoutMode::Regular,
        };
        self.transition_to(target, current_bounds, auxiliary_width_percent)
    }
}

/// Coordinates window visibility, dynamic `.accessory` <-> `.regular` activation policy,
/// resident `main`/`quickbar` lifecycle, on-demand `settings`/`toast` lifecycle,
/// `Regular` <-> `AuxiliarySidebar45` layout transitions, and `window:state` events (`D-0001`..`D-0003`).
pub struct WindowManager {
    policy_state: Mutex<WindowPolicyState>,
    layout_state: Mutex<MainWindowModeStateMachine>,
    bridge: Arc<dyn NativeWindowBridge>,
    toast_generation: AtomicU64,
    quick_bar_pin_menu_item: Mutex<Option<CheckMenuItem<tauri::Wry>>>,
}

impl WindowManager {
    pub fn new(
        bridge: Arc<dyn NativeWindowBridge>,
        main_initially_visible: bool,
        initial_main_mode: WindowMode,
    ) -> Self {
        Self {
            policy_state: Mutex::new(WindowPolicyState::with_initial(
                main_initially_visible,
                initial_main_mode,
            )),
            layout_state: Mutex::new(MainWindowModeStateMachine::new(
                initial_main_mode,
                DEFAULT_AUXILIARY_WIDTH_PERCENT,
            )),
            bridge,
            toast_generation: AtomicU64::new(0),
            quick_bar_pin_menu_item: Mutex::new(None),
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

    pub fn current_main_layout_mode(&self) -> Result<MainWindowLayoutMode, AppError> {
        let state = self
            .layout_state
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Window layout lock poisoned"))?;
        Ok(state.current_mode())
    }

    pub fn saved_regular_bounds(&self) -> Result<Option<MainWindowBounds>, AppError> {
        let state = self
            .layout_state
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Window layout lock poisoned"))?;
        Ok(state.saved_regular_bounds())
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

    /// Returns the existing `settings` window or creates it on demand so low-frequency
    /// WebView processes are not kept resident when closed.
    pub fn ensure_or_create_settings_window(
        &self,
        app: &AppHandle,
    ) -> Result<WebviewWindow, AppError> {
        if let Some(existing) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
            return Ok(existing);
        }

        let current_theme = app
            .try_state::<Arc<PersistenceManager>>()
            .and_then(|pm| pm.settings_get().ok())
            .map(|s| s.settings.editor.theme)
            .unwrap_or(crate::domain::settings::EditorTheme::System);
        let tauri_theme = match current_theme {
            crate::domain::settings::EditorTheme::Light => Some(tauri::Theme::Light),
            crate::domain::settings::EditorTheme::Dark => Some(tauri::Theme::Dark),
            crate::domain::settings::EditorTheme::System => None,
        };

        let win = WebviewWindowBuilder::new(
            app,
            SETTINGS_WINDOW_LABEL,
            WebviewUrl::App("index.html?window=settings".into()),
        )
        .title("QuickNote 设置")
        .inner_size(760.0, 560.0)
        .theme(tauri_theme)
        .visible(false)
        .build()
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to create settings window",
                err.to_string(),
            )
        })?;
        let resolved_dark = match tauri_theme {
            Some(tauri::Theme::Dark) => true,
            Some(tauri::Theme::Light) => false,
            _ => matches!(win.theme(), Ok(tauri::Theme::Dark)),
        };
        let bg = if resolved_dark {
            tauri::window::Color(20, 22, 26, 255)
        } else {
            tauri::window::Color(255, 255, 255, 255)
        };
        let _ = win.set_background_color(Some(bg));
        Ok(win)
    }

    /// Returns the existing `toast` window or creates and configures it as a native
    /// non-activating `NSPanel` on demand.
    pub fn ensure_or_create_toast_window(
        &self,
        app: &AppHandle,
    ) -> Result<WebviewWindow, AppError> {
        if let Some(existing) = app.get_webview_window(TOAST_WINDOW_LABEL) {
            return Ok(existing);
        }

        let win = WebviewWindowBuilder::new(
            app,
            TOAST_WINDOW_LABEL,
            WebviewUrl::App("index.html?window=toast".into()),
        )
        .title("QuickNote Toast")
        .inner_size(380.0, 44.0)
        .decorations(false)
        .resizable(false)
        .visible(false)
        .skip_taskbar(true)
        .build()
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to create toast window",
                err.to_string(),
            )
        })?;

        let ptr = window_ns_ptr_from_window(&win)?;
        self.bridge.configure_toast_panel(ptr)?;
        Ok(win)
    }

    pub fn show_main(
        &self,
        app: &AppHandle,
        mode: Option<WindowMode>,
        auxiliary_width_percent: u8,
    ) -> Result<WindowState, AppError> {
        let clamped_pct = clamp_auxiliary_width_percent(auxiliary_width_percent);
        let main_win = app.get_webview_window(MAIN_WINDOW_LABEL);
        let current_bounds = main_win.as_ref().and_then(capture_window_bounds);

        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.show_main(mode)
        };

        let effective_mode = mode.unwrap_or_else(|| win_state.mode.unwrap_or(WindowMode::Standard));

        let layout_transition = if effective_mode == WindowMode::Fullscreen {
            let mut layout = self.layout_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window layout lock poisoned")
            })?;
            layout.transition_to(MainWindowLayoutMode::Regular, None, Some(clamped_pct));
            None
        } else if mode.is_some() || effective_mode == WindowMode::Auxiliary {
            let mut layout = self.layout_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window layout lock poisoned")
            })?;
            Some(layout.transition_to(
                MainWindowLayoutMode::from(effective_mode),
                current_bounds,
                Some(clamped_pct),
            ))
        } else {
            None
        };

        if effective_mode == WindowMode::Auxiliary {
            // Keep macOS app in Accessory policy while entering auxiliary sidebar mode so
            // presenting and focusing over fullscreen apps (e.g. Chrome) never pulls the
            // user back to the desktop Space.
            self.apply_activation_policy(app, ActivationPolicy::Accessory);

            if let Some(win) = main_win {
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                }

                if let Some(transition) = layout_transition {
                    self.apply_main_layout_transition(
                        app,
                        &win,
                        transition,
                        ActivationPolicy::Accessory,
                    );
                }
            }
        } else {
            let exiting_auxiliary = layout_transition.as_ref().is_some_and(|t| {
                t.previous_mode == MainWindowLayoutMode::AuxiliarySidebar45
                    && t.target_mode == MainWindowLayoutMode::Regular
            });

            if let (Some(win), Some(transition)) = (main_win.as_ref(), layout_transition) {
                self.apply_main_layout_transition(app, win, transition, policy);
            }

            self.apply_activation_policy(app, policy);

            if let Some(win) = main_win {
                let native_handles_show = cfg!(target_os = "macos") && exiting_auxiliary;
                if !native_handles_show {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                }

                if effective_mode == WindowMode::Fullscreen {
                    if let Ok(ns_ptr) = window_ns_ptr_from_window(&win) {
                        let _ = self.bridge.configure_main_window_mode(
                            ns_ptr,
                            WindowMode::Fullscreen,
                            clamped_pct,
                        );
                    }
                    let _ = win.set_fullscreen(true);
                }
            }
        }

        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    /// Toggles the resident main window between `Regular` and `AuxiliarySidebar45` mode.
    /// If the main window is hidden, shows it directly in `AuxiliarySidebar45` mode,
    /// invokes the native auxiliary sidebar bridge, and broadcasts `window:state` to the frontend.
    pub fn toggle_auxiliary_sidebar_mode(
        &self,
        app: &AppHandle,
        auxiliary_width_percent: u8,
    ) -> Result<WindowState, AppError> {
        let is_main_visible = {
            let guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.is_main_visible()
        };

        let target_layout = if !is_main_visible {
            MainWindowLayoutMode::AuxiliarySidebar45
        } else {
            let current = self.current_main_layout_mode()?;
            match current {
                MainWindowLayoutMode::Regular => MainWindowLayoutMode::AuxiliarySidebar45,
                MainWindowLayoutMode::AuxiliarySidebar45 => MainWindowLayoutMode::Regular,
            }
        };

        self.set_main_layout_mode(app, target_layout, auxiliary_width_percent)
    }

    /// Switches the resident main window to an explicit `Regular` or `AuxiliarySidebar45` layout.
    pub fn set_main_layout_mode(
        &self,
        app: &AppHandle,
        target: MainWindowLayoutMode,
        auxiliary_width_percent: u8,
    ) -> Result<WindowState, AppError> {
        self.show_main(
            app,
            Some(target.into_window_mode()),
            auxiliary_width_percent,
        )
    }

    /// Unified native bridge & window-geometry hook for `Regular` <-> `AuxiliarySidebar45` transitions.
    fn apply_main_layout_transition(
        &self,
        _app: &AppHandle,
        win: &WebviewWindow,
        transition: MainWindowModeTransition,
        policy: ActivationPolicy,
    ) {
        if transition.target_mode == MainWindowLayoutMode::Regular
            && win.is_fullscreen().unwrap_or(false)
        {
            let _ = win.set_fullscreen(false);
        }

        // Unified native bridge invocation point: handles macOS full-screen Space overlay
        // (`collectionBehavior`), non-Space-switching focus, and frame save/restore.
        if let Ok(ns_ptr) = window_ns_ptr_from_window(win) {
            let bridge_result = match transition.target_mode {
                MainWindowLayoutMode::AuxiliarySidebar45 => self
                    .bridge
                    .enter_auxiliary_sidebar_mode(ns_ptr, transition.auxiliary_width_percent)
                    .map(|diag| ("auxiliary_enter", diag)),
                MainWindowLayoutMode::Regular => self
                    .bridge
                    .exit_auxiliary_sidebar_mode(ns_ptr)
                    .map(|diag| ("auxiliary_exit", diag)),
            };
            if let Ok((event_name, diag)) = bridge_result {
                log_native_event(
                    event_name,
                    policy,
                    diag.collection_behavior,
                    diag.is_key_window,
                    None,
                );
            }
        }

        #[cfg(not(target_os = "macos"))]
        if transition.target_mode == MainWindowLayoutMode::Regular {
            if let Some(bounds) = transition.restore_bounds {
                let _ = win.set_size(Size::Physical(PhysicalSize::new(
                    bounds.width,
                    bounds.height,
                )));
                let _ = win.set_position(Position::Physical(PhysicalPosition::new(
                    bounds.x, bounds.y,
                )));
            }
        }
    }

    pub fn hide_main(&self, app: &AppHandle) -> Result<WindowState, AppError> {
        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.hide_main()
        };

        if let Some(win) = app.get_webview_window(MAIN_WINDOW_LABEL) {
            let _ = win.hide();
        }

        self.apply_activation_policy(app, policy);
        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    pub fn open_settings(&self, app: &AppHandle) -> Result<WindowState, AppError> {
        let win = self.ensure_or_create_settings_window(app)?;

        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.open_settings()
        };

        self.apply_activation_policy(app, policy);

        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();

        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    /// Closes and destroys the on-demand `settings` window to free its `WKWebView` process.
    /// Safe to invoke even if the settings window has already been destroyed.
    pub fn close_settings(&self, app: &AppHandle) -> Result<WindowState, AppError> {
        let (win_state, policy) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            guard.close_settings()
        };

        if let Some(win) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
            let _ = win.hide();
            let _ = win.destroy();
        }

        self.apply_activation_policy(app, policy);
        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    /// Hides whichever standard window (`settings` or `main`) currently has focus (`⌘W`).
    pub fn hide_focused_window(&self, app: &AppHandle) -> Result<Option<WindowState>, AppError> {
        let settings_focused = app
            .get_webview_window(SETTINGS_WINDOW_LABEL)
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if settings_focused {
            return self.close_settings(app).map(Some);
        }

        let main_focused = app
            .get_webview_window(MAIN_WINDOW_LABEL)
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

        let ptr = window_ns_ptr(app, QUICKBAR_WINDOW_LABEL)?;
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

        if let Ok(ptr) = window_ns_ptr(app, QUICKBAR_WINDOW_LABEL) {
            let diag = self.bridge.hide_quick_bar_panel(ptr)?;
            let event_name = match reason {
                HideReason::SpaceChanged => "space_changed",
                HideReason::FocusLost => "focus_lost",
                HideReason::Esc | HideReason::Hotkey => "panel_hide",
            };
            log_native_event(
                event_name,
                policy,
                diag.collection_behavior,
                diag.is_key_window,
                Some(reason),
            );
        }

        let _ = app.emit("window:state", &win_state);
        Ok(win_state)
    }

    pub fn register_quick_bar_pin_menu_item(
        &self,
        item: CheckMenuItem<tauri::Wry>,
    ) -> Result<(), AppError> {
        let mut guard = self.quick_bar_pin_menu_item.lock().map_err(|_| {
            AppError::new(
                ErrorCode::InternalError,
                "Quick Bar pin menu item lock poisoned",
            )
        })?;
        *guard = Some(item);
        Ok(())
    }

    pub fn set_quick_bar_pinned(
        &self,
        app: &AppHandle,
        pm: &PersistenceManager,
        pinned: bool,
    ) -> Result<bool, AppError> {
        let snapshot = pm.settings_update(SettingsPatch {
            quick_bar: Some(QuickBarSettingsPatch {
                pinned: Some(pinned),
                ..Default::default()
            }),
            ..Default::default()
        })?;
        let applied = snapshot.settings.quick_bar.pinned;
        if let Ok(guard) = self.quick_bar_pin_menu_item.lock() {
            if let Some(item) = guard.as_ref() {
                let _ = item.set_checked(applied);
            }
        }
        let _ = app.emit("settings:status", &snapshot.status);
        Ok(applied)
    }

    /// Called when the Quick Bar panel loses key-window focus (`D-0013`).
    /// No-op when the panel is already hidden or pinned.
    pub fn on_quick_bar_lost_focus(
        &self,
        app: &AppHandle,
        pm: &PersistenceManager,
    ) -> Result<(), AppError> {
        if !self.is_quick_bar_visible()? {
            return Ok(());
        }
        if pm.settings_get()?.settings.quick_bar.pinned {
            return Ok(());
        }
        self.hide_quick_bar(app, HideReason::FocusLost)?;
        Ok(())
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

    /// Shows the 5-second non-activating Toast panel (creating it on demand) and emits
    /// `toast:append-undo` with `AppendResult` (`C8.3`). Destroys the Toast WebView when hidden.
    pub fn show_append_toast(
        &self,
        app: &AppHandle,
        payload: &AppendResult,
    ) -> Result<(), AppError> {
        let policy = self.current_activation_policy()?;
        let win = self.ensure_or_create_toast_window(app)?;
        let ptr = window_ns_ptr_from_window(&win)?;
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
                if let Some(wm) = app_for_main.try_state::<Arc<WindowManager>>() {
                    if wm.toast_generation.load(Ordering::SeqCst) == gen {
                        wm.hide_and_destroy_toast_window(&app_for_main);
                    }
                }
            });
        });

        Ok(())
    }

    /// Immediately hides and destroys the Toast window if alive, cancelling any pending auto-hide timer.
    /// Safe to invoke when Toast has already been destroyed.
    pub fn dismiss_toast(&self, app: &AppHandle) -> Result<(), AppError> {
        self.toast_generation.fetch_add(1, Ordering::SeqCst);
        self.hide_and_destroy_toast_window(app);
        Ok(())
    }

    fn hide_and_destroy_toast_window(&self, app: &AppHandle) {
        if let Some(toast_win) = app.get_webview_window(TOAST_WINDOW_LABEL) {
            if let Ok(toast_ptr) = window_ns_ptr_from_window(&toast_win) {
                if let Ok(diag) = self.bridge.hide_toast_panel(toast_ptr) {
                    if let Ok(policy) = self.current_activation_policy() {
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
            let _ = toast_win.destroy();
        }
    }

    /// Configures resident `quickbar` (and `toast` only if already present) as native non-activating `NSPanel`s.
    pub fn configure_native_panels(&self, app: &AppHandle) -> Result<(), AppError> {
        let qb_ptr = window_ns_ptr(app, QUICKBAR_WINDOW_LABEL)?;
        self.bridge.configure_quick_bar_panel(qb_ptr)?;
        if let Some(toast_win) = app.get_webview_window(TOAST_WINDOW_LABEL) {
            let toast_ptr = window_ns_ptr_from_window(&toast_win)?;
            self.bridge.configure_toast_panel(toast_ptr)?;
        }
        Ok(())
    }

    /// Synchronizes main window native fullscreen transitions with `WindowPolicyState` and emits `window:state`.
    pub fn sync_main_fullscreen_state(
        &self,
        app: &AppHandle,
        fullscreen: bool,
    ) -> Result<WindowState, AppError> {
        let (changed, win_state) = {
            let mut guard = self.policy_state.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Window policy lock poisoned")
            })?;
            let prev = guard.main_window_state();
            let next = guard.set_main_fullscreen(fullscreen);
            let changed = prev.fullscreen != next.fullscreen || prev.mode != next.mode;
            (changed, next)
        };

        if changed {
            if let Ok(mut layout) = self.layout_state.lock() {
                if let Some(mode) = win_state.mode {
                    layout.transition_to(MainWindowLayoutMode::from(mode), None, None);
                }
            }
            let _ = app.emit("window:state", &win_state);
        }

        Ok(win_state)
    }
}

fn capture_window_bounds(win: &WebviewWindow) -> Option<MainWindowBounds> {
    if win.is_fullscreen().unwrap_or(false) {
        return None;
    }
    let pos = win.outer_position().ok()?;
    let size = win.outer_size().ok()?;
    if size.width == 0 || size.height == 0 {
        return None;
    }
    Some(MainWindowBounds {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
}

fn window_ns_ptr(app: &AppHandle, label: &str) -> Result<usize, AppError> {
    let win: WebviewWindow = app.get_webview_window(label).ok_or_else(|| {
        AppError::with_detail(
            ErrorCode::NotFound,
            "Target window is not initialized",
            label,
        )
    })?;
    window_ns_ptr_from_window(&win)
}

fn window_ns_ptr_from_window(win: &WebviewWindow) -> Result<usize, AppError> {
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
