use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, Utc};

use crate::application::index::index_single_note;
use crate::application::persistence::PersistenceManager;
use crate::domain::backup::format_backup_filename;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::frontmatter::{
    apply_yaml_patch, build_frontmatter_block, render_yaml_string, replace_frontmatter_yaml,
    YamlPatchOp, YamlPatchValue,
};
use crate::domain::note::{
    compose_note, compute_body_hash, meta_patch_ops, parse_note, ConflictInfo, CreateNoteArgs,
    NoteDoc, NoteMeta, NoteMetaPatch, NoteType, NoteUpdateMetaArgs, ReadNoteArgs, RelPathArgs,
    RenameNoteArgs, WriteNoteArgs, WriteResult, INBOX_REL_PATH,
};
use crate::domain::template::{builtin_template, render_template, TemplateVars};
use crate::infra::fs::{atomic_write_str, map_io_error, prune_backups};
use crate::infra::sqlite::{delete_note_index_by_rel_path, get_note_key_by_rel_path};
use crate::infra::trash::{default_file_remover, FileRemover};

use super::{
    expand_input_path, file_mtime_and_size, generate_ulid, is_external_file_pinned,
    is_markdown_path, normalize_rel_path, require_library_root, resolve_rel_path,
    sanitize_filename,
};

/// Reads a note by absolute path (supports both library and external `.md` files) (`C4.2`).
pub fn note_read(pm: &PersistenceManager, args: ReadNoteArgs) -> Result<NoteDoc, AppError> {
    let abs_path = expand_input_path(&args.abs_path);
    if !abs_path.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Note file not found",
            abs_path.to_string_lossy().to_string(),
        ));
    }

    let raw = fs::read_to_string(&abs_path).map_err(|e| map_io_error(e, "Failed to read note"))?;
    let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;

    let root = pm.current_library_root()?.map(PathBuf::from);
    let rel_path = root.as_deref().and_then(|r| resolve_rel_path(r, &abs_path));
    let is_library = rel_path.is_some();
    let is_inbox = rel_path.as_deref() == Some(INBOX_REL_PATH);

    if !is_markdown_path(&abs_path) {
        let title = abs_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        let meta = NoteMeta {
            title,
            doc_type: NoteType::Note,
            pinned: if is_library {
                false
            } else {
                is_external_file_pinned(pm, &abs_path)
            },
            ..NoteMeta::default()
        };
        let body_hash = compute_body_hash(&raw);
        let key = format!("abs:{}", abs_path.to_string_lossy());
        return Ok(NoteDoc {
            key,
            rel_path,
            abs_path: abs_path.to_string_lossy().to_string(),
            is_library,
            is_inbox: false,
            meta,
            body: raw,
            body_hash,
            mtime_ms,
            size_bytes,
        });
    }

    let stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let mut parsed = parse_note(&raw, stem);
    if !is_library && !parsed.meta.pinned {
        parsed.meta.pinned = is_external_file_pinned(pm, &abs_path);
    }

    let key = if let Some(ref rel) = rel_path {
        pm.with_db(|conn| get_note_key_by_rel_path(conn, rel))?
            .unwrap_or_else(|| {
                parsed
                    .meta
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("path:{rel}"))
            })
    } else {
        parsed
            .meta
            .id
            .clone()
            .unwrap_or_else(|| format!("abs:{}", abs_path.to_string_lossy()))
    };

    Ok(NoteDoc {
        key,
        rel_path,
        abs_path: abs_path.to_string_lossy().to_string(),
        is_library,
        is_inbox,
        meta: parsed.meta,
        body_hash: compute_body_hash(&parsed.body),
        body: parsed.body,
        mtime_ms,
        size_bytes,
    })
}

