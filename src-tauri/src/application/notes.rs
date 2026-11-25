use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, Utc};

use crate::application::index::index_single_note;
use crate::application::persistence::PersistenceManager;
use crate::domain::backup::format_backup_filename;
use crate::domain::command_book::{filter_and_rank_commands, CommandEntry, CommandSearchArgs};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::frontmatter::{
    apply_yaml_patch, build_frontmatter_block, render_yaml_string, replace_frontmatter_yaml,
    YamlPatchOp, YamlPatchValue,
};
use crate::domain::markdown::{decode_base64_image, normalize_image_ext};
use crate::domain::note::{
    compose_note, compute_body_hash, count_list_items, format_ulid, meta_patch_ops,
    normalize_tag_name, parse_note, rewrite_tags_for_rename, score_related_notes, tag_matches,
    ConflictInfo, CreateNoteArgs, DirNode, NavTreeSnapshot, NoteDoc, NoteMeta, NoteMetaPatch,
    NoteSummary, NoteType, NoteUpdateMetaArgs, NotesFilterArgs, ReadNoteArgs, RelPathArgs,
    RelatedNoteCandidate, RelatedNoteItem, RenameNoteArgs, SaveImageArgs, SavedImageResult,
    SearchItemKind, SearchQueryArgs, SearchResponse, SearchResultItem, SearchSource, TagNode,
    TagRenameArgs, TagRenameResult, WorkspaceSummary, WriteNoteArgs, WriteResult, INBOX_REL_PATH,
};
use crate::domain::template::{builtin_template, render_template, TemplateVars};
use crate::domain::tokenizer::build_fts_query;
use crate::infra::fs::{atomic_write_bytes, atomic_write_str, map_io_error, prune_backups};
use crate::infra::sqlite::{
    delete_note_index_by_rel_path, get_note_key_by_rel_path, list_all_commands,
    list_all_indexed_notes, list_backlink_sources, search_notes_fts,
};

static ULID_SEQ: AtomicU64 = AtomicU64::new(1);

fn generate_ulid() -> String {
    let ts_ms = Utc::now().timestamp_millis().max(0) as u64;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = ULID_SEQ.fetch_add(1, Ordering::Relaxed) as u128;
    let random = (nanos << 32) ^ (seq.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    format_ulid(ts_ms, random)
}

fn file_mtime_and_size(path: &Path) -> Result<(i64, u64), AppError> {
    let meta = fs::metadata(path).map_err(|e| map_io_error(e, "Failed to stat note file"))?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    Ok((mtime_ms, meta.len()))
}

fn require_library_root(pm: &PersistenceManager) -> Result<PathBuf, AppError> {
    let root = pm.current_library_root()?.ok_or_else(|| {
        AppError::new(
            ErrorCode::LibraryNotConfigured,
            "Library root is not configured",
        )
    })?;
    Ok(PathBuf::from(root))
}

fn resolve_rel_path(root: &Path, abs_path: &Path) -> Option<String> {
    if let Ok(rel) = abs_path.strip_prefix(root) {
        return Some(rel.to_string_lossy().replace('\\', "/"));
    }
    let canon_root = fs::canonicalize(root).ok()?;
    let canon_abs = fs::canonicalize(abs_path).ok()?;
    canon_abs
        .strip_prefix(&canon_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

fn sanitize_filename(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for c in title.trim().chars() {
        if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('-');
        } else {
            out.push(c);
        }
    }
    let trimmed = out.trim().trim_matches('.');
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_rel_path(rel: &str) -> Result<String, AppError> {
    let cleaned = rel.trim().replace('\\', "/");
    let trimmed = cleaned.trim_start_matches('/');
    if trimmed.is_empty() || trimmed.contains("..") {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Invalid relative path",
        ));
    }
    Ok(trimmed.to_string())
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

fn expand_input_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_external_file_pinned(pm: &PersistenceManager, abs_path: &Path) -> bool {
    let Ok(pinned_list) = pm.list_pinned_files() else {
        return false;
    };
    let target = normalize_path_for_compare(abs_path);
    pinned_list.into_iter().any(|item| {
        let candidate = expand_input_path(&item);
        normalize_path_for_compare(&candidate) == target
    })
}

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
                    true
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
            "workspaces" => {
                if is_template {
                    false
                } else {
                    match args.workspace_key.as_deref() {
                        Some(ws) => row.project.as_deref() == Some(ws),
                        None => false,
                    }
                }
            }
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
            project: row.project,
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
    }

    Ok(out)
}

