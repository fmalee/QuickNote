use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::commands::notes::export_pdf_with_bridge;
use quicknote_lib::domain::app::ActivationPolicy;
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::note::ExportPdfArgs;
use quicknote_lib::domain::settings::{
    parse_settings_yaml, MainWindowSettingsPatch, SettingsPatch, WindowMode,
};
use quicknote_lib::domain::window::{
    clamp_auxiliary_width_percent, compute_auxiliary_frame, WindowPolicyState,
    DEFAULT_AUXILIARY_WIDTH_PERCENT,
};
use quicknote_lib::infra::macos::NoopNativeBridge;

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("qn-m4-01b-{name}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn settings_export_produces_lossless_yaml() {
    let dir = temp_dir("export");
    let pm = PersistenceManager::open(dir.clone()).expect("open pm");

    let updated = pm
        .settings_update(SettingsPatch {
            main_window: Some(MainWindowSettingsPatch {
                mode: Some(WindowMode::Auxiliary),
                auxiliary_width_percent: Some(52),
            }),
            ..Default::default()
        })
        .expect("update settings");

    let export_path = dir.join("nested/exported/settings-backup.yaml");
    pm.settings_export(&export_path.to_string_lossy())
        .expect("export settings");

    let raw = fs::read_to_string(&export_path).expect("read exported yaml");
    let (parsed, status) = parse_settings_yaml(&raw, None);
    assert!(status.valid);
    assert_eq!(parsed, updated.settings);

    let empty_err = pm.settings_export("   ").expect_err("empty path fails");
    assert_eq!(empty_err.code, ErrorCode::ValidationError);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn settings_import_valid_creates_backup_and_applies_settings() {
    let dir = temp_dir("import-valid");
    let pm = PersistenceManager::open(dir.clone()).expect("open pm");

    let mut import_src_pm_dir = temp_dir("import-src");
    let src_pm = PersistenceManager::open(import_src_pm_dir.clone()).expect("open src pm");
    src_pm
        .settings_update(SettingsPatch {
            main_window: Some(MainWindowSettingsPatch {
                mode: Some(WindowMode::Auxiliary),
                auxiliary_width_percent: Some(40),
            }),
            ..Default::default()
        })
        .expect("update src settings");
    let src_file = import_src_pm_dir.join("import-source.yaml");
    src_pm
        .settings_export(&src_file.to_string_lossy())
        .expect("export src yaml");

    let imported = pm
        .settings_import(&src_file.to_string_lossy())
        .expect("import valid settings");
    assert!(imported.status.valid);
    assert_eq!(imported.status.active_source, "file");
    assert_eq!(imported.settings.main_window.mode, WindowMode::Auxiliary);
    assert_eq!(imported.settings.main_window.auxiliary_width_percent, 40);

    let backups_dir = pm.backups_dir();
    let backup_entries: Vec<_> = fs::read_dir(&backups_dir)
        .expect("read backups dir")
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("settings.yaml.")
        })
        .collect();
    assert!(
        !backup_entries.is_empty(),
        "expected backup of previous settings.yaml in backups/"
    );

    let current = pm.settings_get().expect("get settings");
    assert_eq!(current.settings, imported.settings);

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&mut import_src_pm_dir);
}

