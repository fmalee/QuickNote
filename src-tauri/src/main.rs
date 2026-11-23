#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(err) = quicknote_lib::run() {
        eprintln!("Failed to run QuickNote application: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
