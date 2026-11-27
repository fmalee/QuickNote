use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use quicknote_lib::application::external::directory_workspace_config_delete_uc_with;
use quicknote_lib::application::index::{index_get_status, index_rebuild};
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{
    note_create, note_delete_with, note_read, note_rename, note_update_meta, note_write,
    notes_backfill_missing_ids, notes_get_tree_and_tags, notes_list, search_notes_and_files,
};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::application::reconciler::{
    reconcile_path, reconcile_vault, reconcile_watched_path, write_directory_workspace_config,
    ReconcilePathOutcome,
};
use quicknote_lib::domain::error::{AppError, ErrorCode};
use quicknote_lib::domain::note::{
    CreateNoteArgs, IndexPhase, NoteMetaPatch, NoteUpdateMetaArgs, NotesFilterArgs, ReadNoteArgs,
    RelPathArgs, RenameNoteArgs, SearchQueryArgs, WriteNoteArgs,
};
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::domain::workspace_context::DirectoryWorkspaceConfig;
use quicknote_lib::infra::sqlite::{get_note_key_by_rel_path, list_directory_workspace_configs};
use quicknote_lib::infra::trash::FileRemover;

#[derive(Default)]
struct FakeRemover {
    removed: Mutex<Vec<PathBuf>>,
}

impl FakeRemover {
    fn removed_paths(&self) -> Vec<PathBuf> {
        self.removed.lock().expect("lock removed").clone()
    }
}

impl FileRemover for FakeRemover {
    fn remove(&self, path: &Path) -> Result<(), AppError> {
        self.removed
            .lock()
            .expect("lock removed")
            .push(path.to_path_buf());
        fs::remove_file(path).map_err(|e| {
            AppError::with_detail(ErrorCode::InternalError, "fake remove", e.to_string())
        })
    }
}

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
                pinned: Some(true),
                ..Default::default()
            },
        },
    )
    .expect("update broken-yaml.md");
    assert!(fixed_doc.meta.pinned);

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
    let fake_remover = FakeRemover::default();
    note_delete_with(
        &pm,
        RelPathArgs {
            rel_path: "notes/git-workflow-renamed.md".to_string(),
        },
        &fake_remover,
    )
    .expect("note_delete_with");
    assert_eq!(
        fake_remover.removed_paths(),
        vec![PathBuf::from(&renamed.abs_path)]
    );

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
    assert!(!all_notes.iter().any(|n| n
        .rel_path
        .as_deref()
        .unwrap_or("")
        .ends_with(".quicknote.yaml")));
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
    assert!(!configs_after
        .iter()
        .any(|c| c.vault_rel_dir == "proj-broken"));
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

#[test]
fn test_update_meta_tags_visible_immediately_in_list_and_tree() {
    let (_tmp, vault, pm) = setup_test_env();
    let path = vault
        .join("unknown-keys-comments.md")
        .to_string_lossy()
        .to_string();

    note_update_meta(
        &pm,
        NoteUpdateMetaArgs {
            abs_path: path.clone(),
            patch: NoteMetaPatch {
                tags: Some(vec!["zz-a".to_string(), "zz-b".to_string()]),
                ..Default::default()
            },
        },
    )
    .expect("update tags");

    let listed = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "tags".to_string(),
            tags: vec!["zz-a".to_string()],
            ..Default::default()
        },
    )
    .expect("notes_list by new tag");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].tags, vec!["zz-a".to_string(), "zz-b".to_string()]);

    let nav = notes_get_tree_and_tags(&pm).expect("tree and tags");
    let flat = format!("{:?}", nav.tags);
    assert!(flat.contains("zz-a") && flat.contains("zz-b"), "{flat}");

    // 随后的正文自动保存不得把标签回退
    let doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: path.clone(),
        },
    )
    .expect("read");
    note_write(
        &pm,
        WriteNoteArgs {
            abs_path: path,
            body: format!("{}\nmore\n", doc.body.trim_end()),
            base_body_hash: doc.body_hash,
        },
    )
    .expect("write after meta");
    let listed = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "tags".to_string(),
            tags: vec!["zz-b".to_string()],
            ..Default::default()
        },
    )
    .expect("notes_list after write");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].tags, vec!["zz-a".to_string(), "zz-b".to_string()]);
}

