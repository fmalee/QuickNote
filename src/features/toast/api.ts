import type { AppendResult, UndoAppendResult } from '@/bindings/types';
import { noteUndoLastAppend as ipcNoteUndoLastAppend } from '@/lib/ipc/commands';
import { onToastAppendUndo as ipcOnToastAppendUndo, type UnlistenFn } from '@/lib/ipc/events';

export function onToastAppendUndo(handler: (payload: AppendResult) => void): Promise<UnlistenFn> {
  return ipcOnToastAppendUndo(handler);
}

export function undoLastAppend(): Promise<UndoAppendResult> {
  return ipcNoteUndoLastAppend();
}
