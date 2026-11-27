use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use quicknote_lib::application::capture::{CaptureManager, MAX_UNDO_BODY_BYTES};
use quicknote_lib::application::index::index_rebuild;
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{note_read, note_update_meta, note_write};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::capture::{append_to_body, AppendNoteArgs};
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::frontmatter::split_frontmatter;
use quicknote_lib::domain::note::{NoteMetaPatch, NoteUpdateMetaArgs, ReadNoteArgs, WriteNoteArgs};
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::domain::ui_state::CaptureMode;

static TEST_DIR_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let seq = TEST_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "quicknote_capture_test_{}_{}",
            std::process::id(),
            seq
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
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

fn setup_env_with_fixtures() -> (TempDir, TempDir, PersistenceManager, CaptureManager) {
    let app_data = TempDir::new();
    let vault = TempDir::new();

    let fixtures_vault = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("vault");

    for file in ["inbox.md", "missing-id.md", "code-fence-heading.md"] {
        let src = fixtures_vault.join(file);
        if src.is_file() {
            fs::copy(&src, vault.path().join(file)).expect("copy fixture file");
        }
    }

    let pm = PersistenceManager::open(app_data.path().to_path_buf()).expect("open persistence");
    set_library_root(
        &pm,
        SetRootArgs {
            root: vault.path().to_string_lossy().to_string(),
            create_if_missing: false,
        },
    )
    .expect("set library root");
    index_rebuild(&pm, None).expect("rebuild index");

    let cm = CaptureManager::new();
    (app_data, vault, pm, cm)
}

#[test]
fn append_to_body_handles_prefixes_code_fences_and_inbox_dates() {
    // 1. Prefix deduplication for List + inbox timestamp
    let out_inbox = append_to_body(
        "## 2026-11-23\n- 09:00 昨日条目\n",
        "- 已有列表前缀\n普通文本\n- 14:20 已带时间戳",
        CaptureMode::List,
        true,
        "2026-11-24",
        "14:30",
        false,
    )
    .expect("append inbox");
    assert_eq!(out_inbox.section_heading.as_deref(), Some("## 2026-11-24"));
    assert_eq!(out_inbox.added_lines, 3);
    assert!(out_inbox
        .new_body
        .contains("## 2026-11-24\n- 14:30 已有列表前缀\n- 14:30 普通文本\n- 14:20 已带时间戳"));

    // 2. Prefix deduplication for Todo
    let out_todo = append_to_body(
        "## 待办\n- [ ] 旧任务\n\n## 其他\n内容\n",
        "- 普通转待办\n- [x] 已完成保留\n新任务",
        CaptureMode::Todo,
        false,
        "2026-11-24",
        "14:30",
        false,
    )
    .expect("append todo");
    assert_eq!(out_todo.section_heading.as_deref(), Some("## 待办"));
    assert!(out_todo.new_body.contains(
        "## 待办\n- [ ] 旧任务\n- [ ] 普通转待办\n- [x] 已完成保留\n- [ ] 新任务\n\n## 其他"
    ));

    // 3. Prose mode preserves paragraphs without adding list markers
    let out_prose = append_to_body(
        "# 笔记\n\n第一段\n",
        "第二段\n第三行",
        CaptureMode::Prose,
        false,
        "2026-11-24",
        "14:30",
        false,
    )
    .expect("append prose");
    assert_eq!(out_prose.section_heading, None);
    assert!(out_prose.new_body.ends_with("第一段\n\n第二段\n第三行\n"));

    // 4. Ignore fake headings inside fenced code blocks (``` and ~~~)
    let fenced_doc = "# 标题\n\n```markdown\n## 想法\n这是代码块里的假标题\n```\n\n~~~md\n## 想法\n波浪围栏假标题\n~~~\n\n## 想法\n- 真实条目\n\n## 总结\n结束\n";
    let out_fenced = append_to_body(
        fenced_doc,
        "新想法",
        CaptureMode::List,
        false,
        "2026-11-24",
        "14:30",
        false,
    )
    .expect("append fenced");
    assert_eq!(out_fenced.section_heading.as_deref(), Some("## 想法"));
    assert!(out_fenced.new_body.contains("这是代码块里的假标题\n```\n"));
    assert!(out_fenced
        .new_body
        .contains("- 真实条目\n- 新想法\n\n## 总结"));
}