#[test]
fn settings_import_invalid_yaml_preserves_existing_file_and_memory() {
    let dir = temp_dir("import-invalid");
    let pm = PersistenceManager::open(dir.clone()).expect("open pm");

    pm.settings_update(SettingsPatch {
        main_window: Some(MainWindowSettingsPatch {
            auxiliary_width_percent: Some(50),
            ..Default::default()
        }),
        ..Default::default()
    })
    .expect("set initial valid settings");

    let before_disk = fs::read_to_string(pm.settings_path()).expect("read settings.yaml");
    let before_mem = pm.settings_get().expect("get memory settings");

    let bad_syntax_path = dir.join("bad-syntax.yaml");
    fs::write(&bad_syntax_path, "version: 1\nhotkey: [\n").expect("write bad syntax yaml");
    let syntax_err = pm
        .settings_import(&bad_syntax_path.to_string_lossy())
        .expect_err("bad syntax should fail");
    assert_eq!(syntax_err.code, ErrorCode::SettingsParseError);
    assert!(syntax_err.detail.is_some());

    let unknown_field_path = dir.join("unknown-field.yaml");
    let mut with_unknown = before_disk.clone();
    with_unknown.push_str("\nunknownField: true\n");
    fs::write(&unknown_field_path, &with_unknown).expect("write unknown field yaml");
    let unknown_err = pm
        .settings_import(&unknown_field_path.to_string_lossy())
        .expect_err("unknown field should fail");
    assert_eq!(unknown_err.code, ErrorCode::SettingsParseError);
    let detail = unknown_err.detail.unwrap_or_default();
    assert!(
        detail.contains("unknownField"),
        "expected unknownField in detail, got: {detail}"
    );

    let after_disk = fs::read_to_string(pm.settings_path()).expect("re-read settings.yaml");
    let after_mem = pm.settings_get().expect("re-get memory settings");
    assert_eq!(after_disk, before_disk);
    assert_eq!(after_mem, before_mem);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn note_export_pdf_validates_and_writes_stub_via_noop_bridge() {
    let dir = temp_dir("pdf-export");
    let bridge = NoopNativeBridge;

    let empty_err = export_pdf_with_bridge(
        &bridge,
        ExportPdfArgs {
            html_document: "   ".to_string(),
            dest_path: dir.join("out.pdf").to_string_lossy().to_string(),
        },
    )
    .expect_err("empty html should fail");
    assert_eq!(empty_err.code, ErrorCode::ValidationError);

    let target_without_ext = dir.join("sub/dir/report");
    let written_path = export_pdf_with_bridge(
        &bridge,
        ExportPdfArgs {
            html_document: "<html><body><h1>Export</h1></body></html>".to_string(),
            dest_path: target_without_ext.to_string_lossy().to_string(),
        },
    )
    .expect("pdf export should succeed");

    assert_eq!(written_path, dir.join("sub/dir/report.pdf"));
    let bytes = fs::read(&written_path).expect("read exported pdf");
    assert!(bytes.starts_with(b"%PDF-"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn auxiliary_width_clamping_frame_calculation_and_window_policy() {
    assert_eq!(DEFAULT_AUXILIARY_WIDTH_PERCENT, 45);
    assert_eq!(clamp_auxiliary_width_percent(10), 30);
    assert_eq!(clamp_auxiliary_width_percent(30), 30);
    assert_eq!(clamp_auxiliary_width_percent(45), 45);
    assert_eq!(clamp_auxiliary_width_percent(60), 60);
    assert_eq!(clamp_auxiliary_width_percent(95), 60);

    // Standard 1440x900 screen at 45% -> width 648, right-docked at x = 792
    let (x, y, w, h) = compute_auxiliary_frame(0.0, 25.0, 1440.0, 875.0, 45);
    assert_eq!((x, y, w, h), (792.0, 25.0, 648.0, 875.0));

    // Narrow screen (1000px) at 30% -> 300px would be < 480px, protected to 480px
    let (nx, ny, nw, nh) = compute_auxiliary_frame(100.0, 0.0, 1000.0, 800.0, 20);
    assert_eq!((nx, ny, nw, nh), (620.0, 0.0, 480.0, 800.0));

    // Ultra-narrow screen (< 480px) clamped to screen width
    let (ux, _, uw, _) = compute_auxiliary_frame(0.0, 0.0, 400.0, 700.0, 45);
    assert_eq!((ux, uw), (0.0, 400.0));

    // WindowPolicyState auxiliary mode & activation policy
    let mut policy = WindowPolicyState::default();
    let (aux_state, activation) = policy.show_main(Some(WindowMode::Auxiliary));
    assert_eq!(activation, ActivationPolicy::Regular);
    assert!(aux_state.visible);
    assert!(!aux_state.fullscreen);
    assert_eq!(aux_state.mode, Some(WindowMode::Auxiliary));
}

#[test]
fn window_policy_with_initial_auxiliary_and_none_show_main_preserves_mode() {
    let mut policy = WindowPolicyState::with_initial(true, WindowMode::Auxiliary);
    assert_eq!(policy.activation_policy(), ActivationPolicy::Regular);
    assert_eq!(policy.main_mode(), WindowMode::Auxiliary);
    assert_eq!(policy.main_window_state().mode, Some(WindowMode::Auxiliary));
    assert!(!policy.main_window_state().fullscreen);

    let (state_after_none, activation) = policy.show_main(None);
    assert_eq!(activation, ActivationPolicy::Regular);
    assert_eq!(state_after_none.mode, Some(WindowMode::Auxiliary));
    assert!(state_after_none.visible);

    let fs_policy = WindowPolicyState::with_initial(false, WindowMode::Fullscreen);
    assert_eq!(fs_policy.activation_policy(), ActivationPolicy::Accessory);
    assert!(fs_policy.main_window_state().fullscreen);
    assert_eq!(fs_policy.main_mode(), WindowMode::Fullscreen);
}

#[test]
fn settings_import_syncs_ui_state_hotkey_backup_combo() {
    use quicknote_lib::application::hotkey::HotkeyManager;
    use quicknote_lib::commands::settings::sync_hotkeys_after_import;
    use std::sync::Arc;

    let dir = temp_dir("import-hotkey-sync");
    let pm = Arc::new(PersistenceManager::open(dir.clone()).expect("open persistence"));
    let hm = Arc::new(HotkeyManager::new(
        Arc::new(NoopNativeBridge),
        Arc::clone(&pm),
    ));

    let prev = pm.settings_get().expect("prev settings").settings.hotkey;
    let initial_yaml = fs::read_to_string(pm.settings_path()).expect("read settings.yaml");
    let updated_yaml = initial_yaml.replace("ctrl+alt+cmd+space", "ctrl+shift+space");
    let import_path = dir.join("imported.yaml");
    fs::write(&import_path, updated_yaml).expect("write imported yaml");

    let snapshot = pm
        .settings_import(&import_path.to_string_lossy())
        .expect("import yaml");
    sync_hotkeys_after_import(
        &pm,
        &hm,
        Some(&prev),
        &snapshot,
        quicknote_lib::on_hotkey_pressed,
    )
    .expect("sync hotkeys");

    let ui_state = pm.ui_state_get().expect("ui_state_get");
    assert_eq!(ui_state.hotkey.backup_combo, "ctrl+shift+space");

    let _ = fs::remove_dir_all(&dir);
}
