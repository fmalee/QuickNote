use std::sync::Arc;

use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::hotkey::parse_hotkey_combo;
use crate::domain::ui_state::HotkeyStatus;
use crate::infra::macos::NativeWindowBridge;

/// Coordinates Carbon global hotkey registration, startup self-check, and hot re-registration (`D-0004`).
pub struct HotkeyManager {
    bridge: Arc<dyn NativeWindowBridge>,
    pm: Arc<PersistenceManager>,
}

impl HotkeyManager {
    pub fn new(bridge: Arc<dyn NativeWindowBridge>, pm: Arc<PersistenceManager>) -> Self {
        Self { bridge, pm }
    }

    /// Registers the configured hotkey on startup and persists self-check status to `ui-state.json`.
    pub fn init_and_self_check(
        &self,
        on_hotkey: extern "C" fn(),
    ) -> Result<HotkeyStatus, AppError> {
        let snapshot = self.pm.settings_get()?;
        let combo_raw = snapshot.settings.hotkey.quick_bar;
        let backup_combo = snapshot.settings.hotkey.quick_bar_backup;

        let parsed = parse_hotkey_combo(&combo_raw)?;
        let self_check = self.bridge.register_global_hotkey(&parsed, on_hotkey)?;

        let status = HotkeyStatus {
            combo: parsed.normalized,
            backup_combo,
            self_check,
        };
        self.persist_hotkey_status(&status)?;
        Ok(status)
    }

    /// Validates, hot re-registers, and persists a new global hotkey combination (`C4.2`, `D-0004`).
    pub fn update_hotkey(
        &self,
        combo: &str,
        on_hotkey: extern "C" fn(),
    ) -> Result<HotkeyStatus, AppError> {
        let parsed = parse_hotkey_combo(combo)?;
        let current_ui = self.pm.ui_state_get()?;
        let self_check = self.bridge.register_global_hotkey(&parsed, on_hotkey)?;

        let status = HotkeyStatus {
            combo: parsed.normalized,
            backup_combo: current_ui.hotkey.backup_combo,
            self_check,
        };
        self.persist_hotkey_status(&status)?;
        Ok(status)
    }

    fn persist_hotkey_status(&self, status: &HotkeyStatus) -> Result<(), AppError> {
        let val = serde_json::to_value(status).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize HotkeyStatus",
                err.to_string(),
            )
        })?;
        self.pm.ui_state_set("hotkey", val)?;
        Ok(())
    }
}
