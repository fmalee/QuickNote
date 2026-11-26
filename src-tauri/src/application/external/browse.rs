use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::application::persistence::PersistenceManager;
use crate::domain::bookmark::ExternalBookmarkKind;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::{PathCompleteArgs, PathEntry};
use crate::infra::fs_browser::{
    self, DirectoryEntry, DirectoryListing, FsBrowserError, ListDirectoryOptions, PathProbeResult,
    DEFAULT_MAX_DEPTH, DEFAULT_MAX_ENTRIES,
};

const PATH_COMPLETE_LIMIT: usize = 50;

/// Probes an external path (`~`, hidden directories, `/usr/local/etc`, etc.) using pure Rust FS.
pub fn external_probe_path(raw_input: &str) -> PathProbeResult {
    fs_browser::probe_path(raw_input)
}

/// Lists directory entries under an external path with depth/hidden/query/circuit-breaker controls.
pub fn external_list_directory(
    dir_path: &str,
    opts: Option<ListDirectoryOptions>,
) -> Result<DirectoryListing, AppError> {
    let effective = opts.unwrap_or_default();
    fs_browser::list_directory_entries(dir_path, &effective).map_err(map_fs_browser_error)
}

/// Lists editable text files in the parent directory (and subdirectories up to `max_depth`) of `file_path`.
pub fn external_list_sibling_files(
    file_path: &str,
    max_depth: Option<usize>,
) -> Result<DirectoryListing, AppError> {
    let depth = max_depth.unwrap_or(1);
    fs_browser::list_sibling_files(file_path, depth).map_err(map_fs_browser_error)
}

/// Searches configured `search_roots` and bookmarked directories directly via pure Rust FS traversal.
pub fn external_search_roots(
    pm: &PersistenceManager,
    query: &str,
    max_depth: Option<usize>,
    max_entries: Option<usize>,
) -> Result<Vec<DirectoryEntry>, AppError> {
    let settings = pm.settings_get()?.settings;
    let trimmed_query = query.trim();
    let limit = max_entries.unwrap_or(DEFAULT_MAX_ENTRIES);
    let depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);

    let mut roots: Vec<String> = Vec::new();
    let mut seen_roots: HashSet<String> = HashSet::new();

    for root in &settings.search_roots {
        if let Ok(resolved) = fs_browser::resolve_input_path(root) {
            let key = resolved.to_string_lossy().into_owned();
            if seen_roots.insert(key.clone()) {
                roots.push(key);
            }
        }
    }
    for bookmark in &settings.external_bookmarks {
        if bookmark.kind == ExternalBookmarkKind::Directory {
            if let Ok(resolved) = fs_browser::resolve_input_path(&bookmark.path) {
                let key = resolved.to_string_lossy().into_owned();
                if seen_roots.insert(key.clone()) {
                    roots.push(key);
                }
            }
        }
    }

    let opts = ListDirectoryOptions {
        max_depth: depth,
        include_hidden: true,
        query: if trimmed_query.is_empty() {
            None
        } else {
            Some(trimmed_query.to_string())
        },
        max_entries: limit,
    };

    let mut seen_entries: HashSet<String> = HashSet::new();
    let mut results: Vec<DirectoryEntry> = Vec::new();
    for root in roots {
        let Ok(listing) = fs_browser::list_directory_entries(&root, &opts) else {
            continue;
        };
        for entry in listing.entries {
            if seen_entries.insert(entry.path.clone()) {
                results.push(entry);
            }
        }
    }

    if trimmed_query.is_empty() {
        results.sort_by(|a, b| {
            a.depth.cmp(&b.depth).then_with(|| {
                a.path
                    .to_ascii_lowercase()
                    .cmp(&b.path.to_ascii_lowercase())
            })
        });
    } else {
        results.sort_by(|a, b| {
            b.match_score
                .cmp(&a.match_score)
                .then_with(|| a.depth.cmp(&b.depth))
                .then_with(|| {
                    a.path
                        .to_ascii_lowercase()
                        .cmp(&b.path.to_ascii_lowercase())
                })
        });
    }
    results.truncate(limit);
    Ok(results)
}

