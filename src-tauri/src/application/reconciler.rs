//! Read-only Vault reconciler for Markdown notes and `.quicknote.yaml` directory workspace configs.
//!
//! Disk files (`.md` and `.quicknote.yaml`) are the single source of truth; reconciliation
//! converges disk state one-way into the derived SQLite cache without mutating user files.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::application::index::build_note_index_record;
use crate::application::persistence::{canonicalize_watch_path, PersistenceManager};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::{compute_body_hash, parse_note};
use crate::domain::workspace_context::{
    is_directory_config_file, DirectoryWorkspaceConfig, DIRECTORY_CONFIG_FILENAME,
};
use crate::infra::fs::{atomic_write_str, map_io_error};
use crate::infra::sqlite::{
    delete_directory_workspace_config, delete_note_index_by_rel_path,
    list_directory_workspace_configs, set_library_meta, upsert_directory_workspace_config,
    upsert_note_index, DirectoryWorkspaceConfigRecord,
};

/// Summary statistics produced by [`reconcile_vault`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub notes_scanned: usize,
    pub notes_upserted: usize,
    pub notes_skipped: usize,
    pub notes_pruned: usize,
    pub configs_scanned: usize,
    pub configs_upserted: usize,
    pub configs_skipped: usize,
    pub configs_pruned: usize,
    pub config_parse_errors: usize,
}

/// Outcome of single-path incremental reconciliation via [`reconcile_path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcilePathOutcome {
    Ignored,
    SuppressedSelfWrite,
    NoteUpserted,
    NoteUnchanged,
    NotePruned,
    ConfigUpserted,
    ConfigParseError(String),
    ConfigUnchanged,
    ConfigPruned,
}

#[derive(Debug, Clone)]
struct ExistingNoteMeta {
    key: String,
    mtime_ms: i64,
    size_bytes: i64,
}

#[derive(Debug, Clone)]
struct ScannedNoteFile {
    rel_path: String,
    abs_path: PathBuf,
    mtime_ms: i64,
    size_bytes: i64,
}

#[derive(Debug, Clone)]
struct ScannedConfigFile {
    vault_rel_dir: String,
    abs_path: PathBuf,
    mtime_ms: i64,
}

