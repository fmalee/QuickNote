//! Command-book Markdown parser and ranking engine (`v4` §4.8, `C3.4`, `C4.2`).
//!
//! Pure in-memory functions only (`C1.2`).

use serde::{Deserialize, Serialize};
use specta::Type;

/// Parsed command entry from a `command-book` note (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandEntry {
    pub id: String,
    pub note_rel_path: String,
    pub section: String,
    pub title: String,
    pub description: String,
    pub code: String,
    pub language: Option<String>,
    pub keywords: Vec<String>,
    pub is_danger: bool,
    pub line: u32,
}

/// Arguments for `commands_search` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandSearchArgs {
    pub query: String,
}

struct ActiveFence {
    fence_char: char,
    fence_len: usize,
    section: String,
    title: String,
    description: String,
    language: Option<String>,
    line: u32,
    code_lines: Vec<String>,
}

/// Parses a `command-book` Markdown body into structured command entries (`v4` §4.8).
///
/// `line` is the 1-based line number within `NoteDoc.body` (`D-0006`).
pub fn parse_command_book(note_rel_path: &str, body: &str) -> Vec<CommandEntry> {
    let mut entries: Vec<CommandEntry> = Vec::new();
    let mut section = "默认".to_string();
    let mut pending_h3: Option<(String, u32)> = None;
    let mut pending_desc: Vec<String> = Vec::new();
    let mut last_para: Option<(String, u32)> = None;
    let mut in_para = false;
    let mut last_entry_idx: Option<usize> = None;
    let mut active_fence: Option<ActiveFence> = None;

    for (idx, raw_line) in body.lines().enumerate() {
        let line_no = idx as u32 + 1;
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim_start();
        let leading_spaces = line.len().saturating_sub(trimmed.len());

        if let Some(mut fence) = active_fence.take() {
            let is_closing = if leading_spaces <= 3 {
                let run_len = trimmed
                    .chars()
                    .take_while(|&c| c == fence.fence_char)
                    .count();
                run_len >= fence.fence_len && trimmed[run_len..].trim().is_empty()
            } else {
                false
            };

            if is_closing {
                let entry_idx = entries.len();
                entries.push(CommandEntry {
                    id: format!("{note_rel_path}:{}:{entry_idx}", fence.line),
                    note_rel_path: note_rel_path.to_string(),
                    section: fence.section,
                    title: fence.title,
                    description: fence.description,
                    code: fence.code_lines.join("\n"),
                    language: fence.language,
                    keywords: Vec::new(),
                    is_danger: false,
                    line: fence.line,
                });
                last_entry_idx = Some(entry_idx);
                pending_h3 = None;
                pending_desc.clear();
                last_para = None;
                in_para = false;
            } else {
                fence.code_lines.push(line.to_string());
                active_fence = Some(fence);
            }
            continue;
        }

        if leading_spaces <= 3 && (trimmed.starts_with("```") || trimmed.starts_with("~~~")) {
            let fence_char = if trimmed.starts_with('`') { '`' } else { '~' };
            let fence_len = trimmed.chars().take_while(|&c| c == fence_char).count();
            let info = trimmed[fence_len..].trim();
            let valid_fence = fence_len >= 3 && (fence_char != '`' || !info.contains('`'));
            if valid_fence {
                let language = info
                    .split_whitespace()
                    .next()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);

                let (title, description, entry_line) =
                    if let Some((h3_title, h3_line)) = pending_h3.take() {
                        (h3_title, pending_desc.join("\n"), h3_line)
                    } else if let Some((para_text, para_line)) = last_para.take() {
                        (para_text, String::new(), para_line)
                    } else {
                        let fallback_title = language.clone().unwrap_or_else(|| section.clone());
                        (fallback_title, String::new(), line_no)
                    };

                pending_desc.clear();
                in_para = false;
                last_entry_idx = None;
                active_fence = Some(ActiveFence {
                    fence_char,
                    fence_len,
                    section: section.clone(),
                    title,
                    description,
                    language,
                    line: entry_line,
                    code_lines: Vec::new(),
                });
                continue;
            }
        }

        if let Some((comments, remainder)) = extract_html_comments(trimmed) {
            if let Some(entry_idx) = last_entry_idx {
                if let Some(entry) = entries.get_mut(entry_idx) {
                    for comment in comments {
                        apply_comment_metadata(&comment, entry);
                    }
                }
            }
            if remainder.trim().is_empty() {
                in_para = false;
                continue;
            }
        }

        if let Some((level, heading_text)) = parse_heading(trimmed) {
            in_para = false;
            last_entry_idx = None;
            match level {
                1 => {
                    if section == "默认" {
                        section = heading_text;
                    }
                    pending_h3 = None;
                    pending_desc.clear();
                    last_para = None;
                }
                2 => {
                    section = heading_text;
                    pending_h3 = None;
                    pending_desc.clear();
                    last_para = None;
                }
                3 => {
                    pending_h3 = Some((heading_text, line_no));
                    pending_desc.clear();
                    last_para = None;
                }
                _ => {}
            }
            continue;
        }

        if trimmed.is_empty() {
            in_para = false;
            continue;
        }

        last_entry_idx = None;
        if pending_h3.is_some() {
            pending_desc.push(trimmed.to_string());
        } else if in_para {
            if let Some((ref mut text, _)) = last_para {
                text.push(' ');
                text.push_str(trimmed);
            }
        } else {
            last_para = Some((trimmed.to_string(), line_no));
            in_para = true;
        }
    }

    entries
}

