//! Note domain model: `meta + body` separation, tolerant metadata, body hashing,
//! ULID formatting and IPC contract structs (`C3.4`, `D-0006`, `D-0007`).
//!
//! Pure in-memory functions only (`C1.2`).

use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::{Mapping, Value};
use specta::Type;

use crate::domain::frontmatter::{
    parse_yaml_mapping, split_frontmatter, YamlPatchOp, YamlPatchValue,
};

/// Library-relative path of the single inbox file (`v4` §4.2).
pub const INBOX_REL_PATH: &str = "inbox.md";

/// Closed document type enum (`v4` §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum NoteType {
    #[default]
    Note,
    Doc,
    #[serde(rename = "command-book")]
    CommandBook,
    Template,
}

impl NoteType {
    /// Canonical frontmatter string for this type.
    pub fn as_str(self) -> &'static str {
        match self {
            NoteType::Note => "note",
            NoteType::Doc => "doc",
            NoteType::CommandBook => "command-book",
            NoteType::Template => "template",
        }
    }

    /// Parses a frontmatter `type` value; unknown values return `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "note" => Some(NoteType::Note),
            "doc" => Some(NoteType::Doc),
            "command-book" => Some(NoteType::CommandBook),
            "template" => Some(NoteType::Template),
            _ => None,
        }
    }
}

/// Strongly typed frontmatter metadata (`C3.4`, `D-0006`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    pub id: Option<String>,
    pub title: String,
    pub doc_type: NoteType,
    pub tags: Vec<String>,
    pub project: Option<String>,
    pub open_dir: Option<String>,
    pub code_workspace: Option<String>,
    pub terminal_dir: Option<String>,
    pub created: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub meta_error: Option<String>,
}

/// Full document returned by `note_read` (`C3.4`). `body` never contains the YAML block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteDoc {
    pub key: String,
    pub rel_path: Option<String>,
    pub abs_path: String,
    pub is_library: bool,
    pub is_inbox: bool,
    pub meta: NoteMeta,
    pub body: String,
    pub body_hash: String,
    #[specta(type = specta_typescript::Number)]
    pub mtime_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub size_bytes: u64,
}

/// List row returned by `notes_list` (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub key: String,
    pub rel_path: Option<String>,
    pub abs_path: String,
    pub title: String,
    pub doc_type: NoteType,
    pub tags: Vec<String>,
    pub project: Option<String>,
    pub excerpt: String,
    pub pinned: bool,
    pub archived: bool,
    pub is_inbox: bool,
    #[specta(type = specta_typescript::Number)]
    pub mtime_ms: i64,
    pub meta_error: Option<String>,
}

/// Arguments for `note_read` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReadNoteArgs {
    pub abs_path: String,
}

/// Arguments for `note_create` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteArgs {
    pub title: String,
    pub dir_rel_path: String,
    pub template_name: Option<String>,
    pub body: Option<String>,
}

/// Arguments for `note_write` (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WriteNoteArgs {
    pub abs_path: String,
    pub body: String,
    pub base_body_hash: String,
}

/// Result of `note_write`; `updated_meta` only set when a missing ULID was back-filled (`D-0006`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult {
    pub body_hash: String,
    #[specta(type = specta_typescript::Number)]
    pub mtime_ms: i64,
    pub updated_meta: Option<NoteMeta>,
}

/// Structured conflict info serialized into `AppError.detail` on `ErrorCode::Conflict` (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub abs_path: String,
    pub current_body: String,
    pub current_body_hash: String,
    #[specta(type = specta_typescript::Number)]
    pub current_mtime_ms: i64,
}

fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Partial metadata patch for `note_update_meta` (`C3.4`).
///
/// For `Option<Option<String>>` fields: absent = keep, `null` = remove key, value = set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct NoteMetaPatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub doc_type: Option<NoteType>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[specta(type = Option<String>)]
    pub project: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[specta(type = Option<String>)]
    pub open_dir: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[specta(type = Option<String>)]
    pub code_workspace: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[specta(type = Option<String>)]
    pub terminal_dir: Option<Option<String>>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub archived: Option<bool>,
}

