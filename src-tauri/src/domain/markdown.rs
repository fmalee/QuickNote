use std::collections::HashMap;

use crate::domain::tokenizer::is_cjk;

/// Parsed ATX heading entry for outline navigation (`C8.4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineHeading {
    pub level: u8,
    pub text: String,
    pub slug: String,
    pub line: u32,
}

/// Strips inline Markdown formatting (`**`, `__`, `*`, `_`, `` ` ``, `[text](url)` -> `text`).
pub fn strip_inline_markdown(raw: &str) -> String {
    let links_stripped = replace_markdown_links(raw);
    let no_markers = links_stripped
        .replace("**", "")
        .replace("__", "")
        .replace(['*', '`'], "");

    strip_emphasis_underscores(&no_markers)
}

fn replace_markdown_links(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(open_bracket) = rest.find('[') {
        out.push_str(&rest[..open_bracket]);
        let after_bracket = &rest[open_bracket + 1..];

        if let Some(close_bracket) = after_bracket.find(']') {
            let link_text = &after_bracket[..close_bracket];
            let after_close = &after_bracket[close_bracket + 1..];
            if let Some(after_paren_open) = after_close.strip_prefix('(') {
                if let Some(close_paren) = after_paren_open.find(')') {
                    out.push_str(link_text);
                    rest = &after_paren_open[close_paren + 1..];
                    continue;
                }
            }
        }

        out.push('[');
        rest = after_bracket;
    }

    out.push_str(rest);
    out
}

fn strip_emphasis_underscores(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());

    for (idx, &ch) in chars.iter().enumerate() {
        if ch == '_' {
            let prev_is_word = idx > 0 && chars[idx - 1].is_alphanumeric();
            let next_is_word = idx + 1 < chars.len() && chars[idx + 1].is_alphanumeric();
            if prev_is_word && next_is_word {
                out.push(ch);
            }
        } else {
            out.push(ch);
        }
    }

    out
}

/// Converts a raw Markdown heading into a deduplicated anchor slug (`C8.4`).
pub fn heading_to_slug(raw_heading: &str, seen_counts: &mut HashMap<String, usize>) -> String {
    let cleaned = strip_inline_markdown(raw_heading);
    let lowered = cleaned.trim().to_lowercase();

    let mut raw_slug = String::with_capacity(lowered.len());
    let mut prev_dash = false;

    for ch in lowered.chars() {
        if ch.is_whitespace() || ch == '-' {
            if !prev_dash && !raw_slug.is_empty() {
                raw_slug.push('-');
                prev_dash = true;
            }
        } else if is_cjk(ch) || ch.is_ascii_alphanumeric() || ch == '_' {
            raw_slug.push(ch);
            prev_dash = false;
        }
    }

    let base = raw_slug.trim_matches('-').to_string();
    match seen_counts.get_mut(&base) {
        Some(count) => {
            *count += 1;
            format!("{base}-{}", *count)
        }
        None => {
            seen_counts.insert(base.clone(), 0);
            base
        }
    }
}

/// Extracts ATX headings (`#`..`######`) while skipping fenced code blocks (`` ``` `` / `~~~`) (`C8.4`).
pub fn extract_headings(body: &str) -> Vec<OutlineHeading> {
    let mut headings = Vec::new();
    let mut seen_counts = HashMap::new();
    let mut fence: Option<(char, usize)> = None;

    for (idx, line) in body.lines().enumerate() {
        let trimmed = line.trim_start();
        let leading_spaces = line.len().saturating_sub(trimmed.len());

        if leading_spaces <= 3 {
            if let Some((fence_char, fence_len)) = fence {
                let run_len = trimmed.chars().take_while(|&c| c == fence_char).count();
                if run_len >= fence_len && trimmed[run_len..].trim().is_empty() {
                    fence = None;
                }
                continue;
            } else if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                let fence_char = trimmed.chars().next().unwrap_or('`');
                let run_len = trimmed.chars().take_while(|&c| c == fence_char).count();
                if run_len >= 3 {
                    fence = Some((fence_char, run_len));
                    continue;
                }
            }
        } else if fence.is_some() {
            continue;
        }

        if let Some((level, raw_title)) = parse_atx_heading(trimmed) {
            let text = strip_inline_markdown(raw_title).trim().to_string();
            let slug = heading_to_slug(raw_title, &mut seen_counts);
            headings.push(OutlineHeading {
                level,
                text,
                slug,
                line: (idx + 1) as u32,
            });
        }
    }

    headings
}

fn parse_atx_heading(trimmed: &str) -> Option<(u8, &str)> {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }

    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }

    let mut content = rest.trim();
    // Strip optional closing ATX hashes preceded by whitespace.
    let without_trailing_hashes = content.trim_end_matches('#');
    if without_trailing_hashes.len() < content.len()
        && (without_trailing_hashes.is_empty()
            || without_trailing_hashes.ends_with(' ')
            || without_trailing_hashes.ends_with('\t'))
    {
        content = without_trailing_hashes.trim_end();
    }

    if content.is_empty() {
        return None;
    }

    Some((hashes as u8, content))
}