#[test]
fn test_directory_workspace_config_delete_uses_file_remover() {
    let (_tmp, vault, pm) = setup_test_env();
    fs::create_dir_all(vault.join("proj")).expect("create proj dir");

    let cfg = DirectoryWorkspaceConfig {
        vscode_workspace_path: None,
        project_root: Some("/tmp/proj".to_string()),
        default_terminal_cwd: None,
        external_app_overrides: None,
    };
    let config_path =
        write_directory_workspace_config(&pm, &vault, "proj", &cfg).expect("write .quicknote.yaml");
    assert!(config_path.is_file());

    let fake_remover = FakeRemover::default();
    directory_workspace_config_delete_uc_with(&pm, "proj", &fake_remover)
        .expect("delete directory workspace config");

    assert_eq!(fake_remover.removed_paths(), vec![config_path.clone()]);
    assert!(!config_path.exists());
    let rows = pm
        .with_db(list_directory_workspace_configs)
        .expect("list_directory_workspace_configs");
    assert!(rows.is_empty());
}

#[test]
fn test_projects_tab_root_only_with_inbox_first_and_subdir_filter() {
    let (_tmp, _vault, pm) = setup_test_env();

    let root_notes = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "projects".to_string(),
            dir_rel_path: None,
            ..Default::default()
        },
    )
    .expect("notes_list projects root");

    assert!(!root_notes.is_empty(), "root directory should have notes");
    assert!(
        root_notes[0].is_inbox,
        "inbox.md must be pinned first in root projects list"
    );
    assert!(
        root_notes
            .iter()
            .all(|item| item.rel_path.as_deref().is_some_and(|p| !p.contains('/'))),
        "unselected dir must only return root-level notes"
    );

    let nav = notes_get_tree_and_tags(&pm).expect("notes_get_tree_and_tags");
    let first_dir = nav
        .dirs
        .first()
        .expect("fixture vault should have at least one subdirectory")
        .rel_path
        .clone();

    let subdir_notes = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "projects".to_string(),
            dir_rel_path: Some(first_dir.clone()),
            ..Default::default()
        },
    )
    .expect("notes_list projects subdir");

    assert!(
        !subdir_notes.is_empty(),
        "selected subdir should return its notes"
    );
    let prefix = format!("{first_dir}/");
    assert!(
        subdir_notes.iter().all(|item| item
            .rel_path
            .as_deref()
            .is_some_and(|p| p.starts_with(&prefix))),
        "selected subdir must only return notes under that subdir"
    );
}

