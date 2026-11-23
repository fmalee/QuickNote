import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { SettingsLoadStatus } from '@/bindings/types';

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
