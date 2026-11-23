use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::INBOX_REL_PATH;
use crate::domain::settings::WorkspaceConfig;
use crate::domain::ui_state::{CaptureMode, UiState};

/// Resolved capture target kind (`C8.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    Unconfigured,
    Note,
    Inbox,
}

/// Input parameters for resolving Quick Bar capture target (`C8.1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TargetResolveInput {
    pub main_visible: bool,
    pub active_doc_key: Option<String>,
    pub selected_workspace_key: Option<String>,
    pub is_inbox_view: bool,
}

/// Resolved Quick Bar capture target and display metadata (`C8.1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTarget {
    pub scenario: String,
    pub kind: TargetKind,
    pub target_rel_path: Option<String>,
    pub target_title: String,
    pub section_hint: Option<String>,
    pub workspace_key: Option<String>,
    pub workspace_name: Option<String>,
    pub is_cross_workspace_fallback: bool,
    pub recommended_mode: CaptureMode,
    pub chip_label: String,
}

/// Arguments for `note_append` (`C8.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppendNoteArgs {
    pub target_rel_path: String,
    pub content: String,
    pub mode: CaptureMode,
    pub workspace_key: Option<String>,
}

/// Result returned by `note_append` and emitted on `toast:append-undo` (`C8.2`, `C8.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppendResult {
    pub rel_path: String,
    pub title: String,
    pub added_lines: u32,
    pub section_heading: Option<String>,
    pub body_hash: String,
    #[specta(type = specta_typescript::Number)]
    pub mtime_ms: i64,
    pub undo_available: bool,
}

/// Result returned by `note_undo_last_append` (`C8.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UndoAppendResult {
    pub rel_path: String,
    pub restored_body_hash: String,
    #[specta(type = specta_typescript::Number)]
    pub mtime_ms: i64,
}

/// Arguments for `inbox_extract_to_note` (`C3.4`, `C4.2`, `v4` §4.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InboxExtractArgs {
    pub from_line: u32,
    pub to_line: u32,
    pub new_title: String,
    pub target_dir_rel_path: String,
    pub template_name: String,
}

/// Pure candidate metadata for `resolve_target` without filesystem dependencies (`C1.2`, `C8.1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveDocCandidate {
    pub key: String,
    pub rel_path: String,
    pub title: String,
    pub project: Option<String>,
    pub has_ideas_section: Option<bool>,
    pub has_todo_section: Option<bool>,
}

/// Pure context passed into `resolve_target` (`C8.1`).
pub struct ResolveTargetContext<'a> {
    pub library_configured: bool,
    pub workspaces: &'a BTreeMap<String, WorkspaceConfig>,
    pub ui_state: &'a UiState,
    pub docs: &'a [ResolveDocCandidate],
    pub last_active_ttl_hours: u32,
    pub auto_create_missing_section: bool,
    pub now_ms: i64,
    pub today_date: &'a str,
}

/// Output of pure `append_to_body` transformation (`C8.2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendBodyOutput {
    pub new_body: String,
    pub added_lines: u32,
    pub section_heading: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum SelectedTarget<'a> {
    Doc(&'a ResolveDocCandidate),
    Inbox {
        title: &'a str,
        workspace_key: Option<&'a str>,
        workspace_name: Option<&'a str>,
        is_cross_workspace_fallback: bool,
    },
}

