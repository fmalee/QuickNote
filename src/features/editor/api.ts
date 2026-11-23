import type {
  ExternalChangeEvent,
  InboxExtractArgs,
  NoteDoc,
  NoteSummary,
  NoteUpdateMetaArgs,
  ReadNoteArgs,
  UiState,
  UiStatePatchKey,
  WriteNoteArgs,
  WriteResult,
} from '@/bindings/types';
import {
  inboxExtractToNote,
  isAppError,
  noteRead,
  noteUpdateMeta,
  noteWrite,
  notesList,
  uiStateGet,
  uiStateSet,
} from '@/lib/ipc/commands';
import { onFsExternalChange, type UnlistenFn } from '@/lib/ipc/events';

export interface ConflictInfoPayload {
  currentBody: string;
  currentBodyHash: string;
}

export function readNote(args: ReadNoteArgs): Promise<NoteDoc> {
  return noteRead(args);
}

export function writeNote(args: WriteNoteArgs): Promise<WriteResult> {
  return noteWrite(args);
}

export function updateNoteMeta(args: NoteUpdateMetaArgs): Promise<NoteDoc> {
  return noteUpdateMeta(args);
}

export function extractInboxToNote(args: InboxExtractArgs): Promise<NoteDoc> {
  return inboxExtractToNote(args);
}

export function listTemplates(): Promise<NoteSummary[]> {
  return notesList({
    tab: 'templates',
    dirRelPath: null,
    tags: [],
    workspaceKey: null,
    filterText: null,
  });
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

export function isConflictError(error: unknown): boolean {
  return isAppError(error) && error.code === 'conflict';
}

export function parseConflictInfo(error: unknown): ConflictInfoPayload | null {
  if (!isAppError(error) || error.code !== 'conflict' || !error.detail) {
    return null;
  }
  try {
    const parsed: unknown = JSON.parse(error.detail);
    if (
      typeof parsed === 'object' &&
      parsed !== null &&
      'currentBody' in parsed &&
      'currentBodyHash' in parsed &&
      typeof (parsed as { currentBody: unknown }).currentBody === 'string' &&
      typeof (parsed as { currentBodyHash: unknown }).currentBodyHash === 'string'
    ) {
      return {
        currentBody: (parsed as { currentBody: string }).currentBody,
        currentBodyHash: (parsed as { currentBodyHash: string }).currentBodyHash,
      };
    }
  } catch {
    return null;
  }
  return null;
}
