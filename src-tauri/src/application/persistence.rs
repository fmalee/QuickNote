use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, Utc};
use rusqlite::Connection;

use crate::domain::app::{ActivationPolicy, AppInfo};
use crate::domain::backup::format_backup_filename;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::compute_body_hash;
use crate::domain::settings::{
    apply_settings_patch, parse_settings_yaml, serialize_settings_yaml, Settings,
    SettingsLoadStatus, SettingsPatch, SettingsSnapshot,
};
use crate::domain::ui_state::{UiState, UiStatePatchKey};
use crate::domain::watch::{QuickBarDraft, RecoverySnapshot, SelfWriteGuard};
use crate::infra::fs::{atomic_write_str, init_app_data_dir, map_io_error, prune_backups};
use crate::infra::sqlite::{get_library_meta, init_sqlite};

/// Normalizes a filesystem path for `SelfWriteGuard` and watcher comparisons (`C8.5`).
/// Handles macOS `/var` -> `/private/var` symlinks even when the target file no longer exists.
pub fn canonicalize_watch_path(path: &Path) -> String {
    if let Ok(canon) = fs::canonicalize(path) {
        return canon.to_string_lossy().to_string();
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        if let Ok(canon_parent) = fs::canonicalize(parent) {
            return canon_parent.join(name).to_string_lossy().to_string();
        }
    }
    path.to_string_lossy().to_string()
}

/// Coordinates `$APP_DATA_DIR` persistence (`settings.yaml`, `ui-state.json`, `index.sqlite`) (`C6.1`–`C6.3`).
pub struct PersistenceManager {
    app_data_dir: PathBuf,
    settings: RwLock<Settings>,
    settings_status: RwLock<SettingsLoadStatus>,
    last_valid_settings: RwLock<Option<Settings>>,
    ui_state: RwLock<UiState>,
    db: Mutex<Connection>,
    self_write_guard: Mutex<SelfWriteGuard>,
}

