use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::domain::error::{AppError, ErrorCode};

/// Summary of an enqueue operation into [`PendingOpenFilesQueue`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnqueueOpenFilesReport {
    pub accepted: Vec<String>,
    pub skipped_directories: usize,
    pub skipped_invalid: usize,
}

/// Thread-safe FIFO queue holding OS-delivered file paths (Dock drop, Finder "Open With",
/// or single-instance CLI args) until the main webview drains them via IPC.
#[derive(Debug, Default)]
pub struct PendingOpenFilesQueue {
    paths: Mutex<Vec<String>>,
}

impl PendingOpenFilesQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses `file://` URLs (as emitted by macOS `application:openURLs:` / `RunEvent::Opened`),
    /// filters out directories and missing files, deduplicates against pending entries, and enqueues them.
    pub fn enqueue_urls(&self, urls: &[tauri::Url]) -> Result<EnqueueOpenFilesReport, AppError> {
        let mut raw_paths = Vec::with_capacity(urls.len());
        let mut skipped_invalid = 0usize;

        for url in urls {
            if url.scheme() != "file" {
                skipped_invalid += 1;
                continue;
            }
            match url.to_file_path() {
                Ok(path) => raw_paths.push(path),
                Err(()) => skipped_invalid += 1,
            }
        }

        let mut report = self.enqueue_path_bufs(&raw_paths)?;
        report.skipped_invalid += skipped_invalid;
        Ok(report)
    }

    /// Parses raw path strings (e.g. from `single-instance` argv or CLI launch args) and enqueues valid files.
    pub fn enqueue_raw_args<S: AsRef<str>>(
        &self,
        args: &[S],
        cwd: Option<&Path>,
    ) -> Result<EnqueueOpenFilesReport, AppError> {
        let mut candidates = Vec::new();
        for arg in args {
            let trimmed = arg.as_ref().trim();
            if trimmed.is_empty() || trimmed.starts_with('-') {
                continue;
            }
            if let Ok(url) = tauri::Url::parse(trimmed) {
                if url.scheme() == "file" {
                    if let Ok(path) = url.to_file_path() {
                        candidates.push(path);
                        continue;
                    }
                }
            }
            let path = PathBuf::from(trimmed);
            let resolved = if path.is_absolute() {
                path
            } else if let Some(base) = cwd {
                base.join(path)
            } else {
                path
            };
            candidates.push(resolved);
        }
        self.enqueue_path_bufs(&candidates)
    }

    /// Enqueues concrete filesystem paths, filtering out directories and non-existent entries.
    pub fn enqueue_path_bufs(&self, paths: &[PathBuf]) -> Result<EnqueueOpenFilesReport, AppError> {
        let mut report = EnqueueOpenFilesReport::default();
        let mut guard = self.paths.lock().map_err(|_| {
            AppError::new(
                ErrorCode::InternalError,
                "Pending open-files queue lock poisoned",
            )
        })?;

        for path in paths {
            let meta = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(_) => {
                    report.skipped_invalid += 1;
                    continue;
                }
            };
            if meta.is_dir() {
                report.skipped_directories += 1;
                continue;
            }
            if !meta.is_file() {
                report.skipped_invalid += 1;
                continue;
            }
            let normalized = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            let normalized_str = normalized.to_string_lossy().to_string();
            if !guard.iter().any(|existing| existing == &normalized_str) {
                guard.push(normalized_str.clone());
            }
            if !report.accepted.iter().any(|item| item == &normalized_str) {
                report.accepted.push(normalized_str);
            }
        }

        Ok(report)
    }

    /// Atomically drains all pending file paths in FIFO order.
    pub fn take_all(&self) -> Result<Vec<String>, AppError> {
        let mut guard = self.paths.lock().map_err(|_| {
            AppError::new(
                ErrorCode::InternalError,
                "Pending open-files queue lock poisoned",
            )
        })?;
        Ok(std::mem::take(&mut *guard))
    }

    pub fn len(&self) -> Result<usize, AppError> {
        let guard = self.paths.lock().map_err(|_| {
            AppError::new(
                ErrorCode::InternalError,
                "Pending open-files queue lock poisoned",
            )
        })?;
        Ok(guard.len())
    }

    pub fn is_empty(&self) -> Result<bool, AppError> {
        Ok(self.len()? == 0)
    }
}
