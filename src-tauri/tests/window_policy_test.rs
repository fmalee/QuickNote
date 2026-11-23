use std::sync::Arc;

use quicknote_lib::application::hotkey::HotkeyManager;
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::app::ActivationPolicy;
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::hotkey::parse_hotkey_combo;
use quicknote_lib::domain::settings::WindowMode;
use quicknote_lib::domain::ui_state::HotkeySelfCheck;
use quicknote_lib::domain::window::{HideReason, QuickBarMode, WindowPolicyState, WindowState};
use quicknote_lib::infra::macos::NoopNativeBridge;

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

#[test]
fn headless_hotkey_manager_self_check_and_update() {
    let temp_dir = std::env::temp_dir().join(format!(
        "quicknote-m003-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let pm = Arc::new(PersistenceManager::open(temp_dir.clone()).unwrap_or_else(|e| panic!("{e}")));
    let bridge = Arc::new(NoopNativeBridge);
    let hm = HotkeyManager::new(bridge, Arc::clone(&pm));

    let status = hm
        .init_and_self_check(dummy_hotkey_cb)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(status.combo, "ctrl+alt+space");
    assert_eq!(status.self_check, HotkeySelfCheck::Ok);

    let updated = hm
        .update_hotkey("ctrl+alt+cmd+space", dummy_hotkey_cb)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(updated.combo, "ctrl+alt+cmd+space");
    assert_eq!(updated.self_check, HotkeySelfCheck::Ok);

    let ui = pm.ui_state_get().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ui.hotkey.combo, "ctrl+alt+cmd+space");
    assert_eq!(ui.hotkey.self_check, HotkeySelfCheck::Ok);

    let _ = std::fs::remove_dir_all(temp_dir);
}
