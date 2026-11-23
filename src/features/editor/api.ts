import type {
  ExternalChangeEvent,
  NoteDoc,
  NoteSummary,
  ReadNoteArgs,
  UiState,
  UiStatePatchKey,
} from '@/bindings/types';
import { isAppError, noteRead, notesList, uiStateGet, uiStateSet } from '@/lib/ipc/commands';
import { onFsExternalChange, type UnlistenFn } from '@/lib/ipc/events';

export function readNote(args: ReadNoteArgs): Promise<NoteDoc> {
  return noteRead(args);
}

export function listAllNotes(): Promise<NoteSummary[]> {
  return notesList({
    tab: 'recent',
    dirRelPath: null,
    tags: [],
    workspaceKey: null,
    filterText: null,
  });
}

export function fetchUiState(): Promise<UiState> {
  return uiStateGet();
}

export function patchUiState(key: UiStatePatchKey, value: unknown): Promise<UiState> {
  return uiStateSet({ key, value });
}

export function subscribeFsExternalChange(
  handler: (event: ExternalChangeEvent) => void,
): Promise<UnlistenFn> {
  return onFsExternalChange(handler);
}

export function extractEditorErrorMessage(error: unknown): string {
  if (isAppError(error)) {
    return error.detail ? `${error.message} (${error.detail})` : error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return '读取或恢复标签页失败';
}

export function isNoteNotFoundError(error: unknown): boolean {
  return isAppError(error) && error.code === 'notFound';
}
