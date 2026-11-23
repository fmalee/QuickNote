use std::fs::{self, File};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::backup::{select_backups_to_prune, BackupEntry, MAX_BACKUP_COUNT};
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::settings::{parse_settings_yaml, Settings};
use quicknote_lib::infra::fs::prune_backups;

fn make_temp_dir(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "quicknote_m0_02_{label}_{ts}_{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn invalid_settings_yaml_never_overwrites_disk_and_reports_location() {
    let app_data_dir = make_temp_dir("invalid_settings");
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/vault/settings-invalid.yaml");
    let fixture_bytes = fs::read(&fixture_path).expect("read settings-invalid.yaml fixture");

    let target_settings_path = app_data_dir.join("settings.yaml");
    fs::write(&target_settings_path, &fixture_bytes).expect("seed settings.yaml");

    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");
    let snap = pm.settings_get().expect("settings_get");

    // Disk file must remain byte-for-byte identical.
    let disk_bytes_after = fs::read(&target_settings_path).expect("re-read settings.yaml");
    assert_eq!(disk_bytes_after, fixture_bytes);

    assert!(!snap.status.valid);
    assert_eq!(snap.status.active_source, "default");
    assert!(snap.status.error_line.is_some());
    assert!(snap.status.error_column.is_some());
    assert!(snap.status.error_message.is_some());
    assert_eq!(snap.settings, Settings::default());

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn hot_reload_falls_back_to_last_valid_settings_on_unknown_field_or_syntax_error() {
    let app_data_dir = make_temp_dir("reload_fallback");
    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");

    // Write valid customized settings.yaml.
    let valid_yaml = "editor:\n  font_size: 18\n";
    fs::write(pm.settings_path(), valid_yaml).expect("write valid settings");
    let valid_snap = pm.reload_settings().expect("reload valid settings");
    assert!(valid_snap.status.valid);
    assert_eq!(valid_snap.status.active_source, "file");
    assert_eq!(valid_snap.settings.editor.font_size, 18);

    // Hand-edit with an unknown field (`deny_unknown_fields`).
    let unknown_field_yaml = "editor:\n  font_size: 20\n  nonexistent_key: 123\n";
    fs::write(pm.settings_path(), unknown_field_yaml).expect("write unknown field settings");
    let err_snap = pm.reload_settings().expect("reload invalid settings");

    // Must not overwrite disk file and must keep last_valid (font_size == 18).
    assert_eq!(
        fs::read_to_string(pm.settings_path()).expect("read disk"),
        unknown_field_yaml
    );
    assert!(!err_snap.status.valid);
    assert_eq!(err_snap.status.active_source, "last_valid");
    assert_eq!(err_snap.settings.editor.font_size, 18);
    assert!(err_snap.status.error_message.is_some());

    // Pure parser also reports line/col on type mismatch.
    let (_, type_err_status) =
        parse_settings_yaml("editor:\n  font_size: \"not-a-number\"\n", None);
    assert!(!type_err_status.valid);
    assert_eq!(type_err_status.error_line, Some(2));

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn ui_state_set_persists_whitelisted_keys_and_rejects_invalid_keys() {
    let app_data_dir = make_temp_dir("ui_state");
    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");

    let updated = pm
        .ui_state_set("selectedWorkspace", serde_json::json!("quicknote"))
        .expect("selectedWorkspace must succeed");
    assert_eq!(updated.selected_workspace_key.as_deref(), Some("quicknote"));

    let updated_modes = pm
        .ui_state_set("workspaceModes", serde_json::json!({ "quicknote": "todo" }))
        .expect("workspaceModes must succeed");
    assert_eq!(updated_modes.workspace_modes.len(), 1);

    // Reopen PersistenceManager from disk to confirm atomic persistence.
    drop(pm);
    let reopened =
        PersistenceManager::open(app_data_dir.clone()).expect("reopen PersistenceManager");
    let loaded = reopened.ui_state_get().expect("ui_state_get");
    assert_eq!(loaded.selected_workspace_key.as_deref(), Some("quicknote"));
    assert_eq!(loaded.workspace_modes.len(), 1);

    // Non-whitelisted key must return ValidationError.
    let invalid_key_err = reopened
        .ui_state_set("unknownKey", serde_json::json!(true))
        .expect_err("unknownKey must fail");
    assert_eq!(invalid_key_err.code, ErrorCode::ValidationError);

    // Wrong value type for whitelisted key must return ValidationError.
    let invalid_type_err = reopened
        .ui_state_set("windows", serde_json::json!("not-an-object"))
        .expect_err("invalid type must fail");
    assert_eq!(invalid_type_err.code, ErrorCode::ValidationError);

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn backup_rolling_prune_pure_and_filesystem() {
    let now_secs: i64 = 1_760_000_000;
    let day_secs: i64 = 24 * 60 * 60;

    // Pure function check: 55 entries spanning 40 days.
    let mut entries = Vec::new();
    for i in 0..55 {
        let age_days = if i < 5 {
            31 + i as i64
        } else {
            (i - 5) as i64 / 2
        };
        entries.push(BackupEntry {
            file_name: format!("note.{i:02}.bak"),
            modified_epoch_secs: now_secs - age_days * day_secs,
        });
    }
    let to_delete = select_backups_to_prune(&entries, now_secs);
    assert_eq!(to_delete.len(), 5);
    assert_eq!(entries.len() - to_delete.len(), MAX_BACKUP_COUNT);

    // Real filesystem check with 55 files spanning 40 days.
    let app_data_dir = make_temp_dir("backup_prune");
    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");
    let backups_dir = pm.backups_dir();
    assert!(backups_dir.is_dir());
    assert!(app_data_dir.join("recovery").is_dir());

    let real_now = SystemTime::now();
    let real_now_secs = real_now
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_secs() as i64;

    for i in 0..55u64 {
        let file_path = backups_dir.join(format!("sample.{i:02}.bak"));
        let file = File::create(&file_path).expect("create backup file");
        // First 3 files are >30 days old (35..37 days); remaining 52 are within 0..25 days.
        let age_days = if i < 3 { 35 + i } else { (i - 3) / 2 };
        let mtime = real_now - Duration::from_secs(age_days * 24 * 60 * 60 + i);
        file.set_modified(mtime).expect("set modified time");
    }

    let deleted_count = prune_backups(&backups_dir, real_now_secs).expect("prune_backups");
    assert_eq!(deleted_count, 5);

    let remaining: Vec<_> = fs::read_dir(&backups_dir)
        .expect("read backups dir")
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(remaining.len(), MAX_BACKUP_COUNT);

    // Ensure the 3 >30-day files were definitely removed.
    for old_idx in 0..3 {
        assert!(!backups_dir
            .join(format!("sample.{old_idx:02}.bak"))
            .exists());
    }

    let _ = fs::remove_dir_all(&app_data_dir);
}
