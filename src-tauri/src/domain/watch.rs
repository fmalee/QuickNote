use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::note::NoteMeta;
use crate::domain::ui_state::CaptureMode;

/// Default suppression window for self-write loop detection (`C8.5`).
pub const SELF_WRITE_SUPPRESS_MS: i64 = 1500;

/// Kind of external filesystem change emitted to the frontend (`C4.3`, `C8.5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExternalChangeKind {
    Modified,
    Deleted,
}

/// Payload for `"fs:external-change"` events (`C4.3`, `C8.5`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalChangeEvent {
    pub abs_path: String,
    pub kind: ExternalChangeKind,
    pub new_body: Option<String>,
    pub new_body_hash: Option<String>,
    pub new_meta: Option<NoteMeta>,
}

/// Persisted Quick Bar draft stored at `$APP_DATA_DIR/quickbar-draft.json` (`C6.1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QuickBarDraft {
    pub mode: CaptureMode,
    pub target_rel_path: Option<String>,
    pub selected_workspace_key: Option<String>,
    pub text: String,
    #[specta(type = specta_typescript::Number)]
    pub updated_at: i64,
}

/// Unsaved editor crash-recovery snapshot stored under `$APP_DATA_DIR/recovery/` (`C6.1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySnapshot {
    pub abs_path: String,
    pub rel_path: Option<String>,
    pub dirty_body: String,
    pub base_body_hash: String,
    #[specta(type = specta_typescript::Number)]
    pub updated_at: i64,
}

/// Single recorded internal write entry used for loop suppression (`C8.5`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfWriteRecord {
    pub body_hash: String,
    pub expires_at_ms: i64,
}

/// Pure state machine that suppresses `notify` callbacks triggered by QuickNote's own writes (`C8.5`).
#[derive(Debug, Default)]
pub struct SelfWriteGuard {
    records: HashMap<String, SelfWriteRecord>,
}

impl SelfWriteGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a completed internal write and prunes expired entries.
    pub fn record_write(&mut self, canonical_abs_path: &str, body_hash: &str, now_ms: i64) {
        self.records.retain(|_, rec| rec.expires_at_ms > now_ms);
        self.records.insert(
            canonical_abs_path.to_string(),
            SelfWriteRecord {
                body_hash: body_hash.to_string(),
                expires_at_ms: now_ms + SELF_WRITE_SUPPRESS_MS,
            },
        );
    }

    /// Returns `true` when an observed disk state matches our own recent write (`C8.5`).
    pub fn should_suppress_modify(
        &mut self,
        canonical_abs_path: &str,
        disk_body_hash: &str,
        _now_ms: i64,
    ) -> bool {
        let Some(record) = self.records.get(canonical_abs_path) else {
            return false;
        };

        if disk_body_hash == record.body_hash {
            return true;
        }

        self.records.remove(canonical_abs_path);
        false
    }

    /// Removes cached state for a deleted or moved-away file path.
    pub fn clear_path(&mut self, canonical_abs_path: &str) {
        self.records.remove(canonical_abs_path);
    }
}
