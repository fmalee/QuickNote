use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use quicknote_lib::application::library::{get_library_info, set_library_root};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::infra::sqlite::{
    delete_directory_workspace_config, get_library_meta, init_sqlite,
    list_ancestor_directory_workspace_configs, list_directory_workspace_configs,
    upsert_directory_workspace_config, upsert_note_index, DirectoryWorkspaceConfigRecord,
    NoteIndexRecord, MIGRATION_0001,
};
use rusqlite::{params, Connection};

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
            "directory_workspace_configs",
            "schema_migrations",
        ] {
            assert!(
                names.contains(expected),
                "missing expected SQLite table: {expected}"
            );
        }

        let mut mig_stmt = conn
            .prepare("SELECT id FROM schema_migrations ORDER BY id")
            .expect("prepare schema_migrations");
        let applied: Vec<String> = mig_stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query schema_migrations")
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(applied, vec!["0001_init", "0002_directory_workspace_cache"]);

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

    let dir_ws_exists: i64 = rebuilt
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'directory_workspace_configs'",
            [],
            |r| r.get(0),
        )
        .expect("check directory_workspace_configs");
    assert_eq!(dir_ws_exists, 1);

    // Running init_sqlite a second time on existing DB must also be idempotent.
    drop(rebuilt);
    let _reopened = init_sqlite(&db_path).expect("idempotent reopen");

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn sqlite_migration_0002_upgrades_0001_database_smoothly_and_preserves_data() {
    let app_data_dir = make_temp_dir("upgrade_0001_to_0002");
    let db_path = app_data_dir.join("index.sqlite");

    // Create a legacy 0001-only SQLite database with existing library_meta and notes rows.
    {
        let conn = Connection::open(&db_path).expect("open legacy db");
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE schema_migrations (
               id TEXT PRIMARY KEY,
               applied_at INTEGER NOT NULL
             );",
        )
        .expect("create schema_migrations");
        conn.execute_batch(MIGRATION_0001)
            .expect("apply 0001_init.sql");
        conn.execute(
            "INSERT INTO schema_migrations (id, applied_at) VALUES (?1, ?2)",
            params!["0001_init", 1_700_000_000_000_i64],
        )
        .expect("record 0001_init");
        conn.execute(
            "INSERT INTO library_meta (key, value) VALUES (?1, ?2)",
            params!["schema_probe", "preserved"],
        )
        .expect("insert library_meta");
    }

    // Running init_sqlite must upgrade smoothly to 0002 without losing existing rows.
    let conn = init_sqlite(&db_path).expect("upgrade to 0002");
    assert_eq!(
        get_library_meta(&conn, "schema_probe").expect("get_library_meta"),
        Some("preserved".to_string())
    );

    let mut mig_stmt = conn
        .prepare("SELECT id FROM schema_migrations ORDER BY id")
        .expect("prepare schema_migrations");
    let applied: Vec<String> = mig_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query schema_migrations")
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(applied, vec!["0001_init", "0002_directory_workspace_cache"]);

    // Verify corrupted DB triggers clean remove & rebuild with 0002 schema intact.
    drop(mig_stmt);
    drop(conn);
    fs::write(&db_path, b"not a valid sqlite database").expect("corrupt db file");

    let rebuilt = init_sqlite(&db_path).expect("rebuild corrupted db");
    assert_eq!(
        get_library_meta(&rebuilt, "schema_probe").expect("read after rebuild"),
        None
    );
    assert!(list_directory_workspace_configs(&rebuilt)
        .expect("list after rebuild")
        .is_empty());

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn notes_table_allows_multiple_docs_without_frontmatter_id_using_fallback_keys() {
    let app_data_dir = make_temp_dir("fallback_keys");
    let db_path = app_data_dir.join("index.sqlite");
    let conn = init_sqlite(&db_path).expect("init_sqlite");

    let doc_a = NoteIndexRecord {
        key: "path:aaaa1111".to_string(),
        id: None,
        rel_path: "inbox.md".to_string(),
        title: "Inbox".to_string(),
        doc_type: "note".to_string(),
        project: None,
        mtime_ms: 1000,
        size_bytes: 12,
        body_hash: "h1".to_string(),
        frontmatter_json: "{}".to_string(),
        excerpt: "first".to_string(),
        meta_error: None,
        pinned: false,
        archived: false,
        is_inbox: true,
        updated_at: 1000,
        tags: vec!["inbox".to_string()],
        title_tokens: "inbox".to_string(),
        tag_tokens: "inbox".to_string(),
        body_tokens: "first".to_string(),
        links: vec![],
        commands: vec![],
    };

    let doc_b = NoteIndexRecord {
        key: "path:bbbb2222".to_string(),
        id: None,
        rel_path: "projects/readme.md".to_string(),
        title: "Readme".to_string(),
        doc_type: "doc".to_string(),
        project: None,
        mtime_ms: 2000,
        size_bytes: 24,
        body_hash: "h2".to_string(),
        frontmatter_json: "{}".to_string(),
        excerpt: "second".to_string(),
        meta_error: None,
        pinned: false,
        archived: false,
        is_inbox: false,
        updated_at: 2000,
        tags: vec!["docs".to_string()],
        title_tokens: "readme".to_string(),
        tag_tokens: "docs".to_string(),
        body_tokens: "second".to_string(),
        links: vec![],
        commands: vec![],
    };

    upsert_note_index(&conn, &doc_a).expect("insert doc_a without frontmatter id");
    upsert_note_index(&conn, &doc_b).expect("insert doc_b without frontmatter id");

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM notes WHERE id IS NULL", [], |r| {
            r.get(0)
        })
        .expect("count null-id notes");
    assert_eq!(count, 2);

    let _ = fs::remove_dir_all(&app_data_dir);
}

