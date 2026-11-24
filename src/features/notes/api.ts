import type {
  CreateNoteArgs,
  IndexStatus,
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  NotesFilterArgs,
  RelPathArgs,
  RenameNoteArgs,
  SearchResultItem,
  UiState,
  UiStatePatchKey,
} from '@/bindings/types';
import {
  externalActionRun,
  isAppError,
  noteCreate,
  noteDelete,
  noteRename,
  notesGetTreeAndTags,
  notesList,
  searchNotesAndFiles,
  uiStateGet,
  uiStateSet,
} from '@/lib/ipc/commands';
import { onIndexStatus, type UnlistenFn } from '@/lib/ipc/events';

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
    workspaceKey: null,
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

export function fetchUiState(): Promise<UiState> {
  return uiStateGet();
}

export function patchUiState(key: UiStatePatchKey, value: unknown): Promise<UiState> {
  return uiStateSet({ key, value });
}

export function subscribeIndexStatus(handler: (status: IndexStatus) => void): Promise<UnlistenFn> {
  return onIndexStatus(handler);
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

export function extractNotesErrorMessage(error: unknown): string {
  if (isAppError(error)) {
    return error.detail ? `${error.message} (${error.detail})` : error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return '加载或更新笔记列表失败';
}
