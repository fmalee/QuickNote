//! YAML frontmatter split, tolerant parse and minimal patcher (`D-0006`, `D-0007`, `C6.4`).
//!
//! Pure in-memory functions only (`C1.2`): no filesystem, database or runtime access.

use serde_yaml::{Mapping, Value};

/// Result of splitting a Markdown document into its frontmatter block and body (`D-0006`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterSplit<'a> {
    /// The full raw frontmatter block including both `---` delimiter lines and the trailing newline.
    pub block: Option<&'a str>,
    /// The YAML text between the delimiters (without delimiter lines).
    pub yaml: Option<&'a str>,
    /// The Markdown body after the frontmatter block (lossless: `block + body == raw`).
    pub body: &'a str,
    /// True when the document opens with `---` but never closes it.
    pub unterminated: bool,
}

fn line_content(line: &str) -> &str {
    line.trim_end_matches('\n')
        .trim_end_matches('\r')
        .trim_end_matches([' ', '\t'])
}

/// Splits `raw` into `block` / `yaml` / `body`. Unterminated frontmatter is treated as body.
pub fn split_frontmatter(raw: &str) -> FrontmatterSplit<'_> {
    let start = if raw.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let rest = &raw[start..];
    let no_fm = FrontmatterSplit {
        block: None,
        yaml: None,
        body: raw,
        unterminated: false,
    };

    let Some(first_nl) = rest.find('\n') else {
        return no_fm;
    };
    if line_content(&rest[..=first_nl]) != "---" {
        return no_fm;
    }

    let yaml_start = start + first_nl + 1;
    let mut pos = yaml_start;
    for line in raw[yaml_start..].split_inclusive('\n') {
        let content = line_content(line);
        if content == "---" || content == "..." {
            let block_end = pos + line.len();
            return FrontmatterSplit {
                block: Some(&raw[..block_end]),
                yaml: Some(&raw[yaml_start..pos]),
                body: &raw[block_end..],
                unterminated: false,
            };
        }
        pos += line.len();
    }

    FrontmatterSplit {
        block: None,
        yaml: None,
        body: raw,
        unterminated: true,
    }
}

/// Parses YAML frontmatter text into a mapping. Empty / comment-only YAML yields an empty mapping.
/// Errors carry only location metadata (never content) and use file line numbers
/// (the opening `---` is line 1).
pub fn parse_yaml_mapping(yaml: &str) -> Result<Mapping, String> {
    match serde_yaml::from_str::<Value>(yaml) {
        Ok(Value::Mapping(m)) => Ok(m),
        Ok(Value::Null) => Ok(Mapping::new()),
        Ok(_) => Err("frontmatter 不是键值映射".to_string()),
        Err(err) => match err.location() {
            Some(loc) => Err(format!(
                "YAML 解析失败（第 {} 行，第 {} 列）",
                loc.line() + 1,
                loc.column()
            )),
            None => Err("YAML 解析失败".to_string()),
        },
    }
}

/// A value to write for one frontmatter key (`C6.4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YamlPatchValue {
    /// String scalar; emitted plain when unambiguous, otherwise double-quoted.
    /// ISO date/time strings are always double-quoted.
    Str(String),
    /// Boolean scalar.
    Bool(bool),
    /// Block list (`- item` per line); empty lists emit `[]`.
    List(Vec<String>),
}

/// One minimal patch operation; `value = None` removes the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlPatchOp {
    pub key: String,
    pub value: Option<YamlPatchValue>,
}

impl YamlPatchOp {
    /// Creates a set operation.
    pub fn set(key: &str, value: YamlPatchValue) -> Self {
        Self {
            key: key.to_string(),
            value: Some(value),
        }
    }

    /// Creates a remove operation.
    pub fn remove(key: &str) -> Self {
        Self {
            key: key.to_string(),
            value: None,
        }
    }
}

/// Minimal patch failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YamlPatchError {
    /// Original YAML is broken and cannot be patched in place (caller must back up first).
    Unpatchable(String),
    /// The patched output failed self-verification (patch is discarded).
    VerifyFailed(String),
}

fn looks_like_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
}

