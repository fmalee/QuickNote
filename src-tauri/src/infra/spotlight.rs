//! Cancellable macOS Spotlight (`mdfind`) runner (`v4` §5.3, `C2.1`).
//!
//! Uses parameterized `Command::new(...).args([...])` (never `sh -c`) and kills any
//! previous in-flight search process when a newer request arrives.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::domain::error::{AppError, ErrorCode};
use crate::domain::external::is_spotlight_blacklisted;

/// Maximum number of Spotlight results returned per query (`v4` §5.3).
pub const SPOTLIGHT_HARD_LIMIT: usize = 50;

/// Output of a cancellable Spotlight search.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpotlightSearchResult {
    pub paths: Vec<String>,
    pub truncated: bool,
}

type CommandFactory = Arc<dyn Fn(&str, &str) -> Command + Send + Sync>;

struct ActiveSpotlightProcess {
    generation: u64,
    child: Child,
}

/// Manages a single active Spotlight child process so rapid typing never piles up `mdfind` processes.
pub struct SpotlightRunner {
    generation: AtomicU64,
    active: Mutex<Option<ActiveSpotlightProcess>>,
    command_factory: CommandFactory,
}

impl Default for SpotlightRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotlightRunner {
    /// Creates a production runner invoking `/usr/bin/mdfind`.
    pub fn new() -> Self {
        Self::with_command_factory(|root, query| {
            let mut cmd = Command::new("/usr/bin/mdfind");
            cmd.args(["-onlyin", root, "-name", query]);
            cmd
        })
    }

    /// Creates a runner with a custom command factory (for deterministic concurrency tests).
    pub fn with_command_factory<F>(factory: F) -> Self
    where
        F: Fn(&str, &str) -> Command + Send + Sync + 'static,
    {
        Self {
            generation: AtomicU64::new(0),
            active: Mutex::new(None),
            command_factory: Arc::new(factory),
        }
    }

    /// Runs a cancellable Spotlight search across existing `roots`.
    pub fn search(&self, roots: &[String], query: &str) -> Result<SpotlightSearchResult, AppError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(SpotlightSearchResult::default());
        }
        if trimmed.starts_with('-') || trimmed.chars().any(char::is_control) {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "Spotlight query contains disallowed leading '-' or control characters",
            ));
        }

        let my_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.kill_active_process()?;

        let resolved_roots = resolve_existing_roots(roots);
        if resolved_roots.is_empty() {
            return Ok(SpotlightSearchResult::default());
        }

        let mut paths = Vec::new();
        let mut seen = HashSet::new();
        let mut truncated = false;

        for root in resolved_roots {
            if self.generation.load(Ordering::SeqCst) != my_gen {
                return Ok(SpotlightSearchResult::default());
            }

            let mut cmd = (self.command_factory)(&root, trimmed);
            cmd.stdout(Stdio::piped()).stderr(Stdio::null());

            let mut child = cmd.spawn().map_err(|err| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to spawn Spotlight search process",
                    err.to_string(),
                )
            })?;

            let stdout = child.stdout.take();

            {
                let mut guard = self.active.lock().map_err(|_| {
                    AppError::new(ErrorCode::InternalError, "Spotlight state lock poisoned")
                })?;
                if self.generation.load(Ordering::SeqCst) != my_gen {
                    drop(guard);
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(SpotlightSearchResult::default());
                }
                *guard = Some(ActiveSpotlightProcess {
                    generation: my_gen,
                    child,
                });
            }

            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                for line_res in reader.lines() {
                    if self.generation.load(Ordering::SeqCst) != my_gen {
                        break;
                    }
                    let Ok(raw_line) = line_res else {
                        break;
                    };
                    let path = raw_line.trim();
                    if path.is_empty() || is_spotlight_blacklisted(path) {
                        continue;
                    }
                    if !seen.insert(path.to_string()) {
                        continue;
                    }
                    paths.push(path.to_string());
                    if paths.len() >= SPOTLIGHT_HARD_LIMIT {
                        truncated = true;
                        break;
                    }
                }
            }

            let mine = {
                let mut guard = self.active.lock().map_err(|_| {
                    AppError::new(ErrorCode::InternalError, "Spotlight state lock poisoned")
                })?;
                if guard.as_ref().is_some_and(|a| a.generation == my_gen) {
                    guard.take()
                } else {
                    None
                }
            };
            if let Some(mut proc_handle) = mine {
                if truncated || self.generation.load(Ordering::SeqCst) != my_gen {
                    let _ = proc_handle.child.kill();
                }
                let _ = proc_handle.child.wait();
            }

            if self.generation.load(Ordering::SeqCst) != my_gen {
                return Ok(SpotlightSearchResult::default());
            }
            if truncated {
                break;
            }
        }

        if self.generation.load(Ordering::SeqCst) != my_gen {
            return Ok(SpotlightSearchResult::default());
        }

        Ok(SpotlightSearchResult { paths, truncated })
    }

    fn kill_active_process(&self) -> Result<(), AppError> {
        let prev = {
            let mut guard = self.active.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Spotlight state lock poisoned")
            })?;
            guard.take()
        };
        if let Some(mut old) = prev {
            let _ = old.child.kill();
            let _ = old.child.wait();
        }
        Ok(())
    }
}

/// Expands `~` and keeps only existing directories.
pub fn expand_tilde(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "~" {
        return std::env::var_os("HOME").map(PathBuf::from);
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return std::env::var_os("HOME").map(|home| PathBuf::from(home).join(rest));
    }
    Some(PathBuf::from(trimmed))
}

fn resolve_existing_roots(roots: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for raw in roots {
        let Some(path) = expand_tilde(raw) else {
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        let s = path.to_string_lossy().to_string();
        if seen.insert(s.clone()) {
            out.push(s);
        }
    }
    out
}
