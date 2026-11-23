use crate::domain::app::ActivationPolicy;
use crate::domain::error::AppError;
use crate::domain::hotkey::ParsedHotkey;
use crate::domain::ui_state::HotkeySelfCheck;
use crate::domain::window::HideReason;

/// Diagnostics captured from native panel operations (`C2.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelDiagnostics {
    pub collection_behavior: u64,
    pub is_key_window: bool,
}

impl Default for PanelDiagnostics {
    fn default() -> Self {
        Self {
            collection_behavior: 256,
            is_key_window: false,
        }
    }
}

/// Emits structured JSON diagnostic log to stderr for native bridge events (`C2.4`).
/// Never logs note content, clipboard, or user input (`C2.3`).
pub fn log_native_event(
    event: &str,
    activation_policy: ActivationPolicy,
    collection_behavior: u64,
    is_key_window: bool,
    hide_reason: Option<HideReason>,
) {
    let policy_str = match activation_policy {
        ActivationPolicy::Accessory => "accessory",
        ActivationPolicy::Regular => "regular",
    };
    let reason_val = hide_reason.map(|r| match r {
        HideReason::Esc => "esc",
        HideReason::Hotkey => "hotkey",
        HideReason::SpaceChanged => "space_changed",
    });
    let payload = serde_json::json!({
        "event": event,
        "activationPolicy": policy_str,
        "collectionBehavior": collection_behavior,
        "isKeyWindow": is_key_window,
        "hideReason": reason_val,
    });
    eprintln!("[macos-bridge] {payload}");
}

/// Abstraction over macOS AppKit/Carbon bridge so headless tests run safely without GUI (`M0-03` §3.5).
pub trait NativeWindowBridge: Send + Sync {
    fn configure_quick_bar_panel(&self, ns_window_ptr: usize)
        -> Result<PanelDiagnostics, AppError>;
    fn show_quick_bar_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError>;
    fn hide_quick_bar_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError>;
    fn configure_toast_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError>;
    fn show_toast_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError>;
    fn hide_toast_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError>;
    fn register_space_observer(&self, on_space_changed: extern "C" fn()) -> Result<(), AppError>;
    fn register_quick_bar_esc_handler(&self, on_esc: extern "C" fn());
    fn register_global_hotkey(
        &self,
        hotkey: &ParsedHotkey,
        on_hotkey: extern "C" fn(),
    ) -> Result<HotkeySelfCheck, AppError>;
}

/// Headless / non-macOS no-op bridge for automated tests.
#[derive(Debug, Default)]
pub struct NoopNativeBridge;

impl NativeWindowBridge for NoopNativeBridge {
    fn configure_quick_bar_panel(
        &self,
        _ns_window_ptr: usize,
    ) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }

    fn show_quick_bar_panel(&self, _ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics {
            collection_behavior: 256,
            is_key_window: true,
        })
    }

    fn hide_quick_bar_panel(&self, _ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }

    fn configure_toast_panel(&self, _ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }

    fn show_toast_panel(&self, _ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }

    fn hide_toast_panel(&self, _ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }

    fn register_space_observer(&self, _on_space_changed: extern "C" fn()) -> Result<(), AppError> {
        Ok(())
    }

    fn register_quick_bar_esc_handler(&self, _on_esc: extern "C" fn()) {}

    fn register_global_hotkey(
        &self,
        _hotkey: &ParsedHotkey,
        _on_hotkey: extern "C" fn(),
    ) -> Result<HotkeySelfCheck, AppError> {
        Ok(HotkeySelfCheck::Ok)
    }
}

#[cfg(target_os = "macos")]
pub mod darwin {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{NativeWindowBridge, PanelDiagnostics};
    use crate::domain::error::AppError;
    use crate::domain::hotkey::ParsedHotkey;
    use crate::domain::ui_state::HotkeySelfCheck;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    struct QnPanelDiag {
        collection_behavior: u64,
        is_key_window: bool,
    }

    impl From<QnPanelDiag> for PanelDiagnostics {
        fn from(value: QnPanelDiag) -> Self {
            Self {
                collection_behavior: value.collection_behavior,
                is_key_window: value.is_key_window,
            }
        }
    }

