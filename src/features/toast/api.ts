import { listenEvent, type UnlistenFn } from '@/lib/ipc/events';

export interface ToastAppendUndoPayload {
  relPath: string;
  title: string;
  addedLines?: number;
  sectionHeading?: string | null;
  bodyHash?: string;
  mtimeMs?: number;
  undoAvailable?: boolean;
}

export function onToastAppendUndo(
  handler: (payload: ToastAppendUndoPayload) => void,
): Promise<UnlistenFn> {
  return listenEvent<ToastAppendUndoPayload>('toast:append-undo', handler);
}
