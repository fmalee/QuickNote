use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use chrono::{Local, Utc};
use tauri::{AppHandle, Emitter, Manager};

use crate::application::index::index_single_note;
use crate::application::persistence::PersistenceManager;
use crate::application::window::WindowManager;
use crate::domain::capture::{
    append_to_body, apply_doc_section_hints, resolve_target, AppendNoteArgs, AppendResult,
    ResolveDocCandidate, ResolveTargetContext, ResolvedTarget, TargetKind, TargetResolveInput,
    UndoAppendResult,
};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::{compose_note, compute_body_hash, parse_note, INBOX_REL_PATH};
use crate::domain::ui_state::{LastActiveDocState, UiStatePatchKey};
use crate::domain::watch::{ExternalChangeEvent, ExternalChangeKind};
use crate::infra::fs::{atomic_write_str, map_io_error};
use crate::infra::sqlite::list_all_indexed_notes;

/// Maximum original body size (5 MiB) eligible for single-slot append undo (`C8.3`).
pub const MAX_UNDO_BODY_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone)]
struct LastAppendUndo {
    rel_path: String,
    after_body_hash: String,
    before_body: String,
    #[allow(dead_code)]
    created_at_ms: i64,
}

/// Manages Quick Bar target resolution, body append, and 5-second single-slot undo (`C8.1`–`C8.3`).
#[derive(Debug, Default)]
pub struct CaptureManager {
    last_undo: Mutex<Option<LastAppendUndo>>,
}

