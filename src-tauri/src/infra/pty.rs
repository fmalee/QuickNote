use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::domain::error::{AppError, ErrorCode};

/// Configuration for spawning a PTY shell process (`C8.7`, `D-0005`).
#[derive(Debug, Clone)]
pub struct PtySpawnConfig {
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub shell: String,
}

/// Abstracted PTY session handle supporting both real `portable-pty` and headless test doubles.
pub trait PtySessionHandle: Send + Sync {
    /// Takes the output reader stream (callable once by the reader thread).
    fn take_reader(&self) -> Result<Box<dyn Read + Send>, AppError>;

    /// Writes raw input bytes to the PTY master.
    fn write_all(&self, data: &[u8]) -> Result<(), AppError>;

    /// Resizes the PTY window dimensions.
    fn resize(&self, cols: u16, rows: u16) -> Result<(), AppError>;

    /// Returns `true` if the PTY foreground process group differs from the shell's own pgid.
    fn has_active_foreground_child(&self) -> bool;

    /// Non-blocking poll for process exit status.
    fn try_wait(&self) -> Result<Option<i32>, AppError>;

    /// Terminates the process group cleanly (`SIGHUP` -> 500ms -> 200ms `SIGKILL` -> `wait`) (`C8.7`).
    fn terminate_process_group(&self) -> Result<Option<i32>, AppError>;
}

/// Abstracted PTY backend factory for dependency injection.
pub trait PtyBackend: Send + Sync {
    fn spawn(&self, config: &PtySpawnConfig) -> Result<Arc<dyn PtySessionHandle>, AppError>;
}

/// Production `portable-pty` backend (`D-0005`).
#[derive(Debug, Default)]
pub struct PortablePtyBackend;

impl PortablePtyBackend {
    pub fn new() -> Self {
        Self
    }
}

struct PortablePtySession {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    reader: Mutex<Option<Box<dyn Read + Send>>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    master_fd: Option<i32>,
    shell_pid: Option<u32>,
}

impl PtyBackend for PortablePtyBackend {
    fn spawn(&self, config: &PtySpawnConfig) -> Result<Arc<dyn PtySessionHandle>, AppError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows.max(1),
                cols: config.cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| {
                AppError::with_detail(
                    ErrorCode::TerminalSpawnFailed,
                    "Failed to open PTY pair",
                    err.to_string(),
                )
            })?;

        let mut cmd = CommandBuilder::new(&config.shell);
        cmd.arg("-l");
        cmd.cwd(&config.cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        let lang_is_utf8 = std::env::var("LANG")
            .map(|v| v.to_ascii_uppercase().contains("UTF-8"))
            .unwrap_or(false);
        if !lang_is_utf8 {
            cmd.env("LANG", "zh_CN.UTF-8");
        }

        let child = pair.slave.spawn_command(cmd).map_err(|err| {
            AppError::with_detail(
                ErrorCode::TerminalSpawnFailed,
                "Failed to spawn terminal shell",
                err.to_string(),
            )
        })?;
        drop(pair.slave);

        let shell_pid = child.process_id();
        #[cfg(unix)]
        let master_fd = pair.master.as_raw_fd();
        #[cfg(not(unix))]
        let master_fd: Option<i32> = None;

        let reader = pair.master.try_clone_reader().map_err(|err| {
            AppError::with_detail(
                ErrorCode::TerminalSpawnFailed,
                "Failed to clone PTY reader",
                err.to_string(),
            )
        })?;
        let writer = pair.master.take_writer().map_err(|err| {
            AppError::with_detail(
                ErrorCode::TerminalSpawnFailed,
                "Failed to take PTY writer",
                err.to_string(),
            )
        })?;

        Ok(Arc::new(PortablePtySession {
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            reader: Mutex::new(Some(reader)),
            child: Mutex::new(child),
            master_fd,
            shell_pid,
        }))
    }
}

impl PtySessionHandle for PortablePtySession {
    fn take_reader(&self) -> Result<Box<dyn Read + Send>, AppError> {
        let mut guard = self
            .reader
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PTY reader mutex poisoned"))?;
        guard
            .take()
            .ok_or_else(|| AppError::new(ErrorCode::InternalError, "PTY reader already taken"))
    }

