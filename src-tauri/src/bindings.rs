use std::path::Path;

use specta::Types;
use specta_typescript::Typescript;

use crate::domain::app::{ActivationPolicy, AppInfo};
use crate::domain::error::{AppError, ErrorCode};

/// Exact header line required at line 1 of `src/bindings/types.ts` (`C1.4`).
pub const GENERATED_HEADER: &str = "/* GENERATED — DO NOT EDIT. */";

/// Collects all Rust IPC types exported to TypeScript (`C3.1`, `D-0009`).
pub fn collect_types() -> Types {
    Types::default()
        .register::<ActivationPolicy>()
        .register::<AppInfo>()
        .register::<ErrorCode>()
        .register::<AppError>()
}

/// Renders the full TypeScript bindings content deterministically.
pub fn render_ts_bindings() -> Result<String, AppError> {
    let types = collect_types();
    Typescript::default()
        .header(GENERATED_HEADER)
        .export(&types, specta_serde::Format)
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to render TypeScript IPC bindings",
                err.to_string(),
            )
        })
}

/// Exports the generated TypeScript bindings to `path`.
pub fn export_ts_bindings(path: &Path) -> Result<(), AppError> {
    let rendered = render_ts_bindings()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to create bindings parent directory",
                err.to_string(),
            )
        })?;
    }
    std::fs::write(path, rendered).map_err(|err| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to write TypeScript bindings file",
            err.to_string(),
        )
    })
}
