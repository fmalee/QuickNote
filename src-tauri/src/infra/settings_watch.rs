use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::domain::error::{AppError, ErrorCode};

const SETTINGS_DEBOUNCE_MS: u64 = 150;

/// Watches `$APP_DATA_DIR/settings.yaml` for external edits with debouncing (`D-0008`, `M0-02`).
pub struct SettingsWatcher {
    _watcher: RecommendedWatcher,
    stop_tx: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl SettingsWatcher {
    /// Starts watching `app_data_dir` for changes to `settings.yaml`.
    pub fn start<F>(app_data_dir: &Path, mut on_change: F) -> Result<Self, AppError>
    where
        F: FnMut() + Send + 'static,
    {
        let (event_tx, event_rx) = mpsc::channel::<()>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let touches_settings = event.paths.iter().any(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n == "settings.yaml")
                });
                if touches_settings {
                    let _ = event_tx.send(());
                }
            }
        })
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to initialize settings watcher",
                err.to_string(),
            )
        })?;

        watcher
            .watch(app_data_dir, RecursiveMode::NonRecursive)
            .map_err(|err| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to watch app data directory for settings.yaml",
                    err.to_string(),
                )
            })?;

        let worker = thread::spawn(move || {
            let debounce = Duration::from_millis(SETTINGS_DEBOUNCE_MS);
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
                    Ok(()) => {
                        pending_deadline = Some(Instant::now() + debounce);
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if let Some(deadline) = pending_deadline {
                            if Instant::now() >= deadline {
                                pending_deadline = None;
                                on_change();
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            stop_tx: Some(stop_tx),
            worker: Some(worker),
        })
    }
}

impl Drop for SettingsWatcher {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}
