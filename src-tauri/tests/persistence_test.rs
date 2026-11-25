use std::fs::{self, File};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use quicknote_lib::application::library::{get_library_info, set_library_root};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::backup::{select_backups_to_prune, BackupEntry, MAX_BACKUP_COUNT};
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::settings::{parse_settings_yaml, SetRootArgs, Settings};
use quicknote_lib::domain::ui_state::{
    BackupPathsUiState, LeftSidebarUiState, LeftViewId, NavTab, TerminalPanelUiState, UiState,
    UiStateArgs, UiStatePatchKey,
};
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
        .ui_state_set(
            UiStatePatchKey::SelectedWorkspace,
            serde_json::json!("quicknote"),
        )
        .expect("selectedWorkspace must succeed");
    assert_eq!(updated.selected_workspace_key.as_deref(), Some("quicknote"));

    let updated_modes = pm
        .ui_state_set(
            UiStatePatchKey::WorkspaceModes,
            serde_json::json!({ "quicknote": "todo" }),
        )
        .expect("workspaceModes must succeed");
    assert_eq!(updated_modes.workspace_modes.len(), 1);

    // Reopen PersistenceManager from disk to confirm atomic persistence.
    drop(pm);
    let reopened =
        PersistenceManager::open(app_data_dir.clone()).expect("reopen PersistenceManager");
    let loaded = reopened.ui_state_get().expect("ui_state_get");
    assert_eq!(loaded.selected_workspace_key.as_deref(), Some("quicknote"));
    assert_eq!(loaded.workspace_modes.len(), 1);

    // Non-whitelisted key must be rejected by UiStatePatchKey deserialization.
    let invalid_key_deser = serde_json::from_value::<UiStateArgs>(serde_json::json!({
        "key": "unknownKey",
        "value": true
    }));
    assert!(invalid_key_deser.is_err());

    // Wrong value type for whitelisted key must return ValidationError.
    let invalid_type_err = reopened
        .ui_state_set(UiStatePatchKey::Windows, serde_json::json!("not-an-object"))
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