impl CaptureManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves the Quick Bar capture target using the `S1–S8` truth table (`C8.1`).
    pub fn resolve_target(
        &self,
        pm: &PersistenceManager,
        args: TargetResolveInput,
    ) -> Result<ResolvedTarget, AppError> {
        let now_ms = Utc::now().timestamp_millis();
        let today_date = Local::now().format("%Y-%m-%d").to_string();

        let Some(root_str) = pm.current_library_root()? else {
            let empty_ui = pm.ui_state_get().unwrap_or_default();
            let empty_workspaces = BTreeMap::new();
            let ctx = ResolveTargetContext {
                library_configured: false,
                workspaces: &empty_workspaces,
                ui_state: &empty_ui,
                docs: &[],
                last_active_ttl_hours: 24,
                auto_create_missing_section: false,
                now_ms,
                today_date: &today_date,
            };
            return Ok(resolve_target(&ctx, &args));
        };

        let root = PathBuf::from(&root_str);
        let snapshot = pm.settings_get()?;
        let ui_state = pm.ui_state_get()?;
        let rows = pm.with_db(list_all_indexed_notes)?;

        let docs: Vec<ResolveDocCandidate> = rows
            .into_iter()
            .filter(|row| root.join(&row.rel_path).is_file())
            .map(|row| ResolveDocCandidate {
                key: row.key,
                rel_path: row.rel_path,
                title: row.title,
                project: row.project,
                has_ideas_section: None,
                has_todo_section: None,
            })
            .collect();

        let ctx = ResolveTargetContext {
            library_configured: true,
            workspaces: &snapshot.settings.workspaces,
            ui_state: &ui_state,
            docs: &docs,
            last_active_ttl_hours: snapshot.settings.quick_bar.last_active_ttl_hours,
            auto_create_missing_section: snapshot.settings.quick_bar.auto_create_missing_section,
            now_ms,
            today_date: &today_date,
        };

        let mut resolved = resolve_target(&ctx, &args);
        if resolved.kind == TargetKind::Note {
            if let Some(rel) = resolved.target_rel_path.as_deref() {
                let abs_path = root.join(rel);
                if let Ok(raw) = fs::read_to_string(&abs_path) {
                    let parsed = parse_note(&raw, file_stem(&abs_path));
                    apply_doc_section_hints(
                        &mut resolved,
                        &parsed.body,
                        snapshot.settings.quick_bar.auto_create_missing_section,
                    );
                }
            }
        }

        Ok(resolved)
    }

    /// Appends content to a note or `inbox.md` without altering YAML frontmatter (`C8.2`, `D-0006`).
    pub fn note_append(
        &self,
        pm: &PersistenceManager,
        args: AppendNoteArgs,
        app: Option<&AppHandle>,
    ) -> Result<AppendResult, AppError> {
        let root = require_library_root(pm)?;
        let rel_path = normalize_rel_path(&args.target_rel_path)?;
        let abs_path = root.join(&rel_path);
        let is_inbox = rel_path == INBOX_REL_PATH;

        if is_inbox && !abs_path.exists() {
            let initial_inbox = "---\ntitle: 收件箱\ntype: inbox\n---\n";
            atomic_write_str(&abs_path, initial_inbox)?;
        } else if !abs_path.is_file() {
            return Err(AppError::with_detail(
                ErrorCode::NotFound,
                "Target note file not found",
                rel_path,
            ));
        }

        let raw =
            fs::read_to_string(&abs_path).map_err(|e| map_io_error(e, "Failed to read note"))?;
        let parsed = parse_note(&raw, file_stem(&abs_path));

        let local_now = Local::now();
        let today_date = local_now.format("%Y-%m-%d").to_string();
        let hh_mm = local_now.format("%H:%M").to_string();
        let now_ms = Utc::now().timestamp_millis();

        let auto_create_missing_section = pm
            .settings_get()?
            .settings
            .quick_bar
            .auto_create_missing_section;

        let output = append_to_body(
            &parsed.body,
            &args.content,
            args.mode,
            is_inbox,
            &today_date,
            &hh_mm,
            auto_create_missing_section,
        )?;

        let after_body_hash = compute_body_hash(&output.new_body);
        let undo_available = parsed.body.len() <= MAX_UNDO_BODY_BYTES;

        // Preserve original YAML frontmatter bytes verbatim (`D-0006`).
        let new_raw = compose_note(parsed.block.as_deref(), &output.new_body);
        atomic_write_str(&abs_path, &new_raw)?;

        let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;
        index_single_note(pm, &rel_path, &new_raw, mtime_ms, size_bytes as i64)?;
        pm.record_self_write(&abs_path, &after_body_hash);
        let _ = pm.clear_quickbar_draft();

        {
            let mut guard = self.last_undo.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Capture undo mutex poisoned")
            })?;
            if undo_available {
                *guard = Some(LastAppendUndo {
                    rel_path: rel_path.clone(),
                    after_body_hash: after_body_hash.clone(),
                    before_body: parsed.body,
                    created_at_ms: now_ms,
                });
            } else {
                *guard = None;
            }
        }

        let effective_ws_key = args
            .workspace_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| parsed.meta.project.clone());

        let last_active = LastActiveDocState {
            rel_path: rel_path.clone(),
            workspace_key: effective_ws_key.clone(),
            touched_at_ms: now_ms,
        };
        let last_active_val = serde_json::to_value(last_active).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize last_active_doc",
                err.to_string(),
            )
        })?;
        pm.ui_state_set(UiStatePatchKey::LastActiveDoc, last_active_val)?;

        if !is_inbox {
            if let Some(ws_key) = effective_ws_key {
                let mut ui_state = pm.ui_state_get()?;
                ui_state.workspace_modes.insert(ws_key, args.mode);
                let modes_val = serde_json::to_value(ui_state.workspace_modes).map_err(|err| {
                    AppError::with_detail(
                        ErrorCode::InternalError,
                        "Failed to serialize workspace_modes",
                        err.to_string(),
                    )
                })?;
                pm.ui_state_set(UiStatePatchKey::WorkspaceModes, modes_val)?;
            }
        }

        let result = AppendResult {
            rel_path,
            title: parsed.meta.title,
            added_lines: output.added_lines,
            section_heading: output.section_heading,
            body_hash: after_body_hash,
            mtime_ms,
            undo_available,
        };

        if let Some(handle) = app {
            if let Some(wm) = handle.try_state::<std::sync::Arc<WindowManager>>() {
                if let Err(err) = wm.show_append_toast(handle, &result) {
                    eprintln!("[capture] show_append_toast failed: code={:?}", err.code);
                }
            }
        }

        Ok(result)
    }

    /// Reverts the last append if disk body hash still matches `after_body_hash` (`C8.3`).
    pub fn note_undo_last_append(
        &self,
        pm: &PersistenceManager,
        app: Option<&AppHandle>,
    ) -> Result<UndoAppendResult, AppError> {
        let snapshot = {
            let mut guard = self.last_undo.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Capture undo mutex poisoned")
            })?;
            guard.take()
        }
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::UndoUnavailable,
                "No recent append operation available to undo",
            )
        })?;

        let root = require_library_root(pm)?;
        let abs_path = root.join(&snapshot.rel_path);
        if !abs_path.is_file() {
            return Err(AppError::with_detail(
                ErrorCode::UndoUnavailable,
                "Target note no longer exists on disk",
                snapshot.rel_path,
            ));
        }

        let raw =
            fs::read_to_string(&abs_path).map_err(|e| map_io_error(e, "Failed to read note"))?;
        let parsed = parse_note(&raw, file_stem(&abs_path));
        let current_body_hash = compute_body_hash(&parsed.body);

        if current_body_hash != snapshot.after_body_hash {
            return Err(AppError::new(
                ErrorCode::UndoUnavailable,
                "Note body has been modified since the last append",
            ));
        }

        // Preserve latest disk frontmatter while restoring `before_body` (`C8.3`).
        let restored_raw = compose_note(parsed.block.as_deref(), &snapshot.before_body);
        atomic_write_str(&abs_path, &restored_raw)?;

        let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;
        index_single_note(
            pm,
            &snapshot.rel_path,
            &restored_raw,
            mtime_ms,
            size_bytes as i64,
        )?;
        let restored_body_hash = compute_body_hash(&snapshot.before_body);
        pm.record_self_write(&abs_path, &restored_body_hash);

        if let Some(handle) = app {
            let _ = handle.emit(
                "fs:external-change",
                &ExternalChangeEvent {
                    abs_path: abs_path.to_string_lossy().to_string(),
                    kind: ExternalChangeKind::Modified,
                    new_body: Some(snapshot.before_body),
                    new_body_hash: Some(restored_body_hash.clone()),
                    new_meta: Some(parsed.meta),
                },
            );
        }

        Ok(UndoAppendResult {
            rel_path: snapshot.rel_path,
            restored_body_hash,
            mtime_ms,
        })
    }
}

fn require_library_root(pm: &PersistenceManager) -> Result<PathBuf, AppError> {
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
    Ok(root)
}

fn normalize_rel_path(rel: &str) -> Result<String, AppError> {
    let cleaned = rel.trim().replace('\\', "/");
    let trimmed = cleaned.trim_start_matches('/');
    if trimmed.is_empty() || trimmed.contains("..") {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Invalid relative path",
        ));
    }
    Ok(trimmed.to_string())
}

fn file_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
}

fn file_mtime_and_size(path: &Path) -> Result<(i64, u64), AppError> {
    let meta = fs::metadata(path).map_err(|e| map_io_error(e, "Failed to stat note file"))?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    Ok((mtime_ms, meta.len()))
}
