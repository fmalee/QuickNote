use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use quicknote_lib::application::capture::CaptureManager;
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{
    note_create, note_update_meta, note_write, notes_list, search_notes_and_files,
};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::application::watch::process_external_fs_path;
use quicknote_lib::domain::capture::AppendNoteArgs;
use quicknote_lib::domain::note::{
    CreateNoteArgs, NoteMetaPatch, NoteUpdateMetaArgs, NotesFilterArgs, SearchQueryArgs,
    WriteNoteArgs,
};
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::domain::ui_state::CaptureMode;
use quicknote_lib::domain::watch::{ExternalChangeEvent, ExternalChangeKind, SelfWriteGuard};
use quicknote_lib::infra::vault_watch::VaultWatcher;

static TEST_DIR_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Result<Self, std::io::Error> {
        let seq = TEST_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "quicknote_watch_test_{}_{}",
            std::process::id(),
            seq
        ));
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

fn setup_pm_with_library(
) -> Result<(TempDir, TempDir, Arc<PersistenceManager>), Box<dyn std::error::Error>> {
    let app_dir = TempDir::new()?;
    let vault_dir = TempDir::new()?;
    let pm = Arc::new(PersistenceManager::open(app_dir.path().to_path_buf())?);
    set_library_root(
        &pm,
        SetRootArgs {
            root: vault_dir.path().to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )?;
    Ok((app_dir, vault_dir, pm))
}

#[test]
fn self_write_guard_pure_state_machine() {
    let mut guard = SelfWriteGuard::new();
    let path = "/private/var/tmp/note.md";
    guard.record_write(path, "hash_v1", 1_000);

    // Within 1500ms and same hash -> suppressed.
    assert!(guard.should_suppress_modify(path, "hash_v1", 1_500));
    // Even slightly after 1500ms with unchanged disk hash -> still suppressed.
    assert!(guard.should_suppress_modify(path, "hash_v1", 3_000));
    // Different disk hash -> real external modification, not suppressed.
    assert!(!guard.should_suppress_modify(path, "hash_v2", 1_600));
    // Old record was removed when mismatch happened.
    assert!(!guard.should_suppress_modify(path, "hash_v1", 1_700));

    guard.record_write(path, "hash_v3", 5_000);
    guard.clear_path(path);
    assert!(!guard.should_suppress_modify(path, "hash_v3", 5_100));
}

#[test]
fn internal_writes_are_suppressed_by_process_external_fs_path_and_vault_watcher(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_app_dir, vault_dir, pm) = setup_pm_with_library()?;
    let capture = CaptureManager::new();

    let events: Arc<Mutex<Vec<ExternalChangeEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);
    let pm_clone = Arc::clone(&pm);

    let _watcher = VaultWatcher::start(Some(vault_dir.path()), move |path| {
        if let Ok(Some(ev)) = process_external_fs_path(&pm_clone, path) {
            if let Ok(mut guard) = events_clone.lock() {
                guard.push(ev);
            }
        }
    })?;

    // Give notify watcher a brief moment to register FSEvents on macOS.
    thread::sleep(Duration::from_millis(120));

    // 1. note_create + note_write
    let doc = note_create(
        &pm,
        CreateNoteArgs {
            title: "防回环测试".to_string(),
            dir_rel_path: "notes".to_string(),
            template_name: None,
            body: Some("# 防回环测试\n\n初始正文。\n".to_string()),
        },
    )?;

    let write_res = note_write(
        &pm,
        WriteNoteArgs {
            abs_path: doc.abs_path.clone(),
            body: "# 防回环测试\n\n内部编辑器写入的新正文。\n".to_string(),
            base_body_hash: doc.body_hash.clone(),
        },
    )?;

    // Direct process_external_fs_path check must suppress.
    assert!(process_external_fs_path(&pm, Path::new(&doc.abs_path))?.is_none());

    // 2. note_update_meta
    let updated_doc = note_update_meta(
        &pm,
        NoteUpdateMetaArgs {
            abs_path: doc.abs_path.clone(),
            patch: NoteMetaPatch {
                title: Some("防回环测试-已更新".to_string()),
                ..Default::default()
            },
        },
    )?;
    assert_eq!(updated_doc.body_hash, write_res.body_hash);
    assert!(process_external_fs_path(&pm, Path::new(&doc.abs_path))?.is_none());

    // 3. note_append
    let rel_path = doc
        .rel_path
        .clone()
        .ok_or("Expected rel_path on created note")?;
    capture.note_append(
        &pm,
        AppendNoteArgs {
            target_rel_path: rel_path,
            mode: CaptureMode::List,
            content: "追加的一条捕捉内容".to_string(),
            workspace_key: None,
        },
        None,
    )?;
    assert!(process_external_fs_path(&pm, Path::new(&doc.abs_path))?.is_none());

    // Wait past VaultWatcher debounce (100ms) + FSEvents delivery.
    thread::sleep(Duration::from_millis(450));
    let collected = events.lock().map_err(|_| "events mutex poisoned")?;
    assert_eq!(
        collected.len(),
        0,
        "Expected 0 fs:external-change events for internal writes, got {:?}",
        *collected
    );

    Ok(())
}