/// Builds the navigation snapshot: inbox count, directory tree, hierarchical tags, and workspaces (`C4.2`).
pub fn notes_get_tree_and_tags(pm: &PersistenceManager) -> Result<NavTreeSnapshot, AppError> {
    let root = require_library_root(pm)?;
    let rows = pm.with_db(list_all_indexed_notes)?;

    let inbox_path = root.join(INBOX_REL_PATH);
    let inbox_count = if inbox_path.is_file() {
        let raw = fs::read_to_string(&inbox_path)
            .map_err(|e| map_io_error(e, "Failed to read inbox.md"))?;
        let parsed = parse_note(&raw, "inbox");
        count_list_items(&parsed.body)
    } else {
        0
    };

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

    let settings = pm.reload_settings()?.settings;
    let workspaces = settings
        .workspaces
        .into_iter()
        .map(|(key, ws)| WorkspaceSummary {
            key,
            name: ws.name,
            root: ws.root,
            open_dir: ws.open_dir,
        })
        .collect();

    Ok(NavTreeSnapshot {
        inbox_count,
        dirs,
        tags,
        workspaces,
    })
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

/// Reads a note by absolute path (supports both library and external `.md` files) (`C4.2`).
pub fn note_read(pm: &PersistenceManager, args: ReadNoteArgs) -> Result<NoteDoc, AppError> {
    let abs_path = expand_input_path(&args.abs_path);
    if !abs_path.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Note file not found",
            abs_path.to_string_lossy().to_string(),
        ));
    }

    let raw = fs::read_to_string(&abs_path).map_err(|e| map_io_error(e, "Failed to read note"))?;
    let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;

    let root = pm.current_library_root()?.map(PathBuf::from);
    let rel_path = root.as_deref().and_then(|r| resolve_rel_path(r, &abs_path));
    let is_library = rel_path.is_some();
    let is_inbox = rel_path.as_deref() == Some(INBOX_REL_PATH);

    if !is_markdown_path(&abs_path) {
        let title = abs_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        let meta = NoteMeta {
            title,
            doc_type: NoteType::Note,
            pinned: if is_library {
                false
            } else {
                is_external_file_pinned(pm, &abs_path)
            },
            ..NoteMeta::default()
        };
        let body_hash = compute_body_hash(&raw);
        let key = format!("abs:{}", abs_path.to_string_lossy());
        return Ok(NoteDoc {
            key,
            rel_path,
            abs_path: abs_path.to_string_lossy().to_string(),
            is_library,
            is_inbox: false,
            meta,
            body: raw,
            body_hash,
            mtime_ms,
            size_bytes,
        });
    }

    let stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let mut parsed = parse_note(&raw, stem);
    if !is_library && !parsed.meta.pinned {
        parsed.meta.pinned = is_external_file_pinned(pm, &abs_path);
    }

    let key = if let Some(ref rel) = rel_path {
        pm.with_db(|conn| get_note_key_by_rel_path(conn, rel))?
            .unwrap_or_else(|| {
                parsed
                    .meta
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("path:{rel}"))
            })
    } else {
        parsed
            .meta
            .id
            .clone()
            .unwrap_or_else(|| format!("abs:{}", abs_path.to_string_lossy()))
    };

    Ok(NoteDoc {
        key,
        rel_path,
        abs_path: abs_path.to_string_lossy().to_string(),
        is_library,
        is_inbox,
        meta: parsed.meta,
        body_hash: compute_body_hash(&parsed.body),
        body: parsed.body,
        mtime_ms,
        size_bytes,
    })
}

