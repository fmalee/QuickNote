use std::path::Path;

use specta::Types;
use specta_typescript::Typescript;

use crate::commands::external::{
    DirectoryWorkspaceConfigEntry, ExternalDirectoryEntry, ExternalDirectoryListing,
    ExternalPathEntryKind, ExternalPathProbeResult,
};
use crate::domain::app::{ActivationPolicy, AppInfo, OpenExternalUrlArgs, SetMenuAcceleratorsArgs};
use crate::domain::bookmark::{ExternalBookmark, ExternalBookmarkKind};
use crate::domain::capture::{
    AppendNoteArgs, AppendResult, InboxExtractArgs, ResolvedTarget, TargetKind, TargetResolveInput,
    UndoAppendResult,
};
use crate::domain::command_book::{CommandEntry, CommandSearchArgs};
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::external::{
    ExternalActionId, ExternalActionState, ResolveExternalArgs, RunExternalArgs,
};
use crate::domain::hotkey::HotkeyUpdateArgs;
use crate::domain::note::{
    ConflictInfo, CreateNoteArgs, DirNode, ExportPdfArgs, IndexPhase, IndexStatus, NavTreeSnapshot,
    NoteDoc, NoteMeta, NoteMetaPatch, NoteSummary, NoteType, NoteUpdateMetaArgs, NotesFilterArgs,
    PathCompleteArgs, PathEntry, ReadNoteArgs, RelPathArgs, RelatedNoteItem, RelatedReason,
    RenameNoteArgs, SaveImageArgs, SavedImageResult, SearchItemKind, SearchQueryArgs,
    SearchResponse, SearchResultItem, SearchSource, SpotlightArgs, TagNode, TagRenameArgs,
    TagRenameResult, WorkspaceSummary, WriteNoteArgs, WriteResult,
};
use crate::domain::settings::{
    EditorSettings, EditorSettingsPatch, EditorTheme, EditorsSettings, EditorsSettingsPatch,
    ExportPathArgs, ExternalAppConfig, ExternalAppConfigPatch, HotkeySettings, HotkeySettingsPatch,
    ImportPathArgs, InboxSettings, InboxSettingsPatch, LibraryInfo, MainWindowSettings,
    MainWindowSettingsPatch, PanelId, PanelsSettings, PanelsSettingsPatch, QuickBarSettings,
    QuickBarSettingsPatch, SetRootArgs, Settings, SettingsLoadStatus, SettingsPatch,
    SettingsSnapshot, TerminalPanelSettings, TerminalPanelSettingsPatch, WindowMode,
    WorkspaceConfig,
};
use crate::domain::terminal::{
    TerminalData, TerminalExitPayload, TerminalExitReason, TerminalKillArgs,
    TerminalLifecycleState, TerminalResizeArgs, TerminalSessionInfo, TerminalSpawnArgs,
    TerminalVisibilityArgs, TerminalWriteArgs,
};
use crate::domain::ui_state::{
    BackupPathsUiState, CaptureMode, FiltersUiState, HotkeySelfCheck, HotkeyStatus,
    LastActiveDocState, LeftSidebarUiState, LeftViewId, MainWindowState, NavTab, PanelGroupUiState,
    TabItemState, TabsUiState, TerminalPanelUiState, UiState, UiStateArgs, UiStatePatchKey,
    WindowsUiState,
};
use crate::domain::watch::{
    ExternalChangeEvent, ExternalChangeKind, QuickBarDraft, RecoverySnapshot,
};
use crate::domain::window::{
    HideQuickBarArgs, HideReason, MainWindowArgs, QuickBarArgs, QuickBarMode, WindowState,
};
use crate::domain::workspace_context::{DirectoryWorkspaceConfig, EffectiveWorkspaceContext};

/// Exact header line required at line 1 of `src/bindings/types.ts` (`C1.4`).
pub const GENERATED_HEADER: &str = "/* GENERATED — DO NOT EDIT. */";

