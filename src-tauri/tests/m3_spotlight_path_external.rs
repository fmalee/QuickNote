use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use quicknote_lib::application::external::{
    external_action_run, external_actions_resolve, path_complete, search_spotlight,
};
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{note_read, search_notes_and_files};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::error::{AppError, ErrorCode};
use quicknote_lib::domain::external::{
    is_spotlight_blacklisted, ExternalActionId, ExternalActionState, ResolveExternalArgs,
    RunExternalArgs, SETTINGS_FILE_SENTINEL,
};
use quicknote_lib::domain::note::{
    PathCompleteArgs, ReadNoteArgs, SearchQueryArgs, SearchSource, SpotlightArgs,
};
use quicknote_lib::domain::settings::{
    ExternalAppConfig, SetRootArgs, SettingsPatch, WorkspaceConfig,
};
use quicknote_lib::infra::external_open::ExternalLauncher;
use quicknote_lib::infra::spotlight::{SpotlightRunner, SPOTLIGHT_HARD_LIMIT};

static TEST_DIR_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Result<Self, std::io::Error> {
        let seq = TEST_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "quicknote_m3_01b_test_{}_{}",
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

#[derive(Default)]
struct RecordingLauncher {
    calls: Mutex<Vec<(String, String)>>,
}

impl RecordingLauncher {
    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().expect("recording lock").clone()
    }
}

impl ExternalLauncher for RecordingLauncher {
    fn reveal_in_finder(&self, abs_path: &Path) -> Result<(), AppError> {
        self.calls
            .lock()
            .expect("recording lock")
            .push(("reveal".to_string(), abs_path.to_string_lossy().to_string()));
        Ok(())
    }

    fn open_default(&self, abs_path: &Path) -> Result<(), AppError> {
        self.calls.lock().expect("recording lock").push((
            "default".to_string(),
            abs_path.to_string_lossy().to_string(),
        ));
        Ok(())
    }

    fn open_with_app(&self, app: &ExternalAppConfig, target: &Path) -> Result<(), AppError> {
        self.calls.lock().expect("recording lock").push((
            format!("app:{}", app.bundle_id),
            target.to_string_lossy().to_string(),
        ));
        Ok(())
    }

    fn open_terminal(&self, app: &ExternalAppConfig, dir_path: &Path) -> Result<(), AppError> {
        self.calls.lock().expect("recording lock").push((
            format!("terminal:{}", app.bundle_id),
            dir_path.to_string_lossy().to_string(),
        ));
        Ok(())
    }

    fn copy_to_clipboard(&self, text: &str) -> Result<(), AppError> {
        self.calls
            .lock()
            .expect("recording lock")
            .push(("copy".to_string(), text.to_string()));
        Ok(())
    }
}