/// Creates a new note in `dir_rel_path`, writes it atomically, and indexes it (`C4.2`).
pub fn note_create(pm: &PersistenceManager, args: CreateNoteArgs) -> Result<NoteDoc, AppError> {
    let title = args.title.trim();
    if title.is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Note title cannot be empty",
        ));
    }

    let root = require_library_root(pm)?;
    let dir_clean = args
        .dir_rel_path
        .trim()
        .trim_matches('/')
        .replace('\\', "/");
    if dir_clean.contains("..") {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Invalid directory relative path",
        ));
    }

    let file_name = format!("{}.md", sanitize_filename(title));
    let rel_path = if dir_clean.is_empty() {
        file_name
    } else {
        format!("{dir_clean}/{file_name}")
    };
    let abs_path = root.join(&rel_path);
    if abs_path.exists() {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Note file already exists",
            rel_path,
        ));
    }

    let raw_template = resolve_template_raw(&root, args.template_name.as_deref())?;
    let id = generate_ulid();
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M:%S").to_string();
    let rendered = render_template(
        &raw_template,
        &TemplateVars {
            id: &id,
            title,
            date: &date,
            time: &time,
        },
    );

    let content = match args.body.as_deref() {
        Some(extra) if !extra.trim().is_empty() => {
            let trimmed_tpl = rendered.trim_end_matches('\n');
            let trimmed_extra = extra.trim_start_matches('\n').trim_end_matches('\n');
            format!("{trimmed_tpl}\n\n{trimmed_extra}\n")
        }
        _ if rendered.ends_with('\n') => rendered,
        _ => format!("{rendered}\n"),
    };

    let stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");
    let created_parsed = parse_note(&content, stem);
    pm.record_self_write(&abs_path, &compute_body_hash(&created_parsed.body));
    if let Err(err) = atomic_write_str(&abs_path, &content) {
        pm.clear_self_write(&abs_path);
        return Err(err);
    }
    let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;
    index_single_note(pm, &rel_path, &content, mtime_ms, size_bytes as i64)?;

    note_read(
        pm,
        ReadNoteArgs {
            abs_path: abs_path.to_string_lossy().to_string(),
        },
    )
}

fn resolve_template_raw(root: &Path, template_name: Option<&str>) -> Result<String, AppError> {
    let clean_name = match template_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => {
            let stripped = raw.strip_suffix(".md").unwrap_or(raw);
            if stripped.is_empty()
                || stripped.contains('/')
                || stripped.contains('\\')
                || stripped.contains("..")
            {
                return Err(AppError::new(
                    ErrorCode::ValidationError,
                    "Invalid template name",
                ));
            }
            stripped
        }
        None => "note",
    };

    let custom_path = root.join("_templates").join(format!("{clean_name}.md"));
    if custom_path.is_file() {
        return fs::read_to_string(&custom_path)
            .map_err(|e| map_io_error(e, "Failed to read template file"));
    }

    if let Some(builtin) = builtin_template(clean_name) {
        return Ok(builtin.to_string());
    }

    Err(AppError::with_detail(
        ErrorCode::NotFound,
        "Template not found",
        clean_name.to_string(),
    ))
}

