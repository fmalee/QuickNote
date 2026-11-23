import {
  libraryGetInfo,
  settingsGet,
  windowHideQuickBar,
  windowShowMain,
} from '@/lib/ipc/commands';
import { onWindowState, type UnlistenFn } from '@/lib/ipc/events';
import type { HideReason, LibraryInfo, SettingsSnapshot, WindowState } from '@/bindings/types';

export function fetchQuickBarLibraryInfo(): Promise<LibraryInfo> {
  return libraryGetInfo();
}

export function fetchQuickBarSettings(): Promise<SettingsSnapshot> {
  return settingsGet();
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
