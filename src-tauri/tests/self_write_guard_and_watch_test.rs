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
use quicknote_lib::application::reconciler::{
    reconcile_watched_path, write_directory_workspace_config, ReconcilePathOutcome,
};
use quicknote_lib::application::watch::process_external_fs_path;
use quicknote_lib::domain::capture::AppendNoteArgs;
use quicknote_lib::domain::note::{
    CreateNoteArgs, NoteMetaPatch, NoteUpdateMetaArgs, NotesFilterArgs, SearchQueryArgs,
    WriteNoteArgs,
};
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::domain::ui_state::CaptureMode;
use quicknote_lib::domain::watch::{ExternalChangeEvent, ExternalChangeKind, SelfWriteGuard};
use quicknote_lib::domain::workspace_context::DirectoryWorkspaceConfig;
use quicknote_lib::infra::sqlite::{get_note_key_by_rel_path, list_directory_workspace_configs};
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
    assert!(guard.should_suppress_modify(path, "hash_v1", 2_500));
    // Different disk hash within window -> real external modification, not suppressed.
    assert!(!guard.should_suppress_modify(path, "hash_v2", 1_600));
    // Old record was removed when mismatch happened.
    assert!(!guard.should_suppress_modify(path, "hash_v1", 1_700));

    // Past 1500ms window -> expired, no longer suppressed.
    guard.record_write(path, "hash_v1", 1_000);
    assert!(!guard.should_suppress_modify(path, "hash_v1", 2_501));

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
                pinned: Some(true),
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
            filter_text: None,
        },
    )?;
    assert!(!listed.iter().any(|n| n.rel_path == doc.rel_path));

    Ok(())
}

#[test]
fn vault_watcher_converges_root_manual_md_and_quicknote_yaml_with_self_write_guard(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_app_dir, vault_dir, pm) = setup_pm_with_library()?;
    let vault_root = vault_dir.path().to_path_buf();

    let outcomes: Arc<Mutex<Vec<(PathBuf, ReconcilePathOutcome)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let outcomes_clone = Arc::clone(&outcomes);
    let pm_clone = Arc::clone(&pm);
    let vault_root_clone = vault_root.clone();

    let _watcher = VaultWatcher::start(Some(&vault_root), move |path| {
        if let Ok(outcome) = reconcile_watched_path(&pm_clone, &vault_root_clone, path) {
            if let Ok(mut guard) = outcomes_clone.lock() {
                guard.push((path.to_path_buf(), outcome));
            }
        }
    })?;

    thread::sleep(Duration::from_millis(120));

    // 1. Create a root-level manual.md with no Frontmatter -> watcher must converge it into SQLite.
    let manual_path = vault_root.join("manual.md");
    fs::write(&manual_path, "# 根目录监听文档\n\n监听测试：极光观测站。\n")?;

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if pm.with_db(|conn| get_note_key_by_rel_path(conn, "manual.md"))?
            == Some("path:manual.md".to_string())
            || Instant::now() >= deadline
        {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        pm.with_db(|conn| get_note_key_by_rel_path(conn, "manual.md"))?,
        Some("path:manual.md".to_string())
    );

    // 2. Rename manual.md -> manual-renamed.md at Vault root.
    let renamed_path = vault_root.join("manual-renamed.md");
    fs::rename(&manual_path, &renamed_path)?;

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let old_gone = pm
            .with_db(|conn| get_note_key_by_rel_path(conn, "manual.md"))?
            .is_none();
        let new_present = pm.with_db(|conn| get_note_key_by_rel_path(conn, "manual-renamed.md"))?
            == Some("path:manual-renamed.md".to_string());
        if (old_gone && new_present) || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        pm.with_db(|conn| get_note_key_by_rel_path(conn, "manual.md"))?,
        None
    );
    assert_eq!(
        pm.with_db(|conn| get_note_key_by_rel_path(conn, "manual-renamed.md"))?,
        Some("path:manual-renamed.md".to_string())
    );

    // 3. External creation of root .quicknote.yaml must trigger debounced convergence.
    let root_cfg_path = vault_root.join(".quicknote.yaml");
    fs::write(&root_cfg_path, "projectRoot: /tmp/watched-root\n")?;

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let configs = pm.with_db(list_directory_workspace_configs)?;
        if configs
            .iter()
            .any(|c| c.vault_rel_dir.is_empty() && c.config_json.contains("/tmp/watched-root"))
            || Instant::now() >= deadline
        {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let configs = pm.with_db(list_directory_workspace_configs)?;
    assert!(configs
        .iter()
        .any(|c| c.vault_rel_dir.is_empty() && c.config_json.contains("/tmp/watched-root")));

    // 4. Internal write via write_directory_workspace_config must be suppressed by SelfWriteGuard.
    {
        let mut guard = outcomes.lock().map_err(|_| "outcomes mutex poisoned")?;
        guard.clear();
    }
    let sub_cfg = DirectoryWorkspaceConfig {
        project_root: Some("/tmp/internal-sub".to_string()),
        ..Default::default()
    };
    let sub_cfg_path = write_directory_workspace_config(&pm, &vault_root, "sub-ws", &sub_cfg)?;
    thread::sleep(Duration::from_millis(350));

    let observed = outcomes.lock().map_err(|_| "outcomes mutex poisoned")?;
    assert!(
        !observed.iter().any(|(p, o)| {
            p.ends_with(".quicknote.yaml")
                && p.to_string_lossy().contains("sub-ws")
                && *o == ReconcilePathOutcome::ConfigUpserted
        }),
        "Internal .quicknote.yaml write should not trigger external ConfigUpserted, got {:?}",
        *observed
    );
    drop(observed);

    // 5. External deletion of .quicknote.yaml must prune cached row.
    fs::remove_file(&sub_cfg_path)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let configs = pm.with_db(list_directory_workspace_configs)?;
        if !configs.iter().any(|c| c.vault_rel_dir == "sub-ws") || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let configs_after = pm.with_db(list_directory_workspace_configs)?;
    assert!(!configs_after.iter().any(|c| c.vault_rel_dir == "sub-ws"));

    Ok(())
}