/// Pure `S1–S8` Quick Bar target resolution truth table (`C8.1`).
pub fn resolve_target(
    ctx: &ResolveTargetContext<'_>,
    input: &TargetResolveInput,
) -> ResolvedTarget {
    if !ctx.library_configured {
        return ResolvedTarget {
            scenario: "S1".to_string(),
            kind: TargetKind::Unconfigured,
            target_rel_path: None,
            target_title: "未选库".to_string(),
            section_hint: None,
            workspace_key: None,
            workspace_name: None,
            is_cross_workspace_fallback: false,
            recommended_mode: CaptureMode::List,
            chip_label: "请先选择或创建笔记库".to_string(),
        };
    }

    let valid_last = resolve_valid_last_active(ctx);

    let (scenario, selected) = if input.main_visible {
        if let Some(active_doc) = input
            .active_doc_key
            .as_deref()
            .and_then(|key| find_doc_by_key_or_path(ctx.docs, key))
        {
            ("S2", SelectedTarget::Doc(active_doc))
        } else if let Some(ws_key) = input.selected_workspace_key.as_deref() {
            if let Some((last_doc, last_ws)) = valid_last {
                if last_ws.as_deref() == Some(ws_key) {
                    ("S3", SelectedTarget::Doc(last_doc))
                } else {
                    ("S4", resolve_workspace_fallback(ctx, ws_key, true))
                }
            } else {
                ("S4", resolve_workspace_fallback(ctx, ws_key, false))
            }
        } else if input.is_inbox_view {
            ("S5", inbox_selection(ctx.docs, None, None, false))
        } else if let Some((last_doc, _)) = valid_last {
            ("S5", SelectedTarget::Doc(last_doc))
        } else {
            ("S5", inbox_selection(ctx.docs, None, None, false))
        }
    } else if let Some(ws_key) = input.selected_workspace_key.as_deref() {
        if let Some((last_doc, last_ws)) = valid_last {
            if last_ws.as_deref() == Some(ws_key) {
                ("S6", SelectedTarget::Doc(last_doc))
            } else {
                ("S7", resolve_workspace_fallback(ctx, ws_key, true))
            }
        } else {
            ("S7", resolve_workspace_fallback(ctx, ws_key, false))
        }
    } else if let Some((last_doc, _)) = valid_last {
        ("S8", SelectedTarget::Doc(last_doc))
    } else {
        ("S8", inbox_selection(ctx.docs, None, None, false))
    };

    build_resolved_target(ctx, scenario, selected)
}

/// Refines `section_hint` and `chip_label` for a `Note` target using its actual markdown body.
pub fn apply_doc_section_hints(
    resolved: &mut ResolvedTarget,
    body: &str,
    auto_create_missing_section: bool,
) {
    if resolved.kind != TargetKind::Note {
        return;
    }

    let (section_hint, chip_label) = compute_note_hints(
        &resolved.target_title,
        resolved.workspace_name.as_deref(),
        resolved.recommended_mode,
        Some(has_section_heading(body, "## 想法")),
        Some(has_section_heading(body, "## 待办")),
        auto_create_missing_section,
    );
    resolved.section_hint = section_hint;
    resolved.chip_label = chip_label;
}

/// Checks whether a markdown body contains `heading` (e.g. `"## 想法"`) outside fenced code blocks.
pub fn has_section_heading(body: &str, heading: &str) -> bool {
    let lines: Vec<&str> = body.lines().collect();
    find_heading_line(&lines, heading).is_some()
}

/// Pure section-scanning and prefix-deduplicating body append (`C8.2`).
pub fn append_to_body(
    body: &str,
    content: &str,
    mode: CaptureMode,
    is_inbox: bool,
    today_date: &str,
    hh_mm: &str,
    auto_create_missing_section: bool,
) -> Result<AppendBodyOutput, AppError> {
    let formatted_lines = format_capture_lines(content, mode, is_inbox, hh_mm)?;
    let added_lines = formatted_lines.len() as u32;

    let target_heading = if is_inbox {
        Some(format!("## {today_date}"))
    } else {
        match mode {
            CaptureMode::List => Some("## 想法".to_string()),
            CaptureMode::Todo => Some("## 待办".to_string()),
            CaptureMode::Prose => None,
        }
    };

    let mut lines: Vec<String> = if body.is_empty() {
        Vec::new()
    } else {
        body.lines().map(str::to_string).collect()
    };

    let section_heading = if let Some(heading) = target_heading {
        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        if let Some(heading_idx) = find_heading_line(&line_refs, &heading) {
            let next_heading_idx = find_next_section_boundary(&line_refs, heading_idx + 1);
            let mut insert_idx = next_heading_idx;
            while insert_idx > heading_idx + 1 && lines[insert_idx - 1].trim().is_empty() {
                insert_idx -= 1;
            }

            let had_next_heading = next_heading_idx < lines.len();
            let mut to_insert = formatted_lines;
            if had_next_heading && insert_idx == next_heading_idx {
                to_insert.push(String::new());
            }
            lines.splice(insert_idx..insert_idx, to_insert);
            Some(heading)
        } else if is_inbox || auto_create_missing_section {
            trim_trailing_blank_lines(&mut lines);
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(heading.clone());
            lines.extend(formatted_lines);
            Some(heading)
        } else {
            append_at_eof(&mut lines, formatted_lines, false);
            None
        }
    } else {
        append_at_eof(&mut lines, formatted_lines, true);
        None
    };

    let mut new_body = lines.join("\n");
    if !new_body.is_empty() && !new_body.ends_with('\n') {
        new_body.push('\n');
    }

    Ok(AppendBodyOutput {
        new_body,
        added_lines,
        section_heading,
    })
}