#[test]
fn append_to_code_fence_heading_fixture_skips_fenced_headings() {
    let (_app_data, vault, pm, cm) = setup_env_with_fixtures();
    let target = vault.path().join("code-fence-heading.md");
    if !target.is_file() {
        return;
    }

    let before_raw = fs::read_to_string(&target).expect("read fixture");
    let before_split = split_frontmatter(&before_raw);

    let res = cm
        .note_append(
            &pm,
            AppendNoteArgs {
                target_rel_path: "code-fence-heading.md".to_string(),
                content: "围栏外真实追加".to_string(),
                mode: CaptureMode::List,
            },
            None,
        )
        .expect("append code-fence-heading.md");
    assert_eq!(res.added_lines, 1);

    let after_raw = fs::read_to_string(&target).expect("read after");
    let after_split = split_frontmatter(&after_raw);
    assert_eq!(after_split.block, before_split.block);
    assert!(after_split.body.contains("围栏外真实追加"));
}

#[test]
fn note_append_preserves_missing_id_frontmatter_and_supports_undo() {
    let (_app_data, vault, pm, cm) = setup_env_with_fixtures();
    let missing_id_path = vault.path().join("missing-id.md");
    let before_raw = fs::read_to_string(&missing_id_path).expect("read missing-id.md");
    let before_block = split_frontmatter(&before_raw).block.map(str::to_string);

    let res = cm
        .note_append(
            &pm,
            AppendNoteArgs {
                target_rel_path: "missing-id.md".to_string(),
                content: "测试不改写 frontmatter".to_string(),
                mode: CaptureMode::List,
            },
            None,
        )
        .expect("append missing-id.md");
    assert!(res.undo_available);

    let after_raw = fs::read_to_string(&missing_id_path).expect("read after append");
    let after_block = split_frontmatter(&after_raw).block.map(str::to_string);
    // D-0006: note_append MUST NOT modify or back-fill YAML frontmatter
    assert_eq!(after_block, before_block);
    assert!(after_raw.contains("测试不改写 frontmatter"));

    let undo_res = cm
        .note_undo_last_append(&pm, None)
        .expect("undo missing-id append");
    assert_eq!(undo_res.rel_path, "missing-id.md");

    let restored_raw = fs::read_to_string(&missing_id_path).expect("read restored");
    assert_eq!(restored_raw, before_raw);
}

#[test]
fn inbox_append_creates_today_heading_and_undo_reverts_entire_heading() {
    let (_app_data, vault, pm, cm) = setup_env_with_fixtures();
    let inbox_path = vault.path().join("inbox.md");
    let before_raw = fs::read_to_string(&inbox_path).expect("read inbox.md");
    let today = Local::now().format("%Y-%m-%d").to_string();
    let expected_heading = format!("## {today}");

    let res = cm
        .note_append(
            &pm,
            AppendNoteArgs {
                target_rel_path: "inbox.md".to_string(),
                content: "今日第一条速记".to_string(),
                mode: CaptureMode::List,
            },
            None,
        )
        .expect("append inbox.md");
    assert_eq!(
        res.section_heading.as_deref(),
        Some(expected_heading.as_str())
    );
    assert!(res.undo_available);

    let after_raw = fs::read_to_string(&inbox_path).expect("read inbox after append");
    assert!(after_raw.contains(&expected_heading));
    assert!(after_raw.contains("今日第一条速记"));

    // Undo reverts both the entry and the newly created today heading
    cm.note_undo_last_append(&pm, None)
        .expect("undo inbox append");
    let restored_raw = fs::read_to_string(&inbox_path).expect("read inbox after undo");
    assert_eq!(restored_raw, before_raw);
}