#[test]
fn note_write_registers_self_write_before_watch_can_observe_it(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_app_dir, _vault_dir, pm) = setup_pm_with_library()?;
    let doc = note_create(
        &pm,
        CreateNoteArgs {
            title: "回声测试".to_string(),
            dir_rel_path: "notes".to_string(),
            template_name: None,
            body: Some("# 回声测试\n\n初始正文。\n".to_string()),
        },
    )?;
    // 建库后先消化创建产生的自写记录，再模拟编辑器连续保存。
    let _ = process_external_fs_path(&pm, Path::new(&doc.abs_path))?;

    let _bytes = fs::read(&doc.abs_path)?;
    let first = note_write(
        &pm,
        WriteNoteArgs {
            abs_path: doc.abs_path.clone(),
            body: "# 回声测试\n\n第一次写入。\n".to_string(),
            base_body_hash: doc.body_hash.clone(),
        },
    )?;
    assert!(process_external_fs_path(&pm, Path::new(&doc.abs_path))?.is_none());

    note_write(
        &pm,
        WriteNoteArgs {
            abs_path: doc.abs_path.clone(),
            body: "# 回声测试\n\n第二次写入，内容不同。\n".to_string(),
            base_body_hash: first.body_hash.clone(),
        },
    )?;
    assert!(process_external_fs_path(&pm, Path::new(&doc.abs_path))?.is_none());
    Ok(())
}

#[test]
fn set_library_root_switches_to_vault_isolated_db_for_restart(
) -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = TempDir::new()?;
    let vault_dir = TempDir::new()?;
    let projects_dir = vault_dir.path().join("Projects");
    fs::create_dir_all(&projects_dir)?;
    fs::write(
        projects_dir.join("QuickNote.md"),
        "---\ntitle: QuickNote 改进列表\ntags:\n  - dev\n---\n\n# QuickNote\n",
    )?;

    // 首次启动：未配置 library_root，PersistenceManager 默认打开 cache/index.db。
    {
        let pm = PersistenceManager::open(app_dir.path().to_path_buf())?;
        let info = set_library_root(
            &pm,
            SetRootArgs {
                root: vault_dir.path().to_string_lossy().to_string(),
                create_if_missing: true,
            },
        )?;
        assert!(info.note_count >= 1);
    }

    // 模拟重启：PersistenceManager 读到 settings.yaml 中的 library_root，直接连向 cache/vaults/<hash>.db。
    let reopened = PersistenceManager::open(app_dir.path().to_path_buf())?;
    assert!(
        reopened
            .with_db(|conn| get_note_key_by_rel_path(conn, "Projects/QuickNote.md"))?
            .is_some(),
        "Reopened PersistenceManager must read indexed notes from vault-isolated cache DB"
    );

    Ok(())
}

#[test]
fn self_write_guard_suppresses_rapid_consecutive_autosaves() {
    let mut guard = SelfWriteGuard::new();
    let path = "/vault/Projects/QuickNote.md";
    guard.record_write(path, "hash-v1", 1_000);
    guard.record_write(path, "hash-v2", 1_600);

    // 延迟到达的 V1 回声与 V2 回声在各自窗口内都必须被抑制，而真实外部写入 V3 不应被抑制。
    assert!(guard.should_suppress_modify(path, "hash-v1", 2_000));
    assert!(guard.should_suppress_modify(path, "hash-v2", 2_200));
    assert!(!guard.should_suppress_modify(path, "hash-external-v3", 2_300));
}

#[test]
fn handle_vault_watch_path_reconciles_directory_moves_and_deletes(
) -> Result<(), Box<dyn std::error::Error>> {
    use quicknote_lib::application::watch::{handle_vault_watch_path, VaultWatchOutcome};

    let (_app_dir, vault_dir, pm) = setup_pm_with_library()?;
    let old_dir = vault_dir.path().join("Projects").join("Sub");
    fs::create_dir_all(&old_dir)?;
    let old_file = old_dir.join("Moved.md");
    fs::write(&old_file, "# 移动测试\n\n正文内容。\n")?;

    // 先触发单文件监听收录旧路径。
    let created = handle_vault_watch_path(&pm, &old_file)?;
    assert!(matches!(created, VaultWatchOutcome::ExternalChange(_)));
    assert!(pm
        .with_db(|conn| get_note_key_by_rel_path(conn, "Projects/Sub/Moved.md"))?
        .is_some());

    // 模拟 Finder 将整个目录移到新位置（FSEvents 常只上报目录路径）。
    let new_dir = vault_dir.path().join("Projects").join("RenamedSub");
    fs::rename(&old_dir, &new_dir)?;

    let outcome = handle_vault_watch_path(&pm, &old_dir)?;
    assert_eq!(outcome, VaultWatchOutcome::ReconciledVault);
    assert!(pm
        .with_db(|conn| get_note_key_by_rel_path(conn, "Projects/Sub/Moved.md"))?
        .is_none());
    assert!(pm
        .with_db(|conn| get_note_key_by_rel_path(conn, "Projects/RenamedSub/Moved.md"))?
        .is_some());

    Ok(())
}
