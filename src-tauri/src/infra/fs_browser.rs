use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default maximum directory traversal depth (1 = direct children of root).
pub const DEFAULT_MAX_DEPTH: usize = 3;
/// Default maximum number of returned entries before circuit breaking.
pub const DEFAULT_MAX_ENTRIES: usize = 200;
/// Hard ceiling for traversal depth to prevent runaway recursion.
pub const HARD_MAX_DEPTH: usize = 16;
/// Hard ceiling for returned entries.
pub const HARD_MAX_ENTRIES: usize = 2_000;
/// Hard ceiling for total filesystem entries visited in a single scan.
pub const HARD_MAX_VISITED: usize = 20_000;

/// Maximum byte size sampled from the beginning of a file for text detection.
const TEXT_PROBE_SAMPLE_BYTES: usize = 8 * 1024;
/// Maximum file size eligible to be treated as an editable text file (16 MiB).
pub const MAX_TEXT_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum file size eligible to be treated as a non-UTF-8 text file (4 MiB).
pub const MAX_NON_UTF8_TEXT_BYTES: u64 = 4 * 1024 * 1024;

/// Single-segment directory names always skipped during traversal.
const BLACKLISTED_DIR_NAMES: &[&str] = &[
    "node_modules",
    "target",
    ".pnpm-store",
    ".yarn",
    ".cache",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".tox",
    ".venv",
    "venv",
    ".gradle",
    ".idea",
    ".Trash",
    "Pods",
    "DerivedData",
];

/// Two-segment directory suffixes (parent, child) always skipped during traversal.
const BLACKLISTED_DIR_PAIRS: &[(&str, &str)] =
    &[(".git", "objects"), (".git", "logs"), (".git", "refs")];

/// File extensions immediately rejected as binary without reading file contents.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "tiff", "tif", "psd", "heic", "pdf", "zip",
    "tar", "gz", "tgz", "bz2", "xz", "7z", "rar", "zst", "dmg", "pkg", "iso", "exe", "dll", "so",
    "dylib", "wasm", "o", "a", "class", "pyc", "pyo", "mp3", "mp4", "mov", "avi", "mkv", "wav",
    "flac", "ogg", "m4a", "webm", "woff", "woff2", "ttf", "otf", "eot", "sqlite", "sqlite3", "db",
    "bin", "dat",
];

#[derive(Debug, Error)]
pub enum FsBrowserError {
    #[error("path input is empty")]
    EmptyPath,
    #[error("path has no parent directory: {0}")]
    NoParentDirectory(String),
    #[error("directory does not exist: {0}")]
    NotFound(String),
    #[error("path is not a directory: {0}")]
    NotADirectory(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("failed to access directory {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathEntryKind {
    File,
    Directory,
    Other,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathProbeResult {
    pub raw_input: String,
    pub resolved_path: String,
    pub exists: bool,
    pub kind: PathEntryKind,
    pub readable: bool,
    pub size: Option<u64>,
    pub is_text: bool,
    pub is_utf8: bool,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDirectoryOptions {
    /// Maximum traversal depth (1 = direct children only). Clamped to `HARD_MAX_DEPTH`.
    pub max_depth: usize,
    /// Whether to include hidden entries (names starting with `.`) inside the directory.
    /// Note: the root directory itself may always be hidden (e.g. `~/.config`).
    pub include_hidden: bool,
    /// Optional fuzzy filter query matched against entry name and relative path.
    pub query: Option<String>,
    /// Maximum number of entries to return. Clamped to `HARD_MAX_ENTRIES`.
    pub max_entries: usize,
}

impl Default for ListDirectoryOptions {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            include_hidden: false,
            query: None,
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub rel_path: String,
    pub kind: PathEntryKind,
    pub depth: usize,
    pub size: Option<u64>,
    pub is_hidden: bool,
    pub match_score: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub root_path: String,
    pub entries: Vec<DirectoryEntry>,
    pub truncated: bool,
    pub visited_count: usize,
}

/// Expand leading `~` and lexically normalize an input path into an absolute `PathBuf`.
pub fn resolve_input_path(raw_input: &str) -> Result<PathBuf, FsBrowserError> {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return Err(FsBrowserError::EmptyPath);
    }

    let expanded = expand_tilde(trimmed);
    let abs = if expanded.is_absolute() {
        expanded
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(expanded)
    } else {
        expanded
    };

    Ok(normalize_lexical(&abs))
}

fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
        return PathBuf::from(input);
    }
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(input)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(p) => normalized.push(p.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !normalized.has_root() {
                    normalized.push("..");
                }
            }
            Component::Normal(seg) => normalized.push(seg),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.') && name != "." && name != ".."
}

fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(is_hidden_name)
        .unwrap_or(false)
}