fn find_doc_by_key_or_path<'a>(
    docs: &'a [ResolveDocCandidate],
    key_or_path: &str,
) -> Option<&'a ResolveDocCandidate> {
    docs.iter()
        .find(|d| d.key == key_or_path || d.rel_path == key_or_path)
}

fn find_workspace<'a>(
    workspaces: &'a BTreeMap<String, WorkspaceConfig>,
    key_or_name: &str,
) -> Option<&'a WorkspaceConfig> {
    workspaces.get(key_or_name).or_else(|| {
        workspaces
            .values()
            .find(|w| w.key == key_or_name || w.name == key_or_name)
    })
}

fn resolve_valid_last_active<'a>(
    ctx: &'a ResolveTargetContext<'a>,
) -> Option<(&'a ResolveDocCandidate, Option<String>)> {
    let last = ctx.ui_state.last_active_doc.as_ref()?;
    if ctx.last_active_ttl_hours == 0 {
        return None;
    }
    let ttl_ms = i64::from(ctx.last_active_ttl_hours).saturating_mul(3_600_000);
    let age_ms = ctx.now_ms.saturating_sub(last.touched_at_ms);
    if age_ms < 0 || age_ms > ttl_ms {
        return None;
    }

    let doc = ctx.docs.iter().find(|d| d.rel_path == last.rel_path)?;
    let doc_ws = last.workspace_key.clone().or_else(|| {
        doc.project
            .as_deref()
            .map(|p| normalize_workspace_key(ctx.workspaces, p))
    });
    Some((doc, doc_ws))
}

fn normalize_workspace_key(
    workspaces: &BTreeMap<String, WorkspaceConfig>,
    key_or_name: &str,
) -> String {
    find_workspace(workspaces, key_or_name)
        .map(|w| w.key.clone())
        .unwrap_or_else(|| key_or_name.to_string())
}

fn resolve_workspace_fallback<'a>(
    ctx: &'a ResolveTargetContext<'a>,
    ws_key: &'a str,
    has_other_workspace_last: bool,
) -> SelectedTarget<'a> {
    let ws = find_workspace(ctx.workspaces, ws_key);
    if let Some(entry_rel) = ws.and_then(|w| w.default_entry_doc.as_deref()) {
        if let Some(entry_doc) = ctx.docs.iter().find(|d| d.rel_path == entry_rel) {
            return SelectedTarget::Doc(entry_doc);
        }
    }

    let ws_name = ws.map(|w| w.name.as_str());
    inbox_selection(ctx.docs, Some(ws_key), ws_name, has_other_workspace_last)
}

fn inbox_selection<'a>(
    docs: &'a [ResolveDocCandidate],
    workspace_key: Option<&'a str>,
    workspace_name: Option<&'a str>,
    is_cross_workspace_fallback: bool,
) -> SelectedTarget<'a> {
    let title = docs
        .iter()
        .find(|d| d.rel_path == INBOX_REL_PATH)
        .map(|d| d.title.as_str())
        .filter(|t| !t.is_empty())
        .unwrap_or("收件箱");
    SelectedTarget::Inbox {
        title,
        workspace_key,
        workspace_name,
        is_cross_workspace_fallback,
    }
}

