use std::fs;
use std::path::{Path, PathBuf};

use quicknote_lib::application::index::{index_get_status, index_rebuild};
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{
    note_create, note_delete, note_read, note_rename, note_update_meta, note_write,
    notes_get_tree_and_tags, notes_list, search_notes_and_files,
};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::application::reconciler::{
    reconcile_path, reconcile_vault, reconcile_watched_path, write_directory_workspace_config,
    ReconcilePathOutcome,
};
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::note::{
    CreateNoteArgs, IndexPhase, NoteMetaPatch, NoteUpdateMetaArgs, NotesFilterArgs, ReadNoteArgs,
    RelPathArgs, RenameNoteArgs, SearchQueryArgs, WriteNoteArgs,
};
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::domain::workspace_context::DirectoryWorkspaceConfig;
use quicknote_lib::infra::sqlite::{
    get_note_key_by_rel_path, list_directory_workspace_configs,
};

static TEST_DIR_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Result<Self, std::io::Error> {
        let seq = TEST_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("quicknote_test_{}_{}", std::process::id(), seq));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create_dir_all dst");
    for entry in fs::read_dir(src).expect("read_dir src") {
        let entry = entry.expect("dir entry");
        let ty = entry.file_type().expect("file_type");
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path);
        } else {
            fs::copy(entry.path(), &dst_path).expect("copy file");
        }
    }
}

fn setup_test_env() -> (TempDir, PathBuf, PersistenceManager) {
    let tmp = TempDir::new().expect("create temp dir");
    let app_data = tmp.path().join("app_data");
    let vault_dir = tmp.path().join("vault");

    let fixture_vault = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("vault");
    copy_dir_recursive(&fixture_vault, &vault_dir);

    let pm = PersistenceManager::open(app_data).expect("open PersistenceManager");
    let info = set_library_root(
        &pm,
        SetRootArgs {
            root: vault_dir.to_string_lossy().to_string(),
            create_if_missing: false,
        },
    )
    .expect("set_library_root");

    (tmp, PathBuf::from(info.root.expect("library root")), pm)
}

#[test]
fn test_index_tree_tags_and_notes_list() {
    let (_tmp, _vault, pm) = setup_test_env();

    let status = index_get_status(&pm).expect("index_get_status");
    assert_eq!(status.phase, IndexPhase::Idle);
    assert!(status.notes_count > 0);
    assert!(status.last_completed_at.is_some());

    let rebuilt = index_rebuild(&pm, None).expect("index_rebuild");
    assert_eq!(rebuilt.notes_count, status.notes_count);

    let nav = notes_get_tree_and_tags(&pm).expect("notes_get_tree_and_tags");
    assert!(nav.inbox_count > 0, "inbox.md should have list items");
    assert!(!nav.dirs.is_empty(), "directory tree should not be empty");
    assert!(!nav.tags.is_empty(), "tag tree should not be empty");

    let dev_notes = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "tags".to_string(),
            tags: vec!["dev".to_string()],
            ..Default::default()
        },
    )
    .expect("notes_list by tag dev");
    assert!(
        !dev_notes.is_empty(),
        "parent tag 'dev' should match 'dev' and 'dev/*' notes"
    );
}

#[test]
fn test_duplicate_ulid_marked_without_mutating_disk() {
    let (_tmp, vault, pm) = setup_test_env();

    let dup_a_path = vault.join("duplicate-id-a.md");
    let dup_b_path = vault.join("duplicate-id-b.md");
    let before_a = fs::read_to_string(&dup_a_path).expect("read dup_a");
    let before_b = fs::read_to_string(&dup_b_path).expect("read dup_b");

    let all_notes = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "recent".to_string(),
            ..Default::default()
        },
    )
    .expect("notes_list recent");

    let a_row = all_notes
        .iter()
        .find(|n| n.rel_path.as_deref() == Some("duplicate-id-a.md"))
        .expect("duplicate-id-a indexed");
    let b_row = all_notes
        .iter()
        .find(|n| n.rel_path.as_deref() == Some("duplicate-id-b.md"))
        .expect("duplicate-id-b indexed");

    assert!(a_row.meta_error.is_some(), "dup A should have meta_error");
    assert!(b_row.meta_error.is_some(), "dup B should have meta_error");
    assert_ne!(a_row.key, b_row.key, "dup B key should fallback to path:");

    let after_a = fs::read_to_string(&dup_a_path).expect("re-read dup_a");
    let after_b = fs::read_to_string(&dup_b_path).expect("re-read dup_b");
    assert_eq!(before_a, after_a);
    assert_eq!(before_b, after_b);
}

