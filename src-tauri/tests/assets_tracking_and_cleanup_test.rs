use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{
    assets_clean_unused_with, assets_scan_unused, note_create, note_delete_with, note_save_image,
    note_write,
};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::application::reconciler::{reconcile_vault, reconcile_watched_path};
use quicknote_lib::domain::error::{AppError, ErrorCode};
use quicknote_lib::domain::markdown::extract_relative_asset_refs;
use quicknote_lib::domain::note::{
    CleanUnusedAssetsArgs, CreateNoteArgs, RelPathArgs, SaveImageArgs, ScanUnusedAssetsArgs,
    WriteNoteArgs,
};
use quicknote_lib::domain::settings::{SetRootArgs, SettingsPatch};
use quicknote_lib::infra::sqlite::remove_sqlite_files;
use quicknote_lib::infra::trash::FileRemover;

// Minimal valid 1x1 transparent PNG in base64.
const SAMPLE_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

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
        let path = std::env::temp_dir().join(format!(
            "quicknote_assets_test_{}_{}",
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

fn setup_env() -> (TempDir, PathBuf, PathBuf, PathBuf, PersistenceManager) {
    let dir = TempDir::new().expect("create temp dir");
    let config_dir = dir.path().join("config");
    let cache_dir = dir.path().join("cache");
    let vault_dir = dir.path().join("vault");
    fs::create_dir_all(&config_dir).expect("create config");
    fs::create_dir_all(&cache_dir).expect("create cache");

    let pm =
        PersistenceManager::open_with_dirs(config_dir.clone(), cache_dir.clone()).expect("init pm");
    set_library_root(
        &pm,
        SetRootArgs {
            root: vault_dir.to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect("set library root");

    (dir, config_dir, cache_dir, vault_dir, pm)
}

#[test]
fn extract_relative_asset_refs_supports_markdown_html_frontmatter_and_percent_encoding() {
    let raw = r#"---
title: 测试文档
cover: assets/2026-11/cover.png
---

![截图](assets/2026-11/shot%201.png)
![上级](../assets/2026-11/nested.webp "caption")
[附件](./assets/2026-11/diagram.svg)
<img src="assets/2026-11/html-img.jpg" alt="html" />
[ref-img]: assets/2026-11/ref.png
![外链](https://example.com/assets/2026-11/remote.png)
![越界](../../../etc/passwd)
"#;

    let refs = extract_relative_asset_refs("projects/note.md", raw);
    assert_eq!(
        refs,
        vec![
            "assets/2026-11/cover.png".to_string(),
            "assets/2026-11/diagram.svg".to_string(),
            "assets/2026-11/html-img.jpg".to_string(),
            "assets/2026-11/nested.webp".to_string(),
            "assets/2026-11/ref.png".to_string(),
            "assets/2026-11/shot 1.png".to_string(),
        ]
    );
}

#[test]
fn note_delete_cleans_unreferenced_assets_only_when_refcount_reaches_zero_and_respects_setting() {
    let (_dir, _cfg, _cache, vault_dir, pm) = setup_env();
    let remover = FakeRemover::default();

    let saved_shared = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: None,
            bytes_base64: Some(SAMPLE_PNG_B64.to_string()),
            ext: "png".to_string(),
            context_rel_path: None,
        },
    )
    .expect("save shared image");
    let saved_exclusive = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: None,
            bytes_base64: Some(SAMPLE_PNG_B64.to_string()),
            ext: "png".to_string(),
            context_rel_path: None,
        },
    )
    .expect("save exclusive image");

    let doc_a = note_create(
        &pm,
        CreateNoteArgs {
            dir_rel_path: String::new(),
            title: "Doc A".to_string(),
            template_name: None,
            body: None,
        },
    )
    .expect("create doc A");
    let doc_b = note_create(
        &pm,
        CreateNoteArgs {
            dir_rel_path: String::new(),
            title: "Doc B".to_string(),
            template_name: None,
            body: None,
        },
    )
    .expect("create doc B");

    note_write(
        &pm,
        WriteNoteArgs {
            abs_path: doc_a.abs_path.clone(),
            body: format!(
                "# Doc A\n\n![shared]({})\n![exclusive]({})\n",
                saved_shared.rel_path, saved_exclusive.rel_path
            ),
            base_body_hash: doc_a.body_hash.clone(),
        },
    )
    .expect("write doc A");
    note_write(
        &pm,
        WriteNoteArgs {
            abs_path: doc_b.abs_path.clone(),
            body: format!("# Doc B\n\n![shared]({})\n", saved_shared.rel_path),
            base_body_hash: doc_b.body_hash.clone(),
        },
    )
    .expect("write doc B");

    // Deleting doc A removes exclusive image, keeps shared image still referenced by doc B.
    note_delete_with(
        &pm,
        RelPathArgs {
            rel_path: doc_a.rel_path.clone().expect("rel path A"),
        },
        &remover,
    )
    .expect("delete doc A");

    assert!(
        !vault_dir.join(&saved_exclusive.rel_path).exists(),
        "exclusive image should be moved to trash when its only referencing note is deleted"
    );
    assert!(
        vault_dir.join(&saved_shared.rel_path).exists(),
        "shared image must stay because doc B still references it"
    );

    // Disable autoCleanUnusedAssets, then delete doc B: shared image must stay on disk.
    pm.settings_update(SettingsPatch {
        auto_clean_unused_assets: Some(false),
        ..Default::default()
    })
    .expect("disable auto clean");

    note_delete_with(
        &pm,
        RelPathArgs {
            rel_path: doc_b.rel_path.clone().expect("rel path B"),
        },
        &remover,
    )
    .expect("delete doc B");

    assert!(
        vault_dir.join(&saved_shared.rel_path).exists(),
        "shared image must stay when autoCleanUnusedAssets is disabled"
    );

    // Manual cleanup removes the remaining orphaned asset and prunes its empty YYYY-MM dir,
    // while keeping root `assets/`.
    let cleaned = assets_clean_unused_with(
        &pm,
        CleanUnusedAssetsArgs {
            rel_paths: None,
            protected_rel_paths: Vec::new(),
            grace_period_seconds: Some(0),
        },
        &remover,
    )
    .expect("manual clean");

    assert_eq!(cleaned.deleted_paths, vec![saved_shared.rel_path.clone()]);
    assert!(!vault_dir.join(&saved_shared.rel_path).exists());
    let ym_dir = vault_dir
        .join(&saved_shared.rel_path)
        .parent()
        .expect("ym parent")
        .to_path_buf();
    assert!(
        !ym_dir.exists(),
        "empty assets/YYYY-MM dir should be pruned"
    );
    assert!(
        vault_dir.join("assets").is_dir(),
        "root assets/ directory must never be removed"
    );
    assert!(remover.removed_paths().len() >= 4);
}

#[test]
fn cross_restart_db_rebuild_and_external_watch_keep_asset_references_accurate() {
    let (_dir, config_dir, cache_dir, vault_dir, pm) = setup_env();
    let remover = FakeRemover::default();

    // Save 3 images: one referenced by a note, one referenced by a template, one unreferenced.
    let img_note = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: None,
            bytes_base64: Some(SAMPLE_PNG_B64.to_string()),
            ext: "png".to_string(),
            context_rel_path: None,
        },
    )
    .expect("save note img");
    let img_tpl = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: None,
            bytes_base64: Some(SAMPLE_PNG_B64.to_string()),
            ext: "png".to_string(),
            context_rel_path: None,
        },
    )
    .expect("save template img");
    let img_orphan = note_save_image(
        &pm,
        SaveImageArgs {
            source_path: None,
            bytes_base64: Some(SAMPLE_PNG_B64.to_string()),
            ext: "png".to_string(),
            context_rel_path: None,
        },
    )
    .expect("save orphan img");

    let doc = note_create(
        &pm,
        CreateNoteArgs {
            dir_rel_path: String::new(),
            title: "Guide".to_string(),
            template_name: None,
            body: None,
        },
    )
    .expect("create guide");
    note_write(
        &pm,
        WriteNoteArgs {
            abs_path: doc.abs_path.clone(),
            body: format!("# Guide\n\n![img]({})\n", img_note.rel_path),
            base_body_hash: doc.body_hash.clone(),
        },
    )
    .expect("write guide");

    // Template referencing img_tpl must keep img_tpl protected.
    let tpl_path = vault_dir.join("_templates/with-image.md");
    fs::write(
        &tpl_path,
        format!("---\ntitle: Tpl\n---\n\n![tpl]({})\n", img_tpl.rel_path),
    )
    .expect("write template");

    // Simulate deleting SQLite cache DB + cold restart + full reconcile.
    let db_path = pm.vault_db_path(&vault_dir);
    drop(pm);
    remove_sqlite_files(&db_path).expect("remove sqlite db");

    let pm_restarted = PersistenceManager::open_with_dirs(config_dir, cache_dir)
        .expect("re-init pm after restart");
    pm_restarted
        .with_db(|conn| reconcile_vault(&vault_dir, conn))
        .expect("reconcile after restart");

    // Grace period check: default 600s protects freshly created orphan.
    let scan_with_grace = assets_scan_unused(
        &pm_restarted,
        ScanUnusedAssetsArgs {
            protected_rel_paths: Vec::new(),
            grace_period_seconds: Some(600),
        },
    )
    .expect("scan with grace");
    assert!(scan_with_grace.items.is_empty());
    assert_eq!(scan_with_grace.skipped_recent_count, 1);

    // Draft protection check: passing orphan in protected_rel_paths shields it even with 0s grace.
    let scan_protected = assets_scan_unused(
        &pm_restarted,
        ScanUnusedAssetsArgs {
            protected_rel_paths: vec![img_orphan.rel_path.clone()],
            grace_period_seconds: Some(0),
        },
    )
    .expect("scan protected");
    assert!(scan_protected.items.is_empty());

    // Without draft protection and 0s grace, only img_orphan is reported (note + template images kept).
    let scan_all = assets_scan_unused(
        &pm_restarted,
        ScanUnusedAssetsArgs {
            protected_rel_paths: Vec::new(),
            grace_period_seconds: Some(0),
        },
    )
    .expect("scan all");
    let scanned_paths: Vec<String> = scan_all.items.iter().map(|i| i.rel_path.clone()).collect();
    assert_eq!(scanned_paths, vec![img_orphan.rel_path.clone()]);

    // External editor removes image reference from Guide.md -> watcher reconciles -> img_note becomes unused.
    fs::write(
        &doc.abs_path,
        "---\ntitle: Guide\n---\n\n# Guide\n\nNo more image.\n",
    )
    .expect("external edit");
    reconcile_watched_path(&pm_restarted, &vault_dir, Path::new(&doc.abs_path))
        .expect("reconcile watch");

    let scan_after_watch = assets_scan_unused(
        &pm_restarted,
        ScanUnusedAssetsArgs {
            protected_rel_paths: Vec::new(),
            grace_period_seconds: Some(0),
        },
    )
    .expect("scan after external edit");
    let mut after_paths: Vec<String> = scan_after_watch
        .items
        .iter()
        .map(|i| i.rel_path.clone())
        .collect();
    after_paths.sort();
    let mut expected = vec![img_note.rel_path.clone(), img_orphan.rel_path.clone()];
    expected.sort();
    assert_eq!(after_paths, expected);

    // Clean only img_note via explicit rel_paths subset (simulating tab-close deferred cleanup).
    let clean_subset = assets_clean_unused_with(
        &pm_restarted,
        CleanUnusedAssetsArgs {
            rel_paths: Some(vec![img_note.rel_path.clone(), img_tpl.rel_path.clone()]),
            protected_rel_paths: Vec::new(),
            grace_period_seconds: Some(0),
        },
        &remover,
    )
    .expect("clean subset");
    assert_eq!(clean_subset.deleted_paths, vec![img_note.rel_path.clone()]);
    assert_eq!(clean_subset.skipped_referenced_count, 1);
    assert!(!vault_dir.join(&img_note.rel_path).exists());
    assert!(vault_dir.join(&img_tpl.rel_path).exists());
    assert!(vault_dir.join(&img_orphan.rel_path).exists());
}