/// Performs a full, read-only incremental reconciliation from `vault_root` into SQLite `db`.
///
/// - Recursively traverses `vault_root` and subdirectories, skipping excluded hidden directories
///   (such as `.git`) and `assets/`, while always including files directly under `vault_root`.
/// - Incrementally updates `.md` notes by comparing `(rel_path, mtime_ms, size_bytes)` and prunes
///   deleted entries. Notes without frontmatter or without `id` receive a deterministic
///   `path:<rel_path>` fallback key without mutating disk files.
/// - Parses `.quicknote.yaml` files into `directory_workspace_configs` (never into `notes` or FTS).
///   A malformed `.quicknote.yaml` records `parse_error` and logs a warning without aborting the scan.
pub fn reconcile_vault(vault_root: &Path, db: &Connection) -> Result<ReconcileReport, AppError> {
    if !vault_root.is_dir() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Vault root directory does not exist",
            vault_root.to_string_lossy().to_string(),
        ));
    }

    let now_ms = Utc::now().timestamp_millis();
    let mut report = ReconcileReport::default();

    let mut disk_notes: Vec<ScannedNoteFile> = Vec::new();
    let mut disk_configs: Vec<ScannedConfigFile> = Vec::new();
    scan_vault_recursive(
        vault_root,
        vault_root,
        now_ms,
        &mut disk_notes,
        &mut disk_configs,
    )?;

    disk_notes.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    disk_configs.sort_by(|a, b| a.vault_rel_dir.cmp(&b.vault_rel_dir));

    report.notes_scanned = disk_notes.len();
    report.configs_scanned = disk_configs.len();

    // 1. Reconcile Markdown notes incrementally.
    let existing_notes = load_existing_notes_meta(db)?;
    let mut disk_note_paths: HashSet<String> = HashSet::with_capacity(disk_notes.len());
    let mut key_owners: HashMap<String, String> = HashMap::with_capacity(existing_notes.len());
    for (rel_path, meta) in &existing_notes {
        key_owners.insert(meta.key.clone(), rel_path.clone());
    }

    // Prune deleted notes first so freed keys can be reclaimed cleanly.
    for note in &disk_notes {
        disk_note_paths.insert(note.rel_path.clone());
    }
    for existing_rel in existing_notes.keys() {
        if !disk_note_paths.contains(existing_rel) {
            if let Some(meta) = existing_notes.get(existing_rel) {
                if key_owners.get(&meta.key).map(String::as_str) == Some(existing_rel.as_str()) {
                    key_owners.remove(&meta.key);
                }
            }
            delete_note_index_by_rel_path(db, existing_rel)?;
            report.notes_pruned += 1;
        }
    }

    for note in &disk_notes {
        if let Some(existing) = existing_notes.get(&note.rel_path) {
            if existing.mtime_ms == note.mtime_ms && existing.size_bytes == note.size_bytes {
                report.notes_skipped += 1;
                continue;
            }
        }

        let raw = match fs::read_to_string(&note.abs_path) {
            Ok(content) => content,
            Err(err) => {
                eprintln!(
                    "[reconciler] failed to read markdown file rel_path={}: {}",
                    note.rel_path, err
                );
                continue;
            }
        };

        let rec = build_reconciled_note_record(
            &note.rel_path,
            &raw,
            note.mtime_ms,
            note.size_bytes,
            now_ms,
            &key_owners,
        );
        key_owners.insert(rec.key.clone(), note.rel_path.clone());
        upsert_note_index(db, &rec)?;
        report.notes_upserted += 1;
    }

    // 2. Reconcile `.quicknote.yaml` directory workspace configs incrementally.
    let existing_configs = list_directory_workspace_configs(db)?;
    let existing_config_map: HashMap<String, DirectoryWorkspaceConfigRecord> = existing_configs
        .into_iter()
        .map(|rec| (rec.vault_rel_dir.clone(), rec))
        .collect();
    let mut disk_config_dirs: HashSet<String> = HashSet::with_capacity(disk_configs.len());

    for cfg in &disk_configs {
        disk_config_dirs.insert(cfg.vault_rel_dir.clone());
        if let Some(existing) = existing_config_map.get(&cfg.vault_rel_dir) {
            if existing.mtime_ms == cfg.mtime_ms {
                report.configs_skipped += 1;
                continue;
            }
        }

        let record = read_and_parse_config_record(&cfg.vault_rel_dir, &cfg.abs_path, cfg.mtime_ms);
        if record.parse_error.is_some() {
            report.config_parse_errors += 1;
        }
        upsert_directory_workspace_config(db, &record)?;
        report.configs_upserted += 1;
    }

    for existing_dir in existing_config_map.keys() {
        if !disk_config_dirs.contains(existing_dir) {
            delete_directory_workspace_config(db, existing_dir)?;
            report.configs_pruned += 1;
        }
    }

    set_library_meta(db, "last_indexed_at", &now_ms.to_string())?;

    Ok(report)
}

/// Incrementally reconciles a single vault-relative path (`.md` or `.quicknote.yaml`) into `db`.
///
/// Missing or removed files on disk are pruned from SQLite; existing files are checked against
/// cached metadata and upserted on change. Never mutates user files on disk.
pub fn reconcile_path(
    vault_root: &Path,
    rel_path: &str,
    db: &Connection,
) -> Result<ReconcilePathOutcome, AppError> {
    let Some(normalized_rel) = normalize_rel_path(rel_path) else {
        return Ok(ReconcilePathOutcome::Ignored);
    };
    if normalized_rel.is_empty() {
        return Ok(ReconcilePathOutcome::Ignored);
    }

    let rel_as_path = Path::new(&normalized_rel);
    if is_excluded_vault_rel_path(rel_as_path) {
        return Ok(ReconcilePathOutcome::Ignored);
    }

    let abs_path = vault_root.join(&normalized_rel);

    if is_directory_config_file(&normalized_rel) {
        let vault_rel_dir = parent_rel_dir(&normalized_rel);
        if !abs_path.is_file() {
            delete_directory_workspace_config(db, &vault_rel_dir)?;
            return Ok(ReconcilePathOutcome::ConfigPruned);
        }

        let now_ms = Utc::now().timestamp_millis();
        let (mtime_ms, _) = file_mtime_and_size(&abs_path, now_ms)?;
        if let Some(existing_mtime) = get_existing_config_mtime(db, &vault_rel_dir)? {
            if existing_mtime == mtime_ms {
                return Ok(ReconcilePathOutcome::ConfigUnchanged);
            }
        }

        let record = read_and_parse_config_record(&vault_rel_dir, &abs_path, mtime_ms);
        let parse_error = record.parse_error.clone();
        upsert_directory_workspace_config(db, &record)?;
        return match parse_error {
            Some(err_msg) => Ok(ReconcilePathOutcome::ConfigParseError(err_msg)),
            None => Ok(ReconcilePathOutcome::ConfigUpserted),
        };
    }

    if is_markdown_rel_path(rel_as_path) {
        if !abs_path.is_file() {
            delete_note_index_by_rel_path(db, &normalized_rel)?;
            return Ok(ReconcilePathOutcome::NotePruned);
        }

        let now_ms = Utc::now().timestamp_millis();
        let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path, now_ms)?;
        if let Some((existing_mtime, existing_size)) =
            get_existing_note_mtime_and_size(db, &normalized_rel)?
        {
            if existing_mtime == mtime_ms && existing_size == size_bytes {
                return Ok(ReconcilePathOutcome::NoteUnchanged);
            }
        }

        let raw = fs::read_to_string(&abs_path)
            .map_err(|e| map_io_error(e, "Failed to read markdown file during reconcile_path"))?;
        let existing_notes = load_existing_notes_meta(db)?;
        let key_owners: HashMap<String, String> = existing_notes
            .into_iter()
            .map(|(rel, meta)| (meta.key, rel))
            .collect();

        let rec = build_reconciled_note_record(
            &normalized_rel,
            &raw,
            mtime_ms,
            size_bytes,
            now_ms,
            &key_owners,
        );
        upsert_note_index(db, &rec)?;
        return Ok(ReconcilePathOutcome::NoteUpserted);
    }

    Ok(ReconcilePathOutcome::Ignored)
}