/// Probe an arbitrary path string (`~`, hidden directories, system paths like `/usr/local/etc`, etc.).
/// Returns existence, entry kind, readability, file size, and text/UTF-8 classification.
pub fn probe_path(raw_input: &str) -> PathProbeResult {
    let resolved = match resolve_input_path(raw_input) {
        Ok(p) => p,
        Err(_) => {
            return PathProbeResult {
                raw_input: raw_input.to_string(),
                resolved_path: String::new(),
                exists: false,
                kind: PathEntryKind::NotFound,
                readable: false,
                size: None,
                is_text: false,
                is_utf8: false,
                is_hidden: false,
            };
        }
    };

    let resolved_str = resolved.to_string_lossy().into_owned();
    let hidden = is_hidden_path(&resolved);

    match fs::metadata(&resolved) {
        Ok(meta) => {
            if meta.is_dir() {
                let readable = fs::read_dir(&resolved).is_ok();
                PathProbeResult {
                    raw_input: raw_input.to_string(),
                    resolved_path: resolved_str,
                    exists: true,
                    kind: PathEntryKind::Directory,
                    readable,
                    size: None,
                    is_text: false,
                    is_utf8: false,
                    is_hidden: hidden,
                }
            } else if meta.is_file() {
                let size = meta.len();
                let (readable, is_text, is_utf8) = probe_file_text(&resolved, size);
                PathProbeResult {
                    raw_input: raw_input.to_string(),
                    resolved_path: resolved_str,
                    exists: true,
                    kind: PathEntryKind::File,
                    readable,
                    size: Some(size),
                    is_text,
                    is_utf8,
                    is_hidden: hidden,
                }
            } else {
                PathProbeResult {
                    raw_input: raw_input.to_string(),
                    resolved_path: resolved_str,
                    exists: true,
                    kind: PathEntryKind::Other,
                    readable: false,
                    size: None,
                    is_text: false,
                    is_utf8: false,
                    is_hidden: hidden,
                }
            }
        }
        Err(err) => {
            let exists = err.kind() == ErrorKind::PermissionDenied
                && fs::symlink_metadata(&resolved).is_ok();
            let kind = if exists {
                PathEntryKind::Other
            } else {
                PathEntryKind::NotFound
            };
            PathProbeResult {
                raw_input: raw_input.to_string(),
                resolved_path: resolved_str,
                exists,
                kind,
                readable: false,
                size: None,
                is_text: false,
                is_utf8: false,
                is_hidden: hidden,
            }
        }
    }
}

/// Returns `(readable, is_text, is_utf8)` for a regular file.
fn probe_file_text(path: &Path, file_size: u64) -> (bool, bool, bool) {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return (false, false, false),
    };

    if has_binary_extension(path) {
        return (true, false, false);
    }

    if file_size > MAX_TEXT_FILE_BYTES {
        return (true, false, false);
    }

    if file_size == 0 {
        return (true, true, true);
    }

    let sample_len = (file_size as usize).min(TEXT_PROBE_SAMPLE_BYTES);
    let mut buf = vec![0u8; sample_len];
    let bytes_read = match read_sample(&mut file, &mut buf) {
        Ok(n) => n,
        Err(_) => return (false, false, false),
    };
    let sample = &buf[..bytes_read];

    if sample.is_empty() {
        return (true, true, true);
    }

    if has_binary_magic(sample) || sample.contains(&0x00) {
        return (true, false, false);
    }

    let is_utf8 = is_valid_utf8_prefix(sample);
    if is_utf8 {
        let suspicious = count_suspicious_control_bytes(sample);
        let is_text = suspicious * 20 <= sample.len();
        return (true, is_text, true);
    }

    // Non-UTF-8 file (e.g. GBK / Shift-JIS / Latin-1): reject large files and
    // files with elevated binary control byte density.
    if file_size > MAX_NON_UTF8_TEXT_BYTES {
        return (true, false, false);
    }

    let suspicious = count_suspicious_control_bytes(sample);
    let is_text = suspicious * 50 <= sample.len();
    (true, is_text, false)
}

