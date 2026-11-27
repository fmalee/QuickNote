use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, Utc};

use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::markdown::{
    decode_base64_image, extract_relative_asset_refs, normalize_asset_rel_path, normalize_image_ext,
};
use crate::domain::note::{
    CleanUnusedAssetsArgs, CleanUnusedAssetsResult, SaveImageArgs, SavedImageResult,
    ScanUnusedAssetsArgs, UnusedAssetEntry, UnusedAssetsScanResult,
};
use crate::infra::fs::{atomic_write_bytes, map_io_error};
use crate::infra::sqlite::list_all_referenced_assets;
use crate::infra::trash::{default_file_remover, FileRemover};

use super::{file_mtime_and_size, generate_ulid, require_library_root};

/// Maximum allowed image payload size (20 MiB) for `note_save_image` (`v4` §5.6.3).
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

/// Default grace period (10 minutes) protecting newly saved assets from automatic cleanup
/// before their referencing document is persisted.
pub const DEFAULT_ASSET_CLEAN_GRACE_SECS: u32 = 600;

/// Saves an image from base64 or a local source path into `assets/YYYY-MM/` (`C4.2`, `v4` §5.6.3).
pub fn note_save_image(
    pm: &PersistenceManager,
    args: SaveImageArgs,
) -> Result<SavedImageResult, AppError> {
    let root = require_library_root(pm)?;
    let ext = normalize_image_ext(&args.ext)?;

    let bytes = if let Some(b64) = args
        .bytes_base64
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        decode_base64_image(b64, MAX_IMAGE_BYTES)?
    } else if let Some(src) = args
        .source_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let src_path = PathBuf::from(src);
        if !src_path.is_file() {
            return Err(AppError::with_detail(
                ErrorCode::NotFound,
                "Source image file not found",
                src.to_string(),
            ));
        }
        let meta = fs::metadata(&src_path).map_err(|e| map_io_error(e, "Failed to stat image"))?;
        if meta.len() > MAX_IMAGE_BYTES as u64 {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "Image file exceeds maximum allowed size",
            ));
        }
        let data = fs::read(&src_path).map_err(|e| map_io_error(e, "Failed to read image file"))?;
        if data.len() > MAX_IMAGE_BYTES {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "Image file exceeds maximum allowed size",
            ));
        }
        data
    } else {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Either bytesBase64 or sourcePath must be provided",
        ));
    };

    let now = Local::now();
    let ym = now.format("%Y-%m").to_string();
    let ts = now.format("%Y%m%d-%H%M%S").to_string();
    let ulid = generate_ulid().to_ascii_lowercase();
    let short_ulid = ulid.get(ulid.len().saturating_sub(8)..).unwrap_or(&ulid);
    let rel_path = format!("assets/{ym}/{ts}-{short_ulid}.{ext}");
    let abs_path = root.join(&rel_path);

    atomic_write_bytes(&abs_path, &bytes)?;

    Ok(SavedImageResult { rel_path })
}

/// Scans `assets/` for files that are not referenced by any indexed note, template, or unsaved draft.
pub fn assets_scan_unused(
    pm: &PersistenceManager,
    args: ScanUnusedAssetsArgs,
) -> Result<UnusedAssetsScanResult, AppError> {
    let root = require_library_root(pm)?;
    let protected = collect_protected_assets(pm, &root, &args.protected_rel_paths)?;

    let grace_ms = i64::from(
        args.grace_period_seconds
            .unwrap_or(DEFAULT_ASSET_CLEAN_GRACE_SECS),
    ) * 1000;
    let now_ms = Utc::now().timestamp_millis();

    let disk_assets = scan_disk_assets(&root)?;
    let mut items = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut skipped_recent_count: u32 = 0;

    for entry in disk_assets {
        if protected.contains(&entry.rel_path) {
            continue;
        }
        if grace_ms > 0 && now_ms.saturating_sub(entry.mtime_ms) < grace_ms {
            skipped_recent_count = skipped_recent_count.saturating_add(1);
            continue;
        }
        total_bytes = total_bytes.saturating_add(entry.size_bytes);
        items.push(entry);
    }

    Ok(UnusedAssetsScanResult {
        items,
        total_bytes,
        skipped_recent_count,
    })
}

/// Moves unreferenced files under `assets/` to the system Trash, pruning empty `assets/*` subdirectories.
pub fn assets_clean_unused(
    pm: &PersistenceManager,
    args: CleanUnusedAssetsArgs,
) -> Result<CleanUnusedAssetsResult, AppError> {
    assets_clean_unused_with(pm, args, default_file_remover().as_ref())
}

