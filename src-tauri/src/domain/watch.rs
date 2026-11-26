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

/// Maximum recent self-write hashes retained per path within the suppression window (`C8.5`).
const MAX_SELF_WRITE_HISTORY_PER_PATH: usize = 8;

/// Pure state machine that suppresses `notify` callbacks triggered by QuickNote's own writes (`C8.5`).
/// Keeps a bounded per-path history within [`SELF_WRITE_SUPPRESS_MS`] so rapid consecutive autosaves
/// do not evict earlier in-flight FSEvents hashes and cause false conflict alerts.
#[derive(Debug, Default)]
pub struct SelfWriteGuard {
    records: HashMap<String, Vec<SelfWriteRecord>>,
}

impl SelfWriteGuard {
    pub fn new() -> Self {
        Self::default()
    }

    fn prune_expired(&mut self, now_ms: i64) {
        self.records.retain(|_, entries| {
            entries.retain(|rec| now_ms <= rec.expires_at_ms);
            !entries.is_empty()
        });
    }

    /// Records a completed internal write and prunes expired entries.
    pub fn record_write(&mut self, canonical_abs_path: &str, body_hash: &str, now_ms: i64) {
        self.prune_expired(now_ms);
        let expires_at_ms = now_ms + SELF_WRITE_SUPPRESS_MS;
        let entries = self
            .records
            .entry(canonical_abs_path.to_string())
            .or_default();
        if let Some(existing) = entries.iter_mut().find(|rec| rec.body_hash == body_hash) {
            existing.expires_at_ms = expires_at_ms;
        } else {
            entries.push(SelfWriteRecord {
                body_hash: body_hash.to_string(),
                expires_at_ms,
            });
            if entries.len() > MAX_SELF_WRITE_HISTORY_PER_PATH {
                let overflow = entries.len() - MAX_SELF_WRITE_HISTORY_PER_PATH;
                entries.drain(0..overflow);
            }
        }
    }

    /// Returns `true` when an observed disk state matches any of our own recent writes (`C8.5`).
    pub fn should_suppress_modify(
        &mut self,
        canonical_abs_path: &str,
        disk_body_hash: &str,
        now_ms: i64,
    ) -> bool {
        self.prune_expired(now_ms);
        let Some(entries) = self.records.get(canonical_abs_path) else {
            return false;
        };

        if entries.iter().any(|rec| rec.body_hash == disk_body_hash) {
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