/// Writes `body` with optimistic concurrency check (`base_body_hash`) and ULID back-fill (`D-0006`).
pub fn note_write(pm: &PersistenceManager, args: WriteNoteArgs) -> Result<WriteResult, AppError> {
    let abs_path = expand_input_path(&args.abs_path);
    if !abs_path.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Note file not found",
            abs_path.to_string_lossy().to_string(),
        ));
    }

    let raw = fs::read_to_string(&abs_path).map_err(|e| map_io_error(e, "Failed to read note"))?;
    let (current_mtime_ms, _) = file_mtime_and_size(&abs_path)?;

    if !is_markdown_path(&abs_path) {
        let disk_body_hash = compute_body_hash(&raw);
        if disk_body_hash != args.base_body_hash {
            let conflict = ConflictInfo {
                abs_path: abs_path.to_string_lossy().to_string(),
                current_body: raw,
                current_body_hash: disk_body_hash,
                current_mtime_ms,
            };
            let detail = serde_json::to_string(&conflict).map_err(|e| {
                AppError::with_detail(
                    ErrorCode::InternalError,
                    "Failed to serialize conflict info",
                    e.to_string(),
                )
            })?;
            return Err(AppError::with_detail(
                ErrorCode::Conflict,
                "Document modified on disk",
                detail,
            ));
        }

        let new_body_hash = compute_body_hash(&args.body);
        pm.record_self_write(&abs_path, &new_body_hash);
        if let Err(err) = atomic_write_str(&abs_path, &args.body) {
            pm.clear_self_write(&abs_path);
            return Err(err);
        }
        let (new_mtime_ms, _) = file_mtime_and_size(&abs_path)?;
        let _ = pm.clear_recovery_snapshot(&args.abs_path);
        let abs_lossy = abs_path.to_string_lossy();
        if abs_lossy != args.abs_path {
            let _ = pm.clear_recovery_snapshot(&abs_lossy);
        }

        return Ok(WriteResult {
            body_hash: new_body_hash,
            mtime_ms: new_mtime_ms,
            updated_meta: None,
        });
    }

    let stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let parsed = parse_note(&raw, stem);

    let disk_body_hash = compute_body_hash(&parsed.body);
    if disk_body_hash != args.base_body_hash {
        let conflict = ConflictInfo {
            abs_path: abs_path.to_string_lossy().to_string(),
            current_body: parsed.body,
            current_body_hash: disk_body_hash,
            current_mtime_ms,
        };
        let detail = serde_json::to_string(&conflict).map_err(|e| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to serialize conflict info",
                e.to_string(),
            )
        })?;
        return Err(AppError::with_detail(
            ErrorCode::Conflict,
            "Document modified on disk",
            detail,
        ));
    }

    let root = pm.current_library_root()?.map(PathBuf::from);
    let rel_path = root.as_deref().and_then(|r| resolve_rel_path(r, &abs_path));
    let is_library = rel_path.is_some();

    let mut updated_meta: Option<NoteMeta> = None;
    let new_content = if is_library && parsed.meta.id.is_none() && parsed.yaml_patchable {
        let new_id = generate_ulid();
        let ops = meta_patch_ops(&NoteMetaPatch::default());
        let mut id_ops = ops;
        id_ops.push(YamlPatchOp {
            key: "id".to_string(),
            value: Some(YamlPatchValue::Str(new_id.clone())),
        });
        let base_yaml = parsed.yaml.as_deref().unwrap_or("");
        let patched_yaml = apply_yaml_patch(base_yaml, &id_ops).map_err(|e| {
            AppError::with_detail(
                ErrorCode::InvalidFrontmatter,
                "Failed to patch frontmatter id",
                format!("{e:?}"),
            )
        })?;
        let new_block = build_frontmatter_block(&patched_yaml);
        let mut new_meta = parsed.meta.clone();
        new_meta.id = Some(new_id);
        updated_meta = Some(new_meta);
        compose_note(Some(&new_block), &args.body)
    } else {
        compose_note(parsed.block.as_deref(), &args.body)
    };

    let new_parsed = parse_note(&new_content, stem);
    let new_body_hash = compute_body_hash(&new_parsed.body);
    pm.record_self_write(&abs_path, &new_body_hash);
    if let Err(err) = atomic_write_str(&abs_path, &new_content) {
        pm.clear_self_write(&abs_path);
        return Err(err);
    }
    let (new_mtime_ms, new_size_bytes) = file_mtime_and_size(&abs_path)?;

    if let Some(rel) = rel_path {
        index_single_note(pm, &rel, &new_content, new_mtime_ms, new_size_bytes as i64)?;
    }

    let _ = pm.clear_recovery_snapshot(&args.abs_path);
    let abs_lossy = abs_path.to_string_lossy();
    if abs_lossy != args.abs_path {
        let _ = pm.clear_recovery_snapshot(&abs_lossy);
    }

    Ok(WriteResult {
        body_hash: new_body_hash,
        mtime_ms: new_mtime_ms,
        updated_meta,
    })
}