fn build_resolved_target(
    ctx: &ResolveTargetContext<'_>,
    scenario: &str,
    selected: SelectedTarget<'_>,
) -> ResolvedTarget {
    match selected {
        SelectedTarget::Inbox {
            title,
            workspace_key,
            workspace_name,
            is_cross_workspace_fallback,
        } => {
            let chip_label = if is_cross_workspace_fallback {
                "收件箱 · 跨工作区".to_string()
            } else {
                format!("{title} › {}", ctx.today_date)
            };
            ResolvedTarget {
                scenario: scenario.to_string(),
                kind: TargetKind::Inbox,
                target_rel_path: Some(INBOX_REL_PATH.to_string()),
                target_title: title.to_string(),
                section_hint: Some(ctx.today_date.to_string()),
                workspace_key: workspace_key.map(str::to_string),
                workspace_name: workspace_name.map(str::to_string),
                is_cross_workspace_fallback,
                recommended_mode: CaptureMode::List,
                chip_label,
            }
        }
        SelectedTarget::Doc(doc) => {
            let (ws_key, ws_name) = match doc.project.as_deref() {
                Some(proj) => match find_workspace(ctx.workspaces, proj) {
                    Some(w) => (Some(w.key.clone()), Some(w.name.clone())),
                    None => (Some(proj.to_string()), None),
                },
                None => (None, None),
            };

            if doc.rel_path == INBOX_REL_PATH {
                let title = if doc.title.is_empty() {
                    "收件箱"
                } else {
                    doc.title.as_str()
                };
                return ResolvedTarget {
                    scenario: scenario.to_string(),
                    kind: TargetKind::Inbox,
                    target_rel_path: Some(INBOX_REL_PATH.to_string()),
                    target_title: title.to_string(),
                    section_hint: Some(ctx.today_date.to_string()),
                    workspace_key: ws_key,
                    workspace_name: ws_name,
                    is_cross_workspace_fallback: false,
                    recommended_mode: CaptureMode::List,
                    chip_label: format!("{title} › {}", ctx.today_date),
                };
            }

            let recommended_mode = ws_key
                .as_deref()
                .and_then(|k| ctx.ui_state.workspace_modes.get(k).copied())
                .unwrap_or(CaptureMode::List);

            let (section_hint, chip_label) = compute_note_hints(
                &doc.title,
                ws_name.as_deref(),
                recommended_mode,
                doc.has_ideas_section,
                doc.has_todo_section,
                ctx.auto_create_missing_section,
            );

            ResolvedTarget {
                scenario: scenario.to_string(),
                kind: TargetKind::Note,
                target_rel_path: Some(doc.rel_path.clone()),
                target_title: doc.title.clone(),
                section_hint,
                workspace_key: ws_key,
                workspace_name: ws_name,
                is_cross_workspace_fallback: false,
                recommended_mode,
                chip_label,
            }
        }
    }
}

fn compute_note_hints(
    target_title: &str,
    workspace_name: Option<&str>,
    mode: CaptureMode,
    has_ideas_section: Option<bool>,
    has_todo_section: Option<bool>,
    auto_create_missing_section: bool,
) -> (Option<String>, String) {
    let section_hint = match mode {
        CaptureMode::List => {
            if has_ideas_section == Some(false) && !auto_create_missing_section {
                Some("将追加到文末".to_string())
            } else {
                Some("想法".to_string())
            }
        }
        CaptureMode::Todo => {
            if has_todo_section == Some(false) && !auto_create_missing_section {
                Some("将追加到文末".to_string())
            } else {
                Some("待办".to_string())
            }
        }
        CaptureMode::Prose => None,
    };

    let chip_label = match (section_hint.as_deref(), workspace_name) {
        (Some(sec), Some(ws)) => format!("{target_title} › {sec} · {ws}"),
        (Some(sec), None) => format!("{target_title} › {sec}"),
        (None, Some(ws)) => format!("{target_title} · {ws}"),
        (None, None) => target_title.to_string(),
    };

    (section_hint, chip_label)
}

fn format_capture_lines(
    content: &str,
    mode: CaptureMode,
    is_inbox: bool,
    hh_mm: &str,
) -> Result<Vec<String>, AppError> {
    let normalized = content.replace("\r\n", "\n");
    if normalized.trim().is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Capture content cannot be empty",
        ));
    }

    match mode {
        CaptureMode::Prose => {
            let trimmed = normalized.trim_matches('\n');
            Ok(trimmed.lines().map(str::to_string).collect())
        }
        CaptureMode::List => {
            let mut out = Vec::new();
            for line in normalized.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                out.push(format_list_line(line, is_inbox, hh_mm));
            }
            Ok(out)
        }
        CaptureMode::Todo => {
            let mut out = Vec::new();
            for line in normalized.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                out.push(format_todo_line(line));
            }
            Ok(out)
        }
    }
}

fn format_list_line(line: &str, is_inbox: bool, hh_mm: &str) -> String {
    let leading_len = line.len() - line.trim_start().len();
    let indent = &line[..leading_len];
    let trimmed = &line[leading_len..];

    if let Some((prefix, rest)) = split_list_prefix(trimmed) {
        if is_inbox && !starts_with_hh_mm(rest) {
            format!("{indent}{prefix}{hh_mm} {rest}")
        } else {
            line.to_string()
        }
    } else if is_inbox {
        format!("{indent}- {hh_mm} {trimmed}")
    } else {
        format!("{indent}- {trimmed}")
    }
}

