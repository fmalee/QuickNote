use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use quicknote_lib::application::external::{
    directory_workspace_config_get, directory_workspace_config_write, external_action_run,
    external_actions_resolve, external_bookmark_add, external_bookmark_remove,
    external_bookmarks_list, external_list_directory, external_list_sibling_files,
    external_probe_path, external_search_roots, path_complete,
    resolve_effective_workspace_for_path,
};
use quicknote_lib::application::index::index_rebuild;
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{note_read, search_notes_and_files};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::bookmark::ExternalBookmarkKind;
use quicknote_lib::domain::error::{AppError, ErrorCode};
use quicknote_lib::domain::external::{
    ExternalActionId, ExternalActionState, ResolveExternalArgs, RunExternalArgs,
    SETTINGS_FILE_SENTINEL,
};
use quicknote_lib::domain::note::{PathCompleteArgs, ReadNoteArgs, SearchQueryArgs, SearchSource};
use quicknote_lib::domain::settings::{
    parse_settings_yaml, ExternalAppConfig, SetRootArgs, SettingsPatch,
};
use quicknote_lib::domain::workspace_context::DirectoryWorkspaceConfig;
use quicknote_lib::infra::external_open::ExternalLauncher;
use quicknote_lib::infra::fs_browser::{ListDirectoryOptions, PathEntryKind};

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

    let demo_ws_cfg = DirectoryWorkspaceConfig {
        vscode_workspace_path: Some(code_ws_file.to_string_lossy().to_string()),
        project_root: Some(ws_open.path().to_string_lossy().to_string()),
        default_terminal_cwd: Some(ws_term.path().to_string_lossy().to_string()),
        external_app_overrides: None,
    };
    fs::create_dir_all(lib.path().join("notes/demo")).expect("mkdir notes/demo");
    directory_workspace_config_write(&pm, "notes/demo", &demo_ws_cfg)
        .expect("write notes/demo .quicknote.yaml");

    // 1. Library doc without open_dir / terminal_dir / workspace -> strictly disabled, no fallback!
    let plain_note = lib.path().join("notes/plain.md");
    fs::create_dir_all(plain_note.parent().expect("parent")).expect("mkdir notes");
    fs::write(&plain_note, "---\ntitle: Plain\n---\nBody\n").expect("write plain");

    let plain_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(plain_note.to_string_lossy().to_string()),
            command_id: None,
        },
    )
    .expect("resolve plain");
    assert!(find_state(&plain_states, ExternalActionId::ExternalReveal).enabled);
    assert!(find_state(&plain_states, ExternalActionId::ExternalDefault).enabled);
    assert!(find_state(&plain_states, ExternalActionId::ExternalCopyPath).enabled);
    for id in [
        ExternalActionId::ExternalTerminal,
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
    let plain_vscode = find_state(&plain_states, ExternalActionId::ExternalVscode);
    assert!(
        plain_vscode.enabled,
        "expected ExternalVscode to fall back to doc parent dir for plain library doc"
    );
    assert_eq!(
        plain_vscode.resolved_path.as_deref(),
        plain_note.parent().and_then(|p| p.to_str())
    );

    // 2. Library doc under directory with `.quicknote.yaml` -> inherits workspace dirs.
    let proj_note = lib.path().join("notes/demo/proj.md");
    fs::write(&proj_note, "---\ntitle: Proj\nproject: demo\n---\nBody\n").expect("write proj");
    let proj_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(proj_note.to_string_lossy().to_string()),
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
        Some(code_ws_file.to_string_lossy().to_string())
    );
    assert_eq!(
        find_state(&proj_states, ExternalActionId::ExternalSublime).resolved_path,
        Some(ws_open.path().to_string_lossy().to_string())
    );
    assert_eq!(
        find_state(&proj_states, ExternalActionId::ExternalWorkspace).resolved_path,
        Some(code_ws_file.to_string_lossy().to_string())
    );
    assert!(!find_state(&proj_states, ExternalActionId::ExternalPin).enabled);

    // 3. Library doc own `terminal_dir` / `open_dir` overrides workspace config.
    let override_note = lib.path().join("notes/demo/override.md");
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
            command_id: None,
        },
    )
    .expect("resolve override");
    assert_eq!(
        find_state(&override_states, ExternalActionId::ExternalTerminal).resolved_path,
        Some(doc_term.path().to_string_lossy().to_string())
    );
    assert_eq!(
        find_state(&override_states, ExternalActionId::ExternalSublime).resolved_path,
        Some(doc_open.path().to_string_lossy().to_string())
    );
    assert_eq!(
        find_state(&override_states, ExternalActionId::ExternalVscode).resolved_path,
        Some(code_ws_file.to_string_lossy().to_string())
    );

    // 3b. When no `code_workspace` is configured, `ExternalVscode` falls back to `open_dir`.
    let dir_only_note = lib.path().join("open_dir_only.md");
    fs::write(
        &dir_only_note,
        format!(
            "---\ntitle: OpenDirOnly\nopen_dir: {}\n---\nBody\n",
            doc_open.path().display()
        ),
    )
    .expect("write open_dir_only");
    let dir_only_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(dir_only_note.to_string_lossy().to_string()),
            command_id: None,
        },
    )
    .expect("resolve open_dir_only");
    let dir_only_vscode = find_state(&dir_only_states, ExternalActionId::ExternalVscode);
    assert!(dir_only_vscode.enabled);
    assert_eq!(
        dir_only_vscode.resolved_path,
        Some(doc_open.path().to_string_lossy().to_string())
    );

    // 3c. Missing explicit `code_workspace` disables `ExternalVscode` with clear reason (no silent fallback).
    let missing_ws_note = lib.path().join("missing_ws.md");
    fs::write(
        &missing_ws_note,
        format!(
            "---\ntitle: MissingWs\ncode_workspace: /nonexistent/missing.code-workspace\nopen_dir: {}\n---\nBody\n",
            doc_open.path().display()
        ),
    )
    .expect("write missing_ws");
    let missing_ws_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(missing_ws_note.to_string_lossy().to_string()),
            command_id: None,
        },
    )
    .expect("resolve missing_ws");
    let missing_ws_vscode = find_state(&missing_ws_states, ExternalActionId::ExternalVscode);
    assert!(!missing_ws_vscode.enabled);
    assert!(missing_ws_vscode
        .disabled_reason
        .as_deref()
        .is_some_and(|r| r.contains(".code-workspace")));

    // 4. External file -> parent dir + external.pin enabled.
    let ext_file = ext_dir.path().join("nginx.conf");
    fs::write(&ext_file, "worker_processes 1;").expect("write ext_file");
    let ext_states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(ext_file.to_string_lossy().to_string()),
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
    let cmd_book_with_ws = lib.path().join("commands/demo/deploy.md");
    fs::create_dir_all(cmd_book_with_ws.parent().expect("parent")).expect("mkdir commands/demo");
    directory_workspace_config_write(&pm, "commands/demo", &demo_ws_cfg)
        .expect("write commands/demo .quicknote.yaml");
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
            command_id: Some("commands/demo/deploy.md:6:0".to_string()),
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
            command_id: Some("commands/demo/deploy.md:6:0".to_string()),
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

