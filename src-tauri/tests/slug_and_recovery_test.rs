use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use quicknote_lib::application::capture::CaptureManager;
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{note_create, note_write};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::capture::AppendNoteArgs;
use quicknote_lib::domain::markdown::{extract_headings, heading_to_slug};
use quicknote_lib::domain::note::{parse_note, CreateNoteArgs, WriteNoteArgs, INBOX_REL_PATH};
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::domain::ui_state::CaptureMode;
use quicknote_lib::domain::watch::{QuickBarDraft, RecoverySnapshot};

static TEST_DIR_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Result<Self, std::io::Error> {
        let seq = TEST_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "quicknote_slug_recovery_test_{}_{}",
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

#[test]
fn cn_headings_anchors_fixture_and_code_block_skipping() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/vault/cn-headings-anchors.md");
    let raw = fs::read_to_string(&fixture_path)?;
    let parsed = parse_note(&raw, "cn-headings-anchors");

    let headings = extract_headings(&parsed.body);
    let slugs: Vec<(&str, u8, &str)> = headings
        .iter()
        .map(|h| (h.text.as_str(), h.level, h.slug.as_str()))
        .collect();

    assert_eq!(
        slugs,
        vec![
            ("中文标题与 Anchor 混排", 1, "中文标题与-anchor-混排"),
            ("安装 Install 指南", 2, "安装-install-指南"),
            ("安装 Install 指南", 2, "安装-install-指南-1"),
            ("API：接口说明（v2）", 2, "api接口说明v2"),
            ("搜索中文分词", 3, "搜索中文分词"),
        ]
    );

    // Verify fenced code blocks and inline markdown stripping.
    let sample = "\
# **粗体** 与 [链接文本](https://example.com)

```bash
# 这是注释不应进入大纲
## 也不应生成 slug
```

~~~python
# 同样跳过
~~~

## `inline_code` 标题
";
    let sample_headings = extract_headings(sample);
    assert_eq!(sample_headings.len(), 2);
    assert_eq!(sample_headings[0].text, "粗体 与 链接文本");
    assert_eq!(sample_headings[0].slug, "粗体-与-链接文本");
    assert_eq!(sample_headings[1].text, "inline_code 标题");
    assert_eq!(sample_headings[1].slug, "inline_code-标题");

    // Verify third duplicate gets `-2`.
    let mut seen = HashMap::new();
    assert_eq!(heading_to_slug("重复标题", &mut seen), "重复标题");
    assert_eq!(heading_to_slug("重复标题", &mut seen), "重复标题-1");
    assert_eq!(heading_to_slug("重复标题", &mut seen), "重复标题-2");

    Ok(())
}

#[test]
fn quickbar_draft_and_recovery_snapshots_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = TempDir::new()?;
    let vault_dir = TempDir::new()?;
    let pm = PersistenceManager::open(app_dir.path().to_path_buf())?;
    set_library_root(
        &pm,
        SetRootArgs {
            root: vault_dir.path().to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )?;

    // 1. QuickBarDraft save/load/auto-clear on note_append.
    assert!(pm.load_quickbar_draft()?.is_none());
    let draft = QuickBarDraft {
        mode: CaptureMode::List,
        target_rel_path: Some("inbox.md".to_string()),
        text: "未发送的草稿内容".to_string(),
        updated_at: 1_700_000_000_000,
    };
    pm.save_quickbar_draft(&draft)?;
    assert_eq!(pm.load_quickbar_draft()?, Some(draft));

    let capture = CaptureManager::new();
    capture.note_append(
        &pm,
        AppendNoteArgs {
            target_rel_path: INBOX_REL_PATH.to_string(),
            mode: CaptureMode::List,
            content: "正式提交的捕捉内容".to_string(),
        },
        None,
    )?;
    assert!(
        pm.load_quickbar_draft()?.is_none(),
        "quickbar-draft.json should be cleared after note_append"
    );

    // 2. RecoverySnapshot save/list/auto-clear on note_write.
    let doc = note_create(
        &pm,
        CreateNoteArgs {
            title: "崩溃恢复测试".to_string(),
            dir_rel_path: "notes".to_string(),
            template_name: None,
            body: Some("# 崩溃恢复测试\n\n原始正文。\n".to_string()),
        },
    )?;

    let snap = RecoverySnapshot {
        abs_path: doc.abs_path.clone(),
        rel_path: doc.rel_path.clone(),
        dirty_body: "# 崩溃恢复测试\n\n未保存的脏正文。\n".to_string(),
        base_body_hash: doc.body_hash.clone(),
        updated_at: 1_700_000_123_000,
    };
    pm.save_recovery_snapshot(&snap)?;

    let listed = pm.list_recovery_snapshots()?;
    assert_eq!(listed, vec![snap]);

    note_write(
        &pm,
        WriteNoteArgs {
            abs_path: doc.abs_path.clone(),
            body: "# 崩溃恢复测试\n\n正式保存的正文。\n".to_string(),
            base_body_hash: doc.body_hash,
        },
    )?;

    assert!(
        pm.list_recovery_snapshots()?.is_empty(),
        "recovery snapshot should be cleared after successful note_write"
    );

    Ok(())
}