/// Creates a new note in `dir_rel_path`, writes it atomically, and indexes it (`C4.2`).
pub fn note_create(pm: &PersistenceManager, args: CreateNoteArgs) -> Result<NoteDoc, AppError> {
    let title = args.title.trim();
    if title.is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Note title cannot be empty",
        ));
    }

    let root = require_library_root(pm)?;
    let dir_clean = args
        .dir_rel_path
        .trim()
        .trim_matches('/')
        .replace('\\', "/");
    if dir_clean.contains("..") {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Invalid directory relative path",
        ));
    }

    let file_name = format!("{}.md", sanitize_filename(title));
    let rel_path = if dir_clean.is_empty() {
        file_name
    } else {
        format!("{dir_clean}/{file_name}")
    };
    let abs_path = root.join(&rel_path);
    if abs_path.exists() {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Note file already exists",
            rel_path,
        ));
    }

    let raw_template = resolve_template_raw(&root, args.template_name.as_deref())?;
    let id = generate_ulid();
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M:%S").to_string();
    let rendered = render_template(
        &raw_template,
        &TemplateVars {
            id: &id,
            title,
            date: &date,
            time: &time,
        },
    );

    let content = match args.body.as_deref() {
        Some(extra) if !extra.trim().is_empty() => {
            let trimmed_tpl = rendered.trim_end_matches('\n');
            let trimmed_extra = extra.trim_start_matches('\n').trim_end_matches('\n');
            format!("{trimmed_tpl}\n\n{trimmed_extra}\n")
        }
        _ if rendered.ends_with('\n') => rendered,
        _ => format!("{rendered}\n"),
    };

    let stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");
    let created_parsed = parse_note(&content, stem);
    pm.record_self_write(&abs_path, &compute_body_hash(&created_parsed.body));
    if let Err(err) = atomic_write_str(&abs_path, &content) {
        pm.clear_self_write(&abs_path);
        return Err(err);
    }
    let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;
    index_single_note(pm, &rel_path, &content, mtime_ms, size_bytes as i64)?;

    note_read(
        pm,
        ReadNoteArgs {
            abs_path: abs_path.to_string_lossy().to_string(),
        },
    )
}

fn resolve_template_raw(root: &Path, template_name: Option<&str>) -> Result<String, AppError> {
    let clean_name = match template_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => {
            let stripped = raw.strip_suffix(".md").unwrap_or(raw);
            if stripped.is_empty()
                || stripped.contains('/')
                || stripped.contains('\\')
                || stripped.contains("..")
            {
                return Err(AppError::new(
                    ErrorCode::ValidationError,
                    "Invalid template name",
                ));
            }
            stripped
        }
        None => "note",
    };

    let custom_path = root.join("_templates").join(format!("{clean_name}.md"));
    if custom_path.is_file() {
        return fs::read_to_string(&custom_path)
            .map_err(|e| map_io_error(e, "Failed to read template file"));
    }

    if let Some(builtin) = builtin_template(clean_name) {
        return Ok(builtin.to_string());
    }

    Err(AppError::with_detail(
        ErrorCode::NotFound,
        "Template not found",
        clean_name.to_string(),
    ))
}