#[test]
fn undo_rejects_modified_body_but_succeeds_after_meta_only_update() {
    let (_app_data, vault, pm, cm) = setup_env_with_fixtures();

    // Case A: Body modified externally -> UndoUnavailable
    let note_a = vault.path().join("note-a.md");
    fs::write(
        &note_a,
        "---\ntitle: Note A\n---\n# Note A\n\n## 想法\n- 初始\n",
    )
    .expect("write note-a");
    index_rebuild(&pm, None).expect("rebuild");

    let append_a = cm
        .note_append(
            &pm,
            AppendNoteArgs {
                target_rel_path: "note-a.md".to_string(),
                content: "待撤回条目".to_string(),
                mode: CaptureMode::List,
            },
            None,
        )
        .expect("append note-a");

    let read_a = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: note_a.to_string_lossy().to_string(),
        },
    )
    .expect("read note-a");

    note_write(
        &pm,
        WriteNoteArgs {
            abs_path: note_a.to_string_lossy().to_string(),
            body: format!("{}\n- 外部编辑\n", read_a.body.trim_end()),
            base_body_hash: append_a.body_hash,
        },
    )
    .expect("external write note-a");

    let err = cm
        .note_undo_last_append(&pm, None)
        .expect_err("undo should fail when body changed");
    assert_eq!(err.code, ErrorCode::UndoUnavailable);

    // Case B: Only frontmatter updated via note_update_meta -> undo succeeds and keeps new meta
    let note_b = vault.path().join("note-b.md");
    fs::write(
        &note_b,
        "---\ntitle: Note B\ntags: []\n---\n# Note B\n\n## 想法\n- 初始B\n",
    )
    .expect("write note-b");
    index_rebuild(&pm, None).expect("rebuild");

    cm.note_append(
        &pm,
        AppendNoteArgs {
            target_rel_path: "note-b.md".to_string(),
            content: "追加B".to_string(),
            mode: CaptureMode::List,
        },
        None,
    )
    .expect("append note-b");

    note_update_meta(
        &pm,
        NoteUpdateMetaArgs {
            abs_path: note_b.to_string_lossy().to_string(),
            patch: NoteMetaPatch {
                doc_type: None,
                tags: Some(vec!["updated".to_string()]),
                pinned: Some(true),
                archived: None,
                open_dir: None,
                code_workspace: None,
                terminal_dir: None,
            },
        },
    )
    .expect("update meta only");

    cm.note_undo_last_append(&pm, None)
        .expect("undo should succeed when only meta changed");

    let after_undo_b = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: note_b.to_string_lossy().to_string(),
        },
    )
    .expect("read note-b after undo");
    assert_eq!(after_undo_b.meta.tags, vec!["updated".to_string()]);
    assert!(after_undo_b.meta.pinned);
    assert!(!after_undo_b.body.contains("追加B"));
    assert!(after_undo_b.body.contains("- 初始B"));
}

#[test]
fn large_document_over_5mb_disables_undo_snapshot() {
    let (_app_data, vault, pm, cm) = setup_env_with_fixtures();
    let large_path = vault.path().join("large.md");
    let big_body = "x".repeat(MAX_UNDO_BODY_BYTES + 1024);
    fs::write(&large_path, format!("---\ntitle: Large\n---\n{big_body}\n")).expect("write large");
    index_rebuild(&pm, None).expect("rebuild");

    let res = cm
        .note_append(
            &pm,
            AppendNoteArgs {
                target_rel_path: "large.md".to_string(),
                content: "超大文档追加".to_string(),
                mode: CaptureMode::List,
            },
            None,
        )
        .expect("append large.md");
    assert!(!res.undo_available);

    let err = cm
        .note_undo_last_append(&pm, None)
        .expect_err("undo should be unavailable for >5MB doc");
    assert_eq!(err.code, ErrorCode::UndoUnavailable);
}
