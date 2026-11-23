use std::sync::Arc;

use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::hotkey::parse_hotkey_combo;
use crate::domain::ui_state::{HotkeySelfCheck, HotkeyStatus, UiStatePatchKey};
use crate::infra::macos::NativeWindowBridge;

const BACKUP_HOTKEY_COMBO: &str = "ctrl+alt+cmd+space";

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
    ///
    /// - Registration returning 0 sets `selfCheck = pending` until `on_hotkey_pressed` fires (`D-0004`).
    /// - Invalid configured hotkey string records `selfCheck = conflict` and attempts backup `ctrl+alt+cmd+space`.
    pub fn init_and_self_check(
        &self,
        on_hotkey: extern "C" fn(),
    ) -> Result<HotkeyStatus, AppError> {
        let snapshot = self.pm.settings_get()?;
        let combo_raw = snapshot.settings.hotkey.quick_bar;
        let backup_combo = parse_hotkey_combo(&snapshot.settings.hotkey.quick_bar_backup)
            .map(|p| p.normalized)
            .unwrap_or_else(|_| BACKUP_HOTKEY_COMBO.to_string());

        let status = match parse_hotkey_combo(&combo_raw) {
            Ok(parsed) => {
                let self_check = self.bridge.register_global_hotkey(&parsed, on_hotkey)?;
                HotkeyStatus {
                    combo: parsed.normalized,
                    backup_combo,
                    self_check,
                }
            }
            Err(err) => {
                eprintln!(
                    "[hotkey] invalid configured hotkey on startup; marking conflict and trying backup: code={:?}",
                    err.code
                );
                if let Ok(backup_parsed) = parse_hotkey_combo(&backup_combo)
                    .or_else(|_| parse_hotkey_combo(BACKUP_HOTKEY_COMBO))
                {
                    if let Err(reg_err) = self
                        .bridge
                        .register_global_hotkey(&backup_parsed, on_hotkey)
                    {
                        eprintln!(
                            "[hotkey] backup hotkey registration failed: code={:?}",
                            reg_err.code
                        );
                    }
                }
                let trimmed = combo_raw.trim();
                HotkeyStatus {
                    combo: if trimmed.is_empty() {
                        "invalid".to_string()
                    } else {
                        trimmed.to_string()
                    },
                    backup_combo,
                    self_check: HotkeySelfCheck::Conflict,
                }
            }
        };

        self.persist_hotkey_status(&status)?;
        Ok(status)
    }

    /// Validates, hot re-registers, and persists a new global hotkey combination (`C4.2`, `D-0004`, `D-0008`).
    pub fn update_hotkey(
        &self,
        combo: &str,
        on_hotkey: extern "C" fn(),
    ) -> Result<HotkeyStatus, AppError> {
        let parsed = parse_hotkey_combo(combo)?;
        let current_ui = self.pm.ui_state_get()?;
        let self_check = self.bridge.register_global_hotkey(&parsed, on_hotkey)?;

        if self_check != HotkeySelfCheck::Conflict {
            self.pm.update_hotkey_in_settings(&parsed.normalized)?;
        }

        let status = HotkeyStatus {
            combo: parsed.normalized,
            backup_combo: current_ui.hotkey.backup_combo,
            self_check,
        };
        self.persist_hotkey_status(&status)?;
        Ok(status)
    }

    /// Transitions `selfCheck` from `pending` to `ok` when `on_hotkey_pressed` actually fires (`D-0004`).
    pub fn mark_hotkey_triggered(&self) -> Result<bool, AppError> {
        let current_ui = self.pm.ui_state_get()?;
        if current_ui.hotkey.self_check != HotkeySelfCheck::Pending {
            return Ok(false);
        }

        let verified = HotkeyStatus {
            combo: current_ui.hotkey.combo,
            backup_combo: current_ui.hotkey.backup_combo,
            self_check: HotkeySelfCheck::Ok,
        };
        self.persist_hotkey_status(&verified)?;
        Ok(true)
    }

    fn persist_hotkey_status(&self, status: &HotkeyStatus) -> Result<(), AppError> {
        let val = serde_json::to_value(status).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize HotkeyStatus",
                err.to_string(),
            )
        })?;
        self.pm.ui_state_set(UiStatePatchKey::Hotkey, val)?;
        Ok(())
    }
}
