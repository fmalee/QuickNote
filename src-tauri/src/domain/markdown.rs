use std::collections::{BTreeSet, HashMap};

use crate::domain::error::{AppError, ErrorCode};
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

const ALLOWED_IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp", "ico",
];

/// Normalizes and validates an image file extension (`v4` §5.6.3).
pub fn normalize_image_ext(raw_ext: &str) -> Result<String, AppError> {
    let cleaned = raw_ext.trim().trim_start_matches('.').to_ascii_lowercase();
    if ALLOWED_IMAGE_EXTS.contains(&cleaned.as_str()) {
        Ok(cleaned)
    } else {
        Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Unsupported image extension",
            cleaned,
        ))
    }
}

fn base64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Strictly decodes a base64 image payload (with optional `data:image/...;base64,` prefix) per RFC 4648.
pub fn decode_base64_image(raw: &str, max_bytes: usize) -> Result<Vec<u8>, AppError> {
    let trimmed = raw.trim();
    let payload = if let Some(rest) = trimmed.strip_prefix("data:") {
        let Some(idx) = rest.find(";base64,") else {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "Invalid image data URI",
            ));
        };
        &rest[idx + ";base64,".len()..]
    } else {
        trimmed
    };

    let clean: Vec<u8> = payload
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();

    if clean.is_empty() || !clean.len().is_multiple_of(4) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Invalid base64 image payload length",
        ));
    }

    let chunks = clean.len() / 4;
    let estimated_max = chunks.saturating_mul(3);
    if estimated_max > max_bytes.saturating_add(2) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Image payload exceeds maximum allowed size",
        ));
    }

    let mut out = Vec::with_capacity(estimated_max);
    let (chunks_slice, _) = clean.as_chunks::<4>();
    for (chunk_idx, chunk) in chunks_slice.iter().enumerate() {
        let is_last = chunk_idx + 1 == chunks;
        let a = base64_val(chunk[0])
            .ok_or_else(|| AppError::new(ErrorCode::ValidationError, "Invalid base64 character"))?;
        let b = base64_val(chunk[1])
            .ok_or_else(|| AppError::new(ErrorCode::ValidationError, "Invalid base64 character"))?;

        match (chunk[2], chunk[3]) {
            (b'=', b'=') => {
                if !is_last {
                    return Err(AppError::new(
                        ErrorCode::ValidationError,
                        "Invalid base64 padding position",
                    ));
                }
                out.push((a << 2) | (b >> 4));
            }
            (c_byte, b'=') => {
                if !is_last {
                    return Err(AppError::new(
                        ErrorCode::ValidationError,
                        "Invalid base64 padding position",
                    ));
                }
                let c = base64_val(c_byte).ok_or_else(|| {
                    AppError::new(ErrorCode::ValidationError, "Invalid base64 character")
                })?;
                out.push((a << 2) | (b >> 4));
                out.push((b << 4) | (c >> 2));
            }
            (c_byte, d_byte) => {
                let c = base64_val(c_byte).ok_or_else(|| {
                    AppError::new(ErrorCode::ValidationError, "Invalid base64 character")
                })?;
                let d = base64_val(d_byte).ok_or_else(|| {
                    AppError::new(ErrorCode::ValidationError, "Invalid base64 character")
                })?;
                out.push((a << 2) | (b >> 4));
                out.push((b << 4) | (c >> 2));
                out.push((c << 6) | d);
            }
        }
    }

    if out.is_empty() || out.len() > max_bytes {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Image payload exceeds maximum allowed size",
        ));
    }

    Ok(out)
}