fn read_sample(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match file.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
    Ok(total)
}

fn has_binary_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let lower = ext.to_ascii_lowercase();
    BINARY_EXTENSIONS.iter().any(|b| *b == lower)
}

fn has_binary_magic(sample: &[u8]) -> bool {
    const MAGICS: &[&[u8]] = &[
        b"\x89PNG\r\n\x1a\n",
        b"%PDF-",
        b"PK\x03\x04",
        b"\x1f\x8b\x08",
        b"\x7fELF",
        b"\xCF\xFA\xED\xFE",
        b"\xFE\xED\xFA\xCF",
        b"\xCA\xFE\xBA\xBE",
        b"SQLite format 3\0",
    ];
    MAGICS.iter().any(|magic| sample.starts_with(magic))
}

/// Checks whether `sample` is valid UTF-8, tolerating a truncated multi-byte
/// sequence right at the end of the fixed-size probe buffer.
fn is_valid_utf8_prefix(sample: &[u8]) -> bool {
    match std::str::from_utf8(sample) {
        Ok(_) => true,
        Err(err) => err.error_len().is_none(),
    }
}

fn count_suspicious_control_bytes(sample: &[u8]) -> usize {
    sample
        .iter()
        .filter(|&&b| matches!(b, 0x01..=0x08 | 0x0E..=0x1A | 0x1C..=0x1F | 0x7F))
        .count()
}

fn is_blacklisted_dir(path: &Path, name: &str) -> bool {
    if BLACKLISTED_DIR_NAMES.contains(&name) {
        return true;
    }
    if let Some(parent_name) = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
    {
        if BLACKLISTED_DIR_PAIRS
            .iter()
            .any(|(parent, child)| *parent == parent_name && *child == name)
        {
            return true;
        }
    }
    false
}