#[test]
fn three_tier_workspace_cascade_priority_and_external_actions() {
    let (_app, lib, pm) = setup_pm();

    let parent_open = TempDir::new().expect("parent_open");
    let parent_term = TempDir::new().expect("parent_term");
    let child_open = TempDir::new().expect("child_open");
    let doc_term = TempDir::new().expect("doc_term");

    let parent_ws_file = parent_open.path().join("parent.code-workspace");
    fs::write(&parent_ws_file, "{}").expect("write parent ws");

    // Tier 2: ancestor directory `.quicknote.yaml` files written via reconciler.
    fs::create_dir_all(lib.path().join("projects/app")).expect("mkdir projects/app");
    directory_workspace_config_write(
        &pm,
        "projects",
        &DirectoryWorkspaceConfig {
            vscode_workspace_path: Some(parent_ws_file.to_string_lossy().to_string()),
            project_root: Some(parent_open.path().to_string_lossy().to_string()),
            default_terminal_cwd: Some(parent_term.path().to_string_lossy().to_string()),
            external_app_overrides: None,
        },
    )
    .expect("write projects .quicknote.yaml");

    directory_workspace_config_write(
        &pm,
        "projects/app",
        &DirectoryWorkspaceConfig {
            vscode_workspace_path: None,
            project_root: Some(child_open.path().to_string_lossy().to_string()),
            default_terminal_cwd: None,
            external_app_overrides: None,
        },
    )
    .expect("write projects/app .quicknote.yaml");

    let loaded_child_cfg = directory_workspace_config_get(&pm, "projects/app")
        .expect("get child cfg")
        .expect("child cfg exists");
    assert_eq!(
        loaded_child_cfg.project_root.as_deref(),
        Some(child_open.path().to_string_lossy().as_ref())
    );

    // Tier 1 (highest): Note Frontmatter overrides terminal_dir only.
    let spec_note = lib.path().join("projects/app/spec.md");
    fs::write(
        &spec_note,
        format!(
            "---\ntitle: Spec\nproject: demo\nterminal_dir: {}\n---\nBody\n",
            doc_term.path().to_string_lossy()
        ),
    )
    .expect("write spec.md");

    let effective = resolve_effective_workspace_for_path(&pm, Some(&spec_note.to_string_lossy()))
        .expect("resolve effective workspace");
    assert_eq!(
        effective.default_terminal_cwd.as_deref(),
        Some(doc_term.path().to_string_lossy().as_ref()),
        "Frontmatter must beat ancestor .quicknote.yaml and global default"
    );
    assert_eq!(
        effective.project_root.as_deref(),
        Some(child_open.path().to_string_lossy().as_ref()),
        "Nearest ancestor .quicknote.yaml must beat farther ancestor and global default"
    );
    assert_eq!(
        effective.vscode_workspace_path.as_deref(),
        Some(parent_ws_file.to_string_lossy().as_ref()),
        "Farther ancestor .quicknote.yaml must beat global default"
    );

    // Verify `external_actions_resolve` reflects the exact 3-tier cascade.
    let states = external_actions_resolve(
        &pm,
        ResolveExternalArgs {
            abs_path: Some(spec_note.to_string_lossy().to_string()),
            command_id: None,
        },
    )
    .expect("resolve states");
    assert_eq!(
        find_state(&states, ExternalActionId::ExternalTerminal)
            .resolved_path
            .as_deref(),
        Some(doc_term.path().to_string_lossy().as_ref())
    );
    assert_eq!(
        find_state(&states, ExternalActionId::ExternalVscode)
            .resolved_path
            .as_deref(),
        Some(parent_ws_file.to_string_lossy().as_ref())
    );
    assert_eq!(
        find_state(&states, ExternalActionId::ExternalSublime)
            .resolved_path
            .as_deref(),
        Some(child_open.path().to_string_lossy().as_ref())
    );
    assert_eq!(
        find_state(&states, ExternalActionId::ExternalWorkspace)
            .resolved_path
            .as_deref(),
        Some(parent_ws_file.to_string_lossy().as_ref())
    );

    // Verify `external_action_run` without explicit `dir_path` resolves via 3-tier cascade.
    let launcher = RecordingLauncher::default();
    external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalTerminal,
            abs_path: Some(spec_note.to_string_lossy().to_string()),
            dir_path: None,
        },
    )
    .expect("run terminal via cascade");
    external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalVscode,
            abs_path: Some(spec_note.to_string_lossy().to_string()),
            dir_path: None,
        },
    )
    .expect("run vscode via cascade");
    external_action_run(
        &pm,
        &launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalWorkspace,
            abs_path: Some(spec_note.to_string_lossy().to_string()),
            dir_path: None,
        },
    )
    .expect("run workspace via cascade");

    let calls = launcher.calls();
    assert_eq!(
        calls[0],
        (
            "terminal:com.googlecode.iterm2".to_string(),
            doc_term.path().to_string_lossy().to_string()
        )
    );
    assert_eq!(
        calls[1],
        (
            "app:com.microsoft.VSCode".to_string(),
            parent_ws_file.to_string_lossy().to_string()
        )
    );
    assert_eq!(
        calls[2],
        (
            "app:com.microsoft.VSCode".to_string(),
            parent_ws_file.to_string_lossy().to_string()
        )
    );

    // Without Frontmatter dirs and without ancestor `.quicknote.yaml`, effective context is empty.
    fs::create_dir_all(lib.path().join("other")).expect("mkdir other");
    let plain_other_note = lib.path().join("other/plain_other.md");
    fs::write(
        &plain_other_note,
        "---\ntitle: PlainOther\nproject: demo\n---\nBody\n",
    )
    .expect("write plain_other.md");
    let empty_effective =
        resolve_effective_workspace_for_path(&pm, Some(&plain_other_note.to_string_lossy()))
            .expect("resolve empty context");
    assert_eq!(empty_effective.project_root, None);
    assert_eq!(empty_effective.default_terminal_cwd, None);
    assert_eq!(empty_effective.vscode_workspace_path, None);

    // `external_action_run` for `ExternalVscode` falls back to doc parent dir when neither `code_workspace` nor `open_dir` is set.
    let fallback_launcher = RecordingLauncher::default();
    external_action_run(
        &pm,
        &fallback_launcher,
        RunExternalArgs {
            id: ExternalActionId::ExternalVscode,
            abs_path: Some(plain_other_note.to_string_lossy().to_string()),
            dir_path: None,
        },
    )
    .expect("run vscode fallback to doc parent dir");
    assert_eq!(
        fallback_launcher.calls(),
        vec![(
            "app:com.microsoft.VSCode".to_string(),
            lib.path().join("other").to_string_lossy().to_string()
        )]
    );

    // Manually dropped root markdown + `.quicknote.yaml` reconciled via `index_rebuild`.
    let manual_dir = lib.path().join("manual_sub");
    fs::create_dir_all(&manual_dir).expect("mkdir manual_sub");
    fs::write(
        manual_dir.join(".quicknote.yaml"),
        format!("project_root: {}\n", parent_open.path().to_string_lossy()),
    )
    .expect("write manual .quicknote.yaml");
    fs::write(lib.path().join("root_drop.md"), "# Dropped in root\n").expect("write root_drop.md");
    let rebuild_status = index_rebuild(&pm, None).expect("index_rebuild via reconciler");
    assert!(rebuild_status.notes_count >= 1);
    let manual_cfg = directory_workspace_config_get(&pm, "manual_sub")
        .expect("get manual_sub cfg")
        .expect("manual_sub cfg indexed by index_rebuild");
    assert_eq!(
        manual_cfg.project_root.as_deref(),
        Some(parent_open.path().to_string_lossy().as_ref())
    );
}