/// Completes local filesystem paths for Quick Bar `/path` mode (`v4` §5.3).
pub fn path_complete(args: PathCompleteArgs) -> Result<Vec<PathEntry>, AppError> {
    if args.prefix.is_empty() {
        return Ok(Vec::new());
    }

    let expanded = if args.prefix == "~" {
        let Some(home) = std::env::var_os("HOME") else {
            return Ok(Vec::new());
        };
        format!("{}/", home.to_string_lossy())
    } else if let Some(rest) = args.prefix.strip_prefix("~/") {
        let Some(home) = std::env::var_os("HOME") else {
            return Ok(Vec::new());
        };
        let home_str = home.to_string_lossy();
        if home_str.ends_with('/') {
            format!("{home_str}{rest}")
        } else {
            format!("{home_str}/{rest}")
        }
    } else {
        args.prefix.clone()
    };

    if !expanded.starts_with('/') {
        return Ok(Vec::new());
    }

    let (parent_dir, name_prefix) = if expanded.ends_with('/') {
        (PathBuf::from(&expanded), "")
    } else if let Some(pos) = expanded.rfind('/') {
        if pos == 0 {
            (PathBuf::from("/"), &expanded[1..])
        } else {
            (PathBuf::from(&expanded[..pos]), &expanded[pos + 1..])
        }
    } else {
        return Ok(Vec::new());
    };

    if !parent_dir.is_dir() {
        return Ok(Vec::new());
    }

    let Ok(read_dir) = fs::read_dir(&parent_dir) else {
        return Ok(Vec::new());
    };

    let name_prefix_lower = name_prefix.to_lowercase();
    let show_hidden = name_prefix.starts_with('.');

    let mut entries = Vec::new();
    for item in read_dir {
        let Ok(entry) = item else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" {
            continue;
        }
        if name.starts_with('.') && !show_hidden {
            continue;
        }
        if !name_prefix_lower.is_empty() && !name.to_lowercase().starts_with(&name_prefix_lower) {
            continue;
        }

        let path = entry.path();
        let is_dir = match entry.file_type() {
            Ok(ft) if ft.is_symlink() => path.is_dir(),
            Ok(ft) => ft.is_dir(),
            Err(_) => path.is_dir(),
        };

        entries.push(PathEntry {
            name,
            abs_path: path.to_string_lossy().to_string(),
            is_dir,
        });
    }

    entries.sort_by(|a, b| {
        (!a.is_dir, a.name.to_lowercase()).cmp(&(!b.is_dir, b.name.to_lowercase()))
    });
    entries.truncate(PATH_COMPLETE_LIMIT);
    Ok(entries)
}

fn map_fs_browser_error(err: FsBrowserError) -> AppError {
    match err {
        FsBrowserError::EmptyPath => {
            AppError::new(ErrorCode::ValidationError, "Path input cannot be empty")
        }
        FsBrowserError::NoParentDirectory(p) => AppError::with_detail(
            ErrorCode::ValidationError,
            "Path has no parent directory",
            p,
        ),
        FsBrowserError::NotFound(p) => {
            AppError::with_detail(ErrorCode::NotFound, "Target path does not exist", p)
        }
        FsBrowserError::NotADirectory(p) => {
            AppError::with_detail(ErrorCode::ValidationError, "Path is not a directory", p)
        }
        FsBrowserError::PermissionDenied(p) => AppError::with_detail(
            ErrorCode::PermissionDenied,
            "Permission denied accessing path",
            p,
        ),
        FsBrowserError::Io { path, source } => AppError::with_detail(
            ErrorCode::InternalError,
            format!("Failed to access path: {path}"),
            source.to_string(),
        ),
    }
}