/// Collects all Rust IPC types exported to TypeScript (`C3.1`, `D-0009`).
pub fn collect_types() -> Types {
    Types::default()
        .register::<ActivationPolicy>()
        .register::<AppInfo>()
        .register::<OpenExternalUrlArgs>()
        .register::<SetMenuAcceleratorsArgs>()
        .register::<ErrorCode>()
        .register::<AppError>()
        .register::<WindowMode>()
        .register::<EditorTheme>()
        .register::<PanelId>()
        .register::<LibraryInfo>()
        .register::<SetRootArgs>()
        .register::<WorkspaceConfig>()
        .register::<HotkeySettings>()
        .register::<QuickBarSettings>()
        .register::<MainWindowSettings>()
        .register::<EditorSettings>()
        .register::<TerminalPanelSettings>()
        .register::<PanelsSettings>()
        .register::<InboxSettings>()
        .register::<ExternalAppConfig>()
        .register::<EditorsSettings>()
        .register::<Settings>()
        .register::<SettingsLoadStatus>()
        .register::<SettingsSnapshot>()
        .register::<HotkeySettingsPatch>()
        .register::<QuickBarSettingsPatch>()
        .register::<MainWindowSettingsPatch>()
        .register::<EditorSettingsPatch>()
        .register::<TerminalPanelSettingsPatch>()
        .register::<PanelsSettingsPatch>()
        .register::<InboxSettingsPatch>()
        .register::<ExternalAppConfigPatch>()
        .register::<EditorsSettingsPatch>()
        .register::<SettingsPatch>()
        .register::<CaptureMode>()
        .register::<NavTab>()
        .register::<HotkeySelfCheck>()
        .register::<HotkeyStatus>()
        .register::<MainWindowState>()
        .register::<WindowsUiState>()
        .register::<TabItemState>()
        .register::<TabsUiState>()
        .register::<PanelGroupUiState>()
        .register::<LeftViewId>()
        .register::<LeftSidebarUiState>()
        .register::<TerminalPanelUiState>()
        .register::<BackupPathsUiState>()
        .register::<FiltersUiState>()
        .register::<LastActiveDocState>()
        .register::<UiState>()
        .register::<UiStatePatchKey>()
        .register::<UiStateArgs>()
        .register::<QuickBarMode>()
        .register::<HideReason>()
        .register::<WindowState>()
        .register::<MainWindowArgs>()
        .register::<QuickBarArgs>()
        .register::<HideQuickBarArgs>()
        .register::<HotkeyUpdateArgs>()
        .register::<NoteType>()
        .register::<NoteMeta>()
        .register::<NoteDoc>()
        .register::<NoteSummary>()
        .register::<ReadNoteArgs>()
        .register::<CreateNoteArgs>()
        .register::<WriteNoteArgs>()
        .register::<WriteResult>()
        .register::<SaveImageArgs>()
        .register::<SavedImageResult>()
        .register::<ConflictInfo>()
        .register::<NoteMetaPatch>()
        .register::<NoteUpdateMetaArgs>()
        .register::<RenameNoteArgs>()
        .register::<RelPathArgs>()
        .register::<NotesFilterArgs>()
        .register::<DirNode>()
        .register::<TagNode>()
        .register::<WorkspaceSummary>()
        .register::<NavTreeSnapshot>()
        .register::<IndexPhase>()
        .register::<IndexStatus>()
        .register::<SearchItemKind>()
        .register::<SearchSource>()
        .register::<SearchResultItem>()
        .register::<SearchQueryArgs>()
        .register::<SearchResponse>()
        .register::<SpotlightArgs>()
        .register::<PathCompleteArgs>()
        .register::<PathEntry>()
        .register::<ExternalActionId>()
        .register::<ExternalActionState>()
        .register::<ResolveExternalArgs>()
        .register::<RunExternalArgs>()
        .register::<ExternalBookmarkKind>()
        .register::<ExternalBookmark>()
        .register::<DirectoryWorkspaceConfig>()
        .register::<EffectiveWorkspaceContext>()
        .register::<ExternalPathEntryKind>()
        .register::<ExternalPathProbeResult>()
        .register::<ExternalDirectoryEntry>()
        .register::<ExternalDirectoryListing>()
        .register::<DirectoryWorkspaceConfigEntry>()
        .register::<CommandEntry>()
        .register::<CommandSearchArgs>()
        .register::<RelatedReason>()
        .register::<RelatedNoteItem>()
        .register::<TagRenameArgs>()
        .register::<TagRenameResult>()
        .register::<TargetKind>()
        .register::<TargetResolveInput>()
        .register::<ResolvedTarget>()
        .register::<AppendNoteArgs>()
        .register::<AppendResult>()
        .register::<UndoAppendResult>()
        .register::<InboxExtractArgs>()
        .register::<ExternalChangeKind>()
        .register::<ExternalChangeEvent>()
        .register::<QuickBarDraft>()
        .register::<RecoverySnapshot>()
        .register::<TerminalLifecycleState>()
        .register::<TerminalExitReason>()
        .register::<TerminalSpawnArgs>()
        .register::<TerminalWriteArgs>()
        .register::<TerminalResizeArgs>()
        .register::<TerminalVisibilityArgs>()
        .register::<TerminalKillArgs>()
        .register::<TerminalSessionInfo>()
        .register::<TerminalData>()
        .register::<TerminalExitPayload>()
        .register::<ExportPathArgs>()
        .register::<ImportPathArgs>()
        .register::<ExportPdfArgs>()
}

/// Renders the full TypeScript bindings content deterministically.
pub fn render_ts_bindings() -> Result<String, AppError> {
    let types = collect_types();
    Typescript::default()
        .header(GENERATED_HEADER)
        .export(&types, specta_serde::Format)
        .map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to render TypeScript IPC bindings",
                err.to_string(),
            )
        })
}

/// Exports the generated TypeScript bindings to `path`.
pub fn export_ts_bindings(path: &Path) -> Result<(), AppError> {
    let rendered = render_ts_bindings()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to create bindings parent directory",
                err.to_string(),
            )
        })?;
    }
    std::fs::write(path, rendered).map_err(|err| {
        AppError::with_detail(
            ErrorCode::InternalError,
            "Failed to write TypeScript bindings file",
            err.to_string(),
        )
    })
}