/// Applies a partial metadata patch to frontmatter, preserving unknown keys and comments (`D-0006`).
pub fn note_update_meta(
    pm: &PersistenceManager,
    args: NoteUpdateMetaArgs,
) -> Result<NoteDoc, AppError> {
    let abs_path = expand_input_path(&args.abs_path);
    if !abs_path.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Note file not found",
            abs_path.to_string_lossy().to_string(),
        ));
    }
    if !is_markdown_path(&abs_path) {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Only Markdown files support frontmatter metadata",
            abs_path.to_string_lossy().to_string(),
        ));
    }

    let raw = fs::read_to_string(&abs_path).map_err(|e| map_io_error(e, "Failed to read note"))?;
    let stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let parsed = parse_note(&raw, stem);
    let ops = meta_patch_ops(&args.patch);

    let new_content = if parsed.yaml_patchable {
        let base_yaml = parsed.yaml.as_deref().unwrap_or("");
        let patched_yaml = apply_yaml_patch(base_yaml, &ops).map_err(|e| {
            AppError::with_detail(
                ErrorCode::InvalidFrontmatter,
                "Failed to patch frontmatter",
                format!("{e:?}"),
            )
        })?;
        replace_frontmatter_yaml(&raw, &patched_yaml)
    } else if parsed.block.is_none() {
        // File has no frontmatter yet (not broken YAML): synthesize minimal frontmatter without backing up.
        let merged_meta = merge_meta_with_patch(parsed.meta, &args.patch);
        let minimal_yaml = build_minimal_yaml(&merged_meta);
        let block = build_frontmatter_block(&minimal_yaml);
        compose_note(Some(&block), &parsed.body)
    } else {
        // Broken YAML: back up original file to $APP_DATA_DIR/backups/ first.
        let backups_dir = pm.backups_dir();
        let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let file_name = abs_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("note.md");
        let backup_path = backups_dir.join(format_backup_filename(file_name, &ts));
        atomic_write_str(&backup_path, &raw)?;
        let _ = prune_backups(&backups_dir, Utc::now().timestamp());

        let merged_meta = merge_meta_with_patch(parsed.meta, &args.patch);
        let minimal_yaml = build_minimal_yaml(&merged_meta);
        let block = build_frontmatter_block(&minimal_yaml);
        compose_note(Some(&block), &parsed.body)
    };

    let meta_stem = abs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");
    let updated_parsed = parse_note(&new_content, meta_stem);
    pm.record_self_write(&abs_path, &compute_body_hash(&updated_parsed.body));
    if let Err(err) = atomic_write_str(&abs_path, &new_content) {
        pm.clear_self_write(&abs_path);
        return Err(err);
    }
    let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;

    let root = pm.current_library_root()?.map(PathBuf::from);
    if let Some(rel) = root.as_deref().and_then(|r| resolve_rel_path(r, &abs_path)) {
        index_single_note(pm, &rel, &new_content, mtime_ms, size_bytes as i64)?;
    }

    note_read(
        pm,
        ReadNoteArgs {
            abs_path: args.abs_path,
        },
    )
}

fn merge_meta_with_patch(mut meta: NoteMeta, patch: &NoteMetaPatch) -> NoteMeta {
    if let Some(ref title) = patch.title {
        meta.title = title.clone();
    }
    if let Some(doc_type) = patch.doc_type {
        meta.doc_type = doc_type;
    }
    if let Some(ref tags) = patch.tags {
        meta.tags = tags.clone();
    }
    if let Some(ref project) = patch.project {
        meta.project = project.clone();
    }
    if let Some(ref open_dir) = patch.open_dir {
        meta.open_dir = open_dir.clone();
    }
    if let Some(ref cw) = patch.code_workspace {
        meta.code_workspace = cw.clone();
    }
    if let Some(ref td) = patch.terminal_dir {
        meta.terminal_dir = td.clone();
    }
    if let Some(pinned) = patch.pinned {
        meta.pinned = pinned;
    }
    if let Some(archived) = patch.archived {
        meta.archived = archived;
    }
    meta
}

fn build_minimal_yaml(meta: &NoteMeta) -> String {
    let mut lines = Vec::new();
    if let Some(ref id) = meta.id {
        lines.push(format!("id: {}", render_yaml_string(id)));
    }
    lines.push(format!("title: {}", render_yaml_string(&meta.title)));
    lines.push(format!("type: {}", meta.doc_type.as_str()));
    if meta.tags.is_empty() {
        lines.push("tags: []".to_string());
    } else {
        lines.push("tags:".to_string());
        for t in &meta.tags {
            lines.push(format!("  - {}", render_yaml_string(t)));
        }
    }
    if let Some(ref p) = meta.project {
        lines.push(format!("project: {}", render_yaml_string(p)));
    }
    if let Some(ref open_dir) = meta.open_dir {
        lines.push(format!("open_dir: {}", render_yaml_string(open_dir)));
    }
    if let Some(ref cw) = meta.code_workspace {
        lines.push(format!("code_workspace: {}", render_yaml_string(cw)));
    }
    if let Some(ref td) = meta.terminal_dir {
        lines.push(format!("terminal_dir: {}", render_yaml_string(td)));
    }
    if let Some(ref created) = meta.created {
        let escaped = created.replace('\\', "\\\\").replace('"', "\\\"");
        lines.push(format!("created: \"{escaped}\""));
    }
    lines.push(format!("pinned: {}", meta.pinned));
    lines.push(format!("archived: {}", meta.archived));
    lines.join("\n") + "\n"
}