/// Handles a filesystem watch path under `vault_root` with `SelfWriteGuard` suppression,
/// then incrementally converges the path into SQLite via [`reconcile_path`].
pub fn reconcile_watched_path(
    pm: &PersistenceManager,
    vault_root: &Path,
    abs_path: &Path,
) -> Result<ReconcilePathOutcome, AppError> {
    let canonical_root = PathBuf::from(canonicalize_watch_path(vault_root));
    let canonical_path = PathBuf::from(canonicalize_watch_path(abs_path));

    let Ok(rel) = canonical_path.strip_prefix(&canonical_root) else {
        return Ok(ReconcilePathOutcome::Ignored);
    };

    let rel_path = rel_to_slash_string(rel);
    if rel_path.is_empty() || is_excluded_vault_rel_path(Path::new(&rel_path)) {
        return Ok(ReconcilePathOutcome::Ignored);
    }

    if !canonical_path.exists() {
        pm.clear_self_write(&canonical_path);
        return pm.with_db(|conn| reconcile_path(vault_root, &rel_path, conn));
    }

    if !canonical_path.is_file() {
        return Ok(ReconcilePathOutcome::Ignored);
    }

    if is_directory_config_file(&rel_path) {
        if let Ok(raw) = fs::read_to_string(&canonical_path) {
            let content_hash = compute_body_hash(&raw);
            if pm.should_suppress_external_modify(&canonical_path, &content_hash) {
                let _ = pm.with_db(|conn| reconcile_path(vault_root, &rel_path, conn))?;
                return Ok(ReconcilePathOutcome::SuppressedSelfWrite);
            }
        }
        return pm.with_db(|conn| reconcile_path(vault_root, &rel_path, conn));
    }

    if is_markdown_rel_path(Path::new(&rel_path)) {
        let raw = fs::read_to_string(&canonical_path)
            .map_err(|e| map_io_error(e, "Failed to read watched markdown file"))?;
        let stem = canonical_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("note");
        let parsed = parse_note(&raw, stem);
        let disk_body_hash = compute_body_hash(&parsed.body);

        if pm.should_suppress_external_modify(&canonical_path, &disk_body_hash) {
            let _ = pm.with_db(|conn| reconcile_path(vault_root, &rel_path, conn))?;
            return Ok(ReconcilePathOutcome::SuppressedSelfWrite);
        }
        return pm.with_db(|conn| reconcile_path(vault_root, &rel_path, conn));
    }

    Ok(ReconcilePathOutcome::Ignored)
}