/// Arguments for `note_update_meta` (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteUpdateMetaArgs {
    pub abs_path: String,
    pub patch: NoteMetaPatch,
}

/// Arguments for `note_rename` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RenameNoteArgs {
    pub rel_path: String,
    pub new_rel_path: String,
}

/// Arguments carrying a single library-relative path (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelPathArgs {
    pub rel_path: String,
}

/// List filter for `notes_list` (`C3.4`, `v4` §3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotesFilterArgs {
    pub tab: String,
    pub dir_rel_path: Option<String>,
    pub tags: Vec<String>,
    pub filter_text: Option<String>,
}

/// Directory tree node (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DirNode {
    pub rel_path: String,
    pub name: String,
    pub count: u32,
    pub children: Vec<DirNode>,
}

/// Hierarchical tag tree node (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagNode {
    pub tag: String,
    pub count: u32,
    pub children: Vec<TagNode>,
}

/// Navigation snapshot returned by `notes_get_tree_and_tags` (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NavTreeSnapshot {
    pub dirs: Vec<DirNode>,
    pub tags: Vec<TagNode>,
}

/// Index lifecycle phase (`C3.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum IndexPhase {
    #[default]
    Idle,
    Scanning,
    Indexing,
    Error,
}

/// Index status returned by `index_get_status` / pushed via `index:status` (`C3.4`, `C4.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub phase: IndexPhase,
    pub notes_count: u32,
    pub fts_degraded: bool,
    #[specta(type = Option<specta_typescript::Number>)]
    pub last_completed_at: Option<i64>,
}

/// Search result kind (`C3.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SearchItemKind {
    Note,
    File,
    Dir,
    Action,
}

/// Search result source (`C3.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SearchSource {
    Library,
    Recent,
    Path,
}

/// One search result row (`C3.4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub kind: SearchItemKind,
    pub key: String,
    pub title: String,
    pub subtitle: String,
    pub rel_path: Option<String>,
    pub abs_path: String,
    #[specta(type = specta_typescript::Number)]
    pub score: f64,
    pub source: SearchSource,
}

/// Arguments for `search_notes_and_files` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryArgs {
    pub query: String,
    pub limit: Option<u32>,
    #[specta(type = specta_typescript::Number)]
    pub request_id: u64,
}

/// Search response echoing `request_id` for stale-response discarding (`C2.2`, `C3.4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    #[specta(type = specta_typescript::Number)]
    pub request_id: u64,
    pub items: Vec<SearchResultItem>,
    pub truncated: bool,
}

/// Arguments for `path_complete` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PathCompleteArgs {
    pub prefix: String,
    #[specta(type = specta_typescript::Number)]
    pub request_id: u64,
}

/// One filesystem completion row returned by `path_complete` (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PathEntry {
    pub name: String,
    pub abs_path: String,
    pub is_dir: bool,
}

/// A note document parsed into `meta + body` (`D-0006`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNote {
    /// Raw frontmatter block (delimiters included), preserved byte-for-byte on body writes.
    pub block: Option<String>,
    /// Raw YAML text inside the block.
    pub yaml: Option<String>,
    /// True when the YAML exists and parses into a mapping (safe for minimal patching).
    pub yaml_patchable: bool,
    pub body: String,
    pub meta: NoteMeta,
}

fn scalar_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn get<'a>(m: &'a Mapping, key: &str) -> Option<&'a Value> {
    m.get(Value::String(key.to_string()))
}

fn opt_string_field(m: &Mapping, key: &str, errors: &mut Vec<String>) -> Option<String> {
    match get(m, key) {
        None | Some(Value::Null) => None,
        Some(v) => match scalar_to_string(v) {
            Some(s) => {
                let t = s.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            }
            None => {
                errors.push(format!("字段 {key} 类型错误"));
                None
            }
        },
    }
}

fn bool_field(m: &Mapping, key: &str, errors: &mut Vec<String>) -> bool {
    match get(m, key) {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) if s.eq_ignore_ascii_case("true") => true,
        Some(Value::String(s)) if s.eq_ignore_ascii_case("false") => false,
        Some(_) => {
            errors.push(format!("字段 {key} 类型错误"));
            false
        }
    }
}

