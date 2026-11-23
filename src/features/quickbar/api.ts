import {
  captureResolveTarget,
  libraryGetInfo,
  noteAppend,
  noteCreate,
  notesGetTreeAndTags,
  notesList,
  searchNotesAndFiles,
  settingsGet,
  uiStateGet,
  uiStateSet,
  windowHideQuickBar,
  windowShowMain,
} from '@/lib/ipc/commands';
import { onWindowState, type UnlistenFn } from '@/lib/ipc/events';
import type {
  AppendNoteArgs,
  AppendResult,
  CreateNoteArgs,
  HideReason,
  LibraryInfo,
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  NotesFilterArgs,
  ResolvedTarget,
  SearchQueryArgs,
  SearchResponse,
  SettingsSnapshot,
  TargetResolveInput,
  UiState,
  UiStatePatchKey,
  WindowState,
} from '@/bindings/types';

export function fetchQuickBarLibraryInfo(): Promise<LibraryInfo> {
  return libraryGetInfo();
}

export function fetchQuickBarSettings(): Promise<SettingsSnapshot> {
  return settingsGet();
}

export function fetchQuickBarUiState(): Promise<UiState> {
  return uiStateGet();
}

export function patchQuickBarUiState(key: UiStatePatchKey, value: unknown): Promise<UiState> {
  return uiStateSet({ key, value });
}

export function resolveQuickBarTarget(args: TargetResolveInput): Promise<ResolvedTarget> {
  return captureResolveTarget(args);
}

export function appendQuickBarNote(args: AppendNoteArgs): Promise<AppendResult> {
  return noteAppend(args);
}

export function createQuickBarNote(args: CreateNoteArgs): Promise<NoteDoc> {
  return noteCreate(args);
}

export function listQuickBarTemplates(): Promise<NoteSummary[]> {
  return notesList({
    tab: 'templates',
    dirRelPath: null,
    tags: [],
    workspaceKey: null,
    filterText: null,
  });
}

export function searchQuickBarNotes(args: SearchQueryArgs): Promise<SearchResponse> {
  return searchNotesAndFiles(args);
}

export function fetchQuickBarNavTree(): Promise<NavTreeSnapshot> {
  return notesGetTreeAndTags();
}

export function fetchQuickBarNotesList(args: NotesFilterArgs): Promise<NoteSummary[]> {
  return notesList(args);
}

export function showMainWindowForOnboarding(): Promise<WindowState> {
  return windowShowMain({ mode: null });
}

export function hideQuickBarWindow(reason: HideReason): Promise<WindowState> {
  return windowHideQuickBar({ reason });
}

export function subscribeQuickBarWindowState(
  handler: (state: WindowState) => void,
): Promise<UnlistenFn> {
  return onWindowState(handler);
}
