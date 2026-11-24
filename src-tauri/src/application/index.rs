use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::Utc;
use tauri::{AppHandle, Emitter};

use crate::application::persistence::PersistenceManager;
use crate::domain::command_book::parse_command_book;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::markdown::extract_relative_note_links;
use crate::domain::note::{
    compute_body_hash, make_excerpt, parse_note, IndexPhase, IndexStatus, NoteType, INBOX_REL_PATH,
};
use crate::domain::tokenizer::tokenize_mixed;
use crate::infra::fs::map_io_error;
use crate::infra::sqlite::{
    count_notes, get_library_meta, replace_all_notes_index, set_library_meta, upsert_note_index,
    NoteIndexRecord,
};

/// Returns the current index status (`C4.2`, `C4.3`).
pub fn index_get_status(pm: &PersistenceManager) -> Result<IndexStatus, AppError> {
    let (notes_count, last_completed_at) = pm.with_db(|conn| {
        let count = count_notes(conn)?;
        let last = get_library_meta(conn, "last_indexed_at")?.and_then(|s| s.parse::<i64>().ok());
        Ok((count, last))
    })?;

    Ok(IndexStatus {
        phase: IndexPhase::Idle,
        notes_count,
        fts_degraded: false,
        last_completed_at,
    })
}

/// Rebuilds the SQLite + FTS5 derived index from `library_root` (`C5.2`, `C5.3`).
pub fn index_rebuild(
    pm: &PersistenceManager,
    app: Option<&AppHandle>,
) -> Result<IndexStatus, AppError> {
    let root_str = pm.current_library_root()?.ok_or_else(|| {
        AppError::new(
            ErrorCode::LibraryNotConfigured,
            "Library root is not configured",
        )
    })?;
    let root = PathBuf::from(&root_str);
    if !root.is_dir() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Library root directory does not exist",
            root_str,
        ));
    }

    if let Some(handle) = app {
        let _ = handle.emit(
            "index:status",
            &IndexStatus {
                phase: IndexPhase::Scanning,
                notes_count: 0,
                fts_degraded: false,
                last_completed_at: None,
            },
        );
    }

    let files = scan_library_markdown_files(&root)?;
    let now_ms = Utc::now().timestamp_millis();

    struct ScannedFile {
        rel_path: String,
        raw: String,
        mtime_ms: i64,
        size_bytes: i64,
        id: Option<String>,
    }

    let mut scanned = Vec::with_capacity(files.len());
    let mut id_counts: HashMap<String, usize> = HashMap::new();

    for (rel_path, abs_path) in files {
        let raw = fs::read_to_string(&abs_path)
            .map_err(|e| map_io_error(e, "Failed to read markdown file during index rebuild"))?;
        let meta = fs::metadata(&abs_path)
            .map_err(|e| map_io_error(e, "Failed to stat markdown file during index rebuild"))?;
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(now_ms);
        let size_bytes = meta.len() as i64;

        let stem = Path::new(&rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        let parsed = parse_note(&raw, stem);
        if let Some(ref id) = parsed.meta.id {
            *id_counts.entry(id.clone()).or_insert(0) += 1;
        }

        scanned.push(ScannedFile {
            rel_path,
            raw,
            mtime_ms,
            size_bytes,
            id: parsed.meta.id,
        });
    }

    let mut used_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut records = Vec::with_capacity(scanned.len());

    for item in scanned {
        let dup_err = item.id.as_ref().and_then(|id| {
            if id_counts.get(id).copied().unwrap_or(0) > 1 {
                Some(format!("Duplicate id: {id}"))
            } else {
                None
            }
        });

        let key = match &item.id {
            Some(id) if !used_keys.contains(id) => {
                used_keys.insert(id.clone());
                id.clone()
            }
            _ => format!("path:{}", item.rel_path),
        };

        records.push(build_note_index_record(
            &item.rel_path,
            &item.raw,
            item.mtime_ms,
            item.size_bytes,
            now_ms,
            dup_err.as_deref(),
            Some(key),
        ));
    }

    let notes_count = records.len() as u32;
    pm.with_db(|conn| {
        replace_all_notes_index(conn, &records)?;
        set_library_meta(conn, "last_indexed_at", &now_ms.to_string())?;
        Ok(())
    })?;

    let status = IndexStatus {
        phase: IndexPhase::Idle,
        notes_count,
        fts_degraded: false,
        last_completed_at: Some(now_ms),
    };

    if let Some(handle) = app {
        let _ = handle.emit("index:status", &status);
    }

    Ok(status)
}

