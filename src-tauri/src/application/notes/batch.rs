use std::fs;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter};

use crate::application::index::{index_get_status, index_single_note};
use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::frontmatter::{
    apply_yaml_patch, build_frontmatter_block, replace_frontmatter_yaml, YamlPatchOp,
    YamlPatchValue,
};
use crate::domain::note::{
    compose_note, compute_body_hash, meta_patch_ops, normalize_tag_name, parse_note,
    rewrite_tags_for_rename, tag_matches, BackfillIdsResult, NoteMetaPatch, TagRenameArgs,
    TagRenameResult,
};
use crate::infra::fs::{atomic_write_str, map_io_error};
use crate::infra::sqlite::list_all_indexed_notes;

use super::{file_mtime_and_size, generate_ulid, require_library_root};

/// Renames or merges a tag (and its hierarchical subtags) across all patchable library notes (`C4.2`, `C6.4`, `D-0006`).
pub fn tags_batch_rename(
    pm: &PersistenceManager,
    args: TagRenameArgs,
) -> Result<TagRenameResult, AppError> {
    let root = require_library_root(pm)?;
    let old_tag = normalize_tag_name(&args.old_tag);
    let new_tag = normalize_tag_name(&args.new_tag);

    if old_tag.is_empty() || new_tag.is_empty() || old_tag == new_tag {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "oldTag and newTag must be non-empty and distinct",
        ));
    }

    let rows = pm.with_db(list_all_indexed_notes)?;
    let mut updated_files: u32 = 0;

    for row in rows {
        if row.rel_path.starts_with("_templates/") || row.meta_error.is_some() {
            continue;
        }
        if !row.tags.iter().any(|t| tag_matches(t, &old_tag)) {
            continue;
        }

        let abs_path = root.join(&row.rel_path);
        if !abs_path.is_file() {
            continue;
        }

        let raw = fs::read_to_string(&abs_path)
            .map_err(|e| map_io_error(e, "Failed to read note during tag batch rename"))?;
        let stem = abs_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        let parsed = parse_note(&raw, stem);
        if !parsed.yaml_patchable || parsed.meta.meta_error.is_some() {
            continue;
        }

        let Some(new_tags) = rewrite_tags_for_rename(&parsed.meta.tags, &old_tag, &new_tag) else {
            continue;
        };

        let patch = NoteMetaPatch {
            tags: Some(new_tags),
            ..NoteMetaPatch::default()
        };
        let ops = meta_patch_ops(&patch);
        let base_yaml = parsed.yaml.as_deref().unwrap_or("");
        let patched_yaml = match apply_yaml_patch(base_yaml, &ops) {
            Ok(y) => y,
            Err(_) => continue,
        };
        let new_block = build_frontmatter_block(&patched_yaml);
        let new_content = compose_note(Some(&new_block), &parsed.body);

        pm.record_self_write(&abs_path, &compute_body_hash(&parsed.body));
        if let Err(err) = atomic_write_str(&abs_path, &new_content) {
            pm.clear_self_write(&abs_path);
            return Err(err);
        }
        let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;
        index_single_note(pm, &row.rel_path, &new_content, mtime_ms, size_bytes as i64)?;
        updated_files += 1;
    }

    Ok(TagRenameResult { updated_files })
}

fn collect_backfill_markdown_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), AppError> {
    let entries = fs::read_dir(dir).map_err(|e| map_io_error(e, "Failed to read directory"))?;
    for entry_res in entries {
        let entry = entry_res.map_err(|e| map_io_error(e, "Failed to read directory entry"))?;
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        if name.starts_with('.') || name.contains(".tmp.") {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(root).map_err(|e| {
            AppError::with_detail(ErrorCode::InternalError, "Path strip failed", e.to_string())
        })?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if path.is_dir() {
            if rel_str == "_templates"
                || rel_str.starts_with("_templates/")
                || rel_str == "assets"
                || rel_str.starts_with("assets/")
            {
                continue;
            }
            collect_backfill_markdown_files(root, &path, out)?;
        } else if path.is_file()
            && path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            out.push((rel_str, path));
        }
    }
    Ok(())
}

/// Scans `library_root` and safely back-fills ULID `id` fields for notes missing an ID (`D-0006`).
///
/// - Excludes `_templates/`, hidden entries (`.` prefix), `assets/`, and `.tmp.` files.
/// - Skips notes that already have a non-empty `meta.id`.
/// - Never mutates notes with broken or unterminated YAML frontmatter.
/// - Preserves comments and unknown fields via `apply_yaml_patch` for valid frontmatter,
///   or prepends a minimal `id`-only frontmatter block when no frontmatter exists.
/// - Writes atomically with self-write suppression and syncs SQLite via `index_single_note`.
pub fn notes_backfill_missing_ids(
    pm: &PersistenceManager,
    app: Option<&AppHandle>,
) -> Result<BackfillIdsResult, AppError> {
    let root = require_library_root(pm)?;
    if !root.is_dir() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Library root directory does not exist",
            root.to_string_lossy().to_string(),
        ));
    }

    let mut files = Vec::new();
    collect_backfill_markdown_files(&root, &root, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut result = BackfillIdsResult::default();

    for (rel_path, abs_path) in files {
        result.scanned += 1;

        let Ok(raw) = fs::read_to_string(&abs_path) else {
            result.failed += 1;
            continue;
        };
        let stem = abs_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        let parsed = parse_note(&raw, stem);

        if parsed
            .meta
            .id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty())
        {
            result.skipped_existing += 1;
            continue;
        }

        if (!parsed.yaml_patchable && parsed.block.is_some())
            || (parsed.block.is_none() && parsed.meta.meta_error.is_some())
        {
            result.skipped_broken_yaml += 1;
            continue;
        }

        let new_id = generate_ulid();
        let new_content = if parsed.block.is_some() {
            let base_yaml = parsed.yaml.as_deref().unwrap_or("");
            let ops = vec![YamlPatchOp::set("id", YamlPatchValue::Str(new_id))];
            let Ok(patched_yaml) = apply_yaml_patch(base_yaml, &ops) else {
                result.failed += 1;
                continue;
            };
            replace_frontmatter_yaml(&raw, &patched_yaml)
        } else {
            let minimal_yaml = format!("id: {new_id}\n");
            let block = build_frontmatter_block(&minimal_yaml);
            compose_note(Some(&block), &parsed.body)
        };

        let updated_parsed = parse_note(&new_content, stem);
        pm.record_self_write(&abs_path, &compute_body_hash(&updated_parsed.body));
        if atomic_write_str(&abs_path, &new_content).is_err() {
            pm.clear_self_write(&abs_path);
            result.failed += 1;
            continue;
        }

        let Ok((mtime_ms, size_bytes)) = file_mtime_and_size(&abs_path) else {
            result.failed += 1;
            continue;
        };
        if index_single_note(pm, &rel_path, &new_content, mtime_ms, size_bytes as i64).is_err() {
            result.failed += 1;
            continue;
        }

        result.updated += 1;
    }

    if result.updated > 0 {
        if let Some(handle) = app {
            if let Ok(status) = index_get_status(pm) {
                let _ = handle.emit("index:status", &status);
            }
        }
    }

    Ok(result)
}