/// Extracts normalized library-relative `.md`/`.markdown` links from Markdown body (`C8.6`).
pub fn extract_relative_note_links(source_rel_path: &str, body: &str) -> Vec<String> {
    let mut links = BTreeSet::new();
    let mut fence: Option<(char, usize)> = None;

    for raw_line in body.lines() {
        let line = raw_line.trim_end_matches('\r');
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

        if leading_spaces <= 3 {
            if let Some(target) = parse_reference_link_definition(trimmed) {
                if let Some(normalized) = normalize_relative_md_target(source_rel_path, target) {
                    links.insert(normalized);
                }
                continue;
            }
        }

        let clean_line = strip_inline_code_spans(line);
        let mut rest = clean_line.as_str();
        while let Some(open_bracket) = rest.find('[') {
            let is_image = open_bracket > 0 && rest.as_bytes()[open_bracket - 1] == b'!';
            let after_bracket = &rest[open_bracket + 1..];
            if let Some(close_bracket) = after_bracket.find(']') {
                let after_close = &after_bracket[close_bracket + 1..];
                if let Some(after_paren) = after_close.strip_prefix('(') {
                    if let Some(close_paren) = after_paren.find(')') {
                        if !is_image {
                            let raw_inner = &after_paren[..close_paren];
                            if let Some(target) = parse_link_destination(raw_inner) {
                                if let Some(normalized) =
                                    normalize_relative_md_target(source_rel_path, target)
                                {
                                    links.insert(normalized);
                                }
                            }
                        }
                        rest = &after_paren[close_paren + 1..];
                        continue;
                    }
                }
            }
            rest = after_bracket;
        }
    }

    links.into_iter().collect()
}

fn strip_inline_code_spans(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'`' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'`' {
                i += 1;
            }
            let tick_len = i - start;

            // Search for matching closing backtick run of exact length.
            let mut j = i;
            let mut found_close = None;
            while j < bytes.len() {
                if bytes[j] == b'`' {
                    let close_start = j;
                    while j < bytes.len() && bytes[j] == b'`' {
                        j += 1;
                    }
                    if j - close_start == tick_len {
                        found_close = Some(j);
                        break;
                    }
                } else {
                    j += 1;
                }
            }

            if let Some(end_idx) = found_close {
                i = end_idx;
                continue;
            } else {
                out.push_str(&line[start..i]);
                continue;
            }
        }

        if let Some(ch) = line[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    out
}

fn parse_reference_link_definition(trimmed: &str) -> Option<&str> {
    if !trimmed.starts_with('[') {
        return None;
    }
    let close_bracket = trimmed[1..].find("]:")?;
    let after_colon = trimmed[1 + close_bracket + 2..].trim_start();
    if after_colon.is_empty() {
        return None;
    }
    parse_link_destination(after_colon)
}

fn parse_link_destination(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(after_lt) = trimmed.strip_prefix('<') {
        let end_gt = after_lt.find('>')?;
        let dest = after_lt[..end_gt].trim();
        if dest.is_empty() {
            None
        } else {
            Some(dest)
        }
    } else {
        let dest = trimmed.split_whitespace().next()?;
        if dest.is_empty() {
            None
        } else {
            Some(dest)
        }
    }
}

fn normalize_relative_md_target(source_rel_path: &str, raw_target: &str) -> Option<String> {
    let target = raw_target.trim();
    if target.is_empty() || target.starts_with('#') || target.starts_with('/') {
        return None;
    }

    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("file://")
    {
        return None;
    }

    let cut_idx = target.find(['#', '?']).unwrap_or(target.len());
    let path_only = target[..cut_idx].trim();
    if path_only.is_empty() {
        return None;
    }

    // Reject other explicit URI schemes (e.g. `obsidian://`, `data:`).
    if let Some(colon_idx) = path_only.find(':') {
        if !path_only[..colon_idx].contains('/') {
            return None;
        }
    }

    let path_lower = path_only.to_ascii_lowercase();
    if !path_lower.ends_with(".md") && !path_lower.ends_with(".markdown") {
        return None;
    }

    let source_clean = source_rel_path.trim().replace('\\', "/");
    let mut stack: Vec<&str> = match source_clean.rsplit_once('/') {
        Some((dir, _)) if !dir.is_empty() => dir.split('/').filter(|s| !s.is_empty()).collect(),
        _ => Vec::new(),
    };

    let target_clean = path_only.replace('\\', "/");
    for seg in target_clean.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            other => stack.push(other),
        }
    }

    let normalized = stack.join("/");
    if normalized.is_empty() || normalized == source_clean {
        return None;
    }

    Some(normalized)
}