/// List directory entries with depth, hidden-file, fuzzy query, blacklist, and circuit-breaker controls.
pub fn list_directory_entries(
    dir_path: impl AsRef<Path>,
    opts: &ListDirectoryOptions,
) -> Result<DirectoryListing, FsBrowserError> {
    let raw_str = dir_path.as_ref().to_string_lossy();
    let root = resolve_input_path(&raw_str)?;
    let root_str = root.to_string_lossy().into_owned();

    let meta = fs::metadata(&root).map_err(|err| map_dir_error(&root_str, err))?;
    if !meta.is_dir() {
        return Err(FsBrowserError::NotADirectory(root_str));
    }

    // Verify root directory itself is readable before traversing.
    let _ = fs::read_dir(&root).map_err(|err| map_dir_error(&root_str, err))?;

    let max_depth = opts.max_depth.min(HARD_MAX_DEPTH);
    let max_entries = opts.max_entries.min(HARD_MAX_ENTRIES);
    let query = opts
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty());

    if max_depth == 0 || max_entries == 0 {
        let has_any = fs::read_dir(&root)
            .ok()
            .and_then(|mut it| it.next())
            .is_some();
        return Ok(DirectoryListing {
            root_path: root_str,
            entries: Vec::new(),
            truncated: has_any,
            visited_count: 0,
        });
    }

    let collect_cap = if query.is_some() {
        max_entries.saturating_mul(4).min(HARD_MAX_ENTRIES)
    } else {
        max_entries
    };

    let mut entries = Vec::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root.clone(), 1));

    let mut visited_count: usize = 0;
    let mut truncated = false;

    'bfs: while let Some((current_dir, depth)) = queue.pop_front() {
        let read_dir = match fs::read_dir(&current_dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for item in read_dir {
            if visited_count >= HARD_MAX_VISITED {
                truncated = true;
                break 'bfs;
            }

            let entry = match item {
                Ok(e) => e,
                Err(_) => continue,
            };

            visited_count += 1;

            let name_os = entry.file_name();
            let name = name_os.to_string_lossy().into_owned();
            if name == ".DS_Store" {
                continue;
            }

            let hidden = is_hidden_name(&name);
            if hidden && !opts.include_hidden {
                continue;
            }

            let entry_path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            // Never recurse into symlinked directories to avoid cycles.
            let is_symlink = file_type.is_symlink();
            let (kind, size, should_descend) = if file_type.is_dir() {
                if is_blacklisted_dir(&entry_path, &name) {
                    continue;
                }
                (PathEntryKind::Directory, None, depth < max_depth)
            } else if file_type.is_file() {
                let sz = entry.metadata().ok().map(|m| m.len());
                (PathEntryKind::File, sz, false)
            } else if is_symlink {
                match fs::metadata(&entry_path) {
                    Ok(target_meta) if target_meta.is_dir() => {
                        if is_blacklisted_dir(&entry_path, &name) {
                            continue;
                        }
                        (PathEntryKind::Directory, None, false)
                    }
                    Ok(target_meta) if target_meta.is_file() => {
                        (PathEntryKind::File, Some(target_meta.len()), false)
                    }
                    _ => (PathEntryKind::Other, None, false),
                }
            } else {
                (PathEntryKind::Other, None, false)
            };

            if should_descend {
                queue.push_back((entry_path.clone(), depth + 1));
            }

            let rel_path = entry_path
                .strip_prefix(&root)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .replace('\\', "/");

            let match_score = match query {
                Some(q) => match fuzzy_match_score(q, &name, &rel_path) {
                    Some(score) => score,
                    None => continue,
                },
                None => 0,
            };

            if entries.len() >= collect_cap {
                truncated = true;
                if query.is_none() {
                    break 'bfs;
                }
                // For query mode, keep scanning up to HARD_MAX_VISITED only if
                // we haven't filled collect_cap; once collect_cap is full, stop.
                break 'bfs;
            }

            entries.push(DirectoryEntry {
                name,
                path: entry_path.to_string_lossy().into_owned(),
                rel_path,
                kind,
                depth,
                size,
                is_hidden: hidden,
                match_score,
            });
        }
    }

    if query.is_some() {
        entries.sort_by(|a, b| {
            b.match_score
                .cmp(&a.match_score)
                .then_with(|| a.depth.cmp(&b.depth))
                .then_with(|| {
                    a.rel_path
                        .to_ascii_lowercase()
                        .cmp(&b.rel_path.to_ascii_lowercase())
                })
        });
        if entries.len() > max_entries {
            entries.truncate(max_entries);
            truncated = true;
        }
    } else {
        entries.sort_by(|a, b| {
            a.depth
                .cmp(&b.depth)
                .then_with(|| {
                    let a_dir = a.kind == PathEntryKind::Directory;
                    let b_dir = b.kind == PathEntryKind::Directory;
                    b_dir.cmp(&a_dir)
                })
                .then_with(|| {
                    a.rel_path
                        .to_ascii_lowercase()
                        .cmp(&b.rel_path.to_ascii_lowercase())
                })
        });
    }

    Ok(DirectoryListing {
        root_path: root_str,
        entries,
        truncated,
        visited_count,
    })
}

/// Given a file path (external or inside vault), return editable text files in
/// its parent directory and subdirectories up to `max_depth` (1 = parent directory only).
pub fn list_sibling_files(
    file_path: impl AsRef<Path>,
    max_depth: usize,
) -> Result<DirectoryListing, FsBrowserError> {
    let raw_str = file_path.as_ref().to_string_lossy();
    let resolved = resolve_input_path(&raw_str)?;
    let resolved_str = resolved.to_string_lossy().into_owned();

    let parent = resolved
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| FsBrowserError::NoParentDirectory(resolved_str.clone()))?;
    let parent_str = parent.to_string_lossy().into_owned();

    let meta = fs::metadata(parent).map_err(|err| map_dir_error(&parent_str, err))?;
    if !meta.is_dir() {
        return Err(FsBrowserError::NotADirectory(parent_str));
    }
    let _ = fs::read_dir(parent).map_err(|err| map_dir_error(&parent_str, err))?;

    let effective_max_depth = max_depth.min(HARD_MAX_DEPTH);
    if effective_max_depth == 0 {
        return Ok(DirectoryListing {
            root_path: parent_str,
            entries: Vec::new(),
            truncated: false,
            visited_count: 0,
        });
    }

    let mut entries = Vec::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((parent.to_path_buf(), 1));

    let mut visited_count: usize = 0;
    let mut truncated = false;

    'bfs: while let Some((current_dir, depth)) = queue.pop_front() {
        let read_dir = match fs::read_dir(&current_dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for item in read_dir {
            if visited_count >= HARD_MAX_VISITED {
                truncated = true;
                break 'bfs;
            }

            let entry = match item {
                Ok(e) => e,
                Err(_) => continue,
            };

            visited_count += 1;

            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".DS_Store" {
                continue;
            }

            let entry_path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                if is_blacklisted_dir(&entry_path, &name) {
                    continue;
                }
                if depth < effective_max_depth {
                    queue.push_back((entry_path, depth + 1));
                }
                continue;
            }

            let file_size = if file_type.is_file() {
                match entry.metadata() {
                    Ok(m) => m.len(),
                    Err(_) => continue,
                }
            } else if file_type.is_symlink() {
                match fs::metadata(&entry_path) {
                    Ok(m) if m.is_file() => m.len(),
                    _ => continue,
                }
            } else {
                continue;
            };

            let (readable, is_text, _) = probe_file_text(&entry_path, file_size);
            if !readable || !is_text {
                continue;
            }

            if entries.len() >= DEFAULT_MAX_ENTRIES {
                truncated = true;
                break 'bfs;
            }

            let rel_path = entry_path
                .strip_prefix(parent)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .replace('\\', "/");

            entries.push(DirectoryEntry {
                name: name.clone(),
                path: entry_path.to_string_lossy().into_owned(),
                rel_path,
                kind: PathEntryKind::File,
                depth,
                size: Some(file_size),
                is_hidden: is_hidden_name(&name),
                match_score: 0,
            });
        }
    }

    entries.sort_by(|a, b| {
        a.depth.cmp(&b.depth).then_with(|| {
            a.rel_path
                .to_ascii_lowercase()
                .cmp(&b.rel_path.to_ascii_lowercase())
        })
    });

    Ok(DirectoryListing {
        root_path: parent_str,
        entries,
        truncated,
        visited_count,
    })
}

