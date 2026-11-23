import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AppendResult, IndexStatus, SettingsLoadStatus, WindowState } from '@/bindings/types';

export type { UnlistenFn };

export function listenEvent<TPayload>(
  event: string,
  handler: (payload: TPayload) => void,
): Promise<UnlistenFn> {
  return listen<TPayload>(event, (eventObj) => {
    handler(eventObj.payload);
  });
}

export function onSettingsStatus(
  handler: (status: SettingsLoadStatus) => void,
): Promise<UnlistenFn> {
  return listenEvent<SettingsLoadStatus>('settings:status', handler);
}

export function onWindowState(handler: (state: WindowState) => void): Promise<UnlistenFn> {
  return listenEvent<WindowState>('window:state', handler);
}

export function onIndexStatus(handler: (status: IndexStatus) => void): Promise<UnlistenFn> {
  return listenEvent<IndexStatus>('index:status', handler);
}

export function onToastAppendUndo(handler: (payload: AppendResult) => void): Promise<UnlistenFn> {
  return listenEvent<AppendResult>('toast:append-undo', handler);
}
