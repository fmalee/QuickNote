use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::domain::error::{AppError, ErrorCode};

const VAULT_DEBOUNCE_MS: u64 = 100;

/// Recursively watches `library_root` for `.md` changes with 100ms debouncing and dynamic rebinding (`C8.5`).
pub struct VaultWatcher {
    watcher: Mutex<RecommendedWatcher>,
    watched_root: Mutex<Option<PathBuf>>,
    stop_tx: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl VaultWatcher {
    /// Starts watching `initial_root` (if configured and existing) and invokes `on_path` after debouncing.
    pub fn start<F>(initial_root: Option<&Path>, mut on_path: F) -> Result<Self, AppError>
    where
        F: FnMut(&Path) + Send + 'static,
    {
        let (event_tx, event_rx) = mpsc::channel::<PathBuf>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    if is_watchable_markdown_path(&path) {
                        let _ = event_tx.send(path);
                    }
                }
            }
        })
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to initialize vault watcher",
                err.to_string(),
            )
        })?;

        let mut active_root = None;
        if let Some(root) = initial_root {
            if root.is_dir() {
                watcher
                    .watch(root, RecursiveMode::Recursive)
                    .map_err(|err| {
                        AppError::with_detail(
                            ErrorCode::InternalError,
                            "Failed to watch library root directory",
                            err.to_string(),
                        )
                    })?;
                active_root = Some(root.to_path_buf());
            }
        }

        let worker = thread::spawn(move || {
            let debounce = Duration::from_millis(VAULT_DEBOUNCE_MS);
            let mut pending_paths: HashSet<PathBuf> = HashSet::new();
            let mut pending_deadline: Option<Instant> = None;

            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                let timeout = match pending_deadline {
                    Some(deadline) => deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(50)),
                    None => Duration::from_millis(100),
                };

                match event_rx.recv_timeout(timeout) {
                    Ok(path) => {
                        pending_paths.insert(path);
                        pending_deadline = Some(Instant::now() + debounce);
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if let Some(deadline) = pending_deadline {
                            if Instant::now() >= deadline {
                                pending_deadline = None;
                                let batch: Vec<PathBuf> = pending_paths.drain().collect();
                                for path in batch {
                                    on_path(&path);
                                }
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Ok(Self {
            watcher: Mutex::new(watcher),
            watched_root: Mutex::new(active_root),
            stop_tx: Some(stop_tx),
            worker: Some(worker),
        })
    }

    /// Rebinds the recursive watcher when `library_set_root` changes the active library directory.
    pub fn rebind(&self, new_root: Option<&Path>) -> Result<(), AppError> {
        let mut watcher = self
            .watcher
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Vault watcher lock poisoned"))?;
        let mut root_guard = self
            .watched_root
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Watched root lock poisoned"))?;

        if let Some(old_root) = root_guard.take() {
            let _ = watcher.unwatch(&old_root);
        }

        if let Some(dir) = new_root {
            if dir.is_dir() {
                watcher
                    .watch(dir, RecursiveMode::Recursive)
                    .map_err(|err| {
                        AppError::with_detail(
                            ErrorCode::InternalError,
                            "Failed to watch new library root directory",
                            err.to_string(),
                        )
                    })?;
                *root_guard = Some(dir.to_path_buf());
            }
        }

        Ok(())
    }
}

impl Drop for VaultWatcher {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

fn is_watchable_markdown_path(path: &Path) -> bool {
    let is_md = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !is_md {
        return false;
    }

    for comp in path.components() {
        if let Component::Normal(part) = comp {
            let s = part.to_string_lossy();
            if s.starts_with('.') || s.contains(".tmp.") || s == "assets" {
                return false;
            }
        }
    }

    true
}
