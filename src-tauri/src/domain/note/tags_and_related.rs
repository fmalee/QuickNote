use std::collections::HashSet;

use super::types::{RelatedNoteItem, RelatedReason};

/// Hierarchical tag match: `dev` matches `dev` and `dev/*` (`v4` §3.3).
pub fn tag_matches(tag: &str, filter: &str) -> bool {
    tag == filter
        || (tag.len() > filter.len()
            && tag.starts_with(filter)
            && tag.as_bytes().get(filter.len()) == Some(&b'/'))
}

/// Pure input projection of an indexed note for related-note scoring (`C8.6`).
#[derive(Debug, Clone)]
pub struct RelatedNoteCandidate<'a> {
    pub key: &'a str,
    pub rel_path: &'a str,
    pub title: &'a str,
    pub tags: &'a [String],
    pub mtime_ms: i64,
}

/// Scores and ranks up to 20 related notes using `C8.6` weights.
pub fn score_related_notes(
    current: &RelatedNoteCandidate<'_>,
    candidates: &[RelatedNoteCandidate<'_>],
    backlink_sources: &HashSet<String>,
) -> Vec<RelatedNoteItem> {
    let current_tags: HashSet<&str> = current.tags.iter().map(String::as_str).collect();

    let mut scored: Vec<(f64, i64, RelatedNoteItem)> = Vec::new();

    for cand in candidates {
        if cand.rel_path == current.rel_path || cand.rel_path.starts_with("_templates/") {
            continue;
        }

        let has_backlink = backlink_sources.contains(cand.rel_path);
        let shared_tags = if current_tags.is_empty() || cand.tags.is_empty() {
            0usize
        } else {
            let cand_tags: HashSet<&str> = cand.tags.iter().map(String::as_str).collect();
            cand_tags.intersection(&current_tags).count()
        };

        let mut score = 0.0_f64;
        if has_backlink {
            score += 100.0;
        }
        if shared_tags > 0 {
            score += 15.0 * (shared_tags as f64);
        }

        if score <= 0.0 {
            continue;
        }

        let reason = if has_backlink {
            RelatedReason::Backlink
        } else {
            RelatedReason::SharedTags
        };

        scored.push((
            score,
            cand.mtime_ms,
            RelatedNoteItem {
                key: cand.key.to_string(),
                rel_path: cand.rel_path.to_string(),
                title: cand.title.to_string(),
                reason,
                score,
            },
        ));
    }

    scored.sort_by(|(sa, ma, ia), (sb, mb, ib)| {
        sb.total_cmp(sa)
            .then_with(|| mb.cmp(ma))
            .then_with(|| ia.rel_path.cmp(&ib.rel_path))
    });

    scored
        .into_iter()
        .take(20)
        .map(|(_, _, item)| item)
        .collect()
}

/// Normalizes a tag name by trimming whitespace, leading `#`, and leading/trailing `/`.
pub fn normalize_tag_name(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('#')
        .trim()
        .trim_matches('/')
        .to_string()
}

/// Rewrites `tags` for a tag rename (`old_tag` -> `new_tag`), handling hierarchical subtags
/// and preserving order while deduplicating merged tags. Returns `None` if unchanged.
pub fn rewrite_tags_for_rename(
    tags: &[String],
    old_tag: &str,
    new_tag: &str,
) -> Option<Vec<String>> {
    let old_clean = normalize_tag_name(old_tag);
    let new_clean = normalize_tag_name(new_tag);
    if old_clean.is_empty() || new_clean.is_empty() || old_clean == new_clean {
        return None;
    }

    let mut changed = false;
    let mut out: Vec<String> = Vec::with_capacity(tags.len());

    for tag in tags {
        let rewritten = if tag == &old_clean {
            changed = true;
            new_clean.clone()
        } else if let Some(suffix) = tag.strip_prefix(&format!("{old_clean}/")) {
            changed = true;
            format!("{new_clean}/{suffix}")
        } else {
            tag.clone()
        };

        if !out.contains(&rewritten) {
            out.push(rewritten);
        }
    }

    if changed {
        Some(out)
    } else {
        None
    }
}
