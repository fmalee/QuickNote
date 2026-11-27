use std::fs;
use std::path::Path;

use crate::application::persistence::PersistenceManager;
use crate::domain::command_book::{filter_and_rank_commands, CommandEntry, CommandSearchArgs};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::{
    parse_note, score_related_notes, RelPathArgs, RelatedNoteCandidate, RelatedNoteItem,
    SearchItemKind, SearchQueryArgs, SearchResponse, SearchResultItem, SearchSource,
};
use crate::domain::tokenizer::build_fts_query;
use crate::infra::sqlite::{
    list_all_commands, list_all_indexed_notes, list_backlink_sources, search_notes_fts,
};

use super::{is_markdown_path, normalize_rel_path, require_library_root};

fn resolve_external_file_display_title(path: &Path, fallback: &str) -> String {
    if is_markdown_path(path) {
        let stem = path
            .file_stem()
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback);
        if let Ok(raw) = fs::read_to_string(path) {
            return parse_note(&raw, stem).meta.title;
        }
        return stem.to_string();
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(fallback)
        .to_string()
}

/// Searches indexed notes using FTS5 mixed tokenization and title/path substring boosting (`C5.3`),
/// merged with pinned external files from `pinned-files.json`.
pub fn search_notes_and_files(
    pm: &PersistenceManager,
    args: SearchQueryArgs,
) -> Result<SearchResponse, AppError> {
    let root = require_library_root(pm)?;
    let limit = args.limit.unwrap_or(30).max(1) as usize;
    let raw_query = args.query.trim();
    let raw_lower = raw_query.to_lowercase();

    let mut items: Vec<SearchResultItem> = Vec::new();
    for pinned_path in pm.list_pinned_files().unwrap_or_default() {
        let path_obj = Path::new(&pinned_path);
        if !path_obj.is_file() {
            continue;
        }
        let title = resolve_external_file_display_title(path_obj, &pinned_path);
        if !raw_lower.is_empty()
            && !title.to_lowercase().contains(&raw_lower)
            && !pinned_path.to_lowercase().contains(&raw_lower)
        {
            continue;
        }
        items.push(SearchResultItem {
            kind: SearchItemKind::File,
            key: format!("abs:{pinned_path}"),
            title,
            subtitle: format!("[已固定] {pinned_path}"),
            rel_path: None,
            abs_path: pinned_path,
            score: 1000.0,
            source: SearchSource::Recent,
        });
    }

    let mut fts_truncated = false;
    if let Some(fts_expr) = build_fts_query(raw_query) {
        let hits = pm.with_db(|conn| search_notes_fts(conn, &fts_expr, limit + 1))?;
        fts_truncated = hits.len() > limit;

        for hit in hits.into_iter().take(limit) {
            let mut score = -hit.rank;
            if hit.title.to_lowercase().contains(&raw_lower) {
                score += 50.0;
            }
            if hit.rel_path.to_lowercase().contains(&raw_lower) {
                score += 20.0;
            }
            let abs_path = root.join(&hit.rel_path).to_string_lossy().to_string();
            items.push(SearchResultItem {
                kind: SearchItemKind::Note,
                key: hit.key,
                title: hit.title,
                subtitle: hit.rel_path.clone(),
                rel_path: Some(hit.rel_path),
                abs_path,
                score,
                source: SearchSource::Library,
            });
        }
    }

    items.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let truncated = fts_truncated || items.len() > limit;
    items.truncate(limit);

    Ok(SearchResponse {
        request_id: args.request_id,
        items,
        truncated,
    })
}

/// Searches indexed command-book entries (`C4.2`, `v4` §4.8).
pub fn commands_search(
    pm: &PersistenceManager,
    args: CommandSearchArgs,
) -> Result<Vec<CommandEntry>, AppError> {
    let _ = require_library_root(pm)?;
    let entries = pm.with_db(list_all_commands)?;
    Ok(filter_and_rank_commands(&entries, &args.query))
}

/// Returns up to 20 related notes scored by backlinks, shared tags, and shared project (`C4.2`, `C8.6`).
pub fn notes_get_related(
    pm: &PersistenceManager,
    args: RelPathArgs,
) -> Result<Vec<RelatedNoteItem>, AppError> {
    let _ = require_library_root(pm)?;
    let rel_path = normalize_rel_path(&args.rel_path)?;

    pm.with_db(|conn| {
        let rows = list_all_indexed_notes(conn)?;
        let Some(current_row) = rows.iter().find(|r| r.rel_path == rel_path) else {
            return Err(AppError::with_detail(
                ErrorCode::NotFound,
                "Note not found in index",
                rel_path.clone(),
            ));
        };

        let backlink_sources = list_backlink_sources(conn, &rel_path)?;
        let current = RelatedNoteCandidate {
            key: &current_row.key,
            rel_path: &current_row.rel_path,
            title: &current_row.title,
            tags: &current_row.tags,
            mtime_ms: current_row.mtime_ms,
        };

        let candidates: Vec<RelatedNoteCandidate<'_>> = rows
            .iter()
            .map(|r| RelatedNoteCandidate {
                key: &r.key,
                rel_path: &r.rel_path,
                title: &r.title,
                tags: &r.tags,
                mtime_ms: r.mtime_ms,
            })
            .collect();

        Ok(score_related_notes(
            &current,
            &candidates,
            &backlink_sources,
        ))
    })
}