/// Atomically writes a `.quicknote.yaml` file inside `vault_root` with `SelfWriteGuard` protection
/// and immediately converges the written config into SQLite cache.
pub fn write_directory_workspace_config(
    pm: &PersistenceManager,
    vault_root: &Path,
    vault_rel_dir: &str,
    config: &DirectoryWorkspaceConfig,
) -> Result<PathBuf, AppError> {
    if !vault_root.is_dir() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Vault root directory does not exist",
            vault_root.to_string_lossy().to_string(),
        ));
    }

    let normalized_dir = normalize_rel_path(vault_rel_dir).ok_or_else(|| {
        AppError::with_detail(
            ErrorCode::ValidationError,
            "Invalid vault relative directory for .quicknote.yaml",
            vault_rel_dir.to_string(),
        )
    })?;

    let rel_path = if normalized_dir.is_empty() {
        DIRECTORY_CONFIG_FILENAME.to_string()
    } else {
        format!("{normalized_dir}/{DIRECTORY_CONFIG_FILENAME}")
    };

    if is_excluded_vault_rel_path(Path::new(&rel_path)) {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Target directory is excluded from vault workspace configuration",
            rel_path,
        ));
    }

    let yaml_content = serde_yaml::to_string(config).map_err(|err| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to serialize .quicknote.yaml",
            err.to_string(),
        )
    })?;

    let target_path = vault_root.join(&rel_path);
    let content_hash = compute_body_hash(&yaml_content);

    // Record self-write before and after atomic write so notify loopbacks are suppressed
    // even on first-time file creation across macOS symlinked paths.
    pm.record_self_write(&target_path, &content_hash);
    atomic_write_str(&target_path, &yaml_content)?;
    pm.record_self_write(&target_path, &content_hash);

    pm.with_db(|conn| reconcile_path(vault_root, &rel_path, conn))?;

    Ok(target_path)
}

fn scan_vault_recursive(
    root: &Path,
    dir: &Path,
    now_ms: i64,
    out_notes: &mut Vec<ScannedNoteFile>,
    out_configs: &mut Vec<ScannedConfigFile>,
) -> Result<(), AppError> {
    let entries = fs::read_dir(dir).map_err(|e| map_io_error(e, "Failed to read vault directory"))?;
    for entry_res in entries {
        let entry = match entry_res {
            Ok(item) => item,
            Err(err) => {
                eprintln!("[reconciler] failed to read directory entry: {err}");
                continue;
            }
        };

        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(err) => {
                eprintln!("[reconciler] failed to read file type: {err}");
                continue;
            }
        };

        if file_type.is_dir() {
            if name.starts_with('.') {
                continue;
            }
            let rel = match path.strip_prefix(root) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let rel_str = rel_to_slash_string(rel);
            if rel_str == "assets" || rel_str.starts_with("assets/") {
                continue;
            }
            scan_vault_recursive(root, &path, now_ms, out_notes, out_configs)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        if name.contains(".tmp.") {
            continue;
        }

        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_str = rel_to_slash_string(rel);
        if rel_str.is_empty() {
            continue;
        }

        if name == DIRECTORY_CONFIG_FILENAME {
            let (mtime_ms, _) = match file_mtime_and_size(&path, now_ms) {
                Ok(v) => v,
                Err(err) => {
                    eprintln!(
                        "[reconciler] failed to stat .quicknote.yaml rel_path={rel_str}: {:?}",
                        err.code
                    );
                    continue;
                }
            };
            out_configs.push(ScannedConfigFile {
                vault_rel_dir: parent_rel_dir(&rel_str),
                abs_path: path,
                mtime_ms,
            });
            continue;
        }

        if name.starts_with('.') {
            continue;
        }

        if is_markdown_rel_path(Path::new(&rel_str)) {
            let (mtime_ms, size_bytes) = match file_mtime_and_size(&path, now_ms) {
                Ok(v) => v,
                Err(err) => {
                    eprintln!(
                        "[reconciler] failed to stat markdown file rel_path={rel_str}: {:?}",
                        err.code
                    );
                    continue;
                }
            };
            out_notes.push(ScannedNoteFile {
                rel_path: rel_str,
                abs_path: path,
                mtime_ms,
                size_bytes,
            });
        }
    }

    Ok(())
}

fn read_and_parse_config_record(
    vault_rel_dir: &str,
    abs_path: &Path,
    mtime_ms: i64,
) -> DirectoryWorkspaceConfigRecord {
    let raw = match fs::read_to_string(abs_path) {
        Ok(content) => content,
        Err(err) => {
            let err_msg = format!("Failed to read .quicknote.yaml: {err}");
            eprintln!(
                "[reconciler] warning: unreadable .quicknote.yaml dir='{}': {}",
                vault_rel_dir, err
            );
            return DirectoryWorkspaceConfigRecord {
                vault_rel_dir: vault_rel_dir.to_string(),
                config_json: "{}".to_string(),
                mtime_ms,
                parse_error: Some(err_msg),
            };
        }
    };

    if raw.trim().is_empty() {
        return DirectoryWorkspaceConfigRecord {
            vault_rel_dir: vault_rel_dir.to_string(),
            config_json: "{}".to_string(),
            mtime_ms,
            parse_error: None,
        };
    }

    match serde_yaml::from_str::<DirectoryWorkspaceConfig>(&raw) {
        Ok(cfg) => {
            let config_json = serde_json::to_string(&cfg).unwrap_or_else(|_| "{}".to_string());
            DirectoryWorkspaceConfigRecord {
                vault_rel_dir: vault_rel_dir.to_string(),
                config_json,
                mtime_ms,
                parse_error: None,
            }
        }
        Err(err) => {
            let err_msg = err.to_string();
            eprintln!(
                "[reconciler] warning: malformed .quicknote.yaml dir='{}': {}",
                vault_rel_dir, err_msg
            );
            DirectoryWorkspaceConfigRecord {
                vault_rel_dir: vault_rel_dir.to_string(),
                config_json: "{}".to_string(),
                mtime_ms,
                parse_error: Some(err_msg),
            }
        }
    }
}