#[test]
fn test_note_write_backfills_missing_ulid_and_detects_conflict() {
    let (_tmp, vault, pm) = setup_test_env();
    let missing_path = vault.join("missing-id.md").to_string_lossy().to_string();

    let doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: missing_path.clone(),
        },
    )
    .expect("read missing-id.md");
    assert!(doc.meta.id.is_none());

    let updated_body = format!("{}\nAppended line.\n", doc.body.trim_end());
    let write_res = note_write(
        &pm,
        WriteNoteArgs {
            abs_path: missing_path.clone(),
            body: updated_body,
            base_body_hash: doc.body_hash.clone(),
        },
    )
    .expect("note_write should succeed");

    let meta = write_res
        .updated_meta
        .expect("missing-id.md should back-fill ULID");
    assert!(meta.id.is_some());

    // Writing again with stale base_body_hash must return Conflict.
    let conflict_err = note_write(
        &pm,
        WriteNoteArgs {
            abs_path: missing_path,
            body: "stale write".to_string(),
            base_body_hash: doc.body_hash,
        },
    )
    .expect_err("stale write must conflict");
    assert_eq!(conflict_err.code, ErrorCode::Conflict);
}

#[test]
fn test_note_update_meta_preserves_comments_and_backups_broken_yaml() {
    let (_tmp, vault, pm) = setup_test_env();

    let unknown_path = vault
        .join("unknown-keys-comments.md")
        .to_string_lossy()
        .to_string();

    note_update_meta(
        &pm,
        NoteUpdateMetaArgs {
            abs_path: unknown_path.clone(),
            patch: NoteMetaPatch {
                pinned: Some(true),
                ..Default::default()
            },
        },
    )
    .expect("update unknown-keys-comments.md");

    let raw_unknown = fs::read_to_string(&unknown_path).expect("read unknown-keys-comments.md");
    assert!(
        raw_unknown.contains('#'),
        "YAML comments should be preserved"
    );

    let broken_path = vault.join("broken-yaml.md").to_string_lossy().to_string();

    let fixed_doc = note_update_meta(
        &pm,
        NoteUpdateMetaArgs {
            abs_path: broken_path,
            patch: NoteMetaPatch {
                title: Some("Fixed Broken YAML".to_string()),
                ..Default::default()
            },
        },
    )
    .expect("update broken-yaml.md");
    assert_eq!(fixed_doc.meta.title, "Fixed Broken YAML");

    let backups: Vec<_> = fs::read_dir(pm.backups_dir())
        .expect("read backups dir")
        .filter_map(Result::ok)
        .collect();
    assert!(
        backups
            .iter()
            .any(|e| e.file_name().to_string_lossy().ends_with(".bak")),
        "broken-yaml.md should create a .bak backup"
    );
}

#[test]
fn test_crud_and_fts_search() {
    let (_tmp, _vault, pm) = setup_test_env();

    let created = note_create(
        &pm,
        CreateNoteArgs {
            title: "Git 工作流笔记".to_string(),
            dir_rel_path: "notes".to_string(),
            template_name: None,
            body: Some("使用 git rebase 整理提交历史，而不是 digit。\n".to_string()),
        },
    )
    .expect("note_create");
    let created_rel = created.rel_path.clone().expect("rel_path");

    // Search "git" -> should hit "Git 工作流笔记", and NOT confuse with "digit" if only "digit" existed.
    let res_git = search_notes_and_files(
        &pm,
        SearchQueryArgs {
            query: "git".to_string(),
            limit: Some(10),
            request_id: 42,
        },
    )
    .expect("search git");
    assert_eq!(res_git.request_id, 42);
    assert!(res_git.items.iter().any(|i| i.title == "Git 工作流笔记"));

    // Search Chinese phrase
    let res_zh = search_notes_and_files(
        &pm,
        SearchQueryArgs {
            query: "工作流".to_string(),
            limit: Some(10),
            request_id: 99,
        },
    )
    .expect("search 工作流");
    assert_eq!(res_zh.request_id, 99);
    assert!(res_zh.items.iter().any(|i| i.title == "Git 工作流笔记"));

    // Rename
    let renamed = note_rename(
        &pm,
        RenameNoteArgs {
            rel_path: created_rel,
            new_rel_path: "notes/git-workflow-renamed.md".to_string(),
        },
    )
    .expect("note_rename");
    assert_eq!(
        renamed.rel_path.as_deref(),
        Some("notes/git-workflow-renamed.md")
    );

    // Delete
    note_delete(
        &pm,
        RelPathArgs {
            rel_path: "notes/git-workflow-renamed.md".to_string(),
        },
    )
    .expect("note_delete");

    let deleted_read = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: renamed.abs_path,
        },
    );
    assert!(deleted_read.is_err());
}

