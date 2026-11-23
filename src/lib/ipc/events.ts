import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type {
  AppendResult,
  ExternalChangeEvent,
  IndexStatus,
  SettingsLoadStatus,
  WindowState,
} from '@/bindings/types';

export type { UnlistenFn };

export interface WindowFileDropPayload {
  paths: string[];
  x: number;
  y: number;
}

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

export function onFsExternalChange(
  handler: (event: ExternalChangeEvent) => void,
): Promise<UnlistenFn> {
  return listenEvent<ExternalChangeEvent>('fs:external-change', handler);
}

export function onMenuAction(handler: (actionId: string) => void): Promise<UnlistenFn> {
  return listenEvent<string>('menu:action', handler);
}

export async function onWindowFileDrop(
  handler: (payload: WindowFileDropPayload) => void,
): Promise<UnlistenFn> {
  const webview = getCurrentWebview();
  return webview.onDragDropEvent((event) => {
    if (event.payload.type === 'drop') {
      handler({
        paths: event.payload.paths,
        x: event.payload.position.x,
        y: event.payload.position.y,
      });
    }
  });
}
