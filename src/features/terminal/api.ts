import type {
  TerminalData,
  TerminalExitPayload,
  TerminalKillArgs,
  TerminalResizeArgs,
  TerminalSessionInfo,
  TerminalSpawnArgs,
  TerminalVisibilityArgs,
  TerminalWriteArgs,
} from '@/bindings/types';
import {
  Channel,
  isAppError,
  terminalKill,
  terminalListSessions,
  terminalResize,
  terminalSetVisibility,
  terminalSpawn,
  terminalWrite,
} from '@/lib/ipc/commands';
import { onTerminalExit, onTerminalSessionState, type UnlistenFn } from '@/lib/ipc/events';

export async function spawnTerminalSession(
  args: TerminalSpawnArgs,
  onData: (chunk: TerminalData) => void,
): Promise<TerminalSessionInfo> {
  const channel = new Channel<TerminalData>();
  channel.onmessage = onData;
  return terminalSpawn(args, channel);
}

export function writeTerminalInput(args: TerminalWriteArgs): Promise<void> {
  return terminalWrite(args);
}

export function resizeTerminalPty(args: TerminalResizeArgs): Promise<void> {
  return terminalResize(args);
}

export function setTerminalVisibility(args: TerminalVisibilityArgs): Promise<void> {
  return terminalSetVisibility(args);
}

export function killTerminalSession(args: TerminalKillArgs): Promise<void> {
  return terminalKill(args);
}

export function listTerminalSessions(): Promise<TerminalSessionInfo[]> {
  return terminalListSessions();
}

export function subscribeTerminalSessionState(
  handler: (info: TerminalSessionInfo) => void,
): Promise<UnlistenFn> {
  return onTerminalSessionState(handler);
}

export function subscribeTerminalExit(
  handler: (payload: TerminalExitPayload) => void,
): Promise<UnlistenFn> {
  return onTerminalExit(handler);
}

export function extractTerminalErrorMessage(error: unknown): string {
  if (isAppError(error)) {
    if (error.code === 'terminalLimitReached') {
      return '已达到终端会话并发上限（最多 3 个），请先关闭一个旧会话后再新建';
    }
    if (error.code === 'terminalSpawnFailed') {
      return `启动终端失败：${error.message}`;
    }
    return error.message;
  }
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }
  return '终端操作失败，请稍后重试';
}