#[test]
fn test_reconcile_vault_indexes_root_manual_md_without_frontmatter_and_handles_broken_yaml() {
    let (_tmp, vault, pm) = setup_test_env();

    // 1. Place a plain Markdown file with no Frontmatter directly at Vault root.
    let manual_path = vault.join("manual.md");
    let manual_raw = "# 根目录手工文档\n\n无需 Frontmatter 即可收录的关键词：蓝鲸灯塔。\n";
    fs::write(&manual_path, manual_raw).expect("write manual.md");

    // 2. Place a valid .quicknote.yaml, a malformed .quicknote.yaml, a note after it, and an excluded .git file.
    fs::create_dir_all(vault.join("proj-broken")).expect("create proj-broken");
    fs::write(
        vault.join("proj-broken").join(".quicknote.yaml"),
        "projectRoot: [unterminated\n  bad: :::yaml",
    )
    .expect("write broken .quicknote.yaml");
    fs::write(
        vault.join("proj-broken").join("after-broken.md"),
        "# 损坏配置之后的笔记\n\n确认扫描未中断：海豚港湾。\n",
    )
    .expect("write after-broken.md");

    fs::create_dir_all(vault.join("proj-ok")).expect("create proj-ok");
    fs::write(
        vault.join("proj-ok").join(".quicknote.yaml"),
        "projectRoot: /tmp/proj-ok\ndefaultTerminalCwd: /tmp/proj-ok/src\n",
    )
    .expect("write valid .quicknote.yaml");

    fs::create_dir_all(vault.join(".git")).expect("create .git");
    fs::write(vault.join(".git").join("ignored.md"), "# 不应被收录\n").expect("write .git file");

    // 3. Run full incremental reconciliation.
    let report = pm
        .with_db(|conn| reconcile_vault(&vault, conn))
        .expect("reconcile_vault");
    assert!(report.notes_upserted >= 2);
    assert_eq!(report.configs_scanned, 2);
    assert_eq!(report.configs_upserted, 2);
    assert_eq!(report.config_parse_errors, 1);

    // 4. Read-only guarantee: manual.md must remain byte-for-byte untouched on disk.
    assert_eq!(
        fs::read_to_string(&manual_path).expect("read manual.md"),
        manual_raw
    );

    // 5. manual.md must be indexed with deterministic rel_path fallback key and searchable via FTS.
    let manual_key = pm
        .with_db(|conn| get_note_key_by_rel_path(conn, "manual.md"))
        .expect("query manual.md key");
    assert_eq!(manual_key.as_deref(), Some("path:manual.md"));

    let manual_search = search_notes_and_files(
        &pm,
        SearchQueryArgs {
            query: "蓝鲸灯塔".to_string(),
            limit: Some(10),
            request_id: 201,
        },
    )
    .expect("search 蓝鲸灯塔");
    assert!(manual_search
        .items
        .iter()
        .any(|item| item.rel_path.as_deref() == Some("manual.md")));

    // 6. Malformed YAML must not interrupt scanning subsequent notes.
    let after_broken_search = search_notes_and_files(
        &pm,
        SearchQueryArgs {
            query: "海豚港湾".to_string(),
            limit: Some(10),
            request_id: 202,
        },
    )
    .expect("search 海豚港湾");
    assert!(after_broken_search
        .items
        .iter()
        .any(|item| item.rel_path.as_deref() == Some("proj-broken/after-broken.md")));

    // 7. .quicknote.yaml and .git/ignored.md must never appear in notes list or FTS.
    let all_notes = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "all".to_string(),
            ..Default::default()
        },
    )
    .expect("notes_list all");
    assert!(!all_notes
        .iter()
        .any(|n| n.rel_path.as_deref().unwrap_or("").ends_with(".quicknote.yaml")));
    assert!(!all_notes
        .iter()
        .any(|n| n.rel_path.as_deref() == Some(".git/ignored.md")));

    // 8. Verify directory_workspace_configs rows.
    let configs = pm
        .with_db(list_directory_workspace_configs)
        .expect("list_directory_workspace_configs");
    let broken_cfg = configs
        .iter()
        .find(|c| c.vault_rel_dir == "proj-broken")
        .expect("proj-broken config row");
    assert!(broken_cfg.parse_error.is_some());
    assert_eq!(broken_cfg.config_json, "{}");

    let ok_cfg = configs
        .iter()
        .find(|c| c.vault_rel_dir == "proj-ok")
        .expect("proj-ok config row");
    assert!(ok_cfg.parse_error.is_none());
    assert!(ok_cfg.config_json.contains("/tmp/proj-ok"));

    // 9. Second reconciliation with unchanged files must skip all unchanged notes and configs.
    let second_report = pm
        .with_db(|conn| reconcile_vault(&vault, conn))
        .expect("second reconcile_vault");
    assert_eq!(second_report.notes_upserted, 0);
    assert_eq!(second_report.configs_upserted, 0);
    assert_eq!(second_report.notes_pruned, 0);
    assert_eq!(second_report.configs_pruned, 0);

    // 10. Deleting files and reconciling prunes them from SQLite.
    fs::remove_file(&manual_path).expect("remove manual.md");
    let prune_note = pm
        .with_db(|conn| reconcile_path(&vault, "manual.md", conn))
        .expect("reconcile_path deleted manual.md");
    assert_eq!(prune_note, ReconcilePathOutcome::NotePruned);
    assert_eq!(
        pm.with_db(|conn| get_note_key_by_rel_path(conn, "manual.md"))
            .expect("query deleted manual.md"),
        None
    );

    fs::remove_file(vault.join("proj-broken").join(".quicknote.yaml"))
        .expect("remove broken .quicknote.yaml");
    let prune_cfg = pm
        .with_db(|conn| reconcile_path(&vault, "proj-broken/.quicknote.yaml", conn))
        .expect("reconcile_path deleted .quicknote.yaml");
    assert_eq!(prune_cfg, ReconcilePathOutcome::ConfigPruned);
    let configs_after = pm
        .with_db(list_directory_workspace_configs)
        .expect("list configs after prune");
    assert!(!configs_after.iter().any(|c| c.vault_rel_dir == "proj-broken"));
}