fn tags_field(m: &Mapping, errors: &mut Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    match get(m, "tags") {
        None | Some(Value::Null) => {}
        Some(Value::Sequence(seq)) => {
            for item in seq {
                match scalar_to_string(item) {
                    Some(s) => {
                        let t = s.trim().trim_start_matches('#').trim().to_string();
                        if !t.is_empty() && !out.contains(&t) {
                            out.push(t);
                        }
                    }
                    None => errors.push("字段 tags 含非字符串项".to_string()),
                }
            }
        }
        Some(Value::String(s)) => {
            for part in s.split(',') {
                let t = part.trim().trim_start_matches('#').trim().to_string();
                if !t.is_empty() && !out.contains(&t) {
                    out.push(t);
                }
            }
        }
        Some(_) => errors.push("字段 tags 类型错误".to_string()),
    }
    out
}

/// Builds `NoteMeta` from a parsed YAML mapping; collects tolerant errors instead of failing.
pub fn meta_from_mapping(m: &Mapping, errors: &mut Vec<String>) -> NoteMeta {
    let doc_type = match get(m, "type") {
        None | Some(Value::Null) => NoteType::Note,
        Some(v) => match scalar_to_string(v).as_deref().and_then(NoteType::parse) {
            Some(t) => t,
            None => {
                errors.push("未知 type 值，已按 note 处理".to_string());
                NoteType::Note
            }
        },
    };
    NoteMeta {
        id: opt_string_field(m, "id", errors),
        title: opt_string_field(m, "title", errors).unwrap_or_default(),
        doc_type,
        tags: tags_field(m, errors),
        project: opt_string_field(m, "project", errors),
        open_dir: opt_string_field(m, "open_dir", errors),
        code_workspace: opt_string_field(m, "code_workspace", errors),
        terminal_dir: opt_string_field(m, "terminal_dir", errors),
        created: opt_string_field(m, "created", errors),
        pinned: bool_field(m, "pinned", errors),
        archived: bool_field(m, "archived", errors),
        meta_error: None,
    }
}

/// Returns true when `line` opens or closes a fenced code block (``` or ~~~).
fn fence_marker(line: &str) -> Option<&'static str> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some("```")
    } else if t.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

