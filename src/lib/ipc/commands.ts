import { Channel, invoke } from '@tauri-apps/api/core';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

export { Channel };
import type {
  AppError,
  AppInfo,
  AppendNoteArgs,
  AppendResult,
  CommandEntry,
  CommandSearchArgs,
  CreateNoteArgs,
  DirectoryWorkspaceConfig,
  DirectoryWorkspaceConfigEntry,
  ErrorCode,
  ExportPathArgs,
  ExportPdfArgs,
  ExternalActionState,
  ExternalBookmark,
  ExternalDirectoryListing,
  ExternalPathProbeResult,
  HideQuickBarArgs,
  HotkeyStatus,
  HotkeyUpdateArgs,
  ImportPathArgs,
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
  PathCompleteArgs,
  PathEntry,
  QuickBarArgs,
  ReadNoteArgs,
  RelPathArgs,
  RelatedNoteItem,
  RenameNoteArgs,
  ResolveExternalArgs,
  ResolvedTarget,
  RunExternalArgs,
  SaveImageArgs,
  SavedImageResult,
  SearchQueryArgs,
  SearchResponse,
  SetMenuAcceleratorsArgs,
  SetRootArgs,
  SettingsPatch,
  SettingsSnapshot,
  SpotlightArgs,
  TagRenameArgs,
  TagRenameResult,
  TargetResolveInput,
  TerminalData,
  TerminalKillArgs,
  TerminalResizeArgs,
  TerminalSessionInfo,
  TerminalSpawnArgs,
  TerminalVisibilityArgs,
  TerminalWriteArgs,
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

export function windowSetQuickBarPinned(pinned: boolean): Promise<boolean> {
  return invoke<boolean>('window_set_quick_bar_pinned', { args: { pinned } });
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

export function searchSpotlight(args: SpotlightArgs): Promise<SearchResponse> {
  return invoke<SearchResponse>('search_spotlight', { args });
}

export function pathComplete(args: PathCompleteArgs): Promise<PathEntry[]> {
  return invoke<PathEntry[]>('path_complete', { args });
}

export function externalActionsResolve(args: ResolveExternalArgs): Promise<ExternalActionState[]> {
  return invoke<ExternalActionState[]>('external_actions_resolve', { args });
}

export function externalActionRun(args: RunExternalArgs): Promise<void> {
  return invoke<void>('external_action_run', { args });
}

export function probeExternalPath(rawPath: string): Promise<ExternalPathProbeResult> {
  return invoke<ExternalPathProbeResult>('probe_external_path', { rawPath });
}

export function listExternalDirectory(args: {
  dirPath: string;
  query?: string | null;
  includeHidden?: boolean;
}): Promise<ExternalDirectoryListing> {
  return invoke<ExternalDirectoryListing>('list_external_directory', {
    dirPath: args.dirPath,
    query: args.query ?? null,
    includeHidden: args.includeHidden ?? true,
  });
}

export function listCurrentDocSiblings(docPath: string): Promise<ExternalDirectoryListing> {
  return invoke<ExternalDirectoryListing>('list_current_doc_siblings', { docPath });
}

export function upsertExternalBookmark(bookmark: ExternalBookmark): Promise<ExternalBookmark[]> {
  return invoke<ExternalBookmark[]>('upsert_external_bookmark', { bookmark });
}

export function removeExternalBookmark(path: string): Promise<ExternalBookmark[]> {
  return invoke<ExternalBookmark[]>('remove_external_bookmark', { path });
}

export function listDirectoryWorkspaceConfigs(): Promise<DirectoryWorkspaceConfigEntry[]> {
  return invoke<DirectoryWorkspaceConfigEntry[]>('list_directory_workspace_configs');
}

export function setDirectoryWorkspaceConfig(args: {
  vaultRelDir: string;
  config: DirectoryWorkspaceConfig | null;
}): Promise<void> {
  return invoke<void>('set_directory_workspace_config', {
    vaultRelDir: args.vaultRelDir,
    config: args.config,
  });
}

export function terminalSpawn(
  args: TerminalSpawnArgs,
  onData: Channel<TerminalData>,
): Promise<TerminalSessionInfo> {
  return invoke<TerminalSessionInfo>('terminal_spawn', { args, onData });
}

export function terminalWrite(args: TerminalWriteArgs): Promise<void> {
  return invoke<void>('terminal_write', { args });
}

export function terminalResize(args: TerminalResizeArgs): Promise<void> {
  return invoke<void>('terminal_resize', { args });
}

export function terminalSetVisibility(args: TerminalVisibilityArgs): Promise<void> {
  return invoke<void>('terminal_set_visibility', { args });
}

export function terminalKill(args: TerminalKillArgs): Promise<void> {
  return invoke<void>('terminal_kill', { args });
}

export function terminalListSessions(): Promise<TerminalSessionInfo[]> {
  return invoke<TerminalSessionInfo[]>('terminal_list_sessions');
}

export function settingsExport(args: ExportPathArgs): Promise<void> {
  return invoke<void>('settings_export', { args });
}

export function settingsImport(args: ImportPathArgs): Promise<SettingsSnapshot> {
  return invoke<SettingsSnapshot>('settings_import', { args });
}

export function noteExportPdf(args: ExportPdfArgs): Promise<void> {
  return invoke<void>('note_export_pdf', { args });
}

const BACKUP_YAML_FILTER = [{ name: 'YAML', extensions: ['yaml', 'yml'] }];

/** 弹出系统保存窗口，返回所选路径；用户取消时返回 null。 */
export async function pickBackupSavePath(defaultPath?: string | null): Promise<string | null> {
  const picked = await saveDialog({
    defaultPath: defaultPath ?? undefined,
    filters: BACKUP_YAML_FILTER,
  });
  return typeof picked === 'string' && picked.length > 0 ? picked : null;
}

/** 弹出系统打开窗口，返回所选文件路径；用户取消时返回 null。 */
export async function pickBackupOpenPath(defaultPath?: string | null): Promise<string | null> {
  const picked = await openDialog({
    multiple: false,
    directory: false,
    defaultPath: defaultPath ?? undefined,
    filters: BACKUP_YAML_FILTER,
  });
  return typeof picked === 'string' && picked.length > 0 ? picked : null;
}