fn is_plain_safe(s: &str) -> bool {
    if s.is_empty() || s.trim() != s {
        return false;
    }
    let Some(first) = s.chars().next() else {
        return false;
    };
    if "-?:,[]{}#&*!|>'\"%@`~".contains(first) {
        return false;
    }
    if s.contains(": ") || s.contains(" #") || s.ends_with(':') {
        return false;
    }
    if s.chars().any(|c| c.is_control()) {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "true" | "false" | "yes" | "no" | "on" | "off" | "null" | "y" | "n"
    ) {
        return false;
    }
    if s.parse::<f64>().is_ok() || s.parse::<i64>().is_ok() || looks_like_iso_date(s) {
        return false;
    }
    true
}

/// Renders a YAML scalar string following `C6.4` (ISO dates double-quoted).
pub fn render_yaml_string(s: &str) -> String {
    if is_plain_safe(s) {
        s.to_string()
    } else {
        double_quote(s)
    }
}

fn double_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn key_of_line(line: &str) -> Option<String> {
    let first = line.chars().next()?;
    if first.is_whitespace() || first == '#' || first == '-' {
        return None;
    }
    let content = line_content(line);
    let (key, after) = if let Some(stripped) = content.strip_prefix('"') {
        let end = stripped.find('"')?;
        (stripped[..end].to_string(), &stripped[end + 1..])
    } else if let Some(stripped) = content.strip_prefix('\'') {
        let end = stripped.find('\'')?;
        (stripped[..end].to_string(), &stripped[end + 1..])
    } else {
        let colon = content.find(':')?;
        (content[..colon].trim_end().to_string(), &content[colon..])
    };
    let after = after.trim_start();
    let rest = after.strip_prefix(':')?;
    if rest.is_empty() || rest.starts_with([' ', '\t']) {
        Some(key)
    } else {
        None
    }
}

/// Extracts a trailing ` # comment` (with its leading whitespace) from a key line.
fn trailing_comment(line: &str) -> Option<String> {
    let content = line_content(line);
    let colon = content.find(':')?;
    let value_part = &content[colon + 1..];
    let mut in_single = false;
    let mut in_double = false;
    let mut prev_ws = true;
    for (idx, c) in value_part.char_indices() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double && prev_ws => {
                let before = &value_part[..idx];
                let ws_start = before.trim_end().len();
                return Some(value_part[ws_start..].to_string());
            }
            _ => {}
        }
        prev_ws = c == ' ' || c == '\t';
    }
    None
}

fn is_continuation(line: &str) -> bool {
    line.starts_with([' ', '\t']) || line.starts_with("- ") || line_content(line) == "-"
}

fn is_blank(line: &str) -> bool {
    line_content(line).is_empty()
}

fn span_end(lines: &[String], start: usize) -> usize {
    let mut j = start + 1;
    while j < lines.len() {
        if is_continuation(&lines[j]) && !is_blank(&lines[j]) {
            j += 1;
            continue;
        }
        if is_blank(&lines[j]) {
            let mut k = j + 1;
            while k < lines.len() && is_blank(&lines[k]) {
                k += 1;
            }
            if k < lines.len() && is_continuation(&lines[k]) {
                j = k + 1;
                continue;
            }
        }
        break;
    }
    j
}

fn render_entry(
    key: &str,
    value: &YamlPatchValue,
    comment: Option<&str>,
    list_indent: &str,
    nl: &str,
) -> Vec<String> {
    let comment = comment.unwrap_or("");
    match value {
        YamlPatchValue::Str(s) => vec![format!("{key}: {}{comment}{nl}", render_yaml_string(s))],
        YamlPatchValue::Bool(b) => vec![format!("{key}: {b}{comment}{nl}")],
        YamlPatchValue::List(items) if items.is_empty() => {
            vec![format!("{key}: []{comment}{nl}")]
        }
        YamlPatchValue::List(items) => {
            let mut out = vec![format!("{key}:{comment}{nl}")];
            for item in items {
                out.push(format!("{list_indent}- {}{nl}", render_yaml_string(item)));
            }
            out
        }
    }
}

