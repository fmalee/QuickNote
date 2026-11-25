//! External bookmark domain model and pure normalization, deduplication, and sorting rules (`C1.2`).
//!
//! Pure in-memory functions only: zero filesystem, SQLite, or Tauri runtime dependencies.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Target kind of an external filesystem bookmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExternalBookmarkKind {
    File,
    Directory,
}

/// Persisted external file/directory bookmark entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalBookmark {
    /// Lexically normalized absolute path.
    pub path: String,
    pub kind: ExternalBookmarkKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<specta_typescript::Number>)]
    pub last_opened_at: Option<i64>,
}

/// Lexically normalizes an absolute POSIX-style path without touching the filesystem.
///
/// Rules:
/// - Trims surrounding whitespace; returns `None` if empty or not starting with `/`.
/// - Normalizes `\` separators to `/`.
/// - Collapses repeated `/`, drops `.` segments, resolves `..` segments clamped at `/`.
/// - Strips trailing `/` except for the root path `"/"`.
pub fn normalize_bookmark_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let unified = trimmed.replace('\\', "/");
    if !unified.starts_with('/') {
        return None;
    }

    let mut stack: Vec<&str> = Vec::new();
    for segment in unified.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                stack.pop();
            }
            part => stack.push(part),
        }
    }

    if stack.is_empty() {
        Some("/".to_string())
    } else {
        Some(format!("/{}", stack.join("/")))
    }
}

/// Trims an optional alias and collapses blank values to `None`.
pub fn normalize_bookmark_alias(alias: Option<&str>) -> Option<String> {
    alias
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Returns a normalized copy of `bookmark`, or `None` if its path is not a valid absolute path.
pub fn normalize_bookmark(bookmark: ExternalBookmark) -> Option<ExternalBookmark> {
    let path = normalize_bookmark_path(&bookmark.path)?;
    let alias = normalize_bookmark_alias(bookmark.alias.as_deref());
    Some(ExternalBookmark {
        path,
        kind: bookmark.kind,
        alias,
        pinned: bookmark.pinned,
        last_opened_at: bookmark.last_opened_at,
    })
}

/// Deduplicates bookmarks by normalized absolute path.
///
/// Entries with invalid (relative or empty) paths are dropped.
/// When duplicates exist for the same normalized path:
/// - `pinned` is combined with logical OR.
/// - `last_opened_at` keeps the latest (maximum) timestamp.
/// - `alias` prefers the newer entry's non-empty alias, falling back to the other non-empty alias.
/// - `kind` prefers the newer entry's kind when `incoming.last_opened_at > existing.last_opened_at`.
pub fn deduplicate_bookmarks(bookmarks: Vec<ExternalBookmark>) -> Vec<ExternalBookmark> {
    let mut index_by_path: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<ExternalBookmark> = Vec::new();

    for raw in bookmarks {
        let Some(incoming) = normalize_bookmark(raw) else {
            continue;
        };

        if let Some(&idx) = index_by_path.get(&incoming.path) {
            if let Some(existing) = result.get_mut(idx) {
                let incoming_is_newer = incoming.last_opened_at > existing.last_opened_at;

                existing.pinned = existing.pinned || incoming.pinned;
                existing.last_opened_at = match (existing.last_opened_at, incoming.last_opened_at)
                {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };

                if incoming_is_newer {
                    existing.kind = incoming.kind;
                    if incoming.alias.is_some() {
                        existing.alias = incoming.alias;
                    }
                } else if existing.alias.is_none() && incoming.alias.is_some() {
                    existing.alias = incoming.alias;
                }
            }
        } else {
            let idx = result.len();
            index_by_path.insert(incoming.path.clone(), idx);
            result.push(incoming);
        }
    }

    result
}

fn display_sort_key(bookmark: &ExternalBookmark) -> String {
    if let Some(alias) = bookmark
        .alias
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return alias.to_lowercase();
    }

    bookmark
        .path
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(&bookmark.path)
        .to_lowercase()
}

/// Comparator implementing deterministic bookmark ordering:
/// 1. `pinned` descending (`true` before `false`)
/// 2. `last_opened_at` descending (`Some(newer)` before `Some(older)` before `None`)
/// 3. Display name (alias or basename) ascending, case-insensitive
/// 4. Normalized `path` ascending as final tie-breaker
pub fn compare_bookmarks(a: &ExternalBookmark, b: &ExternalBookmark) -> Ordering {
    b.pinned
        .cmp(&a.pinned)
        .then_with(|| b.last_opened_at.cmp(&a.last_opened_at))
        .then_with(|| display_sort_key(a).cmp(&display_sort_key(b)))
        .then_with(|| a.path.cmp(&b.path))
}