#[test]
fn directory_workspace_configs_crud_and_ancestor_batch_query() {
    let app_data_dir = make_temp_dir("dir_ws_configs");
    let db_path = app_data_dir.join("index.sqlite");
    let conn = init_sqlite(&db_path).expect("init_sqlite");

    let root_cfg = DirectoryWorkspaceConfigRecord {
        vault_rel_dir: "".to_string(),
        config_json: r#"{"cwd":"/vault-root"}"#.to_string(),
        mtime_ms: 100,
        parse_error: None,
    };
    let proj_cfg = DirectoryWorkspaceConfigRecord {
        vault_rel_dir: "/projects/".to_string(),
        config_json: r#"{"cwd":"/work/projects"}"#.to_string(),
        mtime_ms: 200,
        parse_error: None,
    };
    let app_cfg = DirectoryWorkspaceConfigRecord {
        vault_rel_dir: "projects/app".to_string(),
        config_json: r#"{"cwd":"/work/projects/app"}"#.to_string(),
        mtime_ms: 300,
        parse_error: Some("invalid yaml line 2".to_string()),
    };
    let sibling_cfg = DirectoryWorkspaceConfigRecord {
        vault_rel_dir: "projects/other".to_string(),
        config_json: r#"{"cwd":"/work/projects/other"}"#.to_string(),
        mtime_ms: 400,
        parse_error: None,
    };

    upsert_directory_workspace_config(&conn, &root_cfg).expect("upsert root");
    upsert_directory_workspace_config(&conn, &proj_cfg).expect("upsert projects");
    upsert_directory_workspace_config(&conn, &app_cfg).expect("upsert projects/app");
    upsert_directory_workspace_config(&conn, &sibling_cfg).expect("upsert projects/other");

    // Upsert update on existing normalized directory.
    let updated_proj_cfg = DirectoryWorkspaceConfigRecord {
        vault_rel_dir: "projects".to_string(),
        config_json: r#"{"cwd":"/work/projects-v2"}"#.to_string(),
        mtime_ms: 250,
        parse_error: None,
    };
    upsert_directory_workspace_config(&conn, &updated_proj_cfg).expect("update projects");

    let all = list_directory_workspace_configs(&conn).expect("list all");
    assert_eq!(all.len(), 4);
    assert_eq!(all[0].vault_rel_dir, "");
    assert_eq!(all[1].vault_rel_dir, "projects");
    assert_eq!(all[1].config_json, r#"{"cwd":"/work/projects-v2"}"#);
    assert_eq!(all[1].mtime_ms, 250);
    assert_eq!(all[2].vault_rel_dir, "projects/app");
    assert_eq!(all[2].parse_error.as_deref(), Some("invalid yaml line 2"));
    assert_eq!(all[3].vault_rel_dir, "projects/other");

    // Batch-query ancestors for deep child path: nearest ancestor first, excluding siblings.
    let ancestors = list_ancestor_directory_workspace_configs(&conn, "projects/app/docs/sub")
        .expect("list ancestors");
    let ancestor_dirs: Vec<&str> = ancestors.iter().map(|r| r.vault_rel_dir.as_str()).collect();
    assert_eq!(ancestor_dirs, vec!["projects/app", "projects", ""]);

    // Query for root directory only returns root config.
    let root_ancestors =
        list_ancestor_directory_workspace_configs(&conn, ".").expect("list root ancestors");
    assert_eq!(root_ancestors.len(), 1);
    assert_eq!(root_ancestors[0].vault_rel_dir, "");

    // Delete a directory config and verify it disappears from list and ancestor queries.
    delete_directory_workspace_config(&conn, "projects/app/").expect("delete projects/app");
    let ancestors_after_delete =
        list_ancestor_directory_workspace_configs(&conn, "projects/app/docs/sub")
            .expect("list ancestors after delete");
    let dirs_after_delete: Vec<&str> = ancestors_after_delete
        .iter()
        .map(|r| r.vault_rel_dir.as_str())
        .collect();
    assert_eq!(dirs_after_delete, vec!["projects", ""]);

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

    let expected_root = fs::canonicalize(&library_dir)
        .unwrap_or_else(|_| library_dir.clone())
        .to_string_lossy()
        .to_string();
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