fn format_todo_line(line: &str) -> String {
    let leading_len = line.len() - line.trim_start().len();
    let indent = &line[..leading_len];
    let trimmed = &line[leading_len..];

    if trimmed.starts_with("- [ ] ")
        || trimmed.starts_with("- [x] ")
        || trimmed.starts_with("- [X] ")
    {
        line.to_string()
    } else if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        format!("{indent}- [ ] {rest}")
    } else {
        format!("{indent}- [ ] {trimmed}")
    }
}

fn split_list_prefix(trimmed: &str) -> Option<(&str, &str)> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return Some((marker, rest));
        }
    }

    let bytes = trimmed.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_whitespace() {
        let prefix_end = i + 2;
        return Some((&trimmed[..prefix_end], &trimmed[prefix_end..]));
    }
    None
}

fn starts_with_hh_mm(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 6
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2] == b':'
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && b[5].is_ascii_whitespace()
}

fn detect_fence_open(trimmed: &str) -> Option<(char, usize)> {
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let count = trimmed.chars().take_while(|&c| c == first).count();
    if count >= 3 {
        Some((first, count))
    } else {
        None
    }
}

fn is_fence_close(trimmed: &str, fence_char: char, min_count: usize) -> bool {
    let count = trimmed.chars().take_while(|&c| c == fence_char).count();
    count >= min_count && trimmed[count * fence_char.len_utf8()..].trim().is_empty()
}

fn find_heading_line(lines: &[&str], target_heading: &str) -> Option<usize> {
    let mut fence: Option<(char, usize)> = None;
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some((fc, count)) = fence {
            if is_fence_close(trimmed, fc, count) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = detect_fence_open(trimmed) {
            fence = Some(open);
            continue;
        }
        if trimmed == target_heading {
            return Some(idx);
        }
    }
    None
}

fn find_next_section_boundary(lines: &[&str], start_idx: usize) -> usize {
    let mut fence: Option<(char, usize)> = None;
    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        let trimmed = line.trim();
        if let Some((fc, count)) = fence {
            if is_fence_close(trimmed, fc, count) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = detect_fence_open(trimmed) {
            fence = Some(open);
            continue;
        }
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") {
            return idx;
        }
    }
    lines.len()
}

fn trim_trailing_blank_lines(lines: &mut Vec<String>) {
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
}

fn append_at_eof(lines: &mut Vec<String>, formatted: Vec<String>, blank_line_before: bool) {
    trim_trailing_blank_lines(lines);
    if blank_line_before && !lines.is_empty() {
        lines.push(String::new());
    }
    lines.extend(formatted);
}

/// Extracts a 1-based closed line range `[from_line, to_line]` from `body`, returning `(extracted, remaining)`.
pub fn extract_body_lines(
    body: &str,
    from_line: u32,
    to_line: u32,
) -> Result<(String, String), AppError> {
    let lines: Vec<&str> = body.lines().collect();
    let total_lines = lines.len() as u32;
    if from_line < 1 || from_line > to_line || to_line > total_lines {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Invalid line range for extraction",
        ));
    }

    let start = (from_line - 1) as usize;
    let end = to_line as usize;
    let extracted_raw = lines[start..end].join("\n");
    if extracted_raw.trim().is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Extracted content cannot be empty",
        ));
    }
    let extracted = format!("{}\n", extracted_raw.trim_end_matches('\n'));

    let mut before: Vec<&str> = lines[..start].to_vec();
    let mut after: Vec<&str> = lines[end..].to_vec();

    while before.last().is_some_and(|l| l.trim().is_empty()) {
        before.pop();
    }
    while after.first().is_some_and(|l| l.trim().is_empty()) {
        after.remove(0);
    }

    let mut remaining_lines: Vec<&str> = Vec::with_capacity(before.len() + after.len() + 1);
    remaining_lines.extend(before);
    if !remaining_lines.is_empty() && !after.is_empty() {
        remaining_lines.push("");
    }
    remaining_lines.extend(after);

    let remaining = if remaining_lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", remaining_lines.join("\n"))
    };

    Ok((extracted, remaining))
}