/// Derives a title from the first ATX H1 outside fenced code blocks (`v4` §4.3).
pub fn derive_title_from_body(body: &str) -> Option<String> {
    let mut fence: Option<&'static str> = None;
    for line in body.lines() {
        if let Some(marker) = fence_marker(line) {
            match fence {
                Some(open) if open == marker => fence = None,
                Some(_) => {}
                None => fence = Some(marker),
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            let title = rest.trim().trim_end_matches('#').trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

/// Parses a raw Markdown document into `meta + body` with tolerant YAML handling (`D-0006`, `v4` §4.9).
pub fn parse_note(raw: &str, file_stem: &str) -> ParsedNote {
    let split = split_frontmatter(raw);
    let body = split.body.to_string();
    let mut errors: Vec<String> = Vec::new();
    let mut yaml_patchable = false;

    let mut meta = match split.yaml {
        Some(yaml) => match parse_yaml_mapping(yaml) {
            Ok(mapping) => {
                yaml_patchable = true;
                meta_from_mapping(&mapping, &mut errors)
            }
            Err(msg) => {
                errors.push(msg);
                NoteMeta::default()
            }
        },
        None => {
            if split.unterminated {
                errors.push("frontmatter 缺少结束分隔线".to_string());
            }
            NoteMeta::default()
        }
    };

    if meta.title.is_empty() {
        meta.title = derive_title_from_body(&body).unwrap_or_else(|| file_stem.to_string());
    }
    if !errors.is_empty() {
        meta.meta_error = Some(errors.join("; "));
    }

    ParsedNote {
        block: split.block.map(str::to_string),
        yaml: split.yaml.map(str::to_string),
        yaml_patchable,
        body,
        meta,
    }
}

/// Recomposes a document from its preserved frontmatter block and a new body (`D-0006`).
pub fn compose_note(block: Option<&str>, body: &str) -> String {
    match block {
        Some(b) => {
            let mut out = String::with_capacity(b.len() + body.len() + 1);
            out.push_str(b);
            if !b.ends_with('\n') && !body.is_empty() {
                out.push('\n');
            }
            out.push_str(body);
            out
        }
        None => body.to_string(),
    }
}

/// Builds a short plain-text excerpt from the body (skips headings, fences and blank lines).
pub fn make_excerpt(body: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut fence: Option<&'static str> = None;
    for line in body.lines() {
        if let Some(marker) = fence_marker(line) {
            match fence {
                Some(open) if open == marker => fence = None,
                Some(_) => {}
                None => fence = Some(marker),
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("<!--") {
            continue;
        }
        let cleaned = t
            .trim_start_matches("- [ ] ")
            .trim_start_matches("- [x] ")
            .trim_start_matches("- ")
            .trim_start_matches("* ")
            .trim_start_matches("> ");
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(cleaned);
        if out.chars().count() >= max_chars {
            break;
        }
    }
    if out.chars().count() > max_chars {
        out = out.chars().take(max_chars).collect::<String>();
        out.push('…');
    }
    out
}

/// Counts list / todo items in the inbox body, excluding fenced code (`v4` §3.3).
pub fn count_list_items(body: &str) -> u32 {
    let mut fence: Option<&'static str> = None;
    let mut count = 0u32;
    for line in body.lines() {
        if let Some(marker) = fence_marker(line) {
            match fence {
                Some(open) if open == marker => fence = None,
                Some(_) => {}
                None => fence = Some(marker),
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let t = line.trim_start();
        if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
            count = count.saturating_add(1);
        }
    }
    count
}

/// Hierarchical tag match: `dev` matches `dev` and `dev/*` (`v4` §3.3).
pub fn tag_matches(tag: &str, filter: &str) -> bool {
    tag == filter
        || (tag.len() > filter.len()
            && tag.starts_with(filter)
            && tag.as_bytes().get(filter.len()) == Some(&b'/'))
}

/// Converts a `NoteMetaPatch` into minimal YAML patch operations (`C6.4`, `D-0007`).
pub fn meta_patch_ops(patch: &NoteMetaPatch) -> Vec<YamlPatchOp> {
    let mut ops = Vec::new();
    if let Some(title) = &patch.title {
        ops.push(YamlPatchOp::set(
            "title",
            YamlPatchValue::Str(title.trim().to_string()),
        ));
    }
    if let Some(t) = patch.doc_type {
        ops.push(YamlPatchOp::set(
            "type",
            YamlPatchValue::Str(t.as_str().to_string()),
        ));
    }
    if let Some(tags) = &patch.tags {
        let mut clean: Vec<String> = Vec::new();
        for tag in tags {
            let t = tag.trim().trim_start_matches('#').trim().to_string();
            if !t.is_empty() && !clean.contains(&t) {
                clean.push(t);
            }
        }
        ops.push(YamlPatchOp::set("tags", YamlPatchValue::List(clean)));
    }
    let optional = [
        ("project", &patch.project),
        ("open_dir", &patch.open_dir),
        ("code_workspace", &patch.code_workspace),
        ("terminal_dir", &patch.terminal_dir),
    ];
    for (key, value) in optional {
        match value {
            None => {}
            Some(None) => ops.push(YamlPatchOp::remove(key)),
            Some(Some(v)) if v.trim().is_empty() => ops.push(YamlPatchOp::remove(key)),
            Some(Some(v)) => ops.push(YamlPatchOp::set(
                key,
                YamlPatchValue::Str(v.trim().to_string()),
            )),
        }
    }
    if let Some(p) = patch.pinned {
        ops.push(YamlPatchOp::set("pinned", YamlPatchValue::Bool(p)));
    }
    if let Some(a) = patch.archived {
        ops.push(YamlPatchOp::set("archived", YamlPatchValue::Bool(a)));
    }
    ops
}

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Formats a 26-char Crockford Base32 ULID from a 48-bit millisecond timestamp and 80 random bits.
pub fn format_ulid(timestamp_ms: u64, random: u128) -> String {
    let ts = u128::from(timestamp_ms & 0xFFFF_FFFF_FFFF);
    let rnd = random & ((1u128 << 80) - 1);
    let value = (ts << 80) | rnd;
    let mut out = String::with_capacity(26);
    for i in 0..26u32 {
        let shift = 125 - 5 * i;
        let idx = ((value >> shift) & 0x1F) as usize;
        out.push(char::from(CROCKFORD[idx]));
    }
    out
}

/// Computes the optimistic-concurrency body hash: lowercase hex SHA-256 of `body` (`D-0006`).
pub fn compute_body_hash(body: &str) -> String {
    let digest = sha256(body.as_bytes());
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Minimal dependency-free SHA-256 (FIPS 180-4).
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for (k, wi) in SHA256_K.iter().zip(w.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(*k)
                .wrapping_add(*wi);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(v);
        }
    }

    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// Arguments for `note_save_image` (`C4.2`, `v4` §5.6.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveImageArgs {
    pub source_path: Option<String>,
    pub bytes_base64: Option<String>,
    pub ext: String,
    pub context_rel_path: Option<String>,
}

/// Result returned by `note_save_image` (`C4.2`, `v4` §5.6.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedImageResult {
    pub rel_path: String,
}

/// Primary relation reason for `notes_get_related` (`C3.4`, `C8.6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RelatedReason {
    Backlink,
    SharedTags,
    SameProject,
}

/// Related note item returned by `notes_get_related` (`C3.4`, `C8.6`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelatedNoteItem {
    pub key: String,
    pub rel_path: String,
    pub title: String,
    pub reason: RelatedReason,
    #[specta(type = specta_typescript::Number)]
    pub score: f64,
}

/// Arguments for `tags_batch_rename` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagRenameArgs {
    pub old_tag: String,
    pub new_tag: String,
}

/// Result returned by `tags_batch_rename` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagRenameResult {
    pub updated_files: u32,
}

/// Pure input projection of an indexed note for related-note scoring (`C8.6`).
#[derive(Debug, Clone)]
pub struct RelatedNoteCandidate<'a> {
    pub key: &'a str,
    pub rel_path: &'a str,
    pub title: &'a str,
    pub tags: &'a [String],
    pub project: Option<&'a str>,
    pub mtime_ms: i64,
}

