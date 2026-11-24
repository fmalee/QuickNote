//! Safe OS process launchers for external reveal/open/editor/terminal/clipboard actions (`C2.1`, `v4` §5.10).
//!
//! All subprocess invocations use `Command::new(...).args([...])` parameter arrays.
//! Never passes user paths through `sh -c` or AppleScript string interpolation.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::domain::error::{AppError, ErrorCode};
use crate::domain::settings::ExternalAppConfig;

/// Abstraction over macOS external app/clipboard execution for testable application logic.
pub trait ExternalLauncher: Send + Sync {
    fn reveal_in_finder(&self, abs_path: &Path) -> Result<(), AppError>;
    fn open_default(&self, abs_path: &Path) -> Result<(), AppError>;
    fn open_with_app(&self, app: &ExternalAppConfig, target: &Path) -> Result<(), AppError>;
    fn open_terminal(&self, app: &ExternalAppConfig, dir_path: &Path) -> Result<(), AppError>;
    fn copy_to_clipboard(&self, text: &str) -> Result<(), AppError>;
}

/// Default macOS implementation using `open` and `pbcopy`.
#[derive(Debug, Default)]
pub struct MacosExternalLauncher;

impl MacosExternalLauncher {
    fn run_open(args: &[&std::ffi::OsStr]) -> Result<(), String> {
        let output = Command::new("open")
            .args(args)
            .output()
            .map_err(|err| err.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                Err(format!("open exited with status {}", output.status))
            } else {
                Err(stderr)
            }
        }
    }

    fn open_app_target(app: &ExternalAppConfig, target: &Path) -> Result<(), AppError> {
        let bundle_id = app.bundle_id.trim();
        let name = app.name.trim();
        let target_os = target.as_os_str();

        if !bundle_id.is_empty() {
            let flag = std::ffi::OsStr::new("-b");
            let bid = std::ffi::OsStr::new(bundle_id);
            if Self::run_open(&[flag, bid, target_os]).is_ok() {
                return Ok(());
            }
        }

        if !name.is_empty() {
            let flag = std::ffi::OsStr::new("-a");
            let app_name = std::ffi::OsStr::new(name);
            match Self::run_open(&[flag, app_name, target_os]) {
                Ok(()) => return Ok(()),
                Err(err) => {
                    return Err(AppError::with_detail(
                        ErrorCode::ExternalAppMissing,
                        format!("无法打开外部应用：{}", name),
                        err,
                    ));
                }
            }
        }

        Err(AppError::new(
            ErrorCode::ExternalAppMissing,
            if bundle_id.is_empty() {
                "未配置外部应用".to_string()
            } else {
                format!("无法打开外部应用：{bundle_id}")
            },
        ))
    }
}

impl ExternalLauncher for MacosExternalLauncher {
    fn reveal_in_finder(&self, abs_path: &Path) -> Result<(), AppError> {
        Self::run_open(&[std::ffi::OsStr::new("-R"), abs_path.as_os_str()])
            .map_err(|err| AppError::with_detail(ErrorCode::InternalError, "在访达中显示失败", err))
    }

    fn open_default(&self, abs_path: &Path) -> Result<(), AppError> {
        Self::run_open(&[abs_path.as_os_str()]).map_err(|err| {
            AppError::with_detail(ErrorCode::InternalError, "使用默认应用打开失败", err)
        })
    }

    fn open_with_app(&self, app: &ExternalAppConfig, target: &Path) -> Result<(), AppError> {
        Self::open_app_target(app, target)
    }

    fn open_terminal(&self, app: &ExternalAppConfig, dir_path: &Path) -> Result<(), AppError> {
        Self::open_app_target(app, dir_path)
    }

    fn copy_to_clipboard(&self, text: &str) -> Result<(), AppError> {
        let mut child = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "启动 pbcopy 失败",
                    err.to_string(),
                )
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).map_err(|err| {
                AppError::with_detail(ErrorCode::InternalError, "写入剪贴板失败", err.to_string())
            })?;
        }

        let output = child.wait_with_output().map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "等待 pbcopy 失败",
                err.to_string(),
            )
        })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(AppError::with_detail(
                ErrorCode::InternalError,
                "复制到剪贴板失败",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ))
        }
    }
}
