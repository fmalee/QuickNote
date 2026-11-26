use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use quicknote_lib::application::hotkey::HotkeyManager;
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::app::ActivationPolicy;
use quicknote_lib::domain::error::{AppError, ErrorCode};
use quicknote_lib::domain::hotkey::{parse_hotkey_combo, ParsedHotkey};
use quicknote_lib::domain::settings::WindowMode;
use quicknote_lib::domain::ui_state::HotkeySelfCheck;
use quicknote_lib::domain::window::{HideReason, QuickBarMode, WindowPolicyState, WindowState};
use quicknote_lib::infra::macos::{NativeWindowBridge, NoopNativeBridge, PanelDiagnostics};

extern "C" fn dummy_hotkey_cb() {}

#[test]
fn dynamic_activation_policy_full_state_machine() {
    let mut state = WindowPolicyState::default();

    // Initial state: both main and settings hidden -> .accessory
    assert_eq!(state.activation_policy(), ActivationPolicy::Accessory);
    assert!(!state.is_main_visible());
    assert!(!state.is_settings_visible());
    assert!(!state.is_quick_bar_visible());

    // Quick Bar alone never changes .accessory policy
    let qb_show = state.show_quick_bar(Some(QuickBarMode::Capture));
    assert_eq!(qb_show.window_id, "quickbar");
    assert!(qb_show.visible);
    assert_eq!(state.activation_policy(), ActivationPolicy::Accessory);

    let qb_hide = state.hide_quick_bar(HideReason::Esc);
    assert!(!qb_hide.visible);
    assert_eq!(qb_hide.reason, Some(HideReason::Esc));
    assert_eq!(state.activation_policy(), ActivationPolicy::Accessory);

    // Open main window -> .regular
    let (main_open, policy) = state.show_main(Some(WindowMode::Standard));
    assert!(main_open.visible);
    assert_eq!(main_open.mode, Some(WindowMode::Standard));
    assert_eq!(policy, ActivationPolicy::Regular);

    // Open settings while main is open -> still .regular
    let (settings_open, policy) = state.open_settings();
    assert!(settings_open.visible);
    assert_eq!(policy, ActivationPolicy::Regular);

    // Quick Bar open/close while .regular -> remains .regular
    state.show_quick_bar(Some(QuickBarMode::Open));
    assert_eq!(state.activation_policy(), ActivationPolicy::Regular);
    state.hide_quick_bar(HideReason::SpaceChanged);
    assert_eq!(state.activation_policy(), ActivationPolicy::Regular);

    // Hide main while settings is still open -> remains .regular
    let (main_closed, policy) = state.hide_main();
    assert!(!main_closed.visible);
    assert_eq!(policy, ActivationPolicy::Regular);

    // Close settings -> back to .accessory
    let (settings_closed, policy) = state.close_settings();
    assert!(!settings_closed.visible);
    assert_eq!(policy, ActivationPolicy::Accessory);
}

#[test]
fn hide_reason_and_window_state_json_serialization() {
    assert_eq!(
        serde_json::to_string(&HideReason::Esc).ok().as_deref(),
        Some("\"esc\"")
    );
    assert_eq!(
        serde_json::to_string(&HideReason::Hotkey).ok().as_deref(),
        Some("\"hotkey\"")
    );
    assert_eq!(
        serde_json::to_string(&HideReason::SpaceChanged)
            .ok()
            .as_deref(),
        Some("\"space_changed\"")
    );
    assert_eq!(
        serde_json::to_string(&HideReason::FocusLost)
            .ok()
            .as_deref(),
        Some("\"focus_lost\"")
    );

    let mut state = WindowPolicyState::default();
    state.show_quick_bar(Some(QuickBarMode::Capture));
    let qb_focus_lost = state.hide_quick_bar(HideReason::FocusLost);
    assert!(!qb_focus_lost.visible);
    assert_eq!(qb_focus_lost.reason, Some(HideReason::FocusLost));

    let ws = WindowState {
        window_id: "quickbar".to_string(),
        visible: false,
        fullscreen: false,
        mode: Some(WindowMode::Auxiliary),
        reason: Some(HideReason::SpaceChanged),
    };
    let json = serde_json::to_value(&ws).ok();
    assert_eq!(
        json,
        Some(serde_json::json!({
            "windowId": "quickbar",
            "visible": false,
            "fullscreen": false,
            "mode": "auxiliary",
            "reason": "space_changed"
        }))
    );
}

