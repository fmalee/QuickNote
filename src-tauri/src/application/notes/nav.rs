use std::collections::{BTreeMap, HashMap, HashSet};

use crate::application::persistence::PersistenceManager;
use crate::domain::error::AppError;
use crate::domain::note::{
    tag_matches, DirNode, NavTreeSnapshot, NoteSummary, NoteType, NotesFilterArgs, TagNode,
    INBOX_REL_PATH,
};
use crate::infra::sqlite::list_all_indexed_notes;

use super::require_library_root;

/// Lists indexed notes filtered by tab, directory, tags, workspace, or search text (`C4.2`).
pub fn notes_list(
    pm: &PersistenceManager,
    args: NotesFilterArgs,
) -> Result<Vec<NoteSummary>, AppError> {
    let root = require_library_root(pm)?;
    let rows = pm.with_db(list_all_indexed_notes)?;

    let filter_lower = args
        .filter_text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);

    let dir_prefix = args
        .dir_rel_path
        .as_deref()
        .map(|s| s.trim().trim_matches('/'))
        .filter(|s| !s.is_empty())
        .map(|s| format!("{s}/"));

    let mut out = Vec::new();
    for row in rows {
        let doc_type = NoteType::parse(&row.doc_type).unwrap_or(NoteType::Note);
        let is_template = doc_type == NoteType::Template || row.rel_path.starts_with("_templates/");
        let tab_match = match args.tab.as_str() {
            "inbox" => row.is_inbox,
            "templates" => row.rel_path.starts_with("_templates/"),
            "projects" => {
                if is_template {
                    false
                } else if let Some(ref prefix) = dir_prefix {
                    row.rel_path.starts_with(prefix)
                } else {
                    !row.rel_path.contains('/')
                }
            }
            "tags" => {
                if is_template {
                    false
                } else if args.tags.is_empty() {
                    true
                } else {
                    args.tags
                        .iter()
                        .all(|filter_tag| row.tags.iter().any(|t| tag_matches(t, filter_tag)))
                }
            }
            "recent" => !is_template,
            _ => !is_template,
        };

        if !tab_match {
            continue;
        }

        if let Some(ref needle) = filter_lower {
            let in_title = row.title.to_lowercase().contains(needle);
            let in_path = row.rel_path.to_lowercase().contains(needle);
            let in_tags = row.tags.iter().any(|t| t.to_lowercase().contains(needle));
            if !in_title && !in_path && !in_tags {
                continue;
            }
        }

        let abs_path = root.join(&row.rel_path).to_string_lossy().to_string();
        out.push(NoteSummary {
            key: row.key,
            rel_path: Some(row.rel_path),
            abs_path,
            title: row.title,
            doc_type,
            tags: row.tags,
            excerpt: row.excerpt,
            pinned: row.pinned,
            archived: row.archived,
            is_inbox: row.is_inbox,
            mtime_ms: row.mtime_ms,
            meta_error: row.meta_error,
        });
    }

    if args.tab == "recent" {
        out.sort_by_key(|a| std::cmp::Reverse(a.mtime_ms));
    } else if args.tab == "projects" && dir_prefix.is_none() {
        let mut inbox: Vec<NoteSummary> = Vec::new();
        let mut rest: Vec<NoteSummary> = Vec::new();
        for item in out {
            if item.is_inbox {
                inbox.push(item);
            } else {
                rest.push(item);
            }
        }
        rest.sort_by_key(|a| std::cmp::Reverse(a.mtime_ms));
        inbox.extend(rest);
        out = inbox;
    }

    Ok(out)
}

/// Builds the navigation snapshot: directory tree and hierarchical tags (`C4.2`).
pub fn notes_get_tree_and_tags(pm: &PersistenceManager) -> Result<NavTreeSnapshot, AppError> {
    let _ = require_library_root(pm)?;
    let rows = pm.with_db(list_all_indexed_notes)?;

    // Build directory tree from non-template, non-inbox notes.
    let mut dir_counts: BTreeMap<String, u32> = BTreeMap::new();
    for row in &rows {
        if row.rel_path == INBOX_REL_PATH || row.rel_path.starts_with("_templates/") {
            continue;
        }
        let parts: Vec<&str> = row.rel_path.split('/').collect();
        if parts.len() <= 1 {
            continue;
        }
        for depth in 1..parts.len() {
            let dir_rel = parts[..depth].join("/");
            *dir_counts.entry(dir_rel).or_insert(0) += 1;
        }
    }
    let dirs = build_dir_tree(&dir_counts);

    // Build hierarchical tag tree.
    let mut all_tag_prefixes: HashSet<String> = HashSet::new();
    let mut tag_notes: HashMap<String, HashSet<String>> = HashMap::new();
    for row in &rows {
        if row.rel_path.starts_with("_templates/") {
            continue;
        }
        for tag in &row.tags {
            let parts: Vec<&str> = tag.split('/').collect();
            for depth in 1..=parts.len() {
                let prefix = parts[..depth].join("/");
                all_tag_prefixes.insert(prefix.clone());
                tag_notes.entry(prefix).or_default().insert(row.key.clone());
            }
        }
    }
    let tags = build_tag_tree(&all_tag_prefixes, &tag_notes);

    Ok(NavTreeSnapshot { dirs, tags })
}

fn build_dir_tree(dir_counts: &BTreeMap<String, u32>) -> Vec<DirNode> {
    fn insert_children(prefix: Option<&str>, dir_counts: &BTreeMap<String, u32>) -> Vec<DirNode> {
        let mut nodes = Vec::new();
        for (rel_path, &count) in dir_counts {
            let parent = rel_path.rsplit_once('/').map(|(p, _)| p);
            if parent == prefix {
                let name = rel_path
                    .rsplit_once('/')
                    .map(|(_, n)| n)
                    .unwrap_or(rel_path)
                    .to_string();
                let children = insert_children(Some(rel_path.as_str()), dir_counts);
                nodes.push(DirNode {
                    rel_path: rel_path.clone(),
                    name,
                    count,
                    children,
                });
            }
        }
        nodes
    }
    insert_children(None, dir_counts)
}

fn build_tag_tree(
    all_prefixes: &HashSet<String>,
    tag_notes: &HashMap<String, HashSet<String>>,
) -> Vec<TagNode> {
    let mut sorted: Vec<String> = all_prefixes.iter().cloned().collect();
    sorted.sort();

    fn collect_children(
        parent: Option<&str>,
        sorted: &[String],
        tag_notes: &HashMap<String, HashSet<String>>,
    ) -> Vec<TagNode> {
        let mut nodes = Vec::new();
        for tag in sorted {
            let tag_parent = tag.rsplit_once('/').map(|(p, _)| p);
            if tag_parent == parent {
                let count = tag_notes.get(tag).map(|s| s.len() as u32).unwrap_or(0);
                let children = collect_children(Some(tag.as_str()), sorted, tag_notes);
                nodes.push(TagNode {
                    tag: tag.clone(),
                    count,
                    children,
                });
            }
        }
        nodes
    }

    collect_children(None, &sorted, tag_notes)
}
