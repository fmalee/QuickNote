//! CJK + ASCII mixed tokenizer for `notes_fts` writes and query rewriting (`C5.3`).
//!
//! Pure in-memory functions only (`C1.2`).

/// Connector characters that join ASCII sub-tokens into a compound token (`C5.3`).
fn is_connector(c: char) -> bool {
    matches!(c, '.' | '-' | '/' | ':' | '~')
}

/// Word characters for the "ASCII-like" run: `[a-z0-9_]` plus non-CJK Unicode alphanumerics.
fn is_word_char(c: char) -> bool {
    c == '_' || (c.is_alphanumeric() && !is_cjk(c))
}

/// Returns true for CJK ideographs and East Asian syllabaries that have no word separators.
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF   // CJK Unified Ideographs Extension A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
        | 0x20000..=0x2FA1F // Extensions B–F + Compatibility Supplement
        | 0x3040..=0x309F // Hiragana
        | 0x30A0..=0x30FF // Katakana
        | 0xAC00..=0xD7AF // Hangul syllables
    )
}

fn push_token(out: &mut Vec<String>, token: &str) {
    if !token.is_empty() {
        out.push(token.to_string());
    }
}

/// Emits tokens for one ASCII-like run that may contain connectors.
/// Keeps the full compound token plus each split sub-token; never emits character bigrams.
fn emit_word_run(run: &str, out: &mut Vec<String>) {
    let trimmed = run.trim_matches(is_connector);
    if trimmed.is_empty() {
        return;
    }
    let parts: Vec<&str> = trimmed
        .split(is_connector)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() <= 1 {
        push_token(out, trimmed);
        return;
    }
    push_token(out, trimmed);
    for part in parts {
        push_token(out, part);
    }
}

/// Emits CJK tokens: single char when the run length is 1, otherwise all sliding 2-grams.
fn emit_cjk_run(run: &[char], out: &mut Vec<String>) {
    match run.len() {
        0 => {}
        1 => out.push(run[0].to_string()),
        _ => {
            for pair in run.windows(2) {
                let mut s = String::with_capacity(8);
                s.push(pair[0]);
                s.push(pair[1]);
                out.push(s);
            }
        }
    }
}

/// Tokenizes `input` into a vector of lowercase tokens (`C5.3`).
pub fn tokenize_mixed_vec(input: &str) -> Vec<String> {
    let lower = input.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut cjk: Vec<char> = Vec::new();

    for c in lower.chars() {
        if is_cjk(c) {
            if !word.is_empty() {
                emit_word_run(&word, &mut out);
                word.clear();
            }
            cjk.push(c);
        } else if is_word_char(c) || (is_connector(c) && !word.is_empty()) {
            if !cjk.is_empty() {
                emit_cjk_run(&cjk, &mut out);
                cjk.clear();
            }
            word.push(c);
        } else {
            if !word.is_empty() {
                emit_word_run(&word, &mut out);
                word.clear();
            }
            if !cjk.is_empty() {
                emit_cjk_run(&cjk, &mut out);
                cjk.clear();
            }
        }
    }
    if !word.is_empty() {
        emit_word_run(&word, &mut out);
    }
    if !cjk.is_empty() {
        emit_cjk_run(&cjk, &mut out);
    }
    out
}

/// Tokenizes `input` into a single space-separated token string for FTS5 (`C5.3`).
pub fn tokenize_mixed(input: &str) -> String {
    tokenize_mixed_vec(input).join(" ")
}

/// Builds an FTS5 `MATCH` expression from a raw user query (`C5.3`).
///
/// Every token becomes a quoted phrase (so connectors like `nginx.conf` match adjacent parts),
/// tokens are AND-combined, and the last token gets a prefix `*` to support incremental typing.
/// Returns `None` when the query has no searchable tokens.
pub fn build_fts_query(query: &str) -> Option<String> {
    let mut tokens = tokenize_mixed_vec(query);
    tokens.dedup();
    if tokens.is_empty() {
        return None;
    }
    let last = tokens.len() - 1;
    let parts: Vec<String> = tokens
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let escaped = t.replace('"', "\"\"");
            if i == last {
                format!("\"{escaped}\"*")
            } else {
                format!("\"{escaped}\"")
            }
        })
        .collect();
    Some(parts.join(" "))
}

/// Expands hierarchical tags into token text: `dev/quicknote` → `dev/quicknote dev quicknote`.
pub fn tokenize_tags(tags: &[String]) -> String {
    let joined = tags.join(" ");
    tokenize_mixed(&joined)
}