#[test]
fn hotkey_combo_parsing_and_validation() {
    let default_hk = parse_hotkey_combo("ctrl+alt+space").ok();
    assert!(default_hk.is_some());
    if let Some(hk) = default_hk {
        assert_eq!(hk.normalized, "ctrl+alt+space");
        assert!(hk.ctrl && hk.alt && !hk.shift && !hk.cmd);
        assert_eq!(hk.macos_key_code, 49);
    }

    let backup_hk = parse_hotkey_combo("Cmd + Option + Control + Space").ok();
    assert!(backup_hk.is_some());
    if let Some(hk) = backup_hk {
        assert_eq!(hk.normalized, "ctrl+alt+cmd+space");
        assert!(hk.ctrl && hk.alt && !hk.shift && hk.cmd);
    }

    for invalid in [
        "",
        "   ",
        "space",
        "ctrl+alt",
        "ctrl++space",
        "ctrl+ctrl+space",
        "ctrl+alt+a+b",
        "ctrl+alt+unknownkey",
    ] {
        let err = parse_hotkey_combo(invalid).err();
        assert!(err.is_some(), "expected ValidationError for {invalid:?}");
        if let Some(e) = err {
            assert_eq!(e.code, ErrorCode::ValidationError);
        }
    }
}

fn make_temp_dir(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "quicknote-m003-{label}-{ts}-{}",
        std::process::id()
    ))
}

#[derive(Default)]
struct RecordingHotkeyBridge {
    registered_combos: Mutex<Vec<String>>,
}

impl NativeWindowBridge for RecordingHotkeyBridge {
    fn configure_quick_bar_panel(&self, _ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }
    fn show_quick_bar_panel(&self, _ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }
    fn hide_quick_bar_panel(&self, _ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }
    fn configure_toast_panel(&self, _ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }
    fn show_toast_panel(&self, _ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }
    fn hide_toast_panel(&self, _ptr: usize) -> Result<PanelDiagnostics, AppError> {
        Ok(PanelDiagnostics::default())
    }
    fn register_space_observer(&self, _on_space_changed: extern "C" fn()) -> Result<(), AppError> {
        Ok(())
    }
    fn register_quick_bar_esc_handler(&self, _on_esc: extern "C" fn()) {}
    fn register_quick_bar_resign_handler(&self, _on_resign: extern "C" fn()) {}
    fn register_global_hotkey(
        &self,
        hotkey: &ParsedHotkey,
        _on_hotkey: extern "C" fn(),
    ) -> Result<HotkeySelfCheck, AppError> {
        if let Ok(mut guard) = self.registered_combos.lock() {
            guard.push(hotkey.normalized.clone());
        }
        Ok(HotkeySelfCheck::Pending)
    }
}

