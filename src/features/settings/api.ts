import {
  externalActionRun,
  hotkeyUpdate,
  isAppError,
  librarySetRoot,
  noteCreate,
  notesList,
  settingsExport,
  settingsGet,
  settingsImport,
  settingsUpdate,
  uiStateGet,
} from '@/lib/ipc/commands';
import { onSettingsStatus, onWindowState, type UnlistenFn } from '@/lib/ipc/events';
import type {
  HotkeyStatus,
  HotkeyUpdateArgs,
  LibraryInfo,
  NoteDoc,
  NoteSummary,
  SetRootArgs,
  SettingsLoadStatus,
  SettingsPatch,
  SettingsSnapshot,
  UiState,
  WindowState,
} from '@/bindings/types';

export function fetchSettingsSnapshot(): Promise<SettingsSnapshot> {
  return settingsGet();
}

export function fetchUiState(): Promise<UiState> {
  return uiStateGet();
}

export function saveSettingsPatch(patch: SettingsPatch): Promise<SettingsSnapshot> {
  return settingsUpdate(patch);
}

export function exportSettingsBackup(destPath: string): Promise<void> {
  return settingsExport({ destPath });
}

export function importSettingsBackup(srcPath: string): Promise<SettingsSnapshot> {
  return settingsImport({ srcPath });
}

export function changeLibraryRoot(args: SetRootArgs): Promise<LibraryInfo> {
  return librarySetRoot(args);
}

export function listTemplateNotes(): Promise<NoteSummary[]> {
  return notesList({
    tab: 'templates',
    dirRelPath: null,
    tags: [],
    workspaceKey: null,
    filterText: null,
  });
}

export function createTemplateNote(title: string): Promise<NoteDoc> {
  return noteCreate({
    title,
    dirRelPath: '_templates',
    templateName: 'note',
    body: null,
  });
}

export function updateGlobalHotkey(args: HotkeyUpdateArgs): Promise<HotkeyStatus> {
  return hotkeyUpdate(args);
}

export function openSettingsYamlForRepair(): Promise<void> {
  return externalActionRun({
    id: 'external.default',
    absPath: '$SETTINGS_YAML',
    dirPath: null,
  });
}

export function revealSettingsYamlInFinder(): Promise<void> {
  return externalActionRun({
    id: 'external.reveal',
    absPath: '$SETTINGS_YAML',
    dirPath: null,
  });
}

export function subscribeSettingsStatus(
  handler: (status: SettingsLoadStatus) => void,
): Promise<UnlistenFn> {
  return onSettingsStatus(handler);
}

export function subscribeWindowState(handler: (state: WindowState) => void): Promise<UnlistenFn> {
  return onWindowState(handler);
}

export function extractSettingsErrorMessage(error: unknown): string {
  if (isAppError(error)) {
    return error.detail ? `${error.message}（${error.detail}）` : error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return '更新热键失败，请稍后重试。';
}