fn map_dir_error(path: &str, err: std::io::Error) -> FsBrowserError {
    match err.kind() {
        ErrorKind::NotFound => FsBrowserError::NotFound(path.to_string()),
        ErrorKind::PermissionDenied => FsBrowserError::PermissionDenied(path.to_string()),
        _ => FsBrowserError::Io {
            path: path.to_string(),
            source: err,
        },
    }
}

/// Case-insensitive fuzzy scoring for entry name and relative path.
fn fuzzy_match_score(query: &str, name: &str, rel_path: &str) -> Option<u32> {
    let q = query.to_ascii_lowercase();
    let name_lower = name.to_ascii_lowercase();
    let rel_lower = rel_path.to_ascii_lowercase();

    if name_lower == q {
        return Some(1000);
    }

    if name_lower.starts_with(&q) {
        let len_bonus = 100u32.saturating_sub((name_lower.len().saturating_sub(q.len())) as u32);
        return Some(800 + len_bonus);
    }

    if let Some(pos) = name_lower.find(&q) {
        let boundary_bonus = if is_word_boundary(&name_lower, pos) {
            40
        } else {
            0
        };
        let pos_penalty = (pos as u32).min(50);
        return Some(600 + boundary_bonus - pos_penalty);
    }

    if let Some(pos) = rel_lower.find(&q) {
        let boundary_bonus = if is_word_boundary(&rel_lower, pos) {
            30
        } else {
            0
        };
        let pos_penalty = (pos as u32).min(50);
        return Some(450 + boundary_bonus - pos_penalty);
    }

    if let Some(sub_score) = subsequence_score(&q, &name_lower) {
        return Some(300 + sub_score);
    }

    if let Some(sub_score) = subsequence_score(&q, &rel_lower) {
        return Some(150 + sub_score);
    }

    None
}

fn is_word_boundary(haystack: &str, byte_idx: usize) -> bool {
    if byte_idx == 0 {
        return true;
    }
    haystack[..byte_idx]
        .chars()
        .next_back()
        .map(|c| matches!(c, '/' | '-' | '_' | '.' | ' '))
        .unwrap_or(false)
}