#[test]
fn corrupt_ui_state_json_backs_up_and_resets_to_default() {
    let app_data_dir = make_temp_dir("corrupt_ui_state");
    let corrupt_raw = "{ invalid json: [unclosed";
    fs::write(app_data_dir.join("ui-state.json"), corrupt_raw).expect("seed corrupt ui-state.json");

    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open must not fail");
    let state = pm.ui_state_get().expect("ui_state_get");
    assert_eq!(state, UiState::default());

    // ui-state.json on disk is repaired to valid default JSON.
    let disk_state: UiState = serde_json::from_str(
        &fs::read_to_string(pm.ui_state_path()).expect("read repaired ui-state.json"),
    )
    .expect("repaired ui-state.json must parse");
    assert_eq!(disk_state, UiState::default());

    // Corrupt original is preserved in backups/.
    let backup_files: Vec<PathBuf> = fs::read_dir(pm.backups_dir())
        .expect("read backups dir")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("ui-state.json.") && n.ends_with(".bak"))
        })
        .collect();
    assert_eq!(backup_files.len(), 1);
    assert_eq!(
        fs::read_to_string(&backup_files[0]).expect("read backup"),
        corrupt_raw
    );

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn current_library_root_respects_valid_settings_clear_and_falls_back_only_when_invalid() {
    let app_data_dir = make_temp_dir("library_root_truth");
    let vault_dir = app_data_dir.join("vault");
    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");

    let info = set_library_root(
        &pm,
        SetRootArgs {
            root: vault_dir.to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect("set_library_root");
    let expected_vault = fs::canonicalize(&vault_dir).unwrap_or_else(|_| vault_dir.clone());
    assert_eq!(
        info.root.as_deref(),
        Some(expected_vault.to_string_lossy().as_ref())
    );

    // User clears library_root in valid settings.yaml -> must return None (not stale SQLite value).
    fs::write(pm.settings_path(), "library_root: null\n").expect("clear library_root");
    let reloaded = pm.reload_settings().expect("reload_settings");
    assert!(reloaded.status.valid);
    assert_eq!(
        pm.current_library_root().expect("current_library_root"),
        None
    );
    assert_eq!(get_library_info(&pm).expect("get_library_info").root, None);

    // When settings.yaml is invalid on cold start, fall back to SQLite library_meta.
    drop(pm);
    fs::write(
        app_data_dir.join("settings.yaml"),
        "bad_unknown_field: 123\n",
    )
    .expect("write invalid settings");
    let reopened =
        PersistenceManager::open(app_data_dir.clone()).expect("reopen PersistenceManager");
    assert!(!reopened.settings_get().expect("settings_get").status.valid);
    assert_eq!(
        reopened
            .current_library_root()
            .expect("fallback current_library_root")
            .as_deref(),
        Some(expected_vault.to_string_lossy().as_ref())
    );

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn library_set_root_boundaries_and_broken_settings_protection() {
    let app_data_dir = make_temp_dir("library_set_root_bounds");
    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");

    // 1. Empty or whitespace root -> ValidationError.
    let empty_err = set_library_root(
        &pm,
        SetRootArgs {
            root: "   ".to_string(),
            create_if_missing: true,
        },
    )
    .expect_err("empty root must fail");
    assert_eq!(empty_err.code, ErrorCode::ValidationError);

    // 2. Missing directory with create_if_missing=false -> NotFound.
    let missing_dir = app_data_dir.join("does_not_exist");
    let not_found_err = set_library_root(
        &pm,
        SetRootArgs {
            root: missing_dir.to_string_lossy().to_string(),
            create_if_missing: false,
        },
    )
    .expect_err("missing dir without create must fail");
    assert_eq!(not_found_err.code, ErrorCode::NotFound);
    assert!(!missing_dir.exists());

    // 3. Regular file instead of directory -> ValidationError.
    let file_path = app_data_dir.join("not_a_dir.txt");
    fs::write(&file_path, "hello").expect("create regular file");
    let file_err = set_library_root(
        &pm,
        SetRootArgs {
            root: file_path.to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect_err("regular file path must fail");
    assert_eq!(file_err.code, ErrorCode::ValidationError);

    // 4. Broken settings.yaml on disk must NOT be overwritten when set_library_root succeeds.
    drop(pm);
    let broken_settings = "editor:\n  font_size: \"bad\"\n  unknown_key: true\n";
    fs::write(app_data_dir.join("settings.yaml"), broken_settings).expect("seed broken settings");

    let reopened =
        PersistenceManager::open(app_data_dir.clone()).expect("reopen PersistenceManager");
    let created_vault = app_data_dir.join("new_vault");
    let ok_info = set_library_root(
        &reopened,
        SetRootArgs {
            root: created_vault.to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect("set_library_root with create_if_missing=true");
    let expected_created =
        fs::canonicalize(&created_vault).unwrap_or_else(|_| created_vault.clone());
    assert_eq!(
        ok_info.root.as_deref(),
        Some(expected_created.to_string_lossy().as_ref())
    );
    assert!(created_vault.join("inbox.md").is_file());
    assert!(created_vault.join("assets").is_dir());
    assert!(created_vault.join("_templates").is_dir());

    // Corrupt settings.yaml remains untouched on disk.
    assert_eq!(
        fs::read_to_string(app_data_dir.join("settings.yaml")).expect("read settings.yaml"),
        broken_settings
    );

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn ui_state_backup_paths_roundtrip_and_legacy_migration() {
    let app_data_dir = make_temp_dir("backup_paths");
    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");

    let initial = pm.ui_state_get().expect("initial ui_state_get");
    assert_eq!(initial.backup_paths, BackupPathsUiState::default());

    pm.ui_state_set(
        UiStatePatchKey::BackupPaths,
        serde_json::json!({ "lastExportPath": "/tmp/a.yaml" }),
    )
    .expect("backupPaths patch must succeed");

    let updated = pm.ui_state_get().expect("updated ui_state_get");
    assert_eq!(
        updated.backup_paths.last_export_path.as_deref(),
        Some("/tmp/a.yaml")
    );
    assert_eq!(updated.backup_paths.last_import_path, None);

    // Legacy ui-state.json without `backupPaths` loads cleanly with default values.
    let legacy_config_dir = make_temp_dir("backup_paths_legacy_cfg");
    let legacy_cache_dir = make_temp_dir("backup_paths_legacy_cache");
    let legacy_json = serde_json::json!({
        "windows": {
            "main": { "x": 10, "y": 20, "width": 1200, "height": 800, "maximized": false },
            "quickBar": { "x": null, "y": null }
        },
        "tabs": {
            "items": [],
            "activeKey": null,
            "closedStack": []
        },
        "panelGroup": {
            "visible": true,
            "activeId": "outline",
            "width": 260,
            "collapsed": false
        },
        "filters": {
            "activeTab": "inbox",
            "selectedDir": null,
            "selectedTags": [],
            "listScrollTop": 0.0
        },
        "hotkey": {
            "effective": "Ctrl+Alt+Space",
            "registered": true,
            "fallbackUsed": false,
            "conflictReason": null,
            "lastCheckAt": "2026-01-01T00:00:00Z"
        }
    });
    fs::write(
        legacy_config_dir.join("ui-state.json"),
        serde_json::to_string_pretty(&legacy_json).expect("serialize legacy json"),
    )
    .expect("write legacy ui-state.json");

    let legacy_pm =
        PersistenceManager::open_with_dirs(legacy_config_dir.clone(), legacy_cache_dir.clone())
            .expect("open_with_dirs on legacy ui-state.json");
    let legacy_loaded = legacy_pm.ui_state_get().expect("legacy ui_state_get");
    assert_eq!(legacy_loaded.backup_paths, BackupPathsUiState::default());

    let _ = fs::remove_dir_all(&app_data_dir);
    let _ = fs::remove_dir_all(&legacy_config_dir);
    let _ = fs::remove_dir_all(&legacy_cache_dir);
}

#[test]
fn ui_state_left_sidebar_and_terminal_panel_roundtrip() {
    let app_data_dir = make_temp_dir("layout_state");
    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");

    let initial = pm.ui_state_get().expect("initial ui_state_get");
    assert_eq!(initial.left_sidebar, LeftSidebarUiState::default());
    assert_eq!(initial.terminal_panel, TerminalPanelUiState::default());

    pm.ui_state_set(
        UiStatePatchKey::LeftSidebar,
        serde_json::json!({ "width": 320, "activeView": "outline", "collapsed": false }),
    )
    .expect("leftSidebar patch must succeed");

    pm.ui_state_set(
        UiStatePatchKey::TerminalPanel,
        serde_json::json!({ "visible": true, "height": 300, "collapsed": false }),
    )
    .expect("terminalPanel patch must succeed");

    let updated = pm.ui_state_get().expect("updated ui_state_get");
    assert_eq!(
        updated.left_sidebar,
        LeftSidebarUiState {
            width: 320,
            active_view: LeftViewId::Outline,
            collapsed: false,
        }
    );
    assert_eq!(
        updated.terminal_panel,
        TerminalPanelUiState {
            visible: true,
            height: 300,
            collapsed: false,
        }
    );

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn legacy_ui_state_inbox_active_tab_loads_as_projects() {
    let app_data_dir = make_temp_dir("legacy_inbox_tab");
    let mut legacy_json = serde_json::to_value(UiState::default()).expect("serialize default");
    legacy_json["filters"]["activeTab"] = serde_json::json!("inbox");

    fs::write(
        app_data_dir.join("ui-state.json"),
        serde_json::to_string_pretty(&legacy_json).expect("serialize legacy json"),
    )
    .expect("write legacy ui-state.json");

    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");
    let loaded = pm.ui_state_get().expect("ui_state_get");
    assert_eq!(loaded.filters.active_tab, NavTab::Projects);

    let _ = fs::remove_dir_all(&app_data_dir);
}
