//! Note domain model: `meta + body` separation, tolerant metadata, body hashing,
//! ULID formatting and IPC contract structs (`C3.4`, `D-0006`, `D-0007`).
//!
//! Pure in-memory functions only (`C1.2`).

mod parse;
mod tags_and_related;
mod types;

/// Library-relative path of the single inbox file (`v4` §4.2).
pub const INBOX_REL_PATH: &str = "inbox.md";

pub use parse::{
    compose_note, compute_body_hash, count_list_items, derive_title_from_body, format_ulid,
    make_excerpt, meta_from_mapping, meta_patch_ops, parse_note, resolve_markdown_display_title,
    ParsedNote,
};
pub use tags_and_related::{
    normalize_tag_name, rewrite_tags_for_rename, score_related_notes, tag_matches,
    RelatedNoteCandidate,
};
pub use types::{
    BackfillIdsResult, CleanUnusedAssetsArgs, CleanUnusedAssetsResult, ConflictInfo,
    CreateNoteArgs, DirNode, ExportPdfArgs, IndexPhase, IndexStatus, NavTreeSnapshot, NoteDoc,
    NoteMeta, NoteMetaPatch, NoteSummary, NoteType, NoteUpdateMetaArgs, NotesFilterArgs,
    PathCompleteArgs, PathEntry, ReadNoteArgs, RelPathArgs, RelatedNoteItem, RelatedReason,
    RenameNoteArgs, SaveImageArgs, SavedImageResult, ScanUnusedAssetsArgs, SearchItemKind,
    SearchQueryArgs, SearchResponse, SearchResultItem, SearchSource, TagNode, TagRenameArgs,
    TagRenameResult, UnusedAssetEntry, UnusedAssetsScanResult, WriteNoteArgs, WriteResult,
};