#[test]
fn test_notes_backfill_missing_ids_mixed_vault_and_idempotency() {
    let tmp = TempDir::new().expect("tempdir");
    let app_data = tmp.path().join("app_data");
    let pm = PersistenceManager::open(app_data).expect("open persistence");

    // 1. Unconfigured library root must return LibraryNotConfigured.
    let err = notes_backfill_missing_ids(&pm, None).expect_err("unconfigured library");
    assert_eq!(err.code, ErrorCode::LibraryNotConfigured);

    // 2. Prepare a controlled mixed vault.
    let vault = tmp.path().join("vault");
    fs::create_dir_all(vault.join("nested")).expect("create nested");
    fs::create_dir_all(vault.join("_templates")).expect("create _templates");
    fs::create_dir_all(vault.join(".hidden")).expect("create .hidden");
    fs::create_dir_all(vault.join("assets")).expect("create assets");

    let has_id_raw = "---\nid: 01JEXISTING000000000000001\ntitle: 已有 ID 笔记\n---\n\n正文。\n";
    let valid_missing_raw = "---\n# 保留注释\ntitle: 缺 ID 合法笔记\ncustom_owner: joey\ntags:\n  - rust\n---\n\n# 缺 ID 合法笔记\n\n正文段落。\n";
    let no_fm_raw = "# 纯正文笔记\n\n没有 frontmatter 的内容。\n";
    let broken_yaml_raw = "---\ntitle: [unterminated\n---\n\n# 损坏 YAML\n";
    let unterminated_fm_raw = "---\ntitle: 未闭合 frontmatter\n\n正文内容。\n";
    let template_raw = "---\ntitle: \"{{title}}\"\ntype: template\n---\n\n模板正文。\n";
    let hidden_raw = "# 隐藏目录笔记\n";
    let assets_raw = "# 资源目录说明\n";
    let tmp_raw = "# 临时文件\n";

    fs::write(vault.join("has-id.md"), has_id_raw).expect("write has-id.md");
    fs::write(vault.join("valid-missing-id.md"), valid_missing_raw)
        .expect("write valid-missing-id.md");
    fs::write(vault.join("nested/no-frontmatter.md"), no_fm_raw).expect("write no-frontmatter.md");
    fs::write(vault.join("broken-yaml.md"), broken_yaml_raw).expect("write broken-yaml.md");
    fs::write(vault.join("unterminated-fm.md"), unterminated_fm_raw)
        .expect("write unterminated-fm.md");
    fs::write(vault.join("_templates/meeting.md"), template_raw).expect("write template");
    fs::write(vault.join(".hidden/secret.md"), hidden_raw).expect("write hidden");
    fs::write(vault.join("assets/readme.md"), assets_raw).expect("write assets");
    fs::write(vault.join("draft.tmp.md"), tmp_raw).expect("write tmp");

    set_library_root(
        &pm,
        SetRootArgs {
            root: vault.to_string_lossy().to_string(),
            create_if_missing: false,
        },
    )
    .expect("set_library_root");
    index_rebuild(&pm, None).expect("initial index_rebuild");

    // Before backfill, notes without id are indexed under path:<rel_path>.
    let pre_key = pm
        .with_db(|conn| get_note_key_by_rel_path(conn, "valid-missing-id.md"))
        .expect("pre query valid-missing-id");
    assert_eq!(pre_key.as_deref(), Some("path:valid-missing-id.md"));

    // 3. First backfill execution.
    let res1 = notes_backfill_missing_ids(&pm, None).expect("first backfill");
    assert_eq!(res1.scanned, 5);
    assert_eq!(res1.updated, 2);
    assert_eq!(res1.skipped_existing, 1);
    assert_eq!(res1.skipped_broken_yaml, 2);
    assert_eq!(res1.failed, 0);

    // 4. Verify untouched files remain byte-for-byte identical.
    assert_eq!(
        fs::read_to_string(vault.join("has-id.md")).expect("read has-id"),
        has_id_raw
    );
    assert_eq!(
        fs::read_to_string(vault.join("broken-yaml.md")).expect("read broken-yaml"),
        broken_yaml_raw
    );
    assert_eq!(
        fs::read_to_string(vault.join("unterminated-fm.md")).expect("read unterminated-fm"),
        unterminated_fm_raw
    );
    assert_eq!(
        fs::read_to_string(vault.join("_templates/meeting.md")).expect("read template"),
        template_raw
    );
    assert_eq!(
        fs::read_to_string(vault.join(".hidden/secret.md")).expect("read hidden"),
        hidden_raw
    );
    assert_eq!(
        fs::read_to_string(vault.join("assets/readme.md")).expect("read assets"),
        assets_raw
    );
    assert_eq!(
        fs::read_to_string(vault.join("draft.tmp.md")).expect("read tmp"),
        tmp_raw
    );

    // 5. Verify valid-missing-id.md preserved comments & unknown keys and synced SQLite.
    let patched_disk =
        fs::read_to_string(vault.join("valid-missing-id.md")).expect("read valid-missing-id");
    assert!(
        patched_disk.contains("# 保留注释"),
        "existing YAML comment must be preserved"
    );
    assert!(
        patched_disk.contains("custom_owner: joey"),
        "unknown YAML key must be preserved"
    );
    let patched_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: vault
                .join("valid-missing-id.md")
                .to_string_lossy()
                .to_string(),
        },
    )
    .expect("read valid-missing-id doc");
    let patched_id = patched_doc.meta.id.clone().expect("patched ULID");
    assert_eq!(patched_id.len(), 26);
    assert_eq!(
        pm.with_db(|conn| get_note_key_by_rel_path(conn, "valid-missing-id.md"))
            .expect("sqlite key valid-missing-id")
            .as_deref(),
        Some(patched_id.as_str())
    );

    // 6. Verify nested/no-frontmatter.md synthesized minimal frontmatter and synced SQLite.
    let no_fm_disk =
        fs::read_to_string(vault.join("nested/no-frontmatter.md")).expect("read no-frontmatter");
    let no_fm_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: vault
                .join("nested/no-frontmatter.md")
                .to_string_lossy()
                .to_string(),
        },
    )
    .expect("read no-frontmatter doc");
    let no_fm_id = no_fm_doc.meta.id.clone().expect("no_fm ULID");
    assert_eq!(no_fm_id.len(), 26);
    assert_ne!(no_fm_id, patched_id);
    assert_eq!(no_fm_disk, format!("---\nid: {no_fm_id}\n---\n{no_fm_raw}"));
    assert_eq!(
        pm.with_db(|conn| get_note_key_by_rel_path(conn, "nested/no-frontmatter.md"))
            .expect("sqlite key no-frontmatter")
            .as_deref(),
        Some(no_fm_id.as_str())
    );

    // 7. Second execution must be idempotent (updated == 0).
    let res2 = notes_backfill_missing_ids(&pm, None).expect("second backfill");
    assert_eq!(res2.scanned, 5);
    assert_eq!(res2.updated, 0);
    assert_eq!(res2.skipped_existing, 3);
    assert_eq!(res2.skipped_broken_yaml, 2);
    assert_eq!(res2.failed, 0);
}