/// Writes `body` with optimistic concurrency check (`base_body_hash`) and ULID back-fill (`D-0006`).
pub fn note_write(pm: &PersistenceManager, args: WriteNoteArgs) -> Result<WriteResult, AppError> {
    let abs_path = expand_input_path(&args.abs_path);
    if !abs_path.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Note file not found",
            abs_path.to_string_lossy().to_string(),
        ));
    }

    let raw = fs::read_to_string(&abs_path).map_err(|e| map_io_error(e, "Failed to read note"))?;
    let (current_mtime_ms, _) = file_mtime_and_size(&abs_path)?;

    if !is_markdown_path(&abs_path) {
        let disk_body_hash = compute_body_hash(&raw);
        if disk_body_hash != args.base_body_hash {
            let conflict = ConflictInfo {
                abs_path: abs_path.to_string_lossy().to_string(),
                current_body: raw,
                current_body_hash: disk_body_hash,
                current_mtime_ms,
            };
            let detail = serde_json::to_string(&conflict).map_err(|e| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to serialize conflict info",
                    e.to_string(),
                )
            })?;
            return Err(AppError::with_detail(
                ErrorCode::Conflict,
                "Document modified on disk",
                detail,
            ));
        }

        let new_body_hash = compute_body_hash(&args.body);
        pm.record_self_write(&abs_path, &new_body_hash);
        if let Err(err) = atomic_write_str(&abs_path, &args.body) {
            pm.clear_self_write(&abs_path);
            return Err(err);
        }
        let (new_mtime_ms, _) = file_mtime_and_size(&abs_path)?;
        let _ = pm.clear_recovery_snapshot(&args.abs_path);
        let abs_lossy = abs_path.to_string_lossy();
        if abs_lossy != args.abs_path {
            let _ = pm.clear_recovery_snapshot(&abs_lossy);
        }

        return Ok(WriteResult {
            body_hash: new_body_hash,
            mtime_ms: new_mtime_ms,
            updated_meta: None,
        });
    }

    let stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let parsed = parse_note(&raw, stem);

    let disk_body_hash = compute_body_hash(&parsed.body);
    if disk_body_hash != args.base_body_hash {
        let conflict = ConflictInfo {
            abs_path: abs_path.to_string_lossy().to_string(),
            current_body: parsed.body,
            current_body_hash: disk_body_hash,
            current_mtime_ms,
        };
        let detail = serde_json::to_string(&conflict).map_err(|e| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize conflict info",
                e.to_string(),
            )
        })?;
        return Err(AppError::with_detail(
            ErrorCode::Conflict,
            "Document modified on disk",
            detail,
        ));
    }

    let root = pm.current_library_root()?.map(PathBuf::from);
    let rel_path = root.as_deref().and_then(|r| resolve_rel_path(r, &abs_path));
    let is_library = rel_path.is_some();

    let mut updated_meta: Option<NoteMeta> = None;
    let new_content = if is_library && parsed.meta.id.is_none() && parsed.yaml_patchable {
        let new_id = generate_ulid();
        let ops = meta_patch_ops(&NoteMetaPatch::default());
        let mut id_ops = ops;
        id_ops.push(YamlPatchOp {
            key: "id".to_string(),
            value: Some(YamlPatchValue::Str(new_id.clone())),
        });
        let base_yaml = parsed.yaml.as_deref().unwrap_or("");
        let patched_yaml = apply_yaml_patch(base_yaml, &id_ops).map_err(|e| {
            AppError::with_detail(
                ErrorCode::InvalidFrontmatter,
                "Failed to patch frontmatter id",
                format!("{e:?}"),
            )
        })?;
        let new_block = build_frontmatter_block(&patched_yaml);
        let mut new_meta = parsed.meta.clone();
        new_meta.id = Some(new_id);
        updated_meta = Some(new_meta);
        compose_note(Some(&new_block), &args.body)
    } else {
        compose_note(parsed.block.as_deref(), &args.body)
    };

    let new_parsed = parse_note(&new_content, stem);
    let new_body_hash = compute_body_hash(&new_parsed.body);
    pm.record_self_write(&abs_path, &new_body_hash);
    if let Err(err) = atomic_write_str(&abs_path, &new_content) {
        pm.clear_self_write(&abs_path);
        return Err(err);
    }
    let (new_mtime_ms, new_size_bytes) = file_mtime_and_size(&abs_path)?;

    if let Some(rel) = rel_path {
        index_single_note(pm, &rel, &new_content, new_mtime_ms, new_size_bytes as i64)?;
    }

    let _ = pm.clear_recovery_snapshot(&args.abs_path);
    let abs_lossy = abs_path.to_string_lossy();
    if abs_lossy != args.abs_path {
        let _ = pm.clear_recovery_snapshot(&abs_lossy);
    }

    Ok(WriteResult {
        body_hash: new_body_hash,
        mtime_ms: new_mtime_ms,
        updated_meta,
    })
}

