use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::application::notes::{
    assets_clean_unused as clean_unused_assets_uc, assets_scan_unused as scan_unused_assets_uc,
    note_create as create_uc, note_delete as delete_uc, note_read as read_uc,
    note_rename as rename_uc, note_save_image as save_image_uc, note_update_meta as update_meta_uc,
    note_write as write_uc, notes_backfill_missing_ids as backfill_ids_uc,
    notes_get_related as related_uc, notes_get_tree_and_tags as tree_uc, notes_list as list_uc,
    tags_batch_rename as tag_rename_uc,
};
use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::{
    BackfillIdsResult, CleanUnusedAssetsArgs, CleanUnusedAssetsResult, CreateNoteArgs,
    ExportPdfArgs, NavTreeSnapshot, NoteDoc, NoteSummary, NoteUpdateMetaArgs, NotesFilterArgs,
    ReadNoteArgs, RelPathArgs, RelatedNoteItem, RenameNoteArgs, SaveImageArgs, SavedImageResult,
    ScanUnusedAssetsArgs, TagRenameArgs, TagRenameResult, UnusedAssetsScanResult, WriteNoteArgs,
    WriteResult,
};
use crate::infra::fs::{expand_tilde, map_io_error};
use crate::infra::macos::NativeWindowBridge;

const MAX_EXPORT_HTML_BYTES: usize = 20 * 1024 * 1024;

#[tauri::command]
pub fn notes_list(
    pm: State<'_, Arc<PersistenceManager>>,
    args: NotesFilterArgs,
) -> Result<Vec<NoteSummary>, AppError> {
    list_uc(&pm, args)
}

#[tauri::command]
pub fn notes_get_tree_and_tags(
    pm: State<'_, Arc<PersistenceManager>>,
) -> Result<NavTreeSnapshot, AppError> {
    tree_uc(&pm)
}

#[tauri::command]
pub fn note_read(
    pm: State<'_, Arc<PersistenceManager>>,
    args: ReadNoteArgs,
) -> Result<NoteDoc, AppError> {
    read_uc(&pm, args)
}

#[tauri::command]
pub fn note_create(
    pm: State<'_, Arc<PersistenceManager>>,
    args: CreateNoteArgs,
) -> Result<NoteDoc, AppError> {
    create_uc(&pm, args)
}

#[tauri::command]
pub fn note_write(
    pm: State<'_, Arc<PersistenceManager>>,
    args: WriteNoteArgs,
) -> Result<WriteResult, AppError> {
    write_uc(&pm, args)
}

#[tauri::command]
pub fn note_update_meta(
    pm: State<'_, Arc<PersistenceManager>>,
    args: NoteUpdateMetaArgs,
) -> Result<NoteDoc, AppError> {
    update_meta_uc(&pm, args)
}

#[tauri::command]
pub fn note_rename(
    pm: State<'_, Arc<PersistenceManager>>,
    args: RenameNoteArgs,
) -> Result<NoteDoc, AppError> {
    rename_uc(&pm, args)
}

#[tauri::command]
pub fn note_delete(
    pm: State<'_, Arc<PersistenceManager>>,
    args: RelPathArgs,
) -> Result<(), AppError> {
    delete_uc(&pm, args)
}

#[tauri::command]
pub fn note_save_image(
    pm: State<'_, Arc<PersistenceManager>>,
    args: SaveImageArgs,
) -> Result<SavedImageResult, AppError> {
    save_image_uc(&pm, args)
}

#[tauri::command]
pub fn assets_scan_unused(
    pm: State<'_, Arc<PersistenceManager>>,
    args: Option<ScanUnusedAssetsArgs>,
) -> Result<UnusedAssetsScanResult, AppError> {
    scan_unused_assets_uc(&pm, args.unwrap_or_default())
}

#[tauri::command]
pub fn assets_clean_unused(
    pm: State<'_, Arc<PersistenceManager>>,
    args: Option<CleanUnusedAssetsArgs>,
) -> Result<CleanUnusedAssetsResult, AppError> {
    clean_unused_assets_uc(&pm, args.unwrap_or_default())
}

#[tauri::command]
pub fn notes_get_related(
    pm: State<'_, Arc<PersistenceManager>>,
    args: RelPathArgs,
) -> Result<Vec<RelatedNoteItem>, AppError> {
    related_uc(&pm, args)
}

#[tauri::command]
pub fn tags_batch_rename(
    pm: State<'_, Arc<PersistenceManager>>,
    args: TagRenameArgs,
) -> Result<TagRenameResult, AppError> {
    tag_rename_uc(&pm, args)
}

#[tauri::command]
pub fn notes_backfill_missing_ids(
    app: AppHandle,
    pm: State<'_, Arc<PersistenceManager>>,
) -> Result<BackfillIdsResult, AppError> {
    backfill_ids_uc(&pm, Some(&app))
}

/// Validates and exports an HTML document to PDF via the native bridge (`C4.2`, `C2.3`).
pub fn export_pdf_with_bridge(
    bridge: &dyn NativeWindowBridge,
    args: ExportPdfArgs,
) -> Result<PathBuf, AppError> {
    if args.html_document.trim().is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "HTML document must not be empty",
        ));
    }
    if args.html_document.len() > MAX_EXPORT_HTML_BYTES {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "HTML document exceeds maximum 20MB export limit",
        ));
    }

    let trimmed_dest = args.dest_path.trim();
    if trimmed_dest.is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Destination path must not be empty",
        ));
    }

    let mut dest = expand_tilde(trimmed_dest);
    let has_pdf_ext = dest
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));
    if !has_pdf_ext {
        let mut os = dest.into_os_string();
        os.push(".pdf");
        dest = PathBuf::from(os);
    }

    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| map_io_error(e, "Failed to create PDF export directory"))?;
        }
    }

    bridge.render_html_to_pdf(&args.html_document, &dest)?;
    Ok(dest)
}

#[tauri::command]
pub fn note_export_pdf(
    bridge: State<'_, Arc<dyn NativeWindowBridge>>,
    args: ExportPdfArgs,
) -> Result<(), AppError> {
    export_pdf_with_bridge(bridge.inner().as_ref(), args).map(|_| ())
}
