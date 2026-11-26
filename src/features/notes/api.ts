import type {
  CreateNoteArgs,
  DirectoryWorkspaceConfig,
  DirectoryWorkspaceConfigEntry,
  ExternalChangeEvent,
  IndexStatus,
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  NotesFilterArgs,
  RelPathArgs,
  RenameNoteArgs,
  SearchResultItem,
  TagRenameResult,
  UiState,
  UiStatePatchKey,
} from '@/bindings/types';
import {
  externalActionRun,
  isAppError,
  listDirectoryWorkspaceConfigs,
  noteCreate,
  noteDelete,
  noteRename,
  notesGetTreeAndTags,
  notesList,
  searchNotesAndFiles,
  setDirectoryWorkspaceConfig,
  tagsBatchRename,
  uiStateGet,
  uiStateSet,
} from '@/lib/ipc/commands';
import { onFsExternalChange, onIndexStatus, type UnlistenFn } from '@/lib/ipc/events';

export function fetchNavTreeAndTags(): Promise<NavTreeSnapshot> {
  return notesGetTreeAndTags();
}

export function fetchNotesList(args: NotesFilterArgs): Promise<NoteSummary[]> {
  return notesList(args);
}

export function listNoteTemplates(): Promise<NoteSummary[]> {
  return notesList({
    tab: 'templates',
    dirRelPath: null,
    tags: [],
    filterText: null,
  });
}

export function createNote(args: CreateNoteArgs): Promise<NoteDoc> {
  return noteCreate(args);
}

export function renameNote(args: RenameNoteArgs): Promise<NoteDoc> {
  return noteRename(args);
}

export function deleteNote(args: RelPathArgs): Promise<void> {
  return noteDelete(args);
}

export function batchRenameTag(oldTag: string, newTag: string): Promise<TagRenameResult> {
  return tagsBatchRename({ oldTag, newTag });
}

export function fetchUiState(): Promise<UiState> {
  return uiStateGet();
}

export function patchUiState(key: UiStatePatchKey, value: unknown): Promise<UiState> {
  return uiStateSet({ key, value });
}

export function subscribeIndexStatus(handler: (status: IndexStatus) => void): Promise<UnlistenFn> {
  return onIndexStatus(handler);
}

export function subscribeFsExternalChange(
  handler: (event: ExternalChangeEvent) => void,
): Promise<UnlistenFn> {
  return onFsExternalChange(handler);
}

export async function fetchPinnedExternalFiles(): Promise<SearchResultItem[]> {
  const res = await searchNotesAndFiles({
    query: '',
    limit: 50,
    requestId: Date.now(),
  });
  return res.items.filter((item) => item.source === 'recent');
}

export function togglePinExternalFileByPath(absPath: string): Promise<void> {
  return externalActionRun({
    id: 'external.pin',
    absPath,
    dirPath: null,
  });
}

export function revealExternalFileByPath(absPath: string): Promise<void> {
  return externalActionRun({
    id: 'external.reveal',
    absPath,
    dirPath: null,
  });
}

export async function fetchDirectoryWorkspaceConfigMap(): Promise<
  Record<string, DirectoryWorkspaceConfigEntry>
> {
  if (typeof listDirectoryWorkspaceConfigs !== 'function') {
    return {};
  }
  const entries = await listDirectoryWorkspaceConfigs();
  const map: Record<string, DirectoryWorkspaceConfigEntry> = {};
  for (const entry of entries) {
    map[entry.vaultRelDir] = entry;
  }
  return map;
}

export function saveDirectoryWorkspaceConfig(
  vaultRelDir: string,
  config: DirectoryWorkspaceConfig | null,
): Promise<void> {
  return setDirectoryWorkspaceConfig({ vaultRelDir, config });
}

export function openDirectoryWorkspaceInVscode(config: DirectoryWorkspaceConfig): Promise<void> {
  const vscodeWorkspacePath = (
    config.vscodeWorkspacePath ??
    config.vscode_workspace_path ??
    ''
  ).trim();
  if (vscodeWorkspacePath) {
    return externalActionRun({
      id: 'external.workspace',
      absPath: vscodeWorkspacePath,
      dirPath: null,
    });
  }
  const projectRoot = (config.projectRoot ?? config.project_root ?? '').trim();
  const terminalCwd = (config.defaultTerminalCwd ?? config.default_terminal_cwd ?? '').trim();
  const targetDir = projectRoot || terminalCwd;
  return externalActionRun({
    id: 'external.vscode',
    absPath: null,
    dirPath: targetDir || null,
  });
}

export function extractNotesErrorMessage(error: unknown): string {
  if (isAppError(error)) {
    return error.detail ? `${error.message} (${error.detail})` : error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return '加载或更新笔记列表失败';
}