/// Applies a partial metadata patch to frontmatter, preserving unknown keys and comments (`D-0006`).
pub fn note_update_meta(
    pm: &PersistenceManager,
    args: NoteUpdateMetaArgs,
) -> Result<NoteDoc, AppError> {
    let abs_path = expand_input_path(&args.abs_path);
    if !abs_path.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Note file not found",
            abs_path.to_string_lossy().to_string(),
        ));
    }
    if !is_markdown_path(&abs_path) {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Only Markdown files support frontmatter metadata",
            abs_path.to_string_lossy().to_string(),
        ));
    }

    let raw = fs::read_to_string(&abs_path).map_err(|e| map_io_error(e, "Failed to read note"))?;
    let stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let parsed = parse_note(&raw, stem);
    let ops = meta_patch_ops(&args.patch);

    let new_content = if parsed.yaml_patchable {
        let base_yaml = parsed.yaml.as_deref().unwrap_or("");
        let patched_yaml = apply_yaml_patch(base_yaml, &ops).map_err(|e| {
            AppError::with_detail(
                ErrorCode::InvalidFrontmatter,
                "Failed to patch frontmatter",
                format!("{e:?}"),
            )
        })?;
        replace_frontmatter_yaml(&raw, &patched_yaml)
    } else if parsed.block.is_none() {
        // File has no frontmatter yet (not broken YAML): synthesize minimal frontmatter without backing up.
        let merged_meta = merge_meta_with_patch(parsed.meta, &args.patch);
        let minimal_yaml = build_minimal_yaml(&merged_meta);
        let block = build_frontmatter_block(&minimal_yaml);
        compose_note(Some(&block), &parsed.body)
    } else {
        // Broken YAML: back up original file to $APP_DATA_DIR/backups/ first.
        let backups_dir = pm.backups_dir();
        let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let file_name = abs_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("note.md");
        let backup_path = backups_dir.join(format_backup_filename(file_name, &ts));
        atomic_write_str(&backup_path, &raw)?;
        let _ = prune_backups(&backups_dir, Utc::now().timestamp());

        let merged_meta = merge_meta_with_patch(parsed.meta, &args.patch);
        let minimal_yaml = build_minimal_yaml(&merged_meta);
        let block = build_frontmatter_block(&minimal_yaml);
        compose_note(Some(&block), &parsed.body)
    };

    let meta_stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");
    let updated_parsed = parse_note(&new_content, meta_stem);
    pm.record_self_write(&abs_path, &compute_body_hash(&updated_parsed.body));
    if let Err(err) = atomic_write_str(&abs_path, &new_content) {
        pm.clear_self_write(&abs_path);
        return Err(err);
    }
    let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;

    let root = pm.current_library_root()?.map(PathBuf::from);
    if let Some(rel) = root.as_deref().and_then(|r| resolve_rel_path(r, &abs_path)) {
        index_single_note(pm, &rel, &new_content, mtime_ms, size_bytes as i64)?;
    }

    note_read(
        pm,
        ReadNoteArgs {
            abs_path: args.abs_path,
        },
    )
}

fn merge_meta_with_patch(mut meta: NoteMeta, patch: &NoteMetaPatch) -> NoteMeta {
    if let Some(ref title) = patch.title {
        meta.title = title.clone();
    }
    if let Some(doc_type) = patch.doc_type {
        meta.doc_type = doc_type;
    }
    if let Some(ref tags) = patch.tags {
        meta.tags = tags.clone();
    }
    if let Some(ref project) = patch.project {
        meta.project = project.clone();
    }
    if let Some(ref open_dir) = patch.open_dir {
        meta.open_dir = open_dir.clone();
    }
    if let Some(ref cw) = patch.code_workspace {
        meta.code_workspace = cw.clone();
    }
    if let Some(ref td) = patch.terminal_dir {
        meta.terminal_dir = td.clone();
    }
    if let Some(pinned) = patch.pinned {
        meta.pinned = pinned;
    }
    if let Some(archived) = patch.archived {
        meta.archived = archived;
    }
    meta
}