/// Injectable variant of [`assets_clean_unused`] for tests (avoids polluting the real system Trash).
pub fn assets_clean_unused_with(
    pm: &PersistenceManager,
    args: CleanUnusedAssetsArgs,
    remover: &dyn FileRemover,
) -> Result<CleanUnusedAssetsResult, AppError> {
    let root = require_library_root(pm)?;
    let protected = collect_protected_assets(pm, &root, &args.protected_rel_paths)?;

    let grace_ms = i64::from(
        args.grace_period_seconds
            .unwrap_or(DEFAULT_ASSET_CLEAN_GRACE_SECS),
    ) * 1000;
    let now_ms = Utc::now().timestamp_millis();

    let candidates: Vec<UnusedAssetEntry> = match args.rel_paths {
        Some(raw_paths) => {
            let mut unique = BTreeSet::new();
            for raw in raw_paths {
                if let Some(norm) = normalize_asset_rel_path(&raw) {
                    unique.insert(norm);
                }
            }
            let mut out = Vec::new();
            for rel_path in unique {
                let abs_path = root.join(&rel_path);
                if !abs_path.is_file() {
                    continue;
                }
                if let Ok((mtime_ms, size_bytes)) = file_mtime_and_size(&abs_path) {
                    out.push(UnusedAssetEntry {
                        rel_path,
                        size_bytes,
                        mtime_ms,
                    });
                }
            }
            out
        }
        None => scan_disk_assets(&root)?,
    };

    let mut result = CleanUnusedAssetsResult::default();
    for entry in candidates {
        if protected.contains(&entry.rel_path) {
            result.skipped_referenced_count = result.skipped_referenced_count.saturating_add(1);
            continue;
        }
        if grace_ms > 0 && now_ms.saturating_sub(entry.mtime_ms) < grace_ms {
            result.skipped_recent_count = result.skipped_recent_count.saturating_add(1);
            continue;
        }

        let abs_path = root.join(&entry.rel_path);
        if !abs_path.is_file() {
            continue;
        }

        match remover.remove(&abs_path) {
            Ok(()) => {
                result.deleted_paths.push(entry.rel_path);
                result.freed_bytes = result.freed_bytes.saturating_add(entry.size_bytes);
                prune_empty_asset_dirs(&root, &abs_path);
            }
            Err(_) => {
                result.failed_count = result.failed_count.saturating_add(1);
            }
        }
    }

    Ok(result)
}

/// Cleans up assets that became unreferenced immediately after a note was explicitly deleted.
pub(super) fn cleanup_orphaned_assets_for_deleted_note(
    pm: &PersistenceManager,
    candidate_assets: Vec<String>,
    remover: &dyn FileRemover,
) -> Result<CleanUnusedAssetsResult, AppError> {
    if candidate_assets.is_empty() {
        return Ok(CleanUnusedAssetsResult::default());
    }
    assets_clean_unused_with(
        pm,
        CleanUnusedAssetsArgs {
            rel_paths: Some(candidate_assets),
            protected_rel_paths: Vec::new(),
            grace_period_seconds: Some(0),
        },
        remover,
    )
}

fn collect_protected_assets(
    pm: &PersistenceManager,
    root: &Path,
    extra_protected: &[String],
) -> Result<HashSet<String>, AppError> {
    let mut protected = pm.with_db(list_all_referenced_assets)?;

    // Defense-in-depth: scan `_templates/` directly so template images stay protected
    // even if a template was added outside the index lifecycle.
    let templates_dir = root.join("_templates");
    if templates_dir.is_dir() {
        collect_template_asset_refs_recursive(root, &templates_dir, &mut protected);
    }

    for raw in extra_protected {
        if let Some(norm) = normalize_asset_rel_path(raw) {
            protected.insert(norm);
        }
    }

    Ok(protected)
}

fn collect_template_asset_refs_recursive(root: &Path, dir: &Path, out: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            collect_template_asset_refs_recursive(root, &path, out);
        } else if ft.is_file() {
            let is_md = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !is_md {
                continue;
            }
            if let (Ok(rel), Ok(raw)) = (path.strip_prefix(root), fs::read_to_string(&path)) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                for asset in extract_relative_asset_refs(&rel_str, &raw) {
                    out.insert(asset);
                }
            }
        }
    }
}

fn scan_disk_assets(root: &Path) -> Result<Vec<UnusedAssetEntry>, AppError> {
    let assets_dir = root.join("assets");
    if !assets_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    scan_assets_dir_recursive(root, &assets_dir, &mut out)?;
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(out)
}

fn scan_assets_dir_recursive(
    root: &Path,
    dir: &Path,
    out: &mut Vec<UnusedAssetEntry>,
) -> Result<(), AppError> {
    let entries = fs::read_dir(dir)
        .map_err(|e| map_io_error(e, "Failed to read library assets directory"))?;

    for entry in entries {
        let entry = entry.map_err(|e| map_io_error(e, "Failed to read assets directory entry"))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| map_io_error(e, "Failed to inspect assets entry type"))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            scan_assets_dir_recursive(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = match path.strip_prefix(root) {
                Ok(r) => r.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            if let Some(norm) = normalize_asset_rel_path(&rel) {
                if let Ok((mtime_ms, size_bytes)) = file_mtime_and_size(&path) {
                    out.push(UnusedAssetEntry {
                        rel_path: norm,
                        size_bytes,
                        mtime_ms,
                    });
                }
            }
        }
    }

    Ok(())
}

/// Removes empty subdirectories inside `assets/` after deleting an asset file,
/// stopping strictly below `root/assets` so the root `assets/` folder is preserved.
fn prune_empty_asset_dirs(root: &Path, deleted_file: &Path) {
    let assets_root = root.join("assets");
    let mut current = deleted_file.parent();
    while let Some(dir) = current {
        if dir == assets_root || !dir.starts_with(&assets_root) {
            break;
        }
        match fs::remove_dir(dir) {
            Ok(()) => {
                current = dir.parent();
            }
            Err(_) => break,
        }
    }
}
