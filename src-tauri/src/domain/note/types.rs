use serde::{Deserialize, Deserializer, Serialize};
use specta::Type;

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

/// Result of `note_write`; `updated_meta` is set when ULID was back-filled or derived title changed (`D-0006`).
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
    pub doc_type: Option<NoteType>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
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
