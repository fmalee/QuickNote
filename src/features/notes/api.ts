import type {
  CreateNoteArgs,
  IndexStatus,
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  NotesFilterArgs,
  RelPathArgs,
  RenameNoteArgs,
  UiState,
  UiStatePatchKey,
} from '@/bindings/types';
import {
  isAppError,
  noteCreate,
  noteDelete,
  noteRename,
  notesGetTreeAndTags,
  notesList,
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

export function extractNotesErrorMessage(error: unknown): string {
  if (isAppError(error)) {
    return error.detail ? `${error.message} (${error.detail})` : error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return '加载或更新笔记列表失败';
}