fn subsequence_score(query: &str, haystack: &str) -> Option<u32> {
    let mut h_chars = haystack.char_indices().peekable();
    let mut score: u32 = 0;
    let mut prev_match_idx: Option<usize> = None;

    for qc in query.chars() {
        let mut matched = None;
        for (idx, hc) in h_chars.by_ref() {
            if hc == qc {
                matched = Some(idx);
                break;
            }
        }
        let idx = matched?;
        if let Some(prev) = prev_match_idx {
            if idx == prev + 1 {
                score = score.saturating_add(15);
            }
        }
        if is_word_boundary(haystack, idx) {
            score = score.saturating_add(10);
        }
        score = score.saturating_add(5);
        prev_match_idx = Some(idx);
    }

    Some(score.min(140))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(label: &str) -> Result<Self, std::io::Error> {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!(
                "qn_fs_browser_{}_{}_{}",
                label,
                std::process::id(),
                nanos
            ));
            fs::create_dir_all(&dir)?;
            Ok(Self { path: dir })
        }

        fn write_bytes(&self, rel: &str, bytes: &[u8]) -> Result<PathBuf, std::io::Error> {
            let full = self.path.join(rel);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full, bytes)?;
            Ok(full)
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn probe_path_expands_tilde_and_system_paths() {
        let home_probe = probe_path("~");
        assert!(home_probe.exists);
        assert_eq!(home_probe.kind, PathEntryKind::Directory);
        assert!(home_probe.readable);
        assert!(!home_probe.resolved_path.starts_with('~'));

        // System path format normalization (must work even if /usr/local/etc does not exist on CI)
        let sys_probe = probe_path("/usr/local/etc/../etc");
        assert_eq!(sys_probe.resolved_path, "/usr/local/etc");
        assert_eq!(sys_probe.raw_input, "/usr/local/etc/../etc");

        // Standard /etc directory on Unix/macOS
        let etc_probe = probe_path("/etc");
        assert!(etc_probe.exists);
        assert_eq!(etc_probe.kind, PathEntryKind::Directory);
        assert_eq!(etc_probe.resolved_path, "/etc");
    }

    #[test]
    fn probe_path_detects_utf8_non_utf8_and_binary_files() -> TestResult {
        let tmp = TempDirGuard::new("probe_text")?;

        let utf8_file = tmp.write_bytes(".hidden_note.md", "# Hello\n你好世界\n".as_bytes())?;
        let utf8_res = probe_path(&utf8_file.to_string_lossy());
        assert!(utf8_res.exists);
        assert_eq!(utf8_res.kind, PathEntryKind::File);
        assert!(utf8_res.readable);
        assert!(utf8_res.is_text);
        assert!(utf8_res.is_utf8);
        assert!(utf8_res.is_hidden);

        // GBK encoded Chinese text ("中文配置\n") - non-UTF-8 but valid text
        let gbk_bytes: &[u8] = &[0xD6, 0xD0, 0xCE, 0xC4, 0xC5, 0xE4, 0xD6, 0xC3, b'\n'];
        let gbk_file = tmp.write_bytes("legacy.conf", gbk_bytes)?;
        let gbk_res = probe_path(&gbk_file.to_string_lossy());
        assert!(gbk_res.exists);
        assert!(gbk_res.readable);
        assert!(gbk_res.is_text);
        assert!(!gbk_res.is_utf8);

        // Binary file with NUL bytes (no binary extension)
        let bin_file = tmp.write_bytes("blob", &[0x89, 0x50, 0x4E, 0x47, 0x00, 0x01, 0x02])?;
        let bin_res = probe_path(&bin_file.to_string_lossy());
        assert!(bin_res.exists);
        assert!(bin_res.readable);
        assert!(!bin_res.is_text);

        Ok(())
    }

    #[test]
    fn list_directory_supports_hidden_dirs_and_skips_blacklisted_heavy_dirs() -> TestResult {
        let tmp = TempDirGuard::new("hidden_and_blacklist")?;
        let hidden_root = tmp.path.join(".config");
        fs::create_dir_all(&hidden_root)?;

        tmp.write_bytes(".config/settings.yaml", b"theme: dark\n")?;
        tmp.write_bytes(".config/.env", b"API_KEY=demo\n")?;
        tmp.write_bytes(".config/.git/config", b"[core]\n")?;
        tmp.write_bytes(".config/.git/objects/ab/123456", b"raw object")?;
        tmp.write_bytes(".config/node_modules/pkg/index.js", b"module.exports = 1;")?;
        tmp.write_bytes(".config/target/debug/app", b"binary")?;

        // Hidden root itself works even when include_hidden = false
        let opts_no_hidden = ListDirectoryOptions {
            max_depth: 4,
            include_hidden: false,
            query: None,
            max_entries: 100,
        };
        let listing = list_directory_entries(&hidden_root, &opts_no_hidden)?;
        let rels: Vec<&str> = listing
            .entries
            .iter()
            .map(|e| e.rel_path.as_str())
            .collect();
        assert!(rels.contains(&"settings.yaml"));
        assert!(!rels.contains(&".env"));
        assert!(!rels.iter().any(|r| r.contains("node_modules")));
        assert!(!rels.iter().any(|r| r.contains("target")));

        // With include_hidden = true, .env and .git/config appear, but .git/objects is skipped
        let opts_with_hidden = ListDirectoryOptions {
            max_depth: 4,
            include_hidden: true,
            query: None,
            max_entries: 100,
        };
        let listing_hidden = list_directory_entries(&hidden_root, &opts_with_hidden)?;
        let rels_hidden: Vec<&str> = listing_hidden
            .entries
            .iter()
            .map(|e| e.rel_path.as_str())
            .collect();
        assert!(rels_hidden.contains(&".env"));
        assert!(rels_hidden.contains(&".git/config"));
        assert!(!rels_hidden.iter().any(|r| r.contains(".git/objects")));
        assert!(!rels_hidden.iter().any(|r| r.contains("node_modules")));
        assert!(!rels_hidden.iter().any(|r| r.contains("target")));

        Ok(())
    }

    #[test]
    fn list_directory_enforces_max_entries_and_max_depth_and_fuzzy_ranking() -> TestResult {
        let tmp = TempDirGuard::new("limits_and_fuzzy")?;
        for i in 0..25 {
            tmp.write_bytes(&format!("file_{i:02}.txt"), b"hello")?;
        }
        tmp.write_bytes("nginx.conf", b"worker_processes 1;")?;
        tmp.write_bytes("sub/nginx_extra.conf", b"server {}")?;
        tmp.write_bytes("sub/deep/too_deep.txt", b"deep")?;

        // Circuit breaker on max_entries
        let limited_opts = ListDirectoryOptions {
            max_depth: 1,
            include_hidden: false,
            query: None,
            max_entries: 10,
        };
        let limited = list_directory_entries(&tmp.path, &limited_opts)?;
        assert_eq!(limited.entries.len(), 10);
        assert!(limited.truncated);

        // Max depth = 2 excludes depth 3 (`sub/deep/too_deep.txt`)
        let depth_opts = ListDirectoryOptions {
            max_depth: 2,
            include_hidden: false,
            query: Some("deep".to_string()),
            max_entries: 50,
        };
        let depth_res = list_directory_entries(&tmp.path, &depth_opts)?;
        assert!(!depth_res
            .entries
            .iter()
            .any(|e| e.rel_path == "sub/deep/too_deep.txt"));

        // Fuzzy query ranks exact/prefix match `nginx.conf` above `sub/nginx_extra.conf`
        let fuzzy_opts = ListDirectoryOptions {
            max_depth: 3,
            include_hidden: false,
            query: Some("nginx.conf".to_string()),
            max_entries: 50,
        };
        let fuzzy_res = list_directory_entries(&tmp.path, &fuzzy_opts)?;
        assert!(!fuzzy_res.entries.is_empty());
        assert_eq!(fuzzy_res.entries[0].rel_path, "nginx.conf");

        Ok(())
    }

    #[test]
    fn list_sibling_files_returns_editable_files_and_filters_binaries() -> TestResult {
        let tmp = TempDirGuard::new("siblings")?;
        let target = tmp.write_bytes("etc/nginx.conf", b"http {}")?;
        tmp.write_bytes("etc/.hidden.conf", b"secret=1")?;
        tmp.write_bytes("etc/sites/default.conf", b"server {}")?;
        tmp.write_bytes("etc/sites/nested/level3.conf", b"too deep")?;
        tmp.write_bytes("etc/logo.png", &[0x89, 0x50, 0x4E, 0x47, 0x00, 0x00])?;
        tmp.write_bytes("etc/node_modules/a.js", b"ignored")?;

        let siblings = list_sibling_files(&target, 2)?;
        let rels: Vec<&str> = siblings
            .entries
            .iter()
            .map(|e| e.rel_path.as_str())
            .collect();
        assert!(rels.contains(&"nginx.conf"));
        assert!(rels.contains(&".hidden.conf"));
        assert!(rels.contains(&"sites/default.conf"));
        assert!(!rels.contains(&"logo.png"));
        assert!(!rels.contains(&"sites/nested/level3.conf"));
        assert!(!rels.iter().any(|r| r.contains("node_modules")));

        Ok(())
    }
}
