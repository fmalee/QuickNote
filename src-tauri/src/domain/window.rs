use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::app::ActivationPolicy;
use crate::domain::settings::WindowMode;

/// Quick Bar operating mode (`C3.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum QuickBarMode {
    #[default]
    Capture,
    Open,
    Command,
}

/// Reason why Quick Bar was hidden (`C3.4`, `D-0002`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum HideReason {
    Esc,
    Hotkey,
    #[serde(rename = "space_changed")]
    SpaceChanged,
}

/// Window visibility and mode state pushed via `window:state` and window IPC commands (`C3.4`, `C4.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub window_id: String,
    pub visible: bool,
    pub fullscreen: bool,
    pub mode: Option<WindowMode>,
    pub reason: Option<HideReason>,
}

/// Arguments for `window_show_main` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct MainWindowArgs {
    pub mode: Option<WindowMode>,
}

/// Arguments for `window_show_quick_bar` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuickBarArgs {
    pub mode: Option<QuickBarMode>,
}

/// Arguments for `window_hide_quick_bar` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HideQuickBarArgs {
    pub reason: HideReason,
}

/// Pure in-memory state machine for dynamic `.accessory` <-> `.regular` activation policy (`D-0003`, `M0-03`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPolicyState {
    main_visible: bool,
    settings_visible: bool,
    quick_bar_visible: bool,
    main_mode: WindowMode,
    main_fullscreen: bool,
    quick_bar_mode: QuickBarMode,
}

impl Default for WindowPolicyState {
    fn default() -> Self {
        Self {
            main_visible: false,
            settings_visible: false,
            quick_bar_visible: false,
            main_mode: WindowMode::Standard,
            main_fullscreen: false,
            quick_bar_mode: QuickBarMode::Capture,
        }
    }
}

impl WindowPolicyState {
    /// Creates a policy state with explicit initial visibility for main window.
    pub fn with_main_visible(main_visible: bool) -> Self {
        Self {
            main_visible,
            ..Self::default()
        }
    }

    /// Computes the target macOS activation policy (`D-0003`):
    /// `.regular` when either main or settings window is visible; `.accessory` when both are hidden.
    /// Quick Bar visibility never changes the activation policy.
    pub fn activation_policy(&self) -> ActivationPolicy {
        if self.main_visible || self.settings_visible {
            ActivationPolicy::Regular
        } else {
            ActivationPolicy::Accessory
        }
    }

    pub fn is_main_visible(&self) -> bool {
        self.main_visible
    }

    pub fn is_settings_visible(&self) -> bool {
        self.settings_visible
    }

    pub fn is_quick_bar_visible(&self) -> bool {
        self.quick_bar_visible
    }

    pub fn main_mode(&self) -> WindowMode {
        self.main_mode
    }

    pub fn quick_bar_mode(&self) -> QuickBarMode {
        self.quick_bar_mode
    }

    /// Shows the main window, optionally updating its mode, and returns `(WindowState, ActivationPolicy)`.
    pub fn show_main(&mut self, mode: Option<WindowMode>) -> (WindowState, ActivationPolicy) {
        if let Some(m) = mode {
            self.main_mode = m;
            self.main_fullscreen = m == WindowMode::Fullscreen;
        }
        self.main_visible = true;
        (self.main_window_state(), self.activation_policy())
    }

    /// Hides the main window and returns `(WindowState, ActivationPolicy)`.
    pub fn hide_main(&mut self) -> (WindowState, ActivationPolicy) {
        self.main_visible = false;
        (self.main_window_state(), self.activation_policy())
    }

    /// Updates main window fullscreen flag.
    pub fn set_main_fullscreen(&mut self, fullscreen: bool) -> WindowState {
        self.main_fullscreen = fullscreen;
        if fullscreen {
            self.main_mode = WindowMode::Fullscreen;
        } else if self.main_mode == WindowMode::Fullscreen {
            self.main_mode = WindowMode::Standard;
        }
        self.main_window_state()
    }

    /// Opens the settings window and returns `(WindowState, ActivationPolicy)`.
    pub fn open_settings(&mut self) -> (WindowState, ActivationPolicy) {
        self.settings_visible = true;
        (self.settings_window_state(), self.activation_policy())
    }

    /// Closes/hides the settings window and returns `(WindowState, ActivationPolicy)`.
    pub fn close_settings(&mut self) -> (WindowState, ActivationPolicy) {
        self.settings_visible = false;
        (self.settings_window_state(), self.activation_policy())
    }

    /// Shows Quick Bar without altering `activation_policy()`.
    pub fn show_quick_bar(&mut self, mode: Option<QuickBarMode>) -> WindowState {
        if let Some(m) = mode {
            self.quick_bar_mode = m;
        }
        self.quick_bar_visible = true;
        WindowState {
            window_id: "quickbar".to_string(),
            visible: true,
            fullscreen: false,
            mode: None,
            reason: None,
        }
    }

    /// Hides Quick Bar with an explicit `HideReason` without altering `activation_policy()`.
    pub fn hide_quick_bar(&mut self, reason: HideReason) -> WindowState {
        self.quick_bar_visible = false;
        WindowState {
            window_id: "quickbar".to_string(),
            visible: false,
            fullscreen: false,
            mode: None,
            reason: Some(reason),
        }
    }

    pub fn main_window_state(&self) -> WindowState {
        WindowState {
            window_id: "main".to_string(),
            visible: self.main_visible,
            fullscreen: self.main_fullscreen,
            mode: Some(self.main_mode),
            reason: None,
        }
    }

    pub fn settings_window_state(&self) -> WindowState {
        WindowState {
            window_id: "settings".to_string(),
            visible: self.settings_visible,
            fullscreen: false,
            mode: Some(WindowMode::Standard),
            reason: None,
        }
    }
}
