use std::fs;
use std::path::PathBuf;

use quicknote_lib::application::capture::inbox_extract_to_note;
use quicknote_lib::application::index::index_rebuild;
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{
    note_create, note_read, note_save_image, note_update_meta, notes_get_tree_and_tags, notes_list,
    MAX_IMAGE_BYTES,
};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::app::{normalize_combo_to_tauri_accelerator, validate_external_url};
use quicknote_lib::domain::capture::InboxExtractArgs;
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::note::{
    compute_body_hash, parse_note, CreateNoteArgs, NoteMetaPatch, NoteType, NoteUpdateMetaArgs,
    NotesFilterArgs, ReadNoteArgs, SaveImageArgs,
};
use quicknote_lib::domain::settings::{
    EditorSettingsPatch, InboxSettingsPatch, SetRootArgs, SettingsPatch,
};
use quicknote_lib::domain::template::{render_template, TemplateVars, DEFAULT_TEMPLATE_NOTE};
use quicknote_lib::infra::fs::ensure_library_skeleton;

static TEST_DIR_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let seq = TEST_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("quicknote_m2_test_{}_{}", std::process::id(), seq));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn setup_pm_with_library() -> (TempDir, PersistenceManager, PathBuf) {
    let dir = TempDir::new();
    let app_data = dir.path.join("app_data");
    let library = dir.path.join("vault");
    let pm = PersistenceManager::open(app_data).expect("open PersistenceManager");
    set_library_root(
        &pm,
        SetRootArgs {
            root: library.to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect("set library root");
    index_rebuild(&pm, None).expect("rebuild index");
    (dir, pm, library)
}

#[test]
fn settings_update_merges_persists_validates_and_protects_broken_yaml() {
    let dir = TempDir::new();
    let app_data = dir.path.join("app_data");
    let pm = PersistenceManager::open(app_data.clone()).expect("open pm");

    let updated = pm
        .settings_update(SettingsPatch {
            editor: Some(EditorSettingsPatch {
                font_size: Some(18),
                line_height: Some(1.8),
                ..Default::default()
            }),
            inbox: Some(InboxSettingsPatch {
                bloat_threshold_lines: Some(300),
            }),
            ..Default::default()
        })
        .expect("settings_update should succeed");

    assert!(updated.status.valid);
    assert_eq!(updated.status.active_source, "file");
    assert_eq!(updated.settings.editor.font_size, 18);
    assert_eq!(updated.settings.editor.line_height, 1.8);
    assert_eq!(updated.settings.inbox.bloat_threshold_lines, 300);

    // Re-open PersistenceManager and confirm disk persistence.
    let reopened = PersistenceManager::open(app_data.clone()).expect("reopen pm");
    let snap = reopened.settings_get().expect("settings_get");
    assert_eq!(snap.settings.editor.font_size, 18);
    assert_eq!(snap.settings.inbox.bloat_threshold_lines, 300);

    // Out-of-bounds validations.
    let err_font = reopened
        .settings_update(SettingsPatch {
            editor: Some(EditorSettingsPatch {
                font_size: Some(4),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect_err("font_size=4 must fail");
    assert_eq!(err_font.code, ErrorCode::ValidationError);

    let err_bloat = reopened
        .settings_update(SettingsPatch {
            inbox: Some(InboxSettingsPatch {
                bloat_threshold_lines: Some(2),
            }),
            ..Default::default()
        })
        .expect_err("bloat_threshold_lines=2 must fail");
    assert_eq!(err_bloat.code, ErrorCode::ValidationError);

    let err_wait = reopened
        .settings_update(SettingsPatch {
            editor: Some(EditorSettingsPatch {
                auto_save_debounce_ms: Some(2000),
                auto_save_max_wait_ms: Some(1000),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect_err("max_wait < debounce must fail");
    assert_eq!(err_wait.code, ErrorCode::ValidationError);

    // D-0008 read-only protection when settings.yaml is broken.
    let broken_yaml = "editor:\n  font_size: [broken\n";
    fs::write(app_data.join("settings.yaml"), broken_yaml).expect("write broken yaml");
    let reload_snap = reopened.reload_settings().expect("reload broken yaml");
    assert!(!reload_snap.status.valid);

    let err_broken = reopened
        .settings_update(SettingsPatch {
            editor: Some(EditorSettingsPatch {
                font_size: Some(16),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect_err("settings_update must refuse when yaml is broken");
    assert_eq!(err_broken.code, ErrorCode::SettingsParseError);

    let disk_after = fs::read_to_string(app_data.join("settings.yaml")).expect("read disk");
    assert_eq!(disk_after, broken_yaml);
}

#[test]
fn templates_b3_alignment_render_and_note_create_isolation() {
    let (_dir, pm, library) = setup_pm_with_library();

    // Verify B.3 skeleton files.
    ensure_library_skeleton(&library).expect("ensure skeleton");
    let note_tpl = fs::read_to_string(library.join("_templates/note.md")).expect("read note.md");
    assert!(note_tpl.contains("pinned: false"));
    assert!(note_tpl.contains("archived: false"));

    let doc_tpl = fs::read_to_string(library.join("_templates/doc.md")).expect("read doc.md");
    assert!(!doc_tpl.contains("title:"));
    assert!(!doc_tpl.contains("project:"));
    assert!(doc_tpl.contains("open_dir: \"\""));
    assert!(doc_tpl.contains("## 想法"));

    let cmd_tpl =
        fs::read_to_string(library.join("_templates/command-book.md")).expect("read command-book");
    assert!(cmd_tpl.contains("## 常用"));
    assert!(cmd_tpl.contains("<!-- keywords: 示例 demo -->"));

    // Verify render_template with colon in title produces valid YAML.
    let rendered = render_template(
        DEFAULT_TEMPLATE_NOTE,
        &TemplateVars {
            id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            title: "项目: v2",
            date: "2026-11-24",
            time: "04:30:00",
        },
    );
    let parsed = parse_note(&rendered, "fallback");
    assert!(
        parsed.meta.meta_error.is_none(),
        "meta_error: {:?}",
        parsed.meta.meta_error
    );
    assert_eq!(parsed.meta.title, "项目: v2");
    assert!(parsed.body.contains("# 项目: v2"));

    // Custom template + extra body in note_create.
    let custom_tpl = "---\nid: {{id}}\ntitle: {{title}}\ntype: doc\ntags:\n  - meeting\ncreated: \"{{date}}T{{time}}\"\n---\n# {{title}}\n\n## 议程\n";
    fs::write(library.join("_templates/meeting.md"), custom_tpl).expect("write meeting.md");
    index_rebuild(&pm, None).expect("rebuild index");

    let created = note_create(
        &pm,
        CreateNoteArgs {
            title: "周会: 架构评审".to_string(),
            dir_rel_path: "projects".to_string(),
            template_name: Some("meeting".to_string()),
            body: Some("- 讨论 M2 进度".to_string()),
        },
    )
    .expect("note_create with custom template");

    assert_eq!(created.meta.doc_type, NoteType::Doc);
    assert_eq!(created.meta.tags, vec!["meeting"]);
    assert!(created.body.contains("## 议程"));
    assert!(created.body.contains("- 讨论 M2 进度"));

    // notes_list(tab="templates") returns templates, while tab="recent" excludes them.
    let templates = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "templates".to_string(),
            dir_rel_path: None,
            tags: vec![],
            filter_text: None,
        },
    )
    .expect("list templates");
    assert!(templates
        .iter()
        .any(|t| t.rel_path.as_deref() == Some("_templates/note.md")));
    assert!(templates
        .iter()
        .any(|t| t.rel_path.as_deref() == Some("_templates/meeting.md")));
    assert!(templates.iter().all(|t| t.meta_error.is_none()));

    let recent = notes_list(
        &pm,
        NotesFilterArgs {
            tab: "recent".to_string(),
            dir_rel_path: None,
            tags: vec![],
            filter_text: None,
        },
    )
    .expect("list recent");
    assert!(!recent.iter().any(|n| n
        .rel_path
        .as_deref()
        .unwrap_or("")
        .starts_with("_templates/")));
}

#[test]
fn note_update_meta_handles_no_frontmatter_without_backup_and_preserves_unknown_keys() {
    let (dir, pm, library) = setup_pm_with_library();

    // 1. External file with NO frontmatter.
    let external_path = dir.path.join("external_plain.md");
    fs::write(&external_path, "# 外部笔记\n\n正文内容。\n").expect("write external file");

    let updated_ext = note_update_meta(
        &pm,
        NoteUpdateMetaArgs {
            abs_path: external_path.to_string_lossy().to_string(),
            patch: NoteMetaPatch {
                open_dir: Some(Some("/tmp/alpha".to_string())),
                code_workspace: Some(Some("/tmp/alpha.code-workspace".to_string())),
                terminal_dir: Some(Some("/tmp/alpha/term".to_string())),
                pinned: Some(true),
                ..Default::default()
            },
        },
    )
    .expect("update external file meta");

    assert_eq!(updated_ext.meta.open_dir.as_deref(), Some("/tmp/alpha"));
    assert_eq!(
        updated_ext.meta.code_workspace.as_deref(),
        Some("/tmp/alpha.code-workspace")
    );
    assert_eq!(
        updated_ext.meta.terminal_dir.as_deref(),
        Some("/tmp/alpha/term")
    );
    assert!(updated_ext.meta.pinned);

    // Must NOT have created a backup file for a plain markdown file without frontmatter.
    let backup_entries: Vec<_> = fs::read_dir(pm.backups_dir())
        .expect("read backups dir")
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        backup_entries.is_empty(),
        "unexpected backup files created: {:?}",
        backup_entries
    );

    // 2. Library note with unknown keys and YAML comments.
    let custom_note_path = library.join("custom.md");
    let raw_custom = "---\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\ntitle: 原标题\n# keep this comment\ncustom_owner: joey\ntype: note\ntags: []\npinned: false\n---\n# 原标题\n";
    fs::write(&custom_note_path, raw_custom).expect("write custom.md");

    let updated_lib = note_update_meta(
        &pm,
        NoteUpdateMetaArgs {
            abs_path: custom_note_path.to_string_lossy().to_string(),
            patch: NoteMetaPatch {
                pinned: Some(true),
                ..Default::default()
            },
        },
    )
    .expect("update custom.md meta");
    assert!(updated_lib.meta.pinned);

    let after_raw = fs::read_to_string(&custom_note_path).expect("read custom.md");
    assert!(after_raw.contains("# keep this comment"));
    assert!(after_raw.contains("custom_owner: joey"));
}

#[test]
fn note_save_image_supports_base64_and_source_path_and_validates_inputs() {
    let (dir, pm, library) = setup_pm_with_library();

    // 1x1 PNG bytes: [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    let sample_bytes: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x01, 0x02];
    // Base64 of [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x01, 0x02] = "iVBORw0KGgoBAg=="
    let res_b64 = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: None,
            bytes_base64: Some("data:image/png;base64,iVBORw0KGgoBAg==".to_string()),
            ext: "PNG".to_string(),
            context_rel_path: None,
        },
    )
    .expect("save base64 image");
    assert!(res_b64.rel_path.starts_with("assets/"));
    assert!(res_b64.rel_path.ends_with(".png"));
    let saved_b64 = fs::read(library.join(&res_b64.rel_path)).expect("read saved b64 image");
    assert_eq!(saved_b64, sample_bytes);

    // From source_path.
    let src_file = dir.path.join("input.png");
    fs::write(&src_file, &sample_bytes).expect("write source png");
    let res_src = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: Some(src_file.to_string_lossy().to_string()),
            bytes_base64: None,
            ext: ".png".to_string(),
            context_rel_path: None,
        },
    )
    .expect("save source_path image");
    let saved_src = fs::read(library.join(&res_src.rel_path)).expect("read saved src image");
    assert_eq!(saved_src, sample_bytes);

    // Invalid extension.
    let err_ext = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: None,
            bytes_base64: Some("iVBORw0KGgoBAg==".to_string()),
            ext: "exe".to_string(),
            context_rel_path: None,
        },
    )
    .expect_err("exe extension must fail");
    assert_eq!(err_ext.code, ErrorCode::ValidationError);

    // Oversized base64 payload (> 20MB).
    let oversized_b64 = "A".repeat(((MAX_IMAGE_BYTES + 1024) / 3) * 4);
    let err_size = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: None,
            bytes_base64: Some(oversized_b64),
            ext: "png".to_string(),
            context_rel_path: None,
        },
    )
    .expect_err("oversized payload must fail");
    assert_eq!(err_size.code, ErrorCode::ValidationError);
}

#[test]
fn inbox_extract_to_note_moves_lines_safely_and_preserves_inbox_on_error() {
    let (_dir, pm, library) = setup_pm_with_library();

    let inbox_path = library.join("inbox.md");
    let initial_inbox = "---\ntitle: 收件箱\ntype: note\ntags: []\n---\n# 收件箱\n\n第一条想法\n第二条想法\n\n第三条保留\n";
    fs::write(&inbox_path, initial_inbox).expect("write inbox.md");

    // Extract lines 3..=4 ("第一条想法\n第二条想法") into projects/拆分笔记.md.
    let created = inbox_extract_to_note(
        &pm,
        InboxExtractArgs {
            from_line: 3,
            to_line: 4,
            new_title: "拆分笔记".to_string(),
            target_dir_rel_path: "projects".to_string(),
            template_name: "doc".to_string(),
        },
    )
    .expect("extract to note");

    assert_eq!(created.meta.doc_type, NoteType::Doc);
    assert!(created.body.contains("第一条想法"));
    assert!(created.body.contains("第二条想法"));
    assert!(created.body.contains("## 想法"));

    let inbox_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: inbox_path.to_string_lossy().to_string(),
        },
    )
    .expect("read inbox.md");
    assert_eq!(inbox_doc.meta.title, "收件箱");
    assert!(!inbox_doc.body.contains("第一条想法"));
    assert!(!inbox_doc.body.contains("第二条想法"));
    assert!(inbox_doc.body.contains("第三条保留"));

    // Verify self-write guard was recorded for inbox.md.
    assert!(pm.should_suppress_external_modify(&inbox_path, &compute_body_hash(&inbox_doc.body)));

    // Failure case: duplicate target title must leave inbox.md untouched.
    let inbox_before_fail = fs::read_to_string(&inbox_path).expect("read inbox before fail");
    let err_dup = inbox_extract_to_note(
        &pm,
        InboxExtractArgs {
            from_line: 1,
            to_line: 1,
            new_title: "拆分笔记".to_string(),
            target_dir_rel_path: "projects".to_string(),
            template_name: "doc".to_string(),
        },
    )
    .expect_err("duplicate target note must fail");
    assert_eq!(err_dup.code, ErrorCode::ValidationError);

    let inbox_after_fail = fs::read_to_string(&inbox_path).expect("read inbox after fail");
    assert_eq!(inbox_after_fail, inbox_before_fail);
}

#[test]
fn notes_get_tree_and_tags_excludes_template_tags() {
    let (_dir, pm, library) = setup_pm_with_library();

    fs::write(
        library.join("_templates/tagged-tpl.md"),
        "---\nid: {{id}}\ntitle: 模板\ntype: template\ntags:\n  - template-only-tag\n  - nested/tpl-tag\n---\n# 模板\n",
    )
    .expect("write tagged template");
    fs::create_dir_all(library.join("notes")).expect("create notes dir");
    fs::write(
        library.join("notes/regular.md"),
        "---\nid: 01JREGULARNOTE000000000001\ntitle: 普通笔记\ntype: note\ntags:\n  - work/review\n---\n正文\n",
    )
    .expect("write regular note");

    index_rebuild(&pm, None).expect("rebuild index");

    let tree = notes_get_tree_and_tags(&pm).expect("notes_get_tree_and_tags");
    let root_tags: Vec<&str> = tree.tags.iter().map(|t| t.tag.as_str()).collect();
    assert!(root_tags.contains(&"work"));
    assert!(!root_tags.contains(&"template-only-tag"));
    assert!(!root_tags.contains(&"nested"));

    let work_node = tree
        .tags
        .iter()
        .find(|t| t.tag == "work")
        .expect("work tag node");
    assert_eq!(work_node.children.len(), 1);
    assert_eq!(work_node.children[0].tag, "work/review");
}

#[test]
fn validate_external_url_allows_safe_schemes_and_rejects_dangerous_inputs() {
    assert_eq!(
        validate_external_url("  https://example.com/docs?a=1  ").expect("https"),
        "https://example.com/docs?a=1"
    );
    assert_eq!(
        validate_external_url("HTTP://localhost:3000").expect("http"),
        "HTTP://localhost:3000"
    );
    assert_eq!(
        validate_external_url("mailto:user@example.com").expect("mailto"),
        "mailto:user@example.com"
    );

    for bad in [
        "",
        "   ",
        "-a",
        "-https://example.com",
        "https://example.com\nopen",
        "https://example.com\r\n",
        "https://example.com\0",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/html,<script>",
    ] {
        let err = validate_external_url(bad).expect_err("must reject unsafe external url");
        assert_eq!(err.code, ErrorCode::ValidationError);
    }
}

#[test]
fn normalize_combo_to_tauri_accelerator_maps_standard_and_modifier_combos() {
    assert_eq!(
        normalize_combo_to_tauri_accelerator("cmd+b").as_deref(),
        Some("CmdOrCtrl+B")
    );
    assert_eq!(
        normalize_combo_to_tauri_accelerator("alt+cmd+c").as_deref(),
        Some("Alt+CmdOrCtrl+C")
    );
    assert_eq!(
        normalize_combo_to_tauri_accelerator("alt+cmd+1").as_deref(),
        Some("Alt+CmdOrCtrl+1")
    );
    assert_eq!(
        normalize_combo_to_tauri_accelerator("ctrl+cmd+o").as_deref(),
        Some("Ctrl+CmdOrCtrl+O")
    );
    assert_eq!(
        normalize_combo_to_tauri_accelerator("shift+cmd+t").as_deref(),
        Some("Shift+CmdOrCtrl+T")
    );
    assert_eq!(
        normalize_combo_to_tauri_accelerator("ctrl+alt+shift+cmd+space").as_deref(),
        Some("Ctrl+Alt+Shift+CmdOrCtrl+Space")
    );
    assert_eq!(normalize_combo_to_tauri_accelerator(""), None);
    assert_eq!(normalize_combo_to_tauri_accelerator("cmd"), None);
    assert_eq!(normalize_combo_to_tauri_accelerator("cmd+a+b"), None);
}
