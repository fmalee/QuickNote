import { Channel } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppInfo } from '@/bindings/types';

const { mockInvoke, mockListen } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  mockListen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
  Channel: vi.fn(),
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
  noteExportPdf,
  notesGetRelated,
  pathComplete,
  searchSpotlight,
  settingsExport,
  settingsImport,
  tagsBatchRename,
  terminalKill,
  terminalListSessions,
  terminalResize,
  terminalSetVisibility,
  terminalSpawn,
  terminalWrite,
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

  it('invokes M4-01a terminal IPC wrappers', async () => {
    const channel = new Channel();
    const session = {
      sessionId: 'term-1-1',
      workspaceKey: 'ws-a',
      cwd: '/tmp',
      hasActiveForegroundChild: false,
      outputPaused: false,
      state: 'active' as const,
    };
    mockInvoke.mockResolvedValueOnce(session);
    const spawned = await terminalSpawn(
      { workspaceKey: 'ws-a', cwd: '/tmp', cols: 80, rows: 24, shell: null },
      channel as never,
    );
    expect(spawned).toEqual(session);
    expect(mockInvoke).toHaveBeenCalledWith('terminal_spawn', {
      args: { workspaceKey: 'ws-a', cwd: '/tmp', cols: 80, rows: 24, shell: null },
      onData: channel,
    });

    mockInvoke.mockResolvedValueOnce(undefined);
    await terminalWrite({ sessionId: 'term-1-1', dataBase64: 'aGk=' });
    expect(mockInvoke).toHaveBeenCalledWith('terminal_write', {
      args: { sessionId: 'term-1-1', dataBase64: 'aGk=' },
    });

    mockInvoke.mockResolvedValueOnce(undefined);
    await terminalResize({ sessionId: 'term-1-1', cols: 100, rows: 30 });
    expect(mockInvoke).toHaveBeenCalledWith('terminal_resize', {
      args: { sessionId: 'term-1-1', cols: 100, rows: 30 },
    });

    mockInvoke.mockResolvedValueOnce(undefined);
    await terminalSetVisibility({ sessionId: 'term-1-1', visible: false });
    expect(mockInvoke).toHaveBeenCalledWith('terminal_set_visibility', {
      args: { sessionId: 'term-1-1', visible: false },
    });

    mockInvoke.mockResolvedValueOnce([session]);
    const sessions = await terminalListSessions();
    expect(sessions).toEqual([session]);
    expect(mockInvoke).toHaveBeenCalledWith('terminal_list_sessions');

    mockInvoke.mockResolvedValueOnce(undefined);
    await terminalKill({ sessionId: 'term-1-1' });
    expect(mockInvoke).toHaveBeenCalledWith('terminal_kill', {
      args: { sessionId: 'term-1-1' },
    });
  });

  it('invokes M4-01b settingsExport, settingsImport, and noteExportPdf wrappers', async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await settingsExport({ destPath: '~/Desktop/quicknote-settings.yaml' });
    expect(mockInvoke).toHaveBeenCalledWith('settings_export', {
      args: { destPath: '~/Desktop/quicknote-settings.yaml' },
    });

    const snapshot = {
      settings: {},
      status: {
        valid: true,
        activeSource: 'file',
        errorLine: null,
        errorColumn: null,
        errorMessage: null,
      },
    };
    mockInvoke.mockResolvedValueOnce(snapshot);
    const imported = await settingsImport({ srcPath: '~/Desktop/quicknote-settings.yaml' });
    expect(imported).toEqual(snapshot);
    expect(mockInvoke).toHaveBeenCalledWith('settings_import', {
      args: { srcPath: '~/Desktop/quicknote-settings.yaml' },
    });

    mockInvoke.mockResolvedValueOnce(undefined);
    await noteExportPdf({
      htmlDocument: '<html><body><h1>Hello</h1></body></html>',
      destPath: '~/Desktop/note.pdf',
    });
    expect(mockInvoke).toHaveBeenCalledWith('note_export_pdf', {
      args: {
        htmlDocument: '<html><body><h1>Hello</h1></body></html>',
        destPath: '~/Desktop/note.pdf',
      },
    });
  });
});