fn setup_pm() -> (TempDir, TempDir, PersistenceManager) {
    let app_dir = TempDir::new().expect("create app_data_dir");
    let lib_dir = TempDir::new().expect("create library_root");
    let pm = PersistenceManager::open(app_dir.path().to_path_buf()).expect("open pm");
    set_library_root(
        &pm,
        SetRootArgs {
            root: lib_dir.path().to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect("set library root");
    (app_dir, lib_dir, pm)
}

fn find_state(states: &[ExternalActionState], id: ExternalActionId) -> &ExternalActionState {
    states
        .iter()
        .find(|s| s.id == id)
        .expect("all 8 action states present")
}

#[test]
fn spotlight_blacklist_and_hard_limit_50() {
    assert!(is_spotlight_blacklisted("/repo/.git/HEAD"));
    assert!(is_spotlight_blacklisted("/repo/node_modules/pkg/index.js"));
    assert!(is_spotlight_blacklisted("/repo/target/debug/app"));
    assert!(is_spotlight_blacklisted("/repo/dist/bundle.js"));
    assert!(is_spotlight_blacklisted("/repo/build/output.o"));
    assert!(is_spotlight_blacklisted("/repo/.venv/bin/python"));
    assert!(is_spotlight_blacklisted("/repo/docs/.DS_Store"));
    assert!(!is_spotlight_blacklisted("/repo/src/main.rs"));

    let (_app, lib, pm) = setup_pm();
    let root_str = lib.path().to_string_lossy().to_string();
    pm.settings_update(SettingsPatch {
        search_roots: Some(vec![root_str]),
        ..Default::default()
    })
    .expect("update search_roots");

    let runner = SpotlightRunner::with_command_factory(|_root, _query| {
        let mut lines = vec![
            "/tmp/proj/.git/config".to_string(),
            "/tmp/proj/node_modules/a.js".to_string(),
            "/tmp/proj/target/out".to_string(),
            "/tmp/proj/dist/app.js".to_string(),
            "/tmp/proj/build/main.o".to_string(),
            "/tmp/proj/.venv/pyvenv.cfg".to_string(),
            "/tmp/proj/.DS_Store".to_string(),
        ];
        for i in 0..75 {
            lines.push(format!("/tmp/proj/src/file_{i:02}.rs"));
        }
        let payload = lines.join("\n");
        let mut cmd = Command::new("/usr/bin/printf");
        cmd.arg("%s\n").arg(payload);
        cmd
    });

    let resp = search_spotlight(
        &pm,
        &runner,
        SpotlightArgs {
            query: "file".to_string(),
            request_id: 42,
        },
    )
    .expect("search_spotlight");

    assert_eq!(resp.request_id, 42);
    assert!(resp.truncated);
    assert_eq!(resp.items.len(), SPOTLIGHT_HARD_LIMIT);
    assert!(resp
        .items
        .iter()
        .all(|item| item.source == SearchSource::Spotlight
            && !is_spotlight_blacklisted(&item.abs_path)));
}

#[test]
fn spotlight_concurrent_request_cancels_previous_process() {
    let (_app, lib, pm) = setup_pm();
    let root_str = lib.path().to_string_lossy().to_string();
    pm.settings_update(SettingsPatch {
        search_roots: Some(vec![root_str]),
        ..Default::default()
    })
    .expect("update search_roots");

    let runner = Arc::new(SpotlightRunner::with_command_factory(|_root, query| {
        if query == "slow" {
            let mut cmd = Command::new("/bin/sleep");
            cmd.arg("10");
            cmd
        } else {
            let mut cmd = Command::new("/usr/bin/printf");
            cmd.arg("%s\n").arg("/tmp/fast_hit.txt");
            cmd
        }
    }));

    let runner_t1 = Arc::clone(&runner);
    let app_dir_path = _app.path().to_path_buf();
    let t1 = thread::spawn(move || {
        let pm_t1 = PersistenceManager::open(app_dir_path).expect("open pm in thread");
        let start = Instant::now();
        let res = search_spotlight(
            &pm_t1,
            &runner_t1,
            SpotlightArgs {
                query: "slow".to_string(),
                request_id: 1,
            },
        )
        .expect("slow search");
        (res, start.elapsed())
    });

    // Give request 1 a brief moment to spawn `/bin/sleep 10`.
    thread::sleep(Duration::from_millis(80));

    let res2 = search_spotlight(
        &pm,
        &runner,
        SpotlightArgs {
            query: "fast".to_string(),
            request_id: 2,
        },
    )
    .expect("fast search");

    let (res1, elapsed1) = t1.join().expect("join thread 1");
    assert!(
        elapsed1 < Duration::from_secs(2),
        "expected cancelled request 1 to return fast, took {:?}",
        elapsed1
    );
    assert!(res1.items.is_empty());
    assert!(!res1.truncated);

    assert_eq!(res2.request_id, 2);
    assert_eq!(res2.items.len(), 1);
    assert_eq!(res2.items[0].abs_path, "/tmp/fast_hit.txt");
}

#[test]
fn path_complete_dirs_first_prefix_tilde_hidden_and_invalid() {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir_all(dir.path().join("beta_dir")).expect("mkdir beta_dir");
    fs::create_dir_all(dir.path().join("AlphaDir")).expect("mkdir AlphaDir");
    fs::create_dir_all(dir.path().join(".hidden_dir")).expect("mkdir .hidden_dir");
    fs::write(dir.path().join("aardvark.txt"), "a").expect("write aardvark");
    fs::write(dir.path().join("alpha_file.md"), "b").expect("write alpha_file");
    fs::write(dir.path().join(".hidden_file"), "c").expect("write .hidden_file");
    fs::write(dir.path().join(".DS_Store"), "x").expect("write .DS_Store");

    let prefix_all = format!("{}/", dir.path().to_string_lossy());
    let all = path_complete(PathCompleteArgs {
        prefix: prefix_all,
        request_id: 1,
    })
    .expect("path_complete all");
    let names: Vec<&str> = all.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["AlphaDir", "beta_dir", "aardvark.txt", "alpha_file.md"]
    );
    assert!(all[0].is_dir && all[1].is_dir);
    assert!(!all[2].is_dir && !all[3].is_dir);

    let prefix_al = format!("{}/al", dir.path().to_string_lossy());
    let filtered = path_complete(PathCompleteArgs {
        prefix: prefix_al,
        request_id: 2,
    })
    .expect("path_complete prefix");
    let filtered_names: Vec<&str> = filtered.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(filtered_names, vec!["AlphaDir", "alpha_file.md"]);

    let prefix_hidden = format!("{}/.", dir.path().to_string_lossy());
    let hidden = path_complete(PathCompleteArgs {
        prefix: prefix_hidden,
        request_id: 3,
    })
    .expect("path_complete hidden");
    let hidden_names: Vec<&str> = hidden.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(hidden_names, vec![".hidden_dir", ".hidden_file"]);

    let tilde = path_complete(PathCompleteArgs {
        prefix: "~/".to_string(),
        request_id: 4,
    })
    .expect("path_complete tilde");
    assert!(tilde.len() <= 50);

    let invalid = path_complete(PathCompleteArgs {
        prefix: "/definitely_nonexistent_quicknote_dir_12345/".to_string(),
        request_id: 5,
    })
    .expect("path_complete invalid");
    assert!(invalid.is_empty());

    let relative = path_complete(PathCompleteArgs {
        prefix: "relative/path".to_string(),
        request_id: 6,
    })
    .expect("path_complete relative");
    assert!(relative.is_empty());
}

#[test]
fn external_actions_full_matrix_and_pin_sentinel_run() {
    let (_app, lib, pm) = setup_pm();
    let ws_root = TempDir::new().expect("ws_root");
    let ws_open = TempDir::new().expect("ws_open");
    let ws_term = TempDir::new().expect("ws_term");
    let doc_open = TempDir::new().expect("doc_open");
    let doc_term = TempDir::new().expect("doc_term");
    let ext_dir = TempDir::new().expect("ext_dir");

    let code_ws_file = ws_root.path().join("demo.code-workspace");
    fs::write(&code_ws_file, "{}").expect("write code-workspace");

    let mut workspaces = BTreeMap::new();
    workspaces.insert(
        "demo".to_string(),
        WorkspaceConfig {
            name: "Demo".to_string(),
            key: "demo".to_string(),
            root: ws_root.path().to_string_lossy().to_string(),
            open_dir: ws_open.path().to_string_lossy().to_string(),
            code_workspace: Some(code_ws_file.to_string_lossy().to_string()),
            terminal_dir: Some(ws_term.path().to_string_lossy().to_string()),
            default_editor: "vscode".to_string(),
            default_entry_doc: None,
        },
    );
    pm.settings_update(SettingsPatch {
        workspaces: Some(workspaces),
        ..Default::default()
    })
    .expect("update workspaces");

    // 1. Library doc without open_dir / terminal_dir / workspace -> strictly disabled, no fallback!
    let plain_note = lib.path().join("notes/plain.md");
    fs::create_dir_all(plain_note.parent().expect("parent")).expect("mkdir notes");
    fs::write(&plain_note, "---\ntitle: Plain\n---\nBody\n").expect("write plain");

    let plain_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(plain_note.to_string_lossy().to_string()),
            workspace_key: None,
            command_id: None,
        },
    )
    .expect("resolve plain");
    assert!(find_state(&plain_states, ExternalActionId::ExternalReveal).enabled);
    assert!(find_state(&plain_states, ExternalActionId::ExternalDefault).enabled);
    assert!(find_state(&plain_states, ExternalActionId::ExternalCopyPath).enabled);
    for id in [
        ExternalActionId::ExternalTerminal,
        ExternalActionId::ExternalVscode,
        ExternalActionId::ExternalSublime,
        ExternalActionId::ExternalWorkspace,
        ExternalActionId::ExternalPin,
    ] {
        let st = find_state(&plain_states, id);
        assert!(
            !st.enabled,
            "expected {id:?} disabled for plain library doc"
        );
        assert!(st.disabled_reason.is_some());
        assert_ne!(
            st.resolved_path.as_deref(),
            Some(lib.path().to_string_lossy().as_ref())
        );
    }

    // 2. Library doc with `project: demo` -> inherits workspace dirs.
    let proj_note = lib.path().join("notes/proj.md");
    fs::write(&proj_note, "---\ntitle: Proj\nproject: demo\n---\nBody\n").expect("write proj");
    let proj_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(proj_note.to_string_lossy().to_string()),
            workspace_key: None,
            command_id: None,
        },
    )
    .expect("resolve proj");
    assert_eq!(
        find_state(&proj_states, ExternalActionId::ExternalTerminal).resolved_path,
        Some(ws_term.path().to_string_lossy().to_string())
    );
    assert_eq!(
        find_state(&proj_states, ExternalActionId::ExternalVscode).resolved_path,
        Some(ws_open.path().to_string_lossy().to_string())
    );
    assert_eq!(
        find_state(&proj_states, ExternalActionId::ExternalWorkspace).resolved_path,
        Some(code_ws_file.to_string_lossy().to_string())
    );
    assert!(!find_state(&proj_states, ExternalActionId::ExternalPin).enabled);

    // 3. Library doc own `terminal_dir` / `open_dir` overrides workspace config.
    let override_note = lib.path().join("notes/override.md");
    fs::write(
        &override_note,
        format!(
            "---\ntitle: Override\nproject: demo\nopen_dir: {}\nterminal_dir: {}\n---\nBody\n",
            doc_open.path().display(),
            doc_term.path().display()
        ),
    )
    .expect("write override");
    let override_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(override_note.to_string_lossy().to_string()),
            workspace_key: None,
            command_id: None,
        },
    )
    .expect("resolve override");
    assert_eq!(
        find_state(&override_states, ExternalActionId::ExternalTerminal).resolved_path,
        Some(doc_term.path().to_string_lossy().to_string())
    );
    assert_eq!(
        find_state(&override_states, ExternalActionId::ExternalVscode).resolved_path,
        Some(doc_open.path().to_string_lossy().to_string())
    );

    // 4. External file -> parent dir + external.pin enabled.
    let ext_file = ext_dir.path().join("nginx.conf");
    fs::write(&ext_file, "worker_processes 1;").expect("write ext_file");
    let ext_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(ext_file.to_string_lossy().to_string()),
            workspace_key: None,
            command_id: None,
        },
    )
    .expect("resolve external file");
    assert!(find_state(&ext_states, ExternalActionId::ExternalPin).enabled);
    assert_eq!(
        find_state(&ext_states, ExternalActionId::ExternalTerminal).resolved_path,
        Some(ext_dir.path().to_string_lossy().to_string())
    );
    assert_eq!(
        find_state(&ext_states, ExternalActionId::ExternalVscode).resolved_path,
        Some(ext_dir.path().to_string_lossy().to_string())
    );

    // 5. Command context `commandId` priority: current doc workspace -> command book workspace -> disabled.
    let cmd_book_with_ws = lib.path().join("commands/deploy.md");
    fs::create_dir_all(cmd_book_with_ws.parent().expect("parent")).expect("mkdir commands");
    fs::write(
        &cmd_book_with_ws,
        "---\ntitle: Deploy\ntype: command-book\nproject: demo\n---\n## Build\n```sh\necho ok\n```\n",
    )
    .expect("write cmd book");
    let cmd_book_plain = lib.path().join("commands/plain.md");
    fs::write(
        &cmd_book_plain,
        "---\ntitle: PlainCmd\ntype: command-book\n---\n## Test\n```sh\necho hi\n```\n",
    )
    .expect("write plain cmd book");

    // Context doc override wins over command book workspace.
    let cmd_ctx_override = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(override_note.to_string_lossy().to_string()),
            workspace_key: None,
            command_id: Some("commands/deploy.md:6:0".to_string()),
        },
    )
    .expect("resolve cmd ctx override");
    assert_eq!(
        find_state(&cmd_ctx_override, ExternalActionId::ExternalTerminal).resolved_path,
        Some(doc_term.path().to_string_lossy().to_string())
    );

    // No context doc -> falls back to command book's own workspace.
    let cmd_book_fallback = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: None,
            workspace_key: None,
            command_id: Some("commands/deploy.md:6:0".to_string()),
        },
    )
    .expect("resolve cmd book fallback");
    assert_eq!(
        find_state(&cmd_book_fallback, ExternalActionId::ExternalTerminal).resolved_path,
        Some(ws_term.path().to_string_lossy().to_string())
    );

    // Neither context doc nor command book has workspace -> strictly disabled, never $HOME.
    let cmd_none = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: None,
            workspace_key: None,
            command_id: Some("commands/plain.md:6:0".to_string()),
        },
    )
    .expect("resolve cmd none");
    let cmd_term_none = find_state(&cmd_none, ExternalActionId::ExternalTerminal);
    assert!(!cmd_term_none.enabled);
    assert!(cmd_term_none.resolved_path.is_none());

    // 6. `$SETTINGS_YAML` sentinel routing through `external_action_run`.
    let launcher = RecordingLauncher::default();
    external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalDefault,
            abs_path: Some(SETTINGS_FILE_SENTINEL.to_string()),
            dir_path: None,
        },
    )
    .expect("run default on sentinel");
    assert_eq!(
        launcher.calls(),
        vec![(
            "default".to_string(),
            pm.settings_path().to_string_lossy().to_string()
        )]
    );

    // 7. Missing directory for ExternalTerminal returns ErrorCode::WorkspaceDirMissing.
    let missing_dir_err = external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalTerminal,
            abs_path: None,
            dir_path: Some("/nonexistent_quicknote_dir_98765".to_string()),
        },
    )
    .expect_err("missing dir must fail");
    assert_eq!(missing_dir_err.code, ErrorCode::WorkspaceDirMissing);

    // 8. `external.pin` persists to `pinned-files.json` and surfaces pinned at top of `search_notes_and_files`.
    external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalPin,
            abs_path: Some(ext_file.to_string_lossy().to_string()),
            dir_path: None,
        },
    )
    .expect("pin external file");

    let pinned_list = pm.list_pinned_files().expect("list pinned");
    assert_eq!(pinned_list, vec![ext_file.to_string_lossy().to_string()]);

    let pinned_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: ext_file.to_string_lossy().to_string(),
        },
    )
    .expect("read pinned external file");
    assert!(!pinned_doc.is_library);
    assert!(pinned_doc.meta.pinned);

    // External markdown file also reflects pinned-files.json in meta.pinned
    let ext_md = ext_dir.path().join("ext_readme.md");
    fs::write(&ext_md, "---\ntitle: ExtReadme\n---\nHello\n").expect("write ext_md");
    external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalPin,
            abs_path: Some(ext_md.to_string_lossy().to_string()),
            dir_path: None,
        },
    )
    .expect("pin external md");
    let pinned_md_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: ext_md.to_string_lossy().to_string(),
        },
    )
    .expect("read pinned external md");
    assert!(pinned_md_doc.meta.pinned);
    external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalPin,
            abs_path: Some(ext_md.to_string_lossy().to_string()),
            dir_path: None,
        },
    )
    .expect("unpin external md");

    let search_empty = search_notes_and_files(
        &pm,
        SearchQueryArgs {
            query: "".to_string(),
            limit: Some(10),
            request_id: 7,
        },
    )
    .expect("search empty query returns pinned");
    assert_eq!(search_empty.items.len(), 1);
    assert_eq!(search_empty.items[0].source, SearchSource::Recent);
    assert!(search_empty.items[0].subtitle.starts_with("[已固定] "));
    assert_eq!(PathBuf::from(&search_empty.items[0].abs_path), ext_file);

    // Toggle again unpins it.
    external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalPin,
            abs_path: Some(ext_file.to_string_lossy().to_string()),
            dir_path: None,
        },
    )
    .expect("unpin external file");
    assert!(pm.list_pinned_files().expect("list pinned").is_empty());

    let unpinned_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: ext_file.to_string_lossy().to_string(),
        },
    )
    .expect("read unpinned external file");
    assert!(!unpinned_doc.meta.pinned);
}
