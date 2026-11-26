use std::fs;
use std::path::PathBuf;

use chrono::Local;

use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::markdown::{decode_base64_image, normalize_image_ext};
use crate::domain::note::{SaveImageArgs, SavedImageResult};
use crate::infra::fs::{atomic_write_bytes, map_io_error};

use super::{generate_ulid, require_library_root};

/// Maximum allowed image payload size (20 MiB) for `note_save_image` (`v4` §5.6.3).
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

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