#[test]
fn headless_hotkey_manager_self_check_and_update() {
    let temp_dir = make_temp_dir("self_check");
    let pm = Arc::new(PersistenceManager::open(temp_dir.clone()).expect("open PersistenceManager"));
    let bridge = Arc::new(NoopNativeBridge);
    let hm = HotkeyManager::new(bridge, Arc::clone(&pm));

    let status = hm
        .init_and_self_check(dummy_hotkey_cb)
        .expect("init_and_self_check");
    assert_eq!(status.combo, "ctrl+alt+space");
    assert_eq!(status.self_check, HotkeySelfCheck::Pending);

    // Simulating real Carbon hotkey press promotes Pending -> Ok.
    assert!(hm.mark_hotkey_triggered().expect("mark_hotkey_triggered"));
    assert!(!hm.mark_hotkey_triggered().expect("idempotent second press"));
    let verified_ui = pm.ui_state_get().expect("ui_state_get");
    assert_eq!(verified_ui.hotkey.self_check, HotkeySelfCheck::Ok);

    let updated = hm
        .update_hotkey("ctrl+alt+cmd+space", dummy_hotkey_cb)
        .expect("update_hotkey");
    assert_eq!(updated.combo, "ctrl+alt+cmd+space");
    assert_eq!(updated.self_check, HotkeySelfCheck::Pending);

    let ui = pm.ui_state_get().expect("ui_state_get");
    assert_eq!(ui.hotkey.combo, "ctrl+alt+cmd+space");
    assert_eq!(ui.hotkey.self_check, HotkeySelfCheck::Pending);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn hotkey_update_persists_across_persistence_reopen_and_reinit() {
    let temp_dir = make_temp_dir("persist_reopen");
    {
        let pm =
            Arc::new(PersistenceManager::open(temp_dir.clone()).expect("open PersistenceManager"));
        let bridge = Arc::new(RecordingHotkeyBridge::default());
        let hm = HotkeyManager::new(bridge, Arc::clone(&pm));

        let init_status = hm
            .init_and_self_check(dummy_hotkey_cb)
            .expect("initial init_and_self_check");
        assert_eq!(init_status.combo, "ctrl+alt+space");
        assert_eq!(init_status.self_check, HotkeySelfCheck::Pending);

        let updated = hm
            .update_hotkey("ctrl+alt+cmd+space", dummy_hotkey_cb)
            .expect("update_hotkey");
        assert_eq!(updated.combo, "ctrl+alt+cmd+space");
        assert_eq!(updated.self_check, HotkeySelfCheck::Pending);

        let snap = pm.settings_get().expect("settings_get");
        assert_eq!(snap.settings.hotkey.quick_bar, "ctrl+alt+cmd+space");
    }

    // Reopen PersistenceManager and reinitialize HotkeyManager as on app restart.
    let reopened_pm =
        Arc::new(PersistenceManager::open(temp_dir.clone()).expect("reopen PersistenceManager"));
    let reopened_bridge = Arc::new(RecordingHotkeyBridge::default());
    let reopened_hm = HotkeyManager::new(reopened_bridge.clone(), Arc::clone(&reopened_pm));

    let reinit_status = reopened_hm
        .init_and_self_check(dummy_hotkey_cb)
        .expect("reinit init_and_self_check");
    assert_eq!(reinit_status.combo, "ctrl+alt+cmd+space");
    assert_eq!(reinit_status.self_check, HotkeySelfCheck::Pending);

    let recorded = reopened_bridge
        .registered_combos
        .lock()
        .expect("lock")
        .clone();
    assert_eq!(recorded, vec!["ctrl+alt+cmd+space".to_string()]);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn hotkey_update_preserves_corrupt_settings_yaml_and_startup_falls_back_on_invalid_hotkey() {
    let temp_dir = make_temp_dir("corrupt_and_invalid");
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    // Case 1: Invalid hotkey string in valid settings.yaml -> conflict + backup registration.
    let invalid_hotkey_yaml = "hotkey:\n  quick_bar: invalid+combo+key\n";
    fs::write(temp_dir.join("settings.yaml"), invalid_hotkey_yaml).expect("seed settings.yaml");

    let pm = Arc::new(PersistenceManager::open(temp_dir.clone()).expect("open PersistenceManager"));
    let bridge = Arc::new(RecordingHotkeyBridge::default());
    let hm = HotkeyManager::new(bridge.clone(), Arc::clone(&pm));

    let status = hm
        .init_and_self_check(dummy_hotkey_cb)
        .expect("init_and_self_check");
    assert_eq!(status.combo, "invalid+combo+key");
    assert_eq!(status.self_check, HotkeySelfCheck::Conflict);
    let recorded = bridge.registered_combos.lock().expect("lock").clone();
    assert_eq!(recorded, vec!["ctrl+alt+cmd+space".to_string()]);

    // Pressing fallback hotkey must NOT falsely flip Conflict to Ok.
    assert!(!hm.mark_hotkey_triggered().expect("mark_hotkey_triggered"));
    assert_eq!(
        pm.ui_state_get().expect("ui_state_get").hotkey.self_check,
        HotkeySelfCheck::Conflict
    );

    drop(hm);
    drop(pm);

    // Case 2: Corrupt settings.yaml on disk -> update_hotkey must NOT overwrite disk file (D-0008).
    let broken_yaml = "editor:\n  unknown_key: [unclosed\n";
    fs::write(temp_dir.join("settings.yaml"), broken_yaml).expect("seed broken settings.yaml");

    let pm2 =
        Arc::new(PersistenceManager::open(temp_dir.clone()).expect("reopen PersistenceManager"));
    let hm2 = HotkeyManager::new(Arc::new(NoopNativeBridge), Arc::clone(&pm2));
    let updated = hm2
        .update_hotkey("ctrl+alt+cmd+space", dummy_hotkey_cb)
        .expect("update_hotkey with broken settings");
    assert_eq!(updated.combo, "ctrl+alt+cmd+space");
    assert_eq!(
        fs::read_to_string(temp_dir.join("settings.yaml")).expect("read settings.yaml"),
        broken_yaml
    );
    assert_eq!(
        pm2.settings_get()
            .expect("settings_get")
            .settings
            .hotkey
            .quick_bar,
        "ctrl+alt+cmd+space"
    );
    assert_eq!(
        pm2.ui_state_get().expect("ui_state_get").hotkey.combo,
        "ctrl+alt+cmd+space"
    );

    let _ = fs::remove_dir_all(temp_dir);
}
