/// Maximum number of backup files retained in `$APP_DATA_DIR/backups/` (`C6.1`).
pub const MAX_BACKUP_COUNT: usize = 50;

/// Maximum age in seconds (30 days) for backup files (`C6.1`).
pub const MAX_BACKUP_AGE_SECS: i64 = 30 * 24 * 60 * 60;

/// Lightweight metadata for a backup file used in pure pruning decisions (`C6.1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub file_name: String,
    pub modified_epoch_secs: i64,
}

/// Formats a backup filename as `<name>.<YYYYMMDD-HHmmss>.bak` (`C6.1`).
pub fn format_backup_filename(name: &str, timestamp: &str) -> String {
    format!("{name}.{timestamp}.bak")
}

/// Selects backup filenames to remove so that only the newest `MAX_BACKUP_COUNT` (50)
/// entries within `MAX_BACKUP_AGE_SECS` (30 days) of `now_epoch_secs` are retained (`C6.1`).
pub fn select_backups_to_prune(entries: &[BackupEntry], now_epoch_secs: i64) -> Vec<String> {
    let mut within_window: Vec<&BackupEntry> = Vec::new();
    let mut to_delete: Vec<String> = Vec::new();

    for entry in entries {
        let age_secs = now_epoch_secs.saturating_sub(entry.modified_epoch_secs);
        if age_secs > MAX_BACKUP_AGE_SECS {
            to_delete.push(entry.file_name.clone());
        } else {
            within_window.push(entry);
        }
    }

    within_window.sort_by(|a, b| {
        b.modified_epoch_secs
            .cmp(&a.modified_epoch_secs)
            .then_with(|| b.file_name.cmp(&a.file_name))
    });

    for entry in within_window.into_iter().skip(MAX_BACKUP_COUNT) {
        to_delete.push(entry.file_name.clone());
    }

    to_delete
}