#[test]
fn test_write_directory_workspace_config_uses_atomic_write_and_self_write_guard() {
    let (_tmp, vault, pm) = setup_test_env();
    let cfg = DirectoryWorkspaceConfig {
        vscode_workspace_path: Some("/tmp/demo.code-workspace".to_string()),
        project_root: Some("/tmp/demo".to_string()),
        default_terminal_cwd: Some("/tmp/demo/app".to_string()),
        external_app_overrides: None,
    };

    let written_path = write_directory_workspace_config(&pm, &vault, "workspace-dir", &cfg)
        .expect("write_directory_workspace_config");
    assert!(written_path.is_file());

    let cached = pm
        .with_db(list_directory_workspace_configs)
        .expect("list configs");
    let row = cached
        .iter()
        .find(|c| c.vault_rel_dir == "workspace-dir")
        .expect("cached workspace-dir row");
    assert!(row.parse_error.is_none());
    assert!(row.config_json.contains("/tmp/demo.code-workspace"));

    // Immediate watcher callback for the just-written .quicknote.yaml must be suppressed by SelfWriteGuard.
    let outcome = reconcile_watched_path(&pm, &vault, &written_path)
        .expect("reconcile_watched_path self-write");
    assert_eq!(outcome, ReconcilePathOutcome::SuppressedSelfWrite);
}
