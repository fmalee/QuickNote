import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppInfo } from '@/bindings/types';

const { mockInvoke, mockListen } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  mockListen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mockListen,
}));

import {
  appGetInfo,
  commandsSearch,
  externalActionRun,
  externalActionsResolve,
  isAppError,
  notesGetRelated,
  pathComplete,
  searchSpotlight,
  tagsBatchRename,
} from '@/lib/ipc/commands';
import { listenEvent } from '@/lib/ipc/events';

describe('IPC smoke wrapper', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockListen.mockReset();
  });

  it('invokes app_get_info and returns typed AppInfo', async () => {
    const expected: AppInfo = {
      version: '0.1.0',
      platform: 'macos',
      libraryRoot: null,
      activationPolicy: 'accessory',
    };
    mockInvoke.mockResolvedValueOnce(expected);

    const actual = await appGetInfo();

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith('app_get_info');
    expect(actual).toEqual(expected);
  });

  it('validates structured AppError payloads with isAppError', () => {
    expect(
      isAppError({
        code: 'validationError',
        message: 'bad key',
        detail: null,
      }),
    ).toBe(true);

    expect(
      isAppError({
        code: 'settingsParseError',
        message: 'yaml syntax error',
        detail: 'line 12 col 4',
      }),
    ).toBe(true);

    expect(isAppError({ code: 'unknownCode', message: 'nope' })).toBe(false);
    expect(isAppError(new Error('plain error'))).toBe(false);
    expect(isAppError(null)).toBe(false);
  });

  it('unwraps Tauri event payload in listenEvent', async () => {
    const unlisten = vi.fn();
    let capturedCallback: ((event: { payload: { count: number } }) => void) | undefined;
    mockListen.mockImplementationOnce(
      async (_event: string, cb: (event: { payload: { count: number } }) => void) => {
        capturedCallback = cb;
        return unlisten;
      },
    );

    const received: number[] = [];
    const returnedUnlisten = await listenEvent<{ count: number }>('index:status', (payload) => {
      received.push(payload.count);
    });

    expect(mockListen).toHaveBeenCalledTimes(1);
    expect(returnedUnlisten).toBe(unlisten);
    capturedCallback?.({ payload: { count: 42 } });
    expect(received).toEqual([42]);
  });

  it('invokes M3-01a commandsSearch, notesGetRelated, and tagsBatchRename', async () => {
    mockInvoke.mockResolvedValueOnce([]);
    await commandsSearch({ query: 'git' });
    expect(mockInvoke).toHaveBeenCalledWith('commands_search', { args: { query: 'git' } });

    mockInvoke.mockResolvedValueOnce([]);
    await notesGetRelated({ relPath: 'notes/a.md' });
    expect(mockInvoke).toHaveBeenCalledWith('notes_get_related', {
      args: { relPath: 'notes/a.md' },
    });

    mockInvoke.mockResolvedValueOnce({ updatedFiles: 2 });
    const res = await tagsBatchRename({ oldTag: 'dev', newTag: 'work' });
    expect(mockInvoke).toHaveBeenCalledWith('tags_batch_rename', {
      args: { oldTag: 'dev', newTag: 'work' },
    });
    expect(res).toEqual({ updatedFiles: 2 });
  });

  it('invokes M3-01b searchSpotlight, pathComplete, externalActionsResolve, and externalActionRun', async () => {
    mockInvoke.mockResolvedValueOnce({ requestId: 1, items: [], truncated: false });
    await searchSpotlight({ query: 'foo', requestId: 1 });
    expect(mockInvoke).toHaveBeenCalledWith('search_spotlight', {
      args: { query: 'foo', requestId: 1 },
    });

    mockInvoke.mockResolvedValueOnce([]);
    await pathComplete({ prefix: '/tmp/', requestId: 2 });
    expect(mockInvoke).toHaveBeenCalledWith('path_complete', {
      args: { prefix: '/tmp/', requestId: 2 },
    });

    mockInvoke.mockResolvedValueOnce([]);
    await externalActionsResolve({ absPath: '/tmp/a.txt', workspaceKey: null, commandId: null });
    expect(mockInvoke).toHaveBeenCalledWith('external_actions_resolve', {
      args: { absPath: '/tmp/a.txt', workspaceKey: null, commandId: null },
    });

    mockInvoke.mockResolvedValueOnce(undefined);
    await externalActionRun({ id: 'external.reveal', absPath: '/tmp/a.txt', dirPath: null });
    expect(mockInvoke).toHaveBeenCalledWith('external_action_run', {
      args: { id: 'external.reveal', absPath: '/tmp/a.txt', dirPath: null },
    });
  });
});
