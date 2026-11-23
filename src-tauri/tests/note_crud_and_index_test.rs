use std::fs;
use std::path::{Path, PathBuf};

use quicknote_lib::application::index::{index_get_status, index_rebuild};
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{
    note_create, note_delete, note_read, note_rename, note_update_meta, note_write,
    notes_get_tree_and_tags, notes_list, search_notes_and_files,
};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::note::{
    CreateNoteArgs, IndexPhase, NoteMetaPatch, NoteUpdateMetaArgs, NotesFilterArgs, ReadNoteArgs,
    RelPathArgs, RenameNoteArgs, SearchQueryArgs, WriteNoteArgs,
};
use quicknote_lib::domain::settings::SetRootArgs;

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