#[test]
fn external_modify_and_delete_update_index_and_emit_events(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_app_dir, vault_dir, pm) = setup_pm_with_library()?;

    let doc = note_create(
        &pm,
        CreateNoteArgs {
            title: "外部变更样本".to_string(),
            dir_rel_path: "notes".to_string(),
            template_name: None,
            body: Some("# 外部变更样本\n\n原始内容。\n".to_string()),
        },
    )?;

    // Start real VaultWatcher to verify end-to-end delivery as well.
    let events: Arc<Mutex<Vec<ExternalChangeEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);
    let pm_clone = Arc::clone(&pm);
    let _watcher = VaultWatcher::start(Some(vault_dir.path()), move |path| {
        if let Ok(Some(ev)) = process_external_fs_path(&pm_clone, path) {
            if let Ok(mut guard) = events_clone.lock() {
                guard.push(ev);
            }
        }
    })?;
    thread::sleep(Duration::from_millis(120));

    // External process modifies the file directly.
    let external_raw =
        "---\ntitle: 外部变更样本\ntags:\n  - external\n---\n\n# 外部变更样本\n\n外部新增词：星辰大海。\n";
    fs::write(&doc.abs_path, external_raw)?;

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let count = events.lock().map_err(|_| "events mutex poisoned")?.len();
        if count >= 1 || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let first_event = {
        let guard = events.lock().map_err(|_| "events mutex poisoned")?;
        guard.first().cloned()
    };

    let ev = match first_event {
        Some(ev) => ev,
        None => process_external_fs_path(&pm, Path::new(&doc.abs_path))?
            .ok_or("Expected Modified event from external write")?,
    };

    assert_eq!(ev.kind, ExternalChangeKind::Modified);
    assert!(ev
        .new_body
        .as_deref()
        .is_some_and(|b| b.contains("星辰大海") && !b.starts_with("---")));
    assert!(ev.new_body_hash.is_some());
    assert_eq!(
        ev.new_meta.as_ref().map(|m| m.tags.clone()),
        Some(vec!["external".to_string()])
    );

    // SQLite FTS5 index must immediately find the externally added term.
    let search_res = search_notes_and_files(
        &pm,
        SearchQueryArgs {
            query: "星辰大海".to_string(),
            limit: Some(10),
            request_id: 1,
        },
    )?;
    assert_eq!(search_res.items.len(), 1);
    assert_eq!(search_res.items[0].rel_path, doc.rel_path);

    // External process deletes the file directly.
    fs::remove_file(&doc.abs_path)?;
    let deleted_ev = process_external_fs_path(&pm, Path::new(&doc.abs_path))?
        .ok_or("Expected Deleted event after external remove_file")?;
    assert_eq!(deleted_ev.kind, ExternalChangeKind::Deleted);
    assert!(deleted_ev.new_body.is_none());
    assert!(deleted_ev.new_body_hash.is_none());
    assert!(deleted_ev.new_meta.is_none());

    let listed = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "all".to_string(),
            dir_rel_path: None,
            tags: Vec::new(),
            workspace_key: None,
            filter_text: None,
        },
    )?;
    assert!(!listed.iter().any(|n| n.rel_path == doc.rel_path));

    Ok(())
}