/// Renames or moves a library note (`inbox.md` cannot be renamed) (`C4.2`).
pub fn note_rename(pm: &PersistenceManager, args: RenameNoteArgs) -> Result<NoteDoc, AppError> {
    let root = require_library_root(pm)?;
    let old_rel = normalize_rel_path(&args.rel_path)?;
    let new_rel = normalize_rel_path(&args.new_rel_path)?;

    if old_rel == INBOX_REL_PATH || new_rel == INBOX_REL_PATH {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "inbox.md cannot be renamed",
        ));
    }

    let old_abs = root.join(&old_rel);
    let new_abs = root.join(&new_rel);
    if !old_abs.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Source note not found",
            old_rel,
        ));
    }
    if new_abs.exists() {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Target note path already exists",
            new_rel,
        ));
    }

    if let Some(parent) = new_abs.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| map_io_error(e, "Failed to create target directory"))?;
    }
    let rename_stem = new_abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");
    let old_raw = fs::read_to_string(&old_abs)
        .map_err(|e| map_io_error(e, "Failed to read note before rename"))?;
    let old_parsed = parse_note(&old_raw, rename_stem);
    pm.record_self_write(&new_abs, &compute_body_hash(&old_parsed.body));
    if let Err(e) = fs::rename(&old_abs, &new_abs) {
        pm.clear_self_write(&new_abs);
        return Err(map_io_error(e, "Failed to rename note file"));
    }

    let raw =
        fs::read_to_string(&new_abs).map_err(|e| map_io_error(e, "Failed to read renamed note"))?;
    let (mtime_ms, size_bytes) = file_mtime_and_size(&new_abs)?;

    pm.with_db(|conn| delete_note_index_by_rel_path(conn, &old_rel))?;
    index_single_note(pm, &new_rel, &raw, mtime_ms, size_bytes as i64)?;
    pm.clear_self_write(&old_abs);

    note_read(
        pm,
        ReadNoteArgs {
            abs_path: new_abs.to_string_lossy().to_string(),
        },
    )
}

/// Deletes a library note (`inbox.md` cannot be deleted) and removes it from SQLite (`C4.2`).
pub fn note_delete(pm: &PersistenceManager, args: RelPathArgs) -> Result<(), AppError> {
    let root = require_library_root(pm)?;
    let rel = normalize_rel_path(&args.rel_path)?;
    if rel == INBOX_REL_PATH {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "inbox.md cannot be deleted",
        ));
    }

    let abs_path = root.join(&rel);
    if !abs_path.is_file() {
        return Err(AppError::with_detail(
            ErrorCode::NotFound,
            "Note file not found",
            rel,
        ));
    }

    fs::remove_file(&abs_path).map_err(|e| map_io_error(e, "Failed to delete note file"))?;
    pm.with_db(|conn| delete_note_index_by_rel_path(conn, &rel))?;
    pm.clear_self_write(&abs_path);
    Ok(())
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
        let title = path_obj
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&pinned_path)
            .to_string();
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

/// Maximum allowed image payload size (20 MiB) for `note_save_image` (`v4` §5.6.3).
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