impl PersistenceManager {
    /// Initializes `$APP_DATA_DIR`, loads `settings.yaml` and `ui-state.json`, and migrates `index.sqlite`.
    pub fn open(app_data_dir: PathBuf) -> Result<Self, AppError> {
        init_app_data_dir(&app_data_dir)?;

        let settings_path = app_data_dir.join("settings.yaml");
        let (settings, settings_status, last_valid) = if settings_path.exists() {
            let raw = fs::read_to_string(&settings_path)
                .map_err(|e| map_io_error(e, "Failed to read settings.yaml"))?;
            let (parsed, status) = parse_settings_yaml(&raw, None);
            let last_valid = if status.valid {
                Some(parsed.clone())
            } else {
                None
            };
            (parsed, status, last_valid)
        } else {
            let default_settings = Settings::default();
            let yaml = serialize_settings_yaml(&default_settings)?;
            atomic_write_str(&settings_path, &yaml)?;
            (
                default_settings.clone(),
                SettingsLoadStatus {
                    valid: true,
                    active_source: "file".to_string(),
                    error_line: None,
                    error_column: None,
                    error_message: None,
                },
                Some(default_settings),
            )
        };

        let ui_state_path = app_data_dir.join("ui-state.json");
        let ui_state = if ui_state_path.exists() {
            let raw = fs::read_to_string(&ui_state_path)
                .map_err(|e| map_io_error(e, "Failed to read ui-state.json"))?;
            match serde_json::from_str::<UiState>(&raw) {
                Ok(parsed) => parsed,
                Err(err) => {
                    eprintln!(
                        "[persistence] ui-state.json parse failed at line={} col={}; backing up and resetting to default",
                        err.line(),
                        err.column()
                    );
                    let backups_dir = app_data_dir.join("backups");
                    let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
                    let mut backup_path =
                        backups_dir.join(format_backup_filename("ui-state.json", &ts));
                    if backup_path.exists() {
                        let nanos = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.subsec_nanos())
                            .unwrap_or(0);
                        backup_path = backups_dir.join(format_backup_filename(
                            "ui-state.json",
                            &format!("{ts}-{nanos}"),
                        ));
                    }
                    atomic_write_str(&backup_path, &raw)?;
                    if let Err(prune_err) = prune_backups(&backups_dir, Utc::now().timestamp()) {
                        eprintln!(
                            "[persistence] backup prune failed: code={:?}",
                            prune_err.code
                        );
                    }

                    let default_ui = UiState::default();
                    let json = serde_json::to_string_pretty(&default_ui).map_err(|ser_err| {
                        AppError::with_detail(
                            ErrorCode::InternalError,
                            "Failed to serialize default ui-state.json",
                            ser_err.to_string(),
                        )
                    })?;
                    atomic_write_str(&ui_state_path, &json)?;
                    default_ui
                }
            }
        } else {
            let default_ui = UiState::default();
            let json = serde_json::to_string_pretty(&default_ui).map_err(|err| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to serialize default ui-state.json",
                    err.to_string(),
                )
            })?;
            atomic_write_str(&ui_state_path, &json)?;
            default_ui
        };

        let db_path = app_data_dir.join("index.sqlite");
        let conn = init_sqlite(&db_path)?;

        Ok(Self {
            app_data_dir,
            settings: RwLock::new(settings),
            settings_status: RwLock::new(settings_status),
            last_valid_settings: RwLock::new(last_valid),
            ui_state: RwLock::new(ui_state),
            db: Mutex::new(conn),
            self_write_guard: Mutex::new(SelfWriteGuard::new()),
        })
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn settings_path(&self) -> PathBuf {
        self.app_data_dir.join("settings.yaml")
    }

    pub fn ui_state_path(&self) -> PathBuf {
        self.app_data_dir.join("ui-state.json")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.app_data_dir.join("backups")
    }

    pub fn quickbar_draft_path(&self) -> PathBuf {
        self.app_data_dir.join("quickbar-draft.json")
    }

    pub fn pinned_files_path(&self) -> PathBuf {
        self.app_data_dir.join("pinned-files.json")
    }

    pub fn recovery_dir(&self) -> PathBuf {
        self.app_data_dir.join("recovery")
    }

    pub fn recovery_snapshot_path(&self, abs_path: &str) -> PathBuf {
        self.recovery_dir()
            .join(format!("{}.json", compute_body_hash(abs_path)))
    }

    /// Records an internal write in `SelfWriteGuard` to suppress `notify` loopbacks (`C8.5`).
    pub fn record_self_write(&self, path: &Path, body_hash: &str) {
        let canonical = canonicalize_watch_path(path);
        let now_ms = Utc::now().timestamp_millis();
        if let Ok(mut guard) = self.self_write_guard.lock() {
            guard.record_write(&canonical, body_hash, now_ms);
        }
    }

    /// Checks whether an external modify callback matches our own recent write (`C8.5`).
    pub fn should_suppress_external_modify(&self, path: &Path, disk_body_hash: &str) -> bool {
        let canonical = canonicalize_watch_path(path);
        let now_ms = Utc::now().timestamp_millis();
        if let Ok(mut guard) = self.self_write_guard.lock() {
            guard.should_suppress_modify(&canonical, disk_body_hash, now_ms)
        } else {
            false
        }
    }

    /// Clears cached self-write state for a deleted or moved-away file path (`C8.5`).
    pub fn clear_self_write(&self, path: &Path) {
        let canonical = canonicalize_watch_path(path);
        if let Ok(mut guard) = self.self_write_guard.lock() {
            guard.clear_path(&canonical);
        }
    }

    /// Persists the Quick Bar draft to `$APP_DATA_DIR/quickbar-draft.json` (`C6.1`).
    pub fn save_quickbar_draft(&self, draft: &QuickBarDraft) -> Result<(), AppError> {
        let json = serde_json::to_string_pretty(draft).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize quickbar-draft.json",
                err.to_string(),
            )
        })?;
        atomic_write_str(&self.quickbar_draft_path(), &json)
    }

    /// Loads `$APP_DATA_DIR/quickbar-draft.json` if present (`C6.1`).
    pub fn load_quickbar_draft(&self) -> Result<Option<QuickBarDraft>, AppError> {
        let path = self.quickbar_draft_path();
        if !path.is_file() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| map_io_error(e, "Failed to read quickbar-draft.json"))?;
        let parsed = serde_json::from_str::<QuickBarDraft>(&raw).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to parse quickbar-draft.json",
                err.to_string(),
            )
        })?;
        Ok(Some(parsed))
    }

    /// Removes `$APP_DATA_DIR/quickbar-draft.json` if present (`C6.1`).
    pub fn clear_quickbar_draft(&self) -> Result<(), AppError> {
        let path = self.quickbar_draft_path();
        if path.exists() {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(map_io_error(err, "Failed to remove quickbar-draft.json")),
            }
        }
        Ok(())
    }

    /// Persists an unsaved editor recovery snapshot under `$APP_DATA_DIR/recovery/` (`C6.1`).
    pub fn save_recovery_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), AppError> {
        let dir = self.recovery_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .map_err(|e| map_io_error(e, "Failed to create recovery directory"))?;
        }
        let json = serde_json::to_string_pretty(snapshot).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize recovery snapshot",
                err.to_string(),
            )
        })?;
        atomic_write_str(&self.recovery_snapshot_path(&snapshot.abs_path), &json)
    }

    /// Lists all recovery snapshots under `$APP_DATA_DIR/recovery/`, sorted newest first (`C6.1`).
    pub fn list_recovery_snapshots(&self) -> Result<Vec<RecoverySnapshot>, AppError> {
        let dir = self.recovery_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }

        let entries =
            fs::read_dir(&dir).map_err(|e| map_io_error(e, "Failed to read recovery directory"))?;
        let mut snapshots = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path) {
                Ok(raw) => match serde_json::from_str::<RecoverySnapshot>(&raw) {
                    Ok(snap) => snapshots.push(snap),
                    Err(err) => {
                        eprintln!(
                            "[persistence] invalid recovery snapshot json: line={} col={}",
                            err.line(),
                            err.column()
                        );
                    }
                },
                Err(err) => {
                    eprintln!("[persistence] failed to read recovery snapshot: {err}");
                }
            }
        }

        snapshots.sort_by_key(|a| std::cmp::Reverse(a.updated_at));
        Ok(snapshots)
    }

    /// Removes the recovery snapshot corresponding to `abs_path` if present (`C6.1`).
    pub fn clear_recovery_snapshot(&self, abs_path: &str) -> Result<(), AppError> {
        let path = self.recovery_snapshot_path(abs_path);
        if path.exists() {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(map_io_error(err, "Failed to remove recovery snapshot")),
            }
        }
        Ok(())
    }

    /// Returns the current in-memory settings and load diagnostics (`C4.2`).
    pub fn settings_get(&self) -> Result<SettingsSnapshot, AppError> {
        let settings = self
            .settings
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings lock poisoned"))?
            .clone();
        let status = self
            .settings_status
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings status lock poisoned"))?
            .clone();
        Ok(SettingsSnapshot { settings, status })
    }

    /// Applies a partial `SettingsPatch`, validates, and atomically persists `settings.yaml` (`C4.2`, `D-0008`).
    /// Never overwrites `settings.yaml` when the on-disk file is currently invalid (`D-0008`).
    pub fn settings_update(&self, patch: SettingsPatch) -> Result<SettingsSnapshot, AppError> {
        let status_valid = self
            .settings_status
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings status lock poisoned"))?
            .valid;
        if !status_valid {
            return Err(AppError::new(
                ErrorCode::SettingsParseError,
                "settings.yaml 当前存在语法错误，已只读保护，请修复后再修改设置",
            ));
        }

        let current = self
            .settings
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings lock poisoned"))?
            .clone();
        let next = apply_settings_patch(&current, &patch)?;
        let yaml = serialize_settings_yaml(&next)?;
        atomic_write_str(&self.settings_path(), &yaml)?;

        let next_status = SettingsLoadStatus {
            valid: true,
            active_source: "file".to_string(),
            error_line: None,
            error_column: None,
            error_message: None,
        };

        {
            let mut guard = self
                .settings
                .write()
                .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings lock poisoned"))?;
            *guard = next.clone();
        }
        {
            let mut last_guard = self
                .last_valid_settings
                .write()
                .map_err(|_| AppError::new(ErrorCode::InternalError, "Last valid lock poisoned"))?;
            *last_guard = Some(next.clone());
        }
        {
            let mut status_guard = self.settings_status.write().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Settings status lock poisoned")
            })?;
            *status_guard = next_status.clone();
        }

        Ok(SettingsSnapshot {
            settings: next,
            status: next_status,
        })
    }

    /// Re-reads `settings.yaml` from disk in read-only mode; never overwrites invalid files (`D-0008`).
    pub fn reload_settings(&self) -> Result<SettingsSnapshot, AppError> {
        let settings_path = self.settings_path();
        if !settings_path.exists() {
            return self.settings_get();
        }

        let raw = fs::read_to_string(&settings_path)
            .map_err(|e| map_io_error(e, "Failed to read settings.yaml"))?;

        let prev_valid = self
            .last_valid_settings
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Last valid lock poisoned"))?
            .clone();

        let (next_settings, next_status) = parse_settings_yaml(&raw, prev_valid.as_ref());

        {
            let mut guard = self
                .settings
                .write()
                .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings lock poisoned"))?;
            *guard = next_settings.clone();
        }
        {
            let mut status_guard = self.settings_status.write().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Settings status lock poisoned")
            })?;
            *status_guard = next_status.clone();
        }
        if next_status.valid {
            let mut last_guard = self
                .last_valid_settings
                .write()
                .map_err(|_| AppError::new(ErrorCode::InternalError, "Last valid lock poisoned"))?;
            *last_guard = Some(next_settings.clone());
        }

        Ok(SettingsSnapshot {
            settings: next_settings,
            status: next_status,
        })
    }

    /// Returns the current `UiState` snapshot (`C4.2`).
    pub fn ui_state_get(&self) -> Result<UiState, AppError> {
        let state = self
            .ui_state
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "UiState lock poisoned"))?
            .clone();
        Ok(state)
    }

    /// Validates and persists a single whitelisted key update to `ui-state.json` (`C4.2`, `C6.3`).
    pub fn ui_state_set(
        &self,
        key: UiStatePatchKey,
        value: serde_json::Value,
    ) -> Result<UiState, AppError> {
        let mut guard = self
            .ui_state
            .write()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "UiState lock poisoned"))?;
        let mut next = guard.clone();
        next.apply_patch(key, value)?;

        let json = serde_json::to_string_pretty(&next).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize ui-state.json",
                err.to_string(),
            )
        })?;
        atomic_write_str(&self.ui_state_path(), &json)?;
        *guard = next.clone();
        Ok(next)
    }

    /// Resolves the active `library_root`: when `settings.yaml` is valid, its `library_root`
    /// is the single source of truth; falls back to SQLite `library_meta` only when `settings.yaml` is invalid.
    pub fn current_library_root(&self) -> Result<Option<String>, AppError> {
        let status_valid = self
            .settings_status
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings status lock poisoned"))?
            .valid;

        let from_settings = self
            .settings
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings lock poisoned"))?
            .library_root
            .clone();

        if status_valid || from_settings.is_some() {
            return Ok(from_settings);
        }

        let conn = self
            .db
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "SQLite lock poisoned"))?;
        get_library_meta(&conn, "library_root")
    }

    /// Updates `library_root` in memory and persists to `settings.yaml` if currently valid (`D-0008`).
    pub(crate) fn update_library_root_in_settings(&self, root: &str) -> Result<(), AppError> {
        self.update_settings_with(|settings| {
            settings.library_root = Some(root.to_string());
        })
    }

    /// Updates `hotkey.quick_bar` in memory and persists to `settings.yaml` if currently valid (`D-0004`, `D-0008`).
    pub(crate) fn update_hotkey_in_settings(&self, combo: &str) -> Result<(), AppError> {
        self.update_settings_with(|settings| {
            settings.hotkey.quick_bar = combo.to_string();
        })
    }

    fn update_settings_with<F>(&self, mutator: F) -> Result<(), AppError>
    where
        F: FnOnce(&mut Settings),
    {
        let status_valid = self
            .settings_status
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings status lock poisoned"))?
            .valid;

        let updated = {
            let mut guard = self
                .settings
                .write()
                .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings lock poisoned"))?;
            mutator(&mut guard);
            guard.clone()
        };

        if status_valid {
            let yaml = serialize_settings_yaml(&updated)?;
            atomic_write_str(&self.settings_path(), &yaml)?;
            let mut last_guard = self
                .last_valid_settings
                .write()
                .map_err(|_| AppError::new(ErrorCode::InternalError, "Last valid lock poisoned"))?;
            *last_guard = Some(updated);
        }

        Ok(())
    }

    /// Executes a closure with locked access to the SQLite connection.
    pub fn with_db<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&Connection) -> Result<T, AppError>,
    {
        let conn = self
            .db
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "SQLite lock poisoned"))?;
        f(&conn)
    }

    /// Reads deduplicated pinned external file paths from `<app_data_dir>/pinned-files.json`.
    pub fn list_pinned_files(&self) -> Result<Vec<String>, AppError> {
        let path = self.pinned_files_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| map_io_error(e, "Failed to read pinned-files.json"))?;
        let parsed: Vec<String> = serde_json::from_str(&raw).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to parse pinned-files.json",
                err.to_string(),
            )
        })?;
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for item in parsed {
            let trimmed = item.trim();
            if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
                deduped.push(trimmed.to_string());
            }
        }
        Ok(deduped)
    }

    /// Toggles an external file in `<app_data_dir>/pinned-files.json` and returns the new pinned state.
    pub fn toggle_pinned_file(&self, abs_path: &str) -> Result<bool, AppError> {
        let trimmed = abs_path.trim();
        if trimmed.is_empty() {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "abs_path cannot be empty",
            ));
        }

        let mut items = self.list_pinned_files()?;
        let pinned = if let Some(pos) = items.iter().position(|p| p == trimmed) {
            items.remove(pos);
            false
        } else {
            items.push(trimmed.to_string());
            true
        };

        let json = serde_json::to_string_pretty(&items).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize pinned-files.json",
                err.to_string(),
            )
        })?;
        atomic_write_str(&self.pinned_files_path(), &json)?;
        Ok(pinned)
    }

    /// Builds `AppInfo` reflecting the current `library_root` (`C3.4`, `M0-02`).
    pub fn app_info(&self, version: &str, platform: &str) -> Result<AppInfo, AppError> {
        let library_root = self.current_library_root()?;
        Ok(AppInfo::new(
            version,
            platform,
            library_root,
            ActivationPolicy::Accessory,
        ))
    }
}
