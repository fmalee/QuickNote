import { invoke } from '@tauri-apps/api/core';
import type {
  AppError,
  AppInfo,
  ErrorCode,
  LibraryInfo,
  SetRootArgs,
  SettingsSnapshot,
  UiState,
  UiStateArgs,
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

export function libraryGetInfo(): Promise<LibraryInfo> {
  return invoke<LibraryInfo>('library_get_info');
}

export function librarySetRoot(args: SetRootArgs): Promise<LibraryInfo> {
  return invoke<LibraryInfo>('library_set_root', { args });
}

export function settingsGet(): Promise<SettingsSnapshot> {
  return invoke<SettingsSnapshot>('settings_get');
}

export function uiStateGet(): Promise<UiState> {
  return invoke<UiState>('ui_state_get');
}

export function uiStateSet(args: UiStateArgs): Promise<UiState> {
  return invoke<UiState>('ui_state_set', { args });
}
