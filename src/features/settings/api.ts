import { hotkeyUpdate, isAppError, settingsGet, uiStateGet } from '@/lib/ipc/commands';
import { onSettingsStatus, onWindowState, type UnlistenFn } from '@/lib/ipc/events';
import type {
  HotkeyStatus,
  HotkeyUpdateArgs,
  SettingsLoadStatus,
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

export function updateGlobalHotkey(args: HotkeyUpdateArgs): Promise<HotkeyStatus> {
  return hotkeyUpdate(args);
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