/// Filters and ranks command entries by whitespace-separated query tokens (`AND` semantics).
pub fn filter_and_rank_commands(entries: &[CommandEntry], query: &str) -> Vec<CommandEntry> {
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|s| !s.is_empty())
        .collect();

    if tokens.is_empty() {
        let mut out = entries.to_vec();
        out.sort_by(|a, b| {
            a.note_rel_path
                .cmp(&b.note_rel_path)
                .then_with(|| a.line.cmp(&b.line))
        });
        return out;
    }

    let mut scored: Vec<(i32, CommandEntry)> = Vec::new();

    for entry in entries {
        let title_lower = entry.title.to_lowercase();
        let keywords_lower: Vec<String> = entry.keywords.iter().map(|k| k.to_lowercase()).collect();
        let section_lower = entry.section.to_lowercase();
        let desc_lower = entry.description.to_lowercase();
        let code_lower = entry.code.to_lowercase();
        let lang_lower = entry
            .language
            .as_deref()
            .map(str::to_lowercase)
            .unwrap_or_default();

        let mut total_score = 0;
        let mut matched_all = true;

        for token in &tokens {
            let mut token_score = 0;
            if title_lower.contains(token) {
                token_score += 40;
            }
            if keywords_lower.iter().any(|kw| kw.contains(token)) {
                token_score += 35;
            }
            if section_lower.contains(token) {
                token_score += 25;
            }
            if desc_lower.contains(token) {
                token_score += 15;
            }
            if code_lower.contains(token) {
                token_score += 10;
            }
            if !lang_lower.is_empty() && lang_lower.contains(token) {
                token_score += 5;
            }

            if token_score == 0 {
                matched_all = false;
                break;
            }
            total_score += token_score;
        }

        if matched_all && total_score > 0 {
            scored.push((total_score, entry.clone()));
        }
    }

    scored.sort_by(|(score_a, a), (score_b, b)| {
        score_b
            .cmp(score_a)
            .then_with(|| a.note_rel_path.cmp(&b.note_rel_path))
            .then_with(|| a.line.cmp(&b.line))
    });

    scored.into_iter().map(|(_, entry)| entry).collect()
}

fn parse_heading(trimmed: &str) -> Option<(u8, String)> {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let mut content = rest.trim();
    let without_trailing = content.trim_end_matches('#');
    if without_trailing.len() < content.len()
        && (without_trailing.is_empty()
            || without_trailing.ends_with(' ')
            || without_trailing.ends_with('\t'))
    {
        content = without_trailing.trim_end();
    }
    if content.is_empty() {
        None
    } else {
        Some((hashes as u8, content.to_string()))
    }
}

fn extract_html_comments(input: &str) -> Option<(Vec<String>, String)> {
    if !input.contains("<!--") {
        return None;
    }

    let mut comments = Vec::new();
    let mut remainder = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("<!--") {
        remainder.push_str(&rest[..start]);
        let after_start = &rest[start + 4..];
        if let Some(end) = after_start.find("-->") {
            comments.push(after_start[..end].trim().to_string());
            rest = &after_start[end + 3..];
        } else {
            remainder.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }
    remainder.push_str(rest);

    if comments.is_empty() {
        None
    } else {
        Some((comments, remainder))
    }
}

fn apply_comment_metadata(comment: &str, entry: &mut CommandEntry) {
    let trimmed = comment.trim();
    let lower = trimmed.to_ascii_lowercase();

    if lower == "danger" || lower.starts_with("danger:") {
        entry.is_danger = true;
        return;
    }

    if let Some(rest) = lower.strip_prefix("keywords:") {
        let raw_keywords = &trimmed[trimmed.len() - rest.len()..];
        for token in
            raw_keywords.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '，' | '；'))
        {
            let kw = token.trim();
            if !kw.is_empty() && !entry.keywords.iter().any(|existing| existing == kw) {
                entry.keywords.push(kw.to_string());
            }
        }
    }
}
