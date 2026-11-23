use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

/// Unified application error code for all Tauri IPC responses (`C3.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    ValidationError,
    LibraryNotConfigured,
    NotFound,
    PermissionDenied,
    Conflict,
    UndoUnavailable,
    InvalidFrontmatter,
    SettingsParseError,
    ExternalAppMissing,
    WorkspaceDirMissing,
    TerminalLimitReached,
    TerminalSpawnFailed,
    InternalError,
}

/// Unified structured error returned by all Tauri commands (`C3.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>,
}

impl AppError {
    /// Creates an `AppError` without extra structured detail.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    /// Creates an `AppError` carrying sanitized diagnostic detail (never note bodies or secrets).
    pub fn with_detail(
        code: ErrorCode,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            detail: Some(detail.into()),
        }
    }
}