/// Saves an image from base64 or a local source path into `assets/YYYY-MM/` (`C4.2`, `v4` §5.6.3).
pub fn note_save_image(
    pm: &PersistenceManager,
    args: SaveImageArgs,
) -> Result<SavedImageResult, AppError> {
    let root = require_library_root(pm)?;
    let ext = normalize_image_ext(&args.ext)?;

    let bytes = if let Some(b64) = args
        .bytes_base64
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        decode_base64_image(b64, MAX_IMAGE_BYTES)?
    } else if let Some(src) = args
        .source_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let src_path = PathBuf::from(src);
        if !src_path.is_file() {
            return Err(AppError::with_detail(
                ErrorCode::NotFound,
                "Source image file not found",
                src.to_string(),
            ));
        }
        let meta = fs::metadata(&src_path).map_err(|e| map_io_error(e, "Failed to stat image"))?;
        if meta.len() > MAX_IMAGE_BYTES as u64 {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "Image file exceeds maximum allowed size",
            ));
        }
        let data = fs::read(&src_path).map_err(|e| map_io_error(e, "Failed to read image file"))?;
        if data.len() > MAX_IMAGE_BYTES {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "Image file exceeds maximum allowed size",
            ));
        }
        data
    } else {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Either bytesBase64 or sourcePath must be provided",
        ));
    };

    let now = Local::now();
    let ym = now.format("%Y-%m").to_string();
    let ts = now.format("%Y%m%d-%H%M%S").to_string();
    let ulid = generate_ulid().to_ascii_lowercase();
    let short_ulid = ulid.get(ulid.len().saturating_sub(8)..).unwrap_or(&ulid);
    let rel_path = format!("assets/{ym}/{ts}-{short_ulid}.{ext}");
    let abs_path = root.join(&rel_path);

    atomic_write_bytes(&abs_path, &bytes)?;

    Ok(SavedImageResult { rel_path })
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
            project: current_row.project.as_deref(),
            mtime_ms: current_row.mtime_ms,
        };

        let candidates: Vec<RelatedNoteCandidate<'_>> = rows
            .iter()
            .map(|r| RelatedNoteCandidate {
                key: &r.key,
                rel_path: &r.rel_path,
                title: &r.title,
                tags: &r.tags,
                project: r.project.as_deref(),
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

/// Renames or merges a tag (and its hierarchical subtags) across all patchable library notes (`C4.2`, `C6.4`, `D-0006`).
pub fn tags_batch_rename(
    pm: &PersistenceManager,
    args: TagRenameArgs,
) -> Result<TagRenameResult, AppError> {
    let root = require_library_root(pm)?;
    let old_tag = normalize_tag_name(&args.old_tag);
    let new_tag = normalize_tag_name(&args.new_tag);

    if old_tag.is_empty() || new_tag.is_empty() || old_tag == new_tag {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "oldTag and newTag must be non-empty and distinct",
        ));
    }

    let rows = pm.with_db(list_all_indexed_notes)?;
    let mut updated_files: u32 = 0;

    for row in rows {
        if row.rel_path.starts_with("_templates/") || row.meta_error.is_some() {
            continue;
        }
        if !row.tags.iter().any(|t| tag_matches(t, &old_tag)) {
            continue;
        }

        let abs_path = root.join(&row.rel_path);
        if !abs_path.is_file() {
            continue;
        }

        let raw = fs::read_to_string(&abs_path)
            .map_err(|e| map_io_error(e, "Failed to read note during tag batch rename"))?;
        let stem = abs_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        let parsed = parse_note(&raw, stem);
        if !parsed.yaml_patchable || parsed.meta.meta_error.is_some() {
            continue;
        }

        let Some(new_tags) = rewrite_tags_for_rename(&parsed.meta.tags, &old_tag, &new_tag) else {
            continue;
        };

        let patch = NoteMetaPatch {
            tags: Some(new_tags),
            ..NoteMetaPatch::default()
        };
        let ops = meta_patch_ops(&patch);
        let base_yaml = parsed.yaml.as_deref().unwrap_or("");
        let patched_yaml = match apply_yaml_patch(base_yaml, &ops) {
            Ok(y) => y,
            Err(_) => continue,
        };
        let new_block = build_frontmatter_block(&patched_yaml);
        let new_content = compose_note(Some(&new_block), &parsed.body);

        pm.record_self_write(&abs_path, &compute_body_hash(&parsed.body));
        if let Err(err) = atomic_write_str(&abs_path, &new_content) {
            pm.clear_self_write(&abs_path);
            return Err(err);
        }
        let (mtime_ms, size_bytes) = file_mtime_and_size(&abs_path)?;
        index_single_note(pm, &row.rel_path, &new_content, mtime_ms, size_bytes as i64)?;
        updated_files += 1;
    }

    Ok(TagRenameResult { updated_files })
}