/// Upserts a single library note into SQLite (`notes`, `note_tags`, `notes_fts`).
pub fn index_single_note(
    pm: &PersistenceManager,
    rel_path: &str,
    raw: &str,
    mtime_ms: i64,
    size_bytes: i64,
) -> Result<(), AppError> {
    let now_ms = Utc::now().timestamp_millis();
    let rec = build_note_index_record(rel_path, raw, mtime_ms, size_bytes, now_ms, None, None);
    pm.with_db(|conn| upsert_note_index(conn, &rec))
}

/// Builds a `NoteIndexRecord` from raw file content and metadata.
pub fn build_note_index_record(
    rel_path: &str,
    raw: &str,
    mtime_ms: i64,
    size_bytes: i64,
    now_ms: i64,
    duplicate_id_error: Option<&str>,
    key_override: Option<String>,
) -> NoteIndexRecord {
    let stem = Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let mut parsed = parse_note(raw, stem);

    let is_template = rel_path.starts_with("_templates/");
    if is_template {
        parsed.meta.doc_type = NoteType::Template;
        parsed.meta.id = None;
        parsed.meta.meta_error = None;
        if parsed.meta.title.trim().is_empty() || parsed.meta.title.contains("{{") {
            parsed.meta.title = stem.to_string();
        }
    }

    if !is_template {
        if let Some(dup_msg) = duplicate_id_error {
            parsed.meta.meta_error = Some(match parsed.meta.meta_error {
                Some(existing) if !existing.is_empty() => format!("{existing}; {dup_msg}"),
                _ => dup_msg.to_string(),
            });
        }
    }

    let key = key_override.unwrap_or_else(|| {
        if is_template {
            format!("path:{rel_path}")
        } else {
            match &parsed.meta.id {
                Some(id) if !id.is_empty() => id.clone(),
                _ => format!("path:{rel_path}"),
            }
        }
    });

    let body_start_line = parsed
        .block
        .as_deref()
        .map(|b| b.matches('\n').count() as u32 + 1)
        .unwrap_or(1);

    let links = if is_template {
        Vec::new()
    } else {
        extract_relative_note_links(rel_path, &parsed.body)
    };

    let commands = if !is_template
        && (parsed.meta.doc_type == NoteType::CommandBook || rel_path.starts_with("commands/"))
    {
        parse_command_book(rel_path, &parsed.body, body_start_line)
    } else {
        Vec::new()
    };

    let frontmatter_json = serde_json::to_string(&parsed.meta).unwrap_or_else(|_| "{}".to_string());
    let excerpt = make_excerpt(&parsed.body, 160);
    let title_tokens = tokenize_mixed(&parsed.meta.title);
    let tag_tokens = tokenize_mixed(&parsed.meta.tags.join(" "));
    let body_tokens = tokenize_mixed(&parsed.body);

    NoteIndexRecord {
        key,
        id: parsed.meta.id.clone(),
        rel_path: rel_path.to_string(),
        title: parsed.meta.title.clone(),
        doc_type: parsed.meta.doc_type.as_str().to_string(),
        project: parsed.meta.project.clone(),
        mtime_ms,
        size_bytes,
        body_hash: compute_body_hash(&parsed.body),
        frontmatter_json,
        excerpt,
        meta_error: parsed.meta.meta_error,
        pinned: parsed.meta.pinned,
        archived: parsed.meta.archived,
        is_inbox: rel_path == INBOX_REL_PATH,
        updated_at: now_ms,
        tags: parsed.meta.tags,
        title_tokens,
        tag_tokens,
        body_tokens,
        links,
        commands,
    }
}

/// Recursively scans `library_root` for `.md` files, ignoring `.` hidden entries and `assets/`.
pub fn scan_library_markdown_files(
    library_root: &Path,
) -> Result<Vec<(String, PathBuf)>, AppError> {
    let mut out = Vec::new();
    scan_dir_recursive(library_root, library_root, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn scan_dir_recursive(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), AppError> {
    let entries = fs::read_dir(dir).map_err(|e| map_io_error(e, "Failed to read directory"))?;
    for entry_res in entries {
        let entry = entry_res.map_err(|e| map_io_error(e, "Failed to read directory entry"))?;
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(root).map_err(|e| {
            AppError::with_detail(ErrorCode::InternalError, "Path strip failed", e.to_string())
        })?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if path.is_dir() {
            if rel_str == "assets" || rel_str.starts_with("assets/") {
                continue;
            }
            scan_dir_recursive(root, &path, out)?;
        } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push((rel_str, path));
        }
    }
    Ok(())
}
