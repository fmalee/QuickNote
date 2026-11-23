use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use rusqlite::Connection;

use crate::domain::app::{ActivationPolicy, AppInfo};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::settings::{
    parse_settings_yaml, serialize_settings_yaml, Settings, SettingsLoadStatus, SettingsSnapshot,
};
use crate::domain::ui_state::UiState;
use crate::infra::fs::{atomic_write_str, init_app_data_dir, map_io_error};
use crate::infra::sqlite::{get_library_meta, init_sqlite};

/// Coordinates `$APP_DATA_DIR` persistence (`settings.yaml`, `ui-state.json`, `index.sqlite`) (`C6.1`–`C6.3`).
pub struct PersistenceManager {
    app_data_dir: PathBuf,
    settings: RwLock<Settings>,
    settings_status: RwLock<SettingsLoadStatus>,
    last_valid_settings: RwLock<Option<Settings>>,
    ui_state: RwLock<UiState>,
    db: Mutex<Connection>,
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
            serde_json::from_str::<UiState>(&raw).map_err(|err| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to parse ui-state.json",
                    err.to_string(),
                )
            })?
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
    pub fn ui_state_set(&self, key: &str, value: serde_json::Value) -> Result<UiState, AppError> {
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

    /// Resolves the active `library_root` from settings or SQLite `library_meta`.
    pub fn current_library_root(&self) -> Result<Option<String>, AppError> {
        let from_settings = self
            .settings
            .read()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "Settings lock poisoned"))?
            .library_root
            .clone();
        if from_settings.is_some() {
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
            guard.library_root = Some(root.to_string());
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
