use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::domain::error::{AppError, ErrorCode};
use crate::domain::workspace_context::DIRECTORY_CONFIG_FILENAME;

const VAULT_DEBOUNCE_MS: u64 = 100;

#[derive(Debug, Clone)]
struct WatchedRoot {
    raw: PathBuf,
    canonical: PathBuf,
}

impl WatchedRoot {
    fn from_path(root: &Path) -> Self {
        let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        Self {
            raw: root.to_path_buf(),
            canonical,
        }
    }

    fn strip_relative<'a>(&self, path: &'a Path) -> Option<&'a Path> {
        path.strip_prefix(&self.canonical)
            .ok()
            .or_else(|| path.strip_prefix(&self.raw).ok())
    }
}

/// Recursively watches `library_root` for `.md` and `.quicknote.yaml` changes with 100ms
/// debouncing and dynamic rebinding (`C8.5`).
pub struct VaultWatcher {
    watcher: Mutex<RecommendedWatcher>,
    watched_root: Arc<Mutex<Option<WatchedRoot>>>,
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
        let watched_root = Arc::new(Mutex::new(None::<WatchedRoot>));
        let watched_root_for_cb = Arc::clone(&watched_root);

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let root_snapshot = watched_root_for_cb.lock().ok().and_then(|g| g.clone());
                for path in event.paths {
                    if is_watchable_vault_path(root_snapshot.as_ref(), &path) {
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

        if let Some(root) = initial_root {
            if root.is_dir() {
                if let Ok(mut guard) = watched_root.lock() {
                    *guard = Some(WatchedRoot::from_path(root));
                }
                watcher
                    .watch(root, RecursiveMode::Recursive)
                    .map_err(|err| {
                        AppError::with_detail(
                            ErrorCode::InternalError,
                            "Failed to watch library root directory",
                            err.to_string(),
                        )
                    })?;
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
            watched_root,
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
            let _ = watcher.unwatch(&old_root.raw);
        }

        if let Some(dir) = new_root {
            if dir.is_dir() {
                let next_root = WatchedRoot::from_path(dir);
                watcher
                    .watch(dir, RecursiveMode::Recursive)
                    .map_err(|err| {
                        AppError::with_detail(
                            ErrorCode::InternalError,
                            "Failed to watch new library root directory",
                            err.to_string(),
                        )
                    })?;
                *root_guard = Some(next_root);
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

fn is_watchable_vault_path(watched_root: Option<&WatchedRoot>, path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if file_name.contains(".tmp.") {
        return false;
    }

    let is_config = file_name == DIRECTORY_CONFIG_FILENAME;
    let is_md = !file_name.starts_with('.')
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));

    if !is_config && !is_md {
        return false;
    }

    let rel_path = match watched_root {
        Some(root) => match root.strip_relative(path) {
            Some(rel) => rel,
            None => path,
        },
        None => path,
    };

    let components: Vec<String> = rel_path
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    if components.is_empty() {
        return false;
    }

    let last_idx = components.len() - 1;
    for (idx, part) in components.iter().enumerate() {
        if part.contains(".tmp.") {
            return false;
        }
        if idx < last_idx {
            if part.starts_with('.') {
                return false;
            }
            if watched_root.is_some() && idx == 0 && part == "assets" {
                return false;
            }
            if watched_root.is_none() && part == "assets" {
                return false;
            }
        }
    }

    true
}