    extern "C" {
        fn qn_configure_quick_bar_panel(
            ns_window_ptr: *mut c_void,
            on_esc: Option<extern "C" fn()>,
            out_diag: *mut QnPanelDiag,
        );
        fn qn_show_quick_bar_panel(ns_window_ptr: *mut c_void, out_diag: *mut QnPanelDiag);
        fn qn_hide_quick_bar_panel(ns_window_ptr: *mut c_void, out_diag: *mut QnPanelDiag);
        fn qn_configure_toast_panel(ns_window_ptr: *mut c_void, out_diag: *mut QnPanelDiag);
        fn qn_show_toast_panel(ns_window_ptr: *mut c_void, out_diag: *mut QnPanelDiag);
        fn qn_hide_toast_panel(ns_window_ptr: *mut c_void, out_diag: *mut QnPanelDiag);
        fn qn_register_space_observer(on_space_changed: Option<extern "C" fn()>);
        fn qn_register_carbon_hotkey(
            key_code: u32,
            modifiers: u32,
            on_hotkey: Option<extern "C" fn()>,
        ) -> i32;
    }

    static ESC_CALLBACK_PTR: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn dispatch_esc() {
        let raw = ESC_CALLBACK_PTR.load(Ordering::SeqCst);
        if raw != 0 {
            let cb: extern "C" fn() = unsafe { std::mem::transmute(raw) };
            cb();
        }
    }

    /// Real macOS AppKit / Carbon native bridge (`D-0011`).
    #[derive(Debug, Default)]
    pub struct MacosNativeBridge;

    impl NativeWindowBridge for MacosNativeBridge {
        fn configure_quick_bar_panel(
            &self,
            ns_window_ptr: usize,
        ) -> Result<PanelDiagnostics, AppError> {
            let mut diag = QnPanelDiag::default();
            unsafe {
                qn_configure_quick_bar_panel(
                    ns_window_ptr as *mut c_void,
                    Some(dispatch_esc),
                    &mut diag,
                );
            }
            Ok(diag.into())
        }

        fn show_quick_bar_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
            let mut diag = QnPanelDiag::default();
            unsafe {
                qn_show_quick_bar_panel(ns_window_ptr as *mut c_void, &mut diag);
            }
            Ok(diag.into())
        }

        fn hide_quick_bar_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
            let mut diag = QnPanelDiag::default();
            unsafe {
                qn_hide_quick_bar_panel(ns_window_ptr as *mut c_void, &mut diag);
            }
            Ok(diag.into())
        }

        fn configure_toast_panel(
            &self,
            ns_window_ptr: usize,
        ) -> Result<PanelDiagnostics, AppError> {
            let mut diag = QnPanelDiag::default();
            unsafe {
                qn_configure_toast_panel(ns_window_ptr as *mut c_void, &mut diag);
            }
            Ok(diag.into())
        }

        fn show_toast_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
            let mut diag = QnPanelDiag::default();
            unsafe {
                qn_show_toast_panel(ns_window_ptr as *mut c_void, &mut diag);
            }
            Ok(diag.into())
        }

        fn hide_toast_panel(&self, ns_window_ptr: usize) -> Result<PanelDiagnostics, AppError> {
            let mut diag = QnPanelDiag::default();
            unsafe {
                qn_hide_toast_panel(ns_window_ptr as *mut c_void, &mut diag);
            }
            Ok(diag.into())
        }

        fn register_space_observer(
            &self,
            on_space_changed: extern "C" fn(),
        ) -> Result<(), AppError> {
            unsafe {
                qn_register_space_observer(Some(on_space_changed));
            }
            Ok(())
        }

        fn register_quick_bar_esc_handler(&self, on_esc: extern "C" fn()) {
            ESC_CALLBACK_PTR.store(on_esc as usize, Ordering::SeqCst);
        }

        fn register_global_hotkey(
            &self,
            hotkey: &ParsedHotkey,
            on_hotkey: extern "C" fn(),
        ) -> Result<HotkeySelfCheck, AppError> {
            let status = unsafe {
                qn_register_carbon_hotkey(
                    hotkey.macos_key_code,
                    hotkey.macos_modifiers,
                    Some(on_hotkey),
                )
            };
            if status == 0 {
                Ok(HotkeySelfCheck::Ok)
            } else {
                Ok(HotkeySelfCheck::Conflict)
            }
        }
    }
}