#[test]
fn external_fs_browser_bookmarks_and_settings_v5_fields() {
    let (app_dir, _lib, pm) = setup_pm();

    // 1. `max_active_editor_renders` defaults to 5 and validates 2..=20.
    let initial = pm.settings_get().expect("settings_get").settings;
    assert_eq!(initial.max_active_editor_renders, 5);
    assert!(initial.external_bookmarks.is_empty());

    let err_low = pm
        .settings_update(SettingsPatch {
            max_active_editor_renders: Some(1),
            ..Default::default()
        })
        .expect_err("1 must fail validation");
    assert_eq!(err_low.code, ErrorCode::ValidationError);

    let err_high = pm
        .settings_update(SettingsPatch {
            max_active_editor_renders: Some(21),
            ..Default::default()
        })
        .expect_err("21 must fail validation");
    assert_eq!(err_high.code, ErrorCode::ValidationError);

    let updated = pm
        .settings_update(SettingsPatch {
            max_active_editor_renders: Some(8),
            ..Default::default()
        })
        .expect("8 must succeed");
    assert_eq!(updated.settings.max_active_editor_renders, 8);

    // 2. Legacy settings.yaml with `workspaces` deserializes without error and applies defaults.
    let legacy_yaml = r#"
workspaces:
  legacy:
    name: Legacy
    key: legacy
    root: /tmp
    open_dir: /tmp
"#;
    let (parsed_legacy, status) = parse_settings_yaml(legacy_yaml, None);
    assert!(status.valid);
    assert_eq!(parsed_legacy.max_active_editor_renders, 5);
    assert!(parsed_legacy.external_bookmarks.is_empty());

    // 3. External fs browser: probe, directory listing (including hidden), siblings, and search_roots.
    let ext_root = TempDir::new().expect("ext_root");
    let hidden_dir = ext_root.path().join(".hidden_cfg");
    fs::create_dir_all(&hidden_dir).expect("mkdir .hidden_cfg");
    let cfg_file = hidden_dir.join("app.conf");
    let sibling_file = hidden_dir.join("sibling.toml");
    fs::write(&cfg_file, "port=8080\n").expect("write app.conf");
    fs::write(&sibling_file, "name = 'demo'\n").expect("write sibling.toml");

    let probe = external_probe_path(&cfg_file.to_string_lossy());
    assert!(probe.exists);
    assert_eq!(probe.kind, PathEntryKind::File);
    assert!(probe.is_text);

    let listing = external_list_directory(
        &ext_root.path().to_string_lossy(),
        Some(ListDirectoryOptions {
            max_depth: 2,
            include_hidden: true,
            query: Some("app.conf".to_string()),
            max_entries: 50,
        }),
    )
    .expect("list directory");
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.entries[0].name, "app.conf");

    let siblings =
        external_list_sibling_files(&cfg_file.to_string_lossy(), Some(1)).expect("list siblings");
    let sibling_names: Vec<&str> = siblings.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(sibling_names.contains(&"app.conf"));
    assert!(sibling_names.contains(&"sibling.toml"));

    // 4. External bookmarks persist in `settings.yaml` (independent of SQLite) and power `external_search_roots`.
    let added = external_bookmark_add(
        &pm,
        &ext_root.path().to_string_lossy(),
        Some("Ext Config".to_string()),
        true,
    )
    .expect("add bookmark");
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].kind, ExternalBookmarkKind::Directory);
    assert_eq!(added[0].alias.as_deref(), Some("Ext Config"));
    assert!(added[0].pinned);

    let settings_raw = fs::read_to_string(pm.settings_path()).expect("read settings.yaml");
    assert!(settings_raw.contains("external_bookmarks"));

    // Reload a fresh PersistenceManager from the same config_dir to prove non-SQLite persistence.
    let pm_reloaded = PersistenceManager::open_with_dirs(
        app_dir.path().to_path_buf(),
        app_dir.path().join("cache_reloaded"),
    )
    .expect("reload pm");
    let reloaded_bookmarks =
        external_bookmarks_list(&pm_reloaded).expect("list reloaded bookmarks");
    assert_eq!(reloaded_bookmarks.len(), 1);
    assert_eq!(reloaded_bookmarks[0].alias.as_deref(), Some("Ext Config"));

    let search_hits = external_search_roots(&pm_reloaded, "sibling.toml", Some(3), Some(20))
        .expect("external_search_roots");
    assert!(
        search_hits.iter().any(|e| e.name == "sibling.toml"),
        "expected pure-Rust external_search_roots to find hidden dir file via bookmark"
    );

    let after_remove =
        external_bookmark_remove(&pm, &ext_root.path().to_string_lossy()).expect("remove bookmark");
    assert!(after_remove.is_empty());
}