fn build_minimal_yaml(meta: &NoteMeta) -> String {
    let mut lines = Vec::new();
    if let Some(ref id) = meta.id {
        lines.push(format!("id: {}", render_yaml_string(id)));
    }
    lines.push(format!("title: {}", render_yaml_string(&meta.title)));
    lines.push(format!("type: {}", meta.doc_type.as_str()));
    if meta.tags.is_empty() {
        lines.push("tags: []".to_string());
    } else {
        lines.push("tags:".to_string());
        for t in &meta.tags {
            lines.push(format!("  - {}", render_yaml_string(t)));
        }
    }
    if let Some(ref p) = meta.project {
        lines.push(format!("project: {}", render_yaml_string(p)));
    }
    if let Some(ref open_dir) = meta.open_dir {
        lines.push(format!("open_dir: {}", render_yaml_string(open_dir)));
    }
    if let Some(ref cw) = meta.code_workspace {
        lines.push(format!("code_workspace: {}", render_yaml_string(cw)));
    }
    if let Some(ref td) = meta.terminal_dir {
        lines.push(format!("terminal_dir: {}", render_yaml_string(td)));
    }
    if let Some(ref created) = meta.created {
        let escaped = created.replace('\\', "\\\\").replace('"', "\\\"");
        lines.push(format!("created: \"{escaped}\""));
    }
    lines.push(format!("pinned: {}", meta.pinned));
    lines.push(format!("archived: {}", meta.archived));
    lines.join("\n") + "\n"
}

/// Renames or moves a library note (`inbox.md` cannot be renamed) (`C4.2`).
pub fn note_rename(pm: &PersistenceManager, args: RenameNoteArgs) -> Result<NoteDoc, AppError> {
    let root = require_library_root(pm)?;
    let old_rel = normalize_rel_path(&args.rel_path)?;
    let new_rel = normalize_rel_path(&args.new_rel_path)?;

    if old_rel == INBOX_REL_PATH || new_rel == INBOX_REL_PATH {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "inbox.md cannot be renamed",
        ));
    }

    let old_abs = root.join(&old_rel);
    let new_abs = root.join(&new_rel);
    if !old_abs.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Source note not found",
            old_rel,
        ));
    }
    if new_abs.exists() {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Target note path already exists",
            new_rel,
        ));
    }

    if let Some(parent) = new_abs.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| map_io_error(e, "Failed to create target directory"))?;
    }
    let rename_stem = new_abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");
    let old_raw = fs::read_to_string(&old_abs)
        .map_err(|e| map_io_error(e, "Failed to read note before rename"))?;
    let old_parsed = parse_note(&old_raw, rename_stem);
    pm.record_self_write(&new_abs, &compute_body_hash(&old_parsed.body));
    if let Err(e) = fs::rename(&old_abs, &new_abs) {
        pm.clear_self_write(&new_abs);
        return Err(map_io_error(e, "Failed to rename note file"));
    }

    let raw =
        fs::read_to_string(&new_abs).map_err(|e| map_io_error(e, "Failed to read renamed note"))?;
    let (mtime_ms, size_bytes) = file_mtime_and_size(&new_abs)?;

    pm.with_db(|conn| delete_note_index_by_rel_path(conn, &old_rel))?;
    index_single_note(pm, &new_rel, &raw, mtime_ms, size_bytes as i64)?;
    pm.clear_self_write(&old_abs);

    note_read(
        pm,
        ReadNoteArgs {
            abs_path: new_abs.to_string_lossy().to_string(),
        },
    )
}

/// Deletes a library note (`inbox.md` cannot be deleted) by moving it to the system Trash
/// and removes it from SQLite (`C4.2`).
pub fn note_delete(pm: &PersistenceManager, args: RelPathArgs) -> Result<(), AppError> {
    note_delete_with(pm, args, default_file_remover().as_ref())
}

/// Injectable variant of [`note_delete`] for tests.
pub fn note_delete_with(
    pm: &PersistenceManager,
    args: RelPathArgs,
    remover: &dyn FileRemover,
) -> Result<(), AppError> {
    let root = require_library_root(pm)?;
    let rel = normalize_rel_path(&args.rel_path)?;
    if rel == INBOX_REL_PATH {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "inbox.md cannot be deleted",
        ));
    }

    let abs_path = root.join(&rel);
    if !abs_path.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Note file not found",
            rel,
        ));
    }

    remover.remove(&abs_path)?;
    pm.with_db(|conn| delete_note_index_by_rel_path(conn, &rel))?;
    pm.clear_self_write(&abs_path);
    Ok(())
}
