use std::path::PathBuf;
use std::process::ExitCode;

use quicknote_lib::bindings::export_ts_bindings;

fn resolve_output_path() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .ok_or_else(|| "Unable to resolve repository root from CARGO_MANIFEST_DIR".to_string())?;
    Ok(workspace_root.join("src").join("bindings").join("types.ts"))
}

fn main() -> ExitCode {
    let output_path = match resolve_output_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = export_ts_bindings(&output_path) {
        eprintln!("Failed to export TypeScript bindings: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
