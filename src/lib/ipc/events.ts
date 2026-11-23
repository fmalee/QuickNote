import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type { UnlistenFn };

export function listenEvent<TPayload>(
  event: string,
  handler: (payload: TPayload) => void,
): Promise<UnlistenFn> {
  return listen<TPayload>(event, (eventObj) => {
    handler(eventObj.payload);
  });
}
