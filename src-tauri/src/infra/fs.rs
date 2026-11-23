use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use crate::domain::backup::{select_backups_to_prune, BackupEntry};
use crate::domain::error::{AppError, ErrorCode};

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Maps `std::io::Error` into structured `AppError` without leaking sensitive payloads.
pub fn map_io_error(err: std::io::Error, context: &str) -> AppError {
    let code = match err.kind() {
        std::io::ErrorKind::NotFound => ErrorCode::NotFound,
        std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
        _ => ErrorCode::InternalError,
    };
    AppError::with_detail(code, context, err.to_string())
}

/// Initializes `$APP_DATA_DIR` along with `recovery/` and `backups/` subdirectories (`C6.1`).
pub fn init_app_data_dir(app_data_dir: &Path) -> Result<(), AppError> {
    fs::create_dir_all(app_data_dir)
        .map_err(|e| map_io_error(e, "Failed to create app data directory"))?;
    fs::create_dir_all(app_data_dir.join("recovery"))
        .map_err(|e| map_io_error(e, "Failed to create recovery directory"))?;
    fs::create_dir_all(app_data_dir.join("backups"))
        .map_err(|e| map_io_error(e, "Failed to create backups directory"))?;
    Ok(())
}

/// Atomically writes `bytes` to `path` via a same-directory temporary file and rename (`M0-02`).
pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            ErrorCode::ValidationError,
            "Target path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent).map_err(|e| map_io_error(e, "Failed to create parent directory"))?;

    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp_path = parent.join(format!(".{file_name}.tmp.{pid}.{seq}"));

    let write_res = (|| -> std::io::Result<()> {
        let mut file = File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(map_io_error(err, "Failed to write temporary file"));
    }

    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(map_io_error(err, "Failed to atomically rename file"));
    }

    Ok(())
}

/// Atomically writes UTF-8 string `content` to `path`.
pub fn atomic_write_str(path: &Path, content: &str) -> Result<(), AppError> {
    atomic_write_bytes(path, content.as_bytes())
}

/// Prunes `$APP_DATA_DIR/backups/` using the pure `select_backups_to_prune` policy (`C6.1`).
pub fn prune_backups(backups_dir: &Path, now_epoch_secs: i64) -> Result<usize, AppError> {
    if !backups_dir.exists() {
        return Ok(0);
    }

    let read_dir = fs::read_dir(backups_dir)
        .map_err(|e| map_io_error(e, "Failed to read backups directory"))?;

    let mut entries = Vec::new();
    for item in read_dir {
        let entry = item.map_err(|e| map_io_error(e, "Failed to read backup entry"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let metadata = entry
            .metadata()
            .map_err(|e| map_io_error(e, "Failed to read backup metadata"))?;
        let modified = metadata
            .modified()
            .map_err(|e| map_io_error(e, "Failed to read backup modified time"))?;
        let modified_epoch_secs = match modified.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(e) => -(e.duration().as_secs() as i64),
        };
        entries.push(BackupEntry {
            file_name: name.to_string(),
            modified_epoch_secs,
        });
    }

    let to_delete = select_backups_to_prune(&entries, now_epoch_secs);
    let mut deleted = 0usize;
    for name in to_delete {
        let target = backups_dir.join(name);
        if target.exists() {
            fs::remove_file(&target)
                .map_err(|e| map_io_error(e, "Failed to remove expired backup"))?;
            deleted += 1;
        }
    }

    Ok(deleted)
}

/// Expands leading `~` or `~/` using `HOME`.
pub fn expand_tilde(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

/// Initializes a new or missing library root skeleton (`inbox.md`, `assets/`, `_templates/`) (`M0-02`).
/// Never overwrites existing files.
pub fn ensure_library_skeleton(root: &Path) -> Result<(), AppError> {
    fs::create_dir_all(root)
        .map_err(|e| map_io_error(e, "Failed to create library root directory"))?;
    fs::create_dir_all(root.join("assets"))
        .map_err(|e| map_io_error(e, "Failed to create library assets directory"))?;
    let templates_dir = root.join("_templates");
    fs::create_dir_all(&templates_dir)
        .map_err(|e| map_io_error(e, "Failed to create library _templates directory"))?;

    let inbox_path = root.join("inbox.md");
    if !inbox_path.exists() {
        let inbox_content = "---\ntitle: 收件箱\ntype: note\ntags: []\n---\n\n# 收件箱\n";
        atomic_write_str(&inbox_path, inbox_content)?;
    }

    let note_tpl = templates_dir.join("note.md");
    if !note_tpl.exists() {
        let content = "---\nid: \"{{id}}\"\ntitle: \"{{title}}\"\ntype: note\ntags: []\ncreated: \"{{date}}T{{time}}\"\n---\n\n# {{title}}\n\n## 想法\n\n## 待办\n";
        atomic_write_str(&note_tpl, content)?;
    }

    let doc_tpl = templates_dir.join("doc.md");
    if !doc_tpl.exists() {
        let content = "---\nid: \"{{id}}\"\ntitle: \"{{title}}\"\ntype: doc\ntags: []\ncreated: \"{{date}}T{{time}}\"\n---\n\n# {{title}}\n\n## 背景\n\n## 想法\n\n## 待办\n";
        atomic_write_str(&doc_tpl, content)?;
    }

    let cmd_tpl = templates_dir.join("command-book.md");
    if !cmd_tpl.exists() {
        let content = "---\nid: \"{{id}}\"\ntitle: \"{{title}}\"\ntype: command-book\ntags: []\ncreated: \"{{date}}T{{time}}\"\n---\n\n# {{title}}\n\n## 常用命令\n\n### 示例命令\n\n```bash\necho \"hello\"\n```\n";
        atomic_write_str(&cmd_tpl, content)?;
    }

    Ok(())
}
