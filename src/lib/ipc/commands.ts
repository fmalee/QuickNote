import { invoke } from '@tauri-apps/api/core';
import type {
  AppError,
  AppInfo,
  AppendNoteArgs,
  AppendResult,
  CommandEntry,
  CommandSearchArgs,
  CreateNoteArgs,
  ErrorCode,
  HideQuickBarArgs,
  HotkeyStatus,
  HotkeyUpdateArgs,
  InboxExtractArgs,
  IndexStatus,
  LibraryInfo,
  MainWindowArgs,
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  NoteUpdateMetaArgs,
  NotesFilterArgs,
  OpenExternalUrlArgs,
  QuickBarArgs,
  ReadNoteArgs,
  RelPathArgs,
  RelatedNoteItem,
  RenameNoteArgs,
  ResolvedTarget,
  SaveImageArgs,
  SavedImageResult,
  SearchQueryArgs,
  SearchResponse,
  SetMenuAcceleratorsArgs,
  SetRootArgs,
  SettingsPatch,
  SettingsSnapshot,
  TagRenameArgs,
  TagRenameResult,
  TargetResolveInput,
  UiState,
  UiStateArgs,
  UndoAppendResult,
  WindowState,
  WriteNoteArgs,
  WriteResult,
} from '@/bindings/types';

const ERROR_CODES = new Set<ErrorCode>([
  'validationError',
  'libraryNotConfigured',
  'notFound',
  'permissionDenied',
  'conflict',
  'undoUnavailable',
  'invalidFrontmatter',
  'settingsParseError',
  'externalAppMissing',
  'workspaceDirMissing',
  'terminalLimitReached',
  'terminalSpawnFailed',
  'internalError',
]);

export function isAppError(error: unknown): error is AppError {
  if (typeof error !== 'object' || error === null) {
    return false;
  }

  const candidate = error as Record<string, unknown>;
  if (typeof candidate.code !== 'string' || !ERROR_CODES.has(candidate.code as ErrorCode)) {
    return false;
  }
  if (typeof candidate.message !== 'string') {
    return false;
  }
  if (
    candidate.detail !== undefined &&
    candidate.detail !== null &&
    typeof candidate.detail !== 'string'
  ) {
    return false;
  }

  return true;
}

export function appGetInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_get_info');
}

export function appOpenExternalUrl(args: OpenExternalUrlArgs): Promise<void> {
  return invoke<void>('app_open_external_url', { args });
}

export function appSetMenuAccelerators(args: SetMenuAcceleratorsArgs): Promise<void> {
  return invoke<void>('app_set_menu_accelerators', { args });
}

export function libraryGetInfo(): Promise<LibraryInfo> {
  return invoke<LibraryInfo>('library_get_info');
}

export function librarySetRoot(args: SetRootArgs): Promise<LibraryInfo> {
  return invoke<LibraryInfo>('library_set_root', { args });
}

export function settingsGet(): Promise<SettingsSnapshot> {
  return invoke<SettingsSnapshot>('settings_get');
}

export function settingsUpdate(args: SettingsPatch): Promise<SettingsSnapshot> {
  return invoke<SettingsSnapshot>('settings_update', { args });
}

export function uiStateGet(): Promise<UiState> {
  return invoke<UiState>('ui_state_get');
}

export function uiStateSet(args: UiStateArgs): Promise<UiState> {
  return invoke<UiState>('ui_state_set', { args });
}

export function windowShowMain(args?: MainWindowArgs): Promise<WindowState> {
  return invoke<WindowState>('window_show_main', { args: args ?? null });
}

export function windowHideMain(): Promise<WindowState> {
  return invoke<WindowState>('window_hide_main');
}

export function windowShowQuickBar(args?: QuickBarArgs): Promise<WindowState> {
  return invoke<WindowState>('window_show_quick_bar', { args: args ?? null });
}

export function windowHideQuickBar(args: HideQuickBarArgs): Promise<WindowState> {
  return invoke<WindowState>('window_hide_quick_bar', { args });
}

export function windowOpenSettings(): Promise<WindowState> {
  return invoke<WindowState>('window_open_settings');
}

export function hotkeyUpdate(args: HotkeyUpdateArgs): Promise<HotkeyStatus> {
  return invoke<HotkeyStatus>('hotkey_update', { args });
}

export function indexGetStatus(): Promise<IndexStatus> {
  return invoke<IndexStatus>('index_get_status');
}

export function indexRebuild(): Promise<IndexStatus> {
  return invoke<IndexStatus>('index_rebuild');
}

export function notesList(args: NotesFilterArgs): Promise<NoteSummary[]> {
  return invoke<NoteSummary[]>('notes_list', { args });
}

export function notesGetTreeAndTags(): Promise<NavTreeSnapshot> {
  return invoke<NavTreeSnapshot>('notes_get_tree_and_tags');
}

export function noteRead(args: ReadNoteArgs): Promise<NoteDoc> {
  return invoke<NoteDoc>('note_read', { args });
}

export function noteCreate(args: CreateNoteArgs): Promise<NoteDoc> {
  return invoke<NoteDoc>('note_create', { args });
}

export function noteWrite(args: WriteNoteArgs): Promise<WriteResult> {
  return invoke<WriteResult>('note_write', { args });
}

export function noteUpdateMeta(args: NoteUpdateMetaArgs): Promise<NoteDoc> {
  return invoke<NoteDoc>('note_update_meta', { args });
}

export function noteRename(args: RenameNoteArgs): Promise<NoteDoc> {
  return invoke<NoteDoc>('note_rename', { args });
}

export function noteDelete(args: RelPathArgs): Promise<void> {
  return invoke<void>('note_delete', { args });
}

export function noteSaveImage(args: SaveImageArgs): Promise<SavedImageResult> {
  return invoke<SavedImageResult>('note_save_image', { args });
}

export function searchNotesAndFiles(args: SearchQueryArgs): Promise<SearchResponse> {
  return invoke<SearchResponse>('search_notes_and_files', { args });
}

export function captureResolveTarget(args: TargetResolveInput): Promise<ResolvedTarget> {
  return invoke<ResolvedTarget>('capture_resolve_target', { args });
}

export function noteAppend(args: AppendNoteArgs): Promise<AppendResult> {
  return invoke<AppendResult>('note_append', { args });
}

export function noteUndoLastAppend(): Promise<UndoAppendResult> {
  return invoke<UndoAppendResult>('note_undo_last_append');
}

export function inboxExtractToNote(args: InboxExtractArgs): Promise<NoteDoc> {
  return invoke<NoteDoc>('inbox_extract_to_note', { args });
}

export function commandsSearch(args: CommandSearchArgs): Promise<CommandEntry[]> {
  return invoke<CommandEntry[]>('commands_search', { args });
}

export function notesGetRelated(args: RelPathArgs): Promise<RelatedNoteItem[]> {
  return invoke<RelatedNoteItem[]>('notes_get_related', { args });
}

export function tagsBatchRename(args: TagRenameArgs): Promise<TagRenameResult> {
  return invoke<TagRenameResult>('tags_batch_rename', { args });
}
