use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use quicknote_lib::application::library::{get_library_info, set_library_root};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::infra::sqlite::init_sqlite;

fn make_temp_dir(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "quicknote_sqlite_{label}_{ts}_{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn sqlite_migration_creates_all_tables_and_supports_clean_rebuild() {
    let app_data_dir = make_temp_dir("migration");
    let db_path = app_data_dir.join("index.sqlite");

    {
        let conn = init_sqlite(&db_path).expect("init_sqlite");
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name")
            .expect("prepare sqlite_master");
        let names: BTreeSet<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query_map")
            .filter_map(|r| r.ok())
            .collect();

        for expected in [
            "library_meta",
            "notes",
            "note_tags",
            "note_links",
            "commands",
            "notes_fts",
            "schema_migrations",
        ] {
            assert!(
                names.contains(expected),
                "missing expected SQLite table: {expected}"
            );
        }

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .expect("query journal_mode");
        assert_eq!(journal_mode.to_lowercase(), "wal");

        let fk_enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("query foreign_keys");
        assert_eq!(fk_enabled, 1);
    }

    // Delete index.sqlite and re-run migration idempotently.
    fs::remove_file(&db_path).expect("delete index.sqlite");
    let _ = fs::remove_file(app_data_dir.join("index.sqlite-wal"));
    let _ = fs::remove_file(app_data_dir.join("index.sqlite-shm"));

    let rebuilt = init_sqlite(&db_path).expect("rebuild init_sqlite");
    let fts_exists: i64 = rebuilt
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'notes_fts'",
            [],
            |r| r.get(0),
        )
        .expect("check notes_fts");
    assert_eq!(fts_exists, 1);

    // Running init_sqlite a second time on existing DB must also be idempotent.
    drop(rebuilt);
    let _reopened = init_sqlite(&db_path).expect("idempotent reopen");

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn library_set_root_initializes_skeleton_and_updates_app_info() {
    let base_dir = make_temp_dir("library_root");
    let app_data_dir = base_dir.join("app_data");
    let library_dir = base_dir.join("vault");

    let pm = PersistenceManager::open(app_data_dir.clone()).expect("open PersistenceManager");

    // Initially unconfigured: root must be None.
    let initial_info = get_library_info(&pm).expect("get_library_info");
    assert_eq!(initial_info.root, None);
    assert_eq!(initial_info.note_count, 0);

    let initial_app = pm.app_info("0.1.0", "macos").expect("app_info");
    assert_eq!(initial_app.library_root, None);

    // Setting missing root with create_if_missing=false must fail with NotFound.
    let missing_err = set_library_root(
        &pm,
        SetRootArgs {
            root: library_dir.to_string_lossy().to_string(),
            create_if_missing: false,
        },
    )
    .expect_err("missing dir without createIfMissing must fail");
    assert_eq!(missing_err.code, ErrorCode::NotFound);

    // Setting root with create_if_missing=true creates skeleton and updates app_info.
    let configured = set_library_root(
        &pm,
        SetRootArgs {
            root: library_dir.to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect("set_library_root must succeed");

    let expected_root = library_dir.to_string_lossy().to_string();
    assert_eq!(configured.root.as_deref(), Some(expected_root.as_str()));
    assert!(library_dir.join("inbox.md").is_file());
    assert!(library_dir.join("assets").is_dir());
    assert!(library_dir.join("_templates/note.md").is_file());
    assert!(library_dir.join("_templates/doc.md").is_file());
    assert!(library_dir.join("_templates/command-book.md").is_file());

    let updated_app = pm.app_info("0.1.0", "macos").expect("app_info");
    assert_eq!(
        updated_app.library_root.as_deref(),
        Some(expected_root.as_str())
    );

    // Reopen PersistenceManager to verify settings.yaml + SQLite persistence.
    drop(pm);
    let reopened = PersistenceManager::open(app_data_dir.clone()).expect("reopen");
    let persisted_info = get_library_info(&reopened).expect("persisted library_info");
    assert_eq!(persisted_info.root.as_deref(), Some(expected_root.as_str()));

    let _ = fs::remove_dir_all(&base_dir);
}