/// Sorts bookmarks in place using [`compare_bookmarks`].
pub fn sort_bookmarks(bookmarks: &mut [ExternalBookmark]) {
    bookmarks.sort_by(compare_bookmarks);
}

/// Normalizes, deduplicates, and sorts a bookmark list in one pass.
pub fn normalize_and_sort_bookmarks(bookmarks: Vec<ExternalBookmark>) -> Vec<ExternalBookmark> {
    let mut deduped = deduplicate_bookmarks(bookmarks);
    sort_bookmarks(&mut deduped);
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(
        path: &str,
        kind: ExternalBookmarkKind,
        alias: Option<&str>,
        pinned: bool,
        last_opened_at: Option<i64>,
    ) -> ExternalBookmark {
        ExternalBookmark {
            path: path.to_string(),
            kind,
            alias: alias.map(str::to_string),
            pinned,
            last_opened_at,
        }
    }

    #[test]
    fn normalize_bookmark_path_handles_slashes_dots_and_invalid_inputs() {
        assert_eq!(
            normalize_bookmark_path("  /usr/local//etc/./nginx/../nginx/  "),
            Some("/usr/local/etc/nginx".to_string())
        );
        assert_eq!(
            normalize_bookmark_path("/Users/demo/.config/"),
            Some("/Users/demo/.config".to_string())
        );
        assert_eq!(normalize_bookmark_path("/"), Some("/".to_string()));
        assert_eq!(normalize_bookmark_path("/../../.."), Some("/".to_string()));
        assert_eq!(normalize_bookmark_path("relative/path"), None);
        assert_eq!(normalize_bookmark_path("   "), None);
    }

    #[test]
    fn deduplicate_bookmarks_merges_equivalent_paths_and_drops_invalid() {
        let input = vec![
            mk(
                "/Users/dev/project/",
                ExternalBookmarkKind::Directory,
                Some("  Old Name "),
                false,
                Some(100),
            ),
            mk(
                "not/absolute",
                ExternalBookmarkKind::File,
                None,
                true,
                Some(999),
            ),
            mk(
                "/Users/dev//sub/../project/.",
                ExternalBookmarkKind::Directory,
                Some("New Name"),
                true,
                Some(250),
            ),
            mk(
                "/Users/dev/project",
                ExternalBookmarkKind::Directory,
                Some("   "),
                false,
                Some(200),
            ),
        ];

        let deduped = deduplicate_bookmarks(input);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].path, "/Users/dev/project");
        assert_eq!(deduped[0].kind, ExternalBookmarkKind::Directory);
        assert_eq!(deduped[0].alias.as_deref(), Some("New Name"));
        assert!(deduped[0].pinned);
        assert_eq!(deduped[0].last_opened_at, Some(250));
    }

    #[test]
    fn sort_bookmarks_orders_by_pinned_then_recency_then_name_then_path() {
        let input = vec![
            mk(
                "/etc/hosts",
                ExternalBookmarkKind::File,
                None,
                false,
                Some(500),
            ),
            mk(
                "/Users/dev/alpha",
                ExternalBookmarkKind::Directory,
                None,
                true,
                Some(100),
            ),
            mk(
                "/Users/dev/beta",
                ExternalBookmarkKind::Directory,
                Some("Zeta"),
                true,
                Some(300),
            ),
            mk(
                "/Users/dev/gamma",
                ExternalBookmarkKind::Directory,
                None,
                false,
                None,
            ),
            mk(
                "/opt/hosts",
                ExternalBookmarkKind::File,
                None,
                false,
                Some(500),
            ),
        ];

        let sorted = normalize_and_sort_bookmarks(input);
        let paths: Vec<&str> = sorted.iter().map(|b| b.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "/Users/dev/beta",  // pinned, last_opened_at = 300
                "/Users/dev/alpha", // pinned, last_opened_at = 100
                "/etc/hosts",       // unpinned, 500, basename "hosts", path "/etc/hosts" < "/opt/hosts"
                "/opt/hosts",       // unpinned, 500, basename "hosts"
                "/Users/dev/gamma", // unpinned, None
            ]
        );
    }

    #[test]
    fn serde_roundtrip_uses_camel_case_fields() {
        let item = mk(
            "/usr/local/etc",
            ExternalBookmarkKind::Directory,
            Some("Homebrew Etc"),
            true,
            Some(1_700_000_000_000),
        );
        let json = serde_json::to_string(&item);
        assert!(json.is_ok());
        if let Ok(serialized) = json {
            assert!(serialized.contains("\"lastOpenedAt\":1700000000000"));
            assert!(serialized.contains("\"kind\":\"directory\""));
            let decoded: Result<ExternalBookmark, _> = serde_json::from_str(&serialized);
            assert_eq!(decoded.ok(), Some(item));
        }
    }
}
