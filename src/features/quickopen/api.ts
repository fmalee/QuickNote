import type {
  ExternalActionId,
  ExternalBookmark,
  ExternalDirectoryListing,
  ExternalPathProbeResult,
  HideReason,
  NavTreeSnapshot,
  NoteSummary,
  NotesFilterArgs,
  SearchQueryArgs,
  SearchResponse,
  WindowMode,
  WindowState,
} from '@/bindings/types';
import {
  externalActionRun,
  listExternalDirectory as listExternalDirectoryIpc,
  notesGetTreeAndTags,
  notesList,
  probeExternalPath as probeExternalPathIpc,
  searchNotesAndFiles,
  settingsGet,
  windowHideQuickBar,
  windowShowMain,
} from '@/lib/ipc/commands';

export const PENDING_OPEN_FILE_STORAGE_KEY = 'quicknote:pending-open-file-v1';

export interface PendingOpenFileItemPayload {
  kind?: 'file';
  absPath: string;
  newTab: boolean;
  line?: number | null;
  ts: number;
}

export interface PendingOpenTagPayload {
  kind: 'tag';
  tag: string;
  ts: number;
}

export interface PendingOpenActionPayload {
  kind: 'action';
  actionId: string;
  ts: number;
}

export type PendingOpenFilePayload =
  PendingOpenFileItemPayload | PendingOpenTagPayload | PendingOpenActionPayload;

export function fetchQuickOpenNotesAndFiles(args: SearchQueryArgs): Promise<SearchResponse> {
  return searchNotesAndFiles(args);
}

export function probeQuickOpenExternalPath(rawPath: string): Promise<ExternalPathProbeResult> {
  return probeExternalPathIpc(rawPath);
}

export function listQuickOpenExternalDirectory(
  dirPath: string,
  options?: { query?: string | null; includeHidden?: boolean },
): Promise<ExternalDirectoryListing> {
  return listExternalDirectoryIpc({
    dirPath,
    query: options?.query ?? null,
    includeHidden: options?.includeHidden ?? true,
  });
}

export async function fetchQuickOpenExternalBookmarks(): Promise<ExternalBookmark[]> {
  const snapshot = await settingsGet();
  return snapshot.settings.externalBookmarks ?? [];
}

export function fetchQuickOpenTreeAndTags(): Promise<NavTreeSnapshot> {
  return notesGetTreeAndTags();
}

export function fetchQuickOpenNotesList(args: NotesFilterArgs): Promise<NoteSummary[]> {
  return notesList(args);
}

export function runQuickOpenExternalAction(args: {
  id: ExternalActionId;
  absPath: string | null;
  dirPath: string | null;
}): Promise<void> {
  return externalActionRun(args);
}

export function showMainWindowForQuickOpen(mode: WindowMode | null = null): Promise<WindowState> {
  return windowShowMain({ mode });
}

export function hideQuickBarForQuickOpen(reason: HideReason = 'esc'): Promise<WindowState> {
  return windowHideQuickBar({ reason });
}

export function writePendingOpenFile(payload: PendingOpenFilePayload): void {
  try {
    if (typeof globalThis.localStorage === 'undefined') {
      return;
    }
    globalThis.localStorage.setItem(PENDING_OPEN_FILE_STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // 容错：无 localStorage 环境忽略
  }
}

export function consumePendingOpenFile(): PendingOpenFilePayload | null {
  try {
    if (typeof globalThis.localStorage === 'undefined') {
      return null;
    }
    const raw = globalThis.localStorage.getItem(PENDING_OPEN_FILE_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    globalThis.localStorage.removeItem(PENDING_OPEN_FILE_STORAGE_KEY);
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) {
      return null;
    }
    const candidate = parsed as Record<string, unknown>;
    const ts = typeof candidate.ts === 'number' ? candidate.ts : Date.now();
    if (candidate.kind === 'tag') {
      if (typeof candidate.tag !== 'string' || !candidate.tag) {
        return null;
      }
      return {
        kind: 'tag',
        tag: candidate.tag,
        ts,
      };
    }
    if (candidate.kind === 'action') {
      if (typeof candidate.actionId !== 'string' || !candidate.actionId) {
        return null;
      }
      return {
        kind: 'action',
        actionId: candidate.actionId,
        ts,
      };
    }
    if (typeof candidate.absPath !== 'string' || !candidate.absPath) {
      return null;
    }
    return {
      kind: 'file',
      absPath: candidate.absPath,
      newTab: Boolean(candidate.newTab),
      line: typeof candidate.line === 'number' ? candidate.line : null,
      ts,
    };
  } catch {
    return null;
  }
}
