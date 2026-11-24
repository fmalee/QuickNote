import type { CommandEntry, ExternalActionState, HideReason } from '@/bindings/types';
import {
  commandsSearch,
  externalActionRun,
  externalActionsResolve,
  libraryGetInfo,
  settingsGet,
  windowHideQuickBar,
  windowShowMain,
} from '@/lib/ipc/commands';

export interface CommandWorkspaceOption {
  key: string;
  name: string;
  dirPath: string;
}

export function joinLibraryPath(root: string, relPath: string): string {
  const cleanRoot = root.replace(/\/+$/, '');
  const cleanRel = relPath.replace(/^\/+/, '');
  if (!cleanRoot) {
    return cleanRel;
  }
  if (!cleanRel) {
    return cleanRoot;
  }
  return `${cleanRoot}/${cleanRel}`;
}

export async function searchCommands(query: string): Promise<CommandEntry[]> {
  return commandsSearch({ query });
}

export async function resolveCommandExternalActions(args: {
  absPath: string | null;
  workspaceKey: string | null;
  commandId: string;
}): Promise<ExternalActionState[]> {
  return externalActionsResolve({
    absPath: args.absPath,
    workspaceKey: args.workspaceKey,
    commandId: args.commandId,
  });
}

export async function openExternalTerminalAtDir(dirPath: string): Promise<void> {
  await externalActionRun({
    id: 'external.terminal',
    absPath: null,
    dirPath,
  });
}

export async function fetchLibraryRoot(): Promise<string | null> {
  try {
    const info = await libraryGetInfo();
    return info.root ?? null;
  } catch {
    return null;
  }
}

export async function fetchCommandWorkspaces(): Promise<CommandWorkspaceOption[]> {
  try {
    const snapshot = await settingsGet();
    const workspaces = snapshot.settings?.workspaces ?? {};
    return Object.values(workspaces)
      .map((ws) => ({
        key: ws.key,
        name: ws.name || ws.key,
        dirPath: (ws.terminalDir || ws.openDir || ws.root || '').trim(),
      }))
      .filter((item) => item.dirPath.length > 0);
  } catch {
    return [];
  }
}

export async function showMainWindow(): Promise<void> {
  await windowShowMain();
}

export async function hideQuickBarWindow(reason: HideReason = 'esc'): Promise<void> {
  await windowHideQuickBar({ reason });
}

export async function copyTextToClipboard(text: string): Promise<void> {
  if (
    typeof globalThis.navigator !== 'undefined' &&
    globalThis.navigator.clipboard &&
    typeof globalThis.navigator.clipboard.writeText === 'function'
  ) {
    try {
      await globalThis.navigator.clipboard.writeText(text);
      return;
    } catch {
      // 降级到 execCommand 兜底
    }
  }

  if (typeof globalThis.document !== 'undefined') {
    const textarea = globalThis.document.createElement('textarea');
    textarea.value = text;
    textarea.setAttribute('readonly', 'true');
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    textarea.style.pointerEvents = 'none';
    globalThis.document.body.appendChild(textarea);
    textarea.select();
    try {
      if (typeof globalThis.document.execCommand === 'function') {
        globalThis.document.execCommand('copy');
      }
    } finally {
      globalThis.document.body.removeChild(textarea);
    }
  }
}