#[test]
fn test_unified_title_write_sync_and_reconcile_version_upgrade() {
    let tmp = TempDir::new().expect("tempdir");
    let app_dir = tmp.path.join("app_data");
    let vault = tmp.path.join("vault");
    fs::create_dir_all(&vault).expect("mkdir vault");

    // 1. 准备三类文档：含旧 frontmatter title + H1 的 Markdown、无 H1 的 Markdown、非 Markdown 配置文件
    let h1_path = vault.join("with-h1.md");
    fs::write(
        &h1_path,
        "---\nid: 01JTITLEUPGRADE00000000001\ntitle: 旧Frontmatter标题\nproject: legacy\n---\n# 初始一级标题\n\n正文\n",
    )
    .expect("write with-h1.md");

    let no_h1_path = vault.join("plain-doc.md");
    fs::write(
        &no_h1_path,
        "---\nid: 01JTITLEUPGRADE00000000002\ntitle: 忽略的FM标题\n---\n## 只有二级标题\n",
    )
    .expect("write plain-doc.md");

    let yaml_path = vault.join("config.yaml");
    fs::write(&yaml_path, "name: demo\n").expect("write config.yaml");

    let pm = PersistenceManager::open(app_dir.clone()).expect("open pm");
    set_library_root(
        &pm,
        SetRootArgs {
            root: vault.to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect("set root");
    index_rebuild(&pm, None).expect("initial rebuild");

    // 2. 模拟老版本缓存库：强行把 notes.title 改回旧 FM 标题并清除 title_rule_version，模拟跨重启升级
    pm.with_db(|conn| {
        conn.execute(
            "UPDATE notes SET title = '旧Frontmatter标题' WHERE rel_path = 'with-h1.md'",
            [],
        )
        .expect("corrupt legacy title");
        conn.execute(
            "DELETE FROM library_meta WHERE key = 'title_rule_version'",
            [],
        )
        .expect("delete rule version");
        Ok(())
    })
    .expect("simulate legacy db");

    // 3. 再次执行增量对账（即使 mtime/size 未变，版本号不匹配也会强制刷新标题）
    let pm_restarted = PersistenceManager::open(app_dir).expect("reopen pm");
    let report = pm_restarted
        .with_db(|conn| reconcile_vault(&vault, conn))
        .expect("reconcile after upgrade");
    assert!(report.notes_upserted >= 2);

    let list = notes_list(
        &pm_restarted,
        NotesFilterArgs {
            tab: "recent".to_string(),
            dir_rel_path: None,
            tags: vec![],
            filter_text: None,
        },
    )
    .expect("notes_list after upgrade");
    let h1_item = list
        .iter()
        .find(|n| n.rel_path.as_deref() == Some("with-h1.md"))
        .expect("h1 item");
    let no_h1_item = list
        .iter()
        .find(|n| n.rel_path.as_deref() == Some("plain-doc.md"))
        .expect("no h1 item");
    assert_eq!(h1_item.title, "初始一级标题");
    assert_eq!(no_h1_item.title, "plain-doc");

    // 4. 修改正文一级标题并落盘保存：WriteResult.updated_meta 必须返回新标题，且列表同步更新
    let read_before = note_read(
        &pm_restarted,
        ReadNoteArgs {
            abs_path: h1_path.to_string_lossy().to_string(),
        },
    )
    .expect("read with-h1");
    assert_eq!(read_before.meta.title, "初始一级标题");

    let write_res = note_write(
        &pm_restarted,
        WriteNoteArgs {
            abs_path: h1_path.to_string_lossy().to_string(),
            body: "# 修改后的一级标题\n\n正文\n".to_string(),
            base_body_hash: read_before.body_hash,
        },
    )
    .expect("write updated h1");
    let updated_meta = write_res
        .updated_meta
        .expect("updated_meta must be returned when derived H1 title changes");
    assert_eq!(updated_meta.title, "修改后的一级标题");

    let list_after_write = notes_list(
        &pm_restarted,
        NotesFilterArgs {
            tab: "recent".to_string(),
            dir_rel_path: None,
            tags: vec![],
            filter_text: None,
        },
    )
    .expect("notes_list after write");
    let h1_after = list_after_write
        .iter()
        .find(|n| n.rel_path.as_deref() == Some("with-h1.md"))
        .expect("h1 after");
    assert_eq!(h1_after.title, "修改后的一级标题");

    // 5. 非 Markdown 文件读取时标题必须保留完整文件名（含扩展名）
    let yaml_doc = note_read(
        &pm_restarted,
        ReadNoteArgs {
            abs_path: yaml_path.to_string_lossy().to_string(),
        },
    )
    .expect("read config.yaml");
    assert_eq!(yaml_doc.meta.title, "config.yaml");
}