fn build_reconciled_note_record(
    rel_path: &str,
    raw: &str,
    mtime_ms: i64,
    size_bytes: i64,
    now_ms: i64,
    key_owners: &HashMap<String, String>,
) -> crate::infra::sqlite::NoteIndexRecord {
    let stem = Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let parsed = parse_note(raw, stem);

    let is_template = rel_path.starts_with("_templates/");
    let mut duplicate_id_error: Option<String> = None;
    let mut key_override: Option<String> = None;

    if !is_template {
        if let Some(id) = parsed.meta.id.as_deref().filter(|s| !s.is_empty()) {
            if let Some(owner_rel) = key_owners.get(id) {
                if owner_rel != rel_path {
                    duplicate_id_error = Some(format!("Duplicate id: {id}"));
                    key_override = Some(format!("path:{rel_path}"));
                }
            }
        }
    }

    build_note_index_record(
        rel_path,
        raw,
        mtime_ms,
        size_bytes,
        now_ms,
        duplicate_id_error.as_deref(),
        key_override,
    )
}

fn load_existing_notes_meta(db: &Connection) -> Result<HashMap<String, ExistingNoteMeta>, AppError> {
    let mut stmt = db
        .prepare("SELECT rel_path, key, mtime_ms, size_bytes FROM notes")
        .map_err(|e| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to prepare existing notes query",
                e.to_string(),
            )
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ExistingNoteMeta {
                    key: row.get(1)?,
                    mtime_ms: row.get(2)?,
                    size_bytes: row.get(3)?,
                },
            ))
        })
        .map_err(|e| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to query existing notes",
                e.to_string(),
            )
        })?;

    let mut out = HashMap::new();
    for row in rows {
        let (rel_path, meta) = row.map_err(|e| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to read existing note row",
                e.to_string(),
            )
        })?;
        out.insert(rel_path, meta);
    }
    Ok(out)
}

fn get_existing_note_mtime_and_size(
    db: &Connection,
    rel_path: &str,
) -> Result<Option<(i64, i64)>, AppError> {
    db.query_row(
        "SELECT mtime_ms, size_bytes FROM notes WHERE rel_path = ?1",
        params![rel_path],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )
    .optional()
    .map_err(|e| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to query note mtime and size",
            e.to_string(),
        )
    })
}

fn get_existing_config_mtime(
    db: &Connection,
    vault_rel_dir: &str,
) -> Result<Option<i64>, AppError> {
    db.query_row(
        "SELECT mtime_ms FROM directory_workspace_configs WHERE vault_rel_dir = ?1",
        params![vault_rel_dir],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(|e| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to query directory workspace config mtime",
            e.to_string(),
        )
    })
}

fn file_mtime_and_size(path: &Path, fallback_ms: i64) -> Result<(i64, i64), AppError> {
    let meta = fs::metadata(path).map_err(|e| map_io_error(e, "Failed to read file metadata"))?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(fallback_ms);
    Ok((mtime_ms, meta.len() as i64))
}

fn normalize_rel_path(raw: &str) -> Option<String> {
    let replaced = raw.replace('\\', "/");
    let mut segments = Vec::new();
    for segment in replaced.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return None;
        }
        segments.push(segment);
    }
    Some(segments.join("/"))
}

fn parent_rel_dir(rel_path: &str) -> String {
    match rel_path.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

fn is_markdown_rel_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

fn is_excluded_vault_rel_path(rel: &Path) -> bool {
    let components: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    if components.is_empty() {
        return true;
    }

    let last_idx = components.len() - 1;
    for (idx, part) in components.iter().enumerate() {
        if part.contains(".tmp.") {
            return true;
        }
        if idx == 0 && part == "assets" {
            return true;
        }
        if idx < last_idx {
            if part.starts_with('.') {
                return true;
            }
        } else if part != DIRECTORY_CONFIG_FILENAME && part.starts_with('.') {
            return true;
        }
    }

    false
}

fn rel_to_slash_string(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
