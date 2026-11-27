use std::fs;
use std::path::PathBuf;

use quicknote_lib::application::notes::{note_read, MAX_EDITABLE_FILE_BYTES};
use quicknote_lib::application::open_files::PendingOpenFilesQueue;
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::note::ReadNoteArgs;

fn make_temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quicknote_native_open_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn pending_open_files_queue_decodes_urls_filters_dirs_dedups_and_drains_atomically() {
    let dir = make_temp_dir("queue");
    let sub_dir = dir.join("folder");
    fs::create_dir_all(&sub_dir).expect("create subdir");

    let file_cn = dir.join("中文 笔记.md");
    let file_plain = dir.join("Dockerfile");
    fs::write(&file_cn, "# 你好").expect("write cn");
    fs::write(&file_plain, "FROM scratch\n").expect("write plain");

    let url_cn = tauri::Url::from_file_path(&file_cn).expect("url cn");
    let url_plain = tauri::Url::from_file_path(&file_plain).expect("url plain");
    let url_dir = tauri::Url::from_file_path(&sub_dir).expect("url dir");
    let url_missing = tauri::Url::from_file_path(dir.join("missing.txt")).expect("url missing");
    let url_http = tauri::Url::parse("https://example.com/a.md").expect("url http");

    let queue = PendingOpenFilesQueue::new();
    let report = queue
        .enqueue_urls(&[
            url_cn.clone(),
            url_dir,
            url_missing,
            url_http,
            url_plain.clone(),
            url_cn,
        ])
        .expect("enqueue_urls");

    assert_eq!(report.accepted.len(), 2);
    assert_eq!(report.skipped_directories, 1);
    assert_eq!(report.skipped_invalid, 2);
    assert_eq!(queue.len().expect("len"), 2);

    let drained = queue.take_all().expect("take_all");
    assert_eq!(drained.len(), 2);
    assert!(drained[0].ends_with("中文 笔记.md"));
    assert!(drained[1].ends_with("Dockerfile"));

    // Second drain must be empty.
    assert!(queue.take_all().expect("second take_all").is_empty());

    // Raw CLI args should skip flags and resolve relative paths against cwd.
    let cli_report = queue
        .enqueue_raw_args(&["--inspect", "Dockerfile", "folder"], Some(&dir))
        .expect("enqueue_raw_args");
    assert_eq!(cli_report.accepted.len(), 1);
    assert_eq!(cli_report.skipped_directories, 1);
    let drained_cli = queue.take_all().expect("take_all cli");
    assert_eq!(drained_cli.len(), 1);
    assert!(drained_cli[0].ends_with("Dockerfile"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn note_read_supports_external_text_and_rejects_binary_and_oversized_files() {
    let dir = make_temp_dir("note_read_guard");
    let app_data = dir.join("app_data");
    let pm = PersistenceManager::open(app_data).expect("open pm");

    // 1. Extensionless external UTF-8 text file is readable.
    let makefile = dir.join("Makefile");
    fs::write(&makefile, "all:\n\techo ok\n").expect("write Makefile");
    let doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: makefile.to_string_lossy().to_string(),
        },
    )
    .expect("read Makefile");
    assert!(!doc.is_library);
    assert!(doc.key.starts_with("abs:"));
    assert_eq!(doc.body, "all:\n\techo ok\n");

    // 2. Binary file containing NUL bytes is rejected with ValidationError.
    let bin_file = dir.join("image.png");
    fs::write(&bin_file, b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR").expect("write png");
    let bin_err = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: bin_file.to_string_lossy().to_string(),
        },
    )
    .expect_err("binary file must fail");
    assert_eq!(bin_err.code, ErrorCode::ValidationError);

    // 3. Invalid UTF-8 bytes without NUL are also rejected with ValidationError.
    let bad_utf8 = dir.join("latin1.txt");
    fs::write(&bad_utf8, [0xFFu8, 0xFEu8, 0x80u8, 0x81u8]).expect("write bad utf8");
    let utf8_err = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: bad_utf8.to_string_lossy().to_string(),
        },
    )
    .expect_err("invalid utf8 must fail");
    assert_eq!(utf8_err.code, ErrorCode::ValidationError);

    // 4. File exceeding MAX_EDITABLE_FILE_BYTES (10 MiB) is rejected with ValidationError.
    let oversized = dir.join("huge.log");
    let f = fs::File::create(&oversized).expect("create oversized");
    f.set_len(MAX_EDITABLE_FILE_BYTES + 1)
        .expect("set oversized len");
    drop(f);
    let size_err = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: oversized.to_string_lossy().to_string(),
        },
    )
    .expect_err("oversized file must fail");
    assert_eq!(size_err.code, ErrorCode::ValidationError);

    let _ = fs::remove_dir_all(&dir);
}
