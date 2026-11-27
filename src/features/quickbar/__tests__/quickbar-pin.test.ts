import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SettingsSnapshot } from '@/bindings/types';

const { mockWindowSetQuickBarPinned } = vi.hoisted(() => ({
  mockWindowSetQuickBarPinned: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  windowSetQuickBarPinned: mockWindowSetQuickBarPinned,
  windowShowMain: vi.fn(),
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onWindowState: vi.fn().mockResolvedValue(() => {}),
  onSettingsStatus: vi.fn().mockResolvedValue(() => {}),
  onToastAppendUndo: vi.fn().mockResolvedValue(() => {}),
  onFsExternalChange: vi.fn().mockResolvedValue(() => {}),
  listenEvent: vi.fn().mockResolvedValue(() => {}),
}));

import { useQuickBarStore } from '@/features/quickbar';
import { useSettingsStore } from '@/features/settings';

function makeSettingsSnapshot(pinned: boolean): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/tmp/vault',
      hotkey: { quickBar: 'ctrl+alt+space', quickBarBackup: 'ctrl+alt+cmd+space' },
      quickBar: {
        defaultTarget: 'last_active_or_inbox',
        lastActiveTtlHours: 24,
        destroyAfterHiddenMinutes: 10,
        autoCreateMissingSection: false,
        pinned,
      },
      mainWindow: { mode: 'standard', auxiliaryWidthPercent: 45 },
      editor: {
        theme: 'system',
        fontFamily: '',
        codeFontFamily: '',
        fontSize: 15,
        lineHeight: 1.7,
        maxWidth: 900,
        tabSizeProse: 2,
        tabSizeCode: 4,
        autoSaveDebounceMs: 600,
        autoSaveMaxWaitMs: 2000,
        singleClickLink: false,
        showTabCloseButton: false,
      },
      panels: {
        default: 'outline',
        width: 260,
        terminal: {
          visibleByDefault: false,
          width: 360,
          shell: '/bin/zsh',
          idleRecycleMinutes: 30,
          maxConcurrentSessions: 3,
        },
      },
      inbox: { bloatThresholdLines: 500 },
      terminal: { name: 'Terminal.app', bundleId: 'com.apple.Terminal' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: [],
    },
    status: {
      valid: true,
      activeSource: 'file',
      errorLine: null,
      errorColumn: null,
      errorMessage: null,
    },
  };
}

describe('quickbar pin toggle', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockWindowSetQuickBarPinned.mockReset();
  });

  it('calls windowSetQuickBarPinned(true) when quickBar.pinned is false', async () => {
    mockWindowSetQuickBarPinned.mockResolvedValueOnce(true);
    const settingsStore = useSettingsStore();
    settingsStore.snapshot = makeSettingsSnapshot(false);

    const store = useQuickBarStore();
    expect(store.isPinned).toBe(false);

    await store.togglePin();

    expect(mockWindowSetQuickBarPinned).toHaveBeenCalledTimes(1);
    expect(mockWindowSetQuickBarPinned).toHaveBeenCalledWith(true);
    expect(store.errorMessage).toBeNull();
  });

  it('calls windowSetQuickBarPinned(false) when quickBar.pinned is true', async () => {
    mockWindowSetQuickBarPinned.mockResolvedValueOnce(false);
    const settingsStore = useSettingsStore();
    settingsStore.snapshot = makeSettingsSnapshot(true);

    const store = useQuickBarStore();
    expect(store.isPinned).toBe(true);

    await store.togglePin();

    expect(mockWindowSetQuickBarPinned).toHaveBeenCalledTimes(1);
    expect(mockWindowSetQuickBarPinned).toHaveBeenCalledWith(false);
    expect(store.errorMessage).toBeNull();
  });

  it('sets errorMessage when windowSetQuickBarPinned rejects', async () => {
    mockWindowSetQuickBarPinned.mockRejectedValueOnce(new Error('pin failed'));
    const settingsStore = useSettingsStore();
    settingsStore.snapshot = makeSettingsSnapshot(false);

    const store = useQuickBarStore();
    await store.togglePin();

    expect(store.errorMessage).toBe('pin failed');
  });
});