    fn write_all(&self, data: &[u8]) -> Result<(), AppError> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PTY writer mutex poisoned"))?;
        writer.write_all(data).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to write to PTY",
                err.to_string(),
            )
        })?;
        writer.flush().map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to flush PTY writer",
                err.to_string(),
            )
        })
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<(), AppError> {
        let master = self
            .master
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PTY master mutex poisoned"))?;
        master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to resize PTY",
                    err.to_string(),
                )
            })
    }

    fn has_active_foreground_child(&self) -> bool {
        #[cfg(unix)]
        {
            if let (Some(fd), Some(shell_pid)) = (self.master_fd, self.shell_pid) {
                let fg_pgid = unsafe { libc::tcgetpgrp(fd) };
                if fg_pgid > 0 && fg_pgid != (shell_pid as libc::pid_t) {
                    return true;
                }
            }
        }
        false
    }

    fn try_wait(&self) -> Result<Option<i32>, AppError> {
        let mut child = self
            .child
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PTY child mutex poisoned"))?;
        match child.try_wait() {
            Ok(Some(status)) => Ok(Some(status.exit_code() as i32)),
            Ok(None) => Ok(None),
            Err(err) => Err(AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to poll PTY child status",
                err.to_string(),
            )),
        }
    }

    fn terminate_process_group(&self) -> Result<Option<i32>, AppError> {
        if let Some(code) = self.try_wait()? {
            return Ok(Some(code));
        }

        #[cfg(unix)]
        if let Some(pid) = self.shell_pid {
            let pgid = -(pid as libc::pid_t);
            unsafe {
                libc::kill(pgid, libc::SIGHUP);
            }
        }

        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if let Some(code) = self.try_wait()? {
                return Ok(Some(code));
            }
            thread::sleep(Duration::from_millis(25));
        }

        thread::sleep(Duration::from_millis(200));
        if let Some(code) = self.try_wait()? {
            return Ok(Some(code));
        }

        #[cfg(unix)]
        if let Some(pid) = self.shell_pid {
            let pgid = -(pid as libc::pid_t);
            unsafe {
                libc::kill(pgid, libc::SIGKILL);
            }
        }

        let mut child = self
            .child
            .lock()
            .map_err(|_| AppError::new(ErrorCode::InternalError, "PTY child mutex poisoned"))?;
        if let Err(err) = child.kill() {
            eprintln!("[pty] fallback child.kill returned error: {err}");
        }
        match child.wait() {
            Ok(status) => Ok(Some(status.exit_code() as i32)),
            Err(err) => Err(AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to wait for terminated PTY child",
                err.to_string(),
            )),
        }
    }
}

/// Resolves the shell executable in priority: `args.shell` > `settings.shell` > `$SHELL` > `/bin/zsh`.
pub fn resolve_terminal_shell(arg_shell: Option<&str>, settings_shell: &str) -> String {
    if let Some(explicit) = arg_shell.map(str::trim).filter(|s| !s.is_empty()) {
        return explicit.to_string();
    }
    let configured = settings_shell.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }
    if let Ok(env_shell) = std::env::var("SHELL") {
        let trimmed = env_shell.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "/bin/zsh".to_string()
}

/// Expands leading `~` and verifies that the target working directory exists and is a directory.
pub fn resolve_terminal_cwd(raw_cwd: &str) -> Result<PathBuf, AppError> {
    let trimmed = raw_cwd.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            ErrorCode::WorkspaceDirMissing,
            "Terminal working directory cannot be empty",
        ));
    }

    let expanded = if trimmed == "~" {
        home_dir_path()?
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        home_dir_path()?.join(rest)
    } else {
        PathBuf::from(trimmed)
    };

    if !Path::exists(&expanded) || !expanded.is_dir() {
        return Err(AppError::new(
            ErrorCode::WorkspaceDirMissing,
            "Terminal working directory does not exist or is not a directory",
        ));
    }

    Ok(expanded)
}

fn home_dir_path() -> Result<PathBuf, AppError> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| AppError::new(ErrorCode::WorkspaceDirMissing, "HOME directory is not set"))
}