/// Scores and ranks up to 20 related notes using `C8.6` weights.
pub fn score_related_notes(
    current: &RelatedNoteCandidate<'_>,
    candidates: &[RelatedNoteCandidate<'_>],
    backlink_sources: &std::collections::HashSet<String>,
) -> Vec<RelatedNoteItem> {
    let current_tags: std::collections::HashSet<&str> =
        current.tags.iter().map(String::as_str).collect();
    let current_project = current.project.map(str::trim).filter(|s| !s.is_empty());

    let mut scored: Vec<(f64, i64, RelatedNoteItem)> = Vec::new();

    for cand in candidates {
        if cand.rel_path == current.rel_path || cand.rel_path.starts_with("_templates/") {
            continue;
        }

        let has_backlink = backlink_sources.contains(cand.rel_path);
        let shared_tags = if current_tags.is_empty() || cand.tags.is_empty() {
            0usize
        } else {
            let cand_tags: std::collections::HashSet<&str> =
                cand.tags.iter().map(String::as_str).collect();
            cand_tags.intersection(&current_tags).count()
        };
        let same_project = match (
            current_project,
            cand.project.map(str::trim).filter(|s| !s.is_empty()),
        ) {
            (Some(cp), Some(dp)) => cp == dp,
            _ => false,
        };

        let mut score = 0.0_f64;
        if has_backlink {
            score += 100.0;
        }
        if shared_tags > 0 {
            score += 15.0 * (shared_tags as f64);
        }
        if same_project {
            score += 10.0;
        }

        if score <= 0.0 {
            continue;
        }

        let reason = if has_backlink {
            RelatedReason::Backlink
        } else if shared_tags > 0 {
            RelatedReason::SharedTags
        } else {
            RelatedReason::SameProject
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

/// Arguments for `note_export_pdf` (`C3.4`, `C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportPdfArgs {
    pub html_document: String,
    pub dest_path: String,
}

/// Summary statistics returned by `notes_backfill_missing_ids` (`C3.4`, `D-0006`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackfillIdsResult {
    pub scanned: u32,
    pub updated: u32,
    pub skipped_existing: u32,
    pub skipped_broken_yaml: u32,
    pub failed: u32,
}