fn value_matches(actual: Option<&Value>, expected: &Option<YamlPatchValue>) -> bool {
    match (actual, expected) {
        (None, None) => true,
        (Some(Value::String(a)), Some(YamlPatchValue::Str(e))) => a == e,
        (Some(Value::Bool(a)), Some(YamlPatchValue::Bool(e))) => a == e,
        (Some(Value::Sequence(seq)), Some(YamlPatchValue::List(e))) => {
            seq.len() == e.len()
                && seq
                    .iter()
                    .zip(e.iter())
                    .all(|(a, b)| matches!(a, Value::String(s) if s == b))
        }
        _ => false,
    }
}

/// Applies minimal key-level patches to `yaml`, preserving comments, unknown keys and order (`C6.4`).
///
/// Only the line span owned by each target key is replaced; new keys are appended at the end,
/// except `id`, which is inserted as the first line. The result is re-parsed and verified.
pub fn apply_yaml_patch(yaml: &str, ops: &[YamlPatchOp]) -> Result<String, YamlPatchError> {
    parse_yaml_mapping(yaml).map_err(YamlPatchError::Unpatchable)?;

    let nl = if yaml.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = yaml.split_inclusive('\n').map(str::to_string).collect();

    for op in ops {
        let found = lines
            .iter()
            .position(|l| key_of_line(l).as_deref() == Some(op.key.as_str()));
        match (found, &op.value) {
            (Some(i), value) => {
                let end = span_end(&lines, i);
                match value {
                    None => {
                        lines.drain(i..end);
                    }
                    Some(v) => {
                        let comment = trailing_comment(&lines[i]);
                        let list_indent = if lines[i + 1..end].iter().any(|l| l.starts_with('-')) {
                            ""
                        } else {
                            "  "
                        };
                        let rendered =
                            render_entry(&op.key, v, comment.as_deref(), list_indent, nl);
                        lines.splice(i..end, rendered);
                    }
                }
            }
            (None, None) => {}
            (None, Some(v)) => {
                let rendered = render_entry(&op.key, v, None, "  ", nl);
                if op.key == "id" {
                    lines.splice(0..0, rendered);
                } else {
                    if let Some(last) = lines.last_mut() {
                        if !last.ends_with('\n') {
                            last.push_str(nl);
                        }
                    }
                    lines.extend(rendered);
                }
            }
        }
    }

    let out: String = lines.concat();
    let parsed = parse_yaml_mapping(&out).map_err(YamlPatchError::VerifyFailed)?;
    for op in ops {
        let actual = parsed.get(Value::String(op.key.clone()));
        if !value_matches(actual, &op.value) {
            return Err(YamlPatchError::VerifyFailed(format!(
                "补丁校验失败：字段 {}",
                op.key
            )));
        }
    }
    Ok(out)
}

/// Builds a complete frontmatter block from YAML text: `---\n<yaml>---\n`.
pub fn build_frontmatter_block(yaml: &str) -> String {
    let nl = if yaml.contains("\r\n") { "\r\n" } else { "\n" };
    let mut block = format!("---{nl}{yaml}");
    if !yaml.is_empty() && !yaml.ends_with('\n') {
        block.push_str(nl);
    }
    block.push_str("---");
    block.push_str(nl);
    block
}

/// Replaces only the YAML text inside an existing frontmatter block, keeping delimiters intact.
/// When `raw` has no frontmatter, a new block is prepended.
pub fn replace_frontmatter_yaml(raw: &str, new_yaml: &str) -> String {
    let split = split_frontmatter(raw);
    match (split.block, split.yaml) {
        (Some(block), Some(yaml)) => {
            // `yaml` is a sub-slice of `raw`; derive its byte offset from the slice addresses.
            let yaml_start = (yaml.as_ptr() as usize).saturating_sub(raw.as_ptr() as usize);
            let yaml_end = yaml_start + yaml.len();
            let mut out = String::with_capacity(raw.len() + new_yaml.len());
            out.push_str(&block[..yaml_start]);
            out.push_str(new_yaml);
            if !new_yaml.is_empty() && !new_yaml.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&block[yaml_end..]);
            out.push_str(split.body);
            out
        }
        _ => {
            let mut out = build_frontmatter_block(new_yaml);
            out.push_str(raw);
            out
        }
    }
}
