import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { HotkeyStatus, SettingsSnapshot, UiState } from '@/bindings/types';

const {
  mockSettingsGet,
  mockUiStateGet,
  mockHotkeyUpdate,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockSettingsGet: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockHotkeyUpdate: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  settingsGet: mockSettingsGet,
  uiStateGet: mockUiStateGet,
  hotkeyUpdate: mockHotkeyUpdate,
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onSettingsStatus: mockOnSettingsStatus,
  listenEvent: vi.fn(),
  onWindowState: mockOnWindowState,
}));

import { SettingsErrorBanner, SettingsView, useSettingsStore } from '@/features/settings';

function makeInvalidSettingsSnapshot(): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/Users/demo/Documents/QuickNote',
      hotkey: { quickBar: 'ctrl+alt+space', quickBarBackup: 'ctrl+alt+cmd+space' },
      quickBar: {
        defaultTarget: 'last_active_or_inbox',
        lastActiveTtlHours: 24,
        destroyAfterHiddenMinutes: 10,
        autoCreateMissingSection: false,
      },
      mainWindow: { mode: 'standard', auxiliaryWidthPercent: 45 },
      editor: {
        theme: 'system',
        fontSize: 15,
        lineHeight: 1.7,
        maxWidth: 900,
        tabSizeProse: 2,
        tabSizeCode: 4,
        autoSaveDebounceMs: 600,
        autoSaveMaxWaitMs: 2000,
        singleClickLink: false,
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
      workspaces: {},
    },
    status: {
      valid: false,
      activeSource: 'fallback_default',
      errorLine: 14,
      errorColumn: 3,
      errorMessage: 'unknown field `bad_key`',
    },
  };
}

function makeUiState(hotkey: HotkeyStatus): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    selectedWorkspaceKey: null,
    lastActiveDoc: null,
    workspaceModes: {},
    hotkey,
  };
}

describe('settings-status-and-hotkey', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnSettingsStatus.mockResolvedValue(() => {});
    mockOnWindowState.mockResolvedValue(() => {});
  });

  it('renders exact errorLine and errorColumn when settings.yaml is invalid', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const snapshot = makeInvalidSettingsSnapshot();
    mockSettingsGet.mockResolvedValueOnce(snapshot);
    mockUiStateGet.mockResolvedValueOnce(
      makeUiState({
        combo: 'ctrl+alt+space',
        backupCombo: 'ctrl+alt+cmd+space',
        selfCheck: 'ok',
      }),
    );

    const store = useSettingsStore();
    await store.initialize();

    expect(store.isSettingsInvalid).toBe(true);

    const bannerApp = createSSRApp(SettingsErrorBanner, { status: snapshot.status });
    const bannerHtml = await renderToString(bannerApp);
    expect(bannerHtml).toContain('第 14 行');
    expect(bannerHtml).toContain('第 3 列');
    expect(bannerHtml).toContain('unknown field `bad_key`');
  });

  it('shows conflict alert and calls hotkey_update to switch to ctrl+alt+cmd+space', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    mockSettingsGet.mockResolvedValueOnce(makeInvalidSettingsSnapshot());
    mockUiStateGet.mockResolvedValueOnce(
      makeUiState({
        combo: 'ctrl+alt+space',
        backupCombo: 'ctrl+alt+cmd+space',
        selfCheck: 'conflict',
      }),
    );
    mockHotkeyUpdate.mockResolvedValueOnce({
      combo: 'ctrl+alt+cmd+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    });

    const store = useSettingsStore();
    await store.initialize();

    expect(store.isHotkeyConflict).toBe(true);

    const viewApp = createSSRApp(SettingsView);
    viewApp.use(pinia);
    const html = await renderToString(viewApp);
    expect(html).toContain('data-testid="hotkey-conflict-banner"');
    expect(html).toContain('一键切至备用热键');

    const updated = await store.switchToBackupHotkey();
    expect(mockHotkeyUpdate).toHaveBeenCalledWith({ combo: 'ctrl+alt+cmd+space' });
    expect(updated?.combo).toBe('ctrl+alt+cmd+space');
    expect(store.isHotkeyConflict).toBe(false);
  });

  it('refreshes settings snapshot and theme via settingsGet when settings:status reports valid=true', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    let capturedStatusHandler: ((status: SettingsSnapshot['status']) => void) | undefined;
    mockOnSettingsStatus.mockImplementationOnce(
      async (handler: (status: SettingsSnapshot['status']) => void) => {
        capturedStatusHandler = handler;
        return () => {};
      },
    );

    const initialInvalid = makeInvalidSettingsSnapshot();
    const fixedSnapshot: SettingsSnapshot = {
      ...initialInvalid,
      settings: {
        ...initialInvalid.settings,
        editor: {
          ...initialInvalid.settings.editor,
          theme: 'dark',
          fontSize: 18,
        },
      },
      status: {
        valid: true,
        activeSource: 'file',
        errorLine: null,
        errorColumn: null,
        errorMessage: null,
      },
    };

    mockSettingsGet.mockResolvedValueOnce(initialInvalid).mockResolvedValueOnce(fixedSnapshot);
    mockUiStateGet.mockResolvedValueOnce(
      makeUiState({
        combo: 'ctrl+alt+space',
        backupCombo: 'ctrl+alt+cmd+space',
        selfCheck: 'pending',
      }),
    );

    const store = useSettingsStore();
    await store.initialize();

    expect(store.isSettingsInvalid).toBe(true);
    expect(store.snapshot?.settings.editor.fontSize).toBe(15);
    expect(mockSettingsGet).toHaveBeenCalledTimes(1);

    // Invalid status event must NOT trigger an extra settingsGet().
    capturedStatusHandler?.({
      valid: false,
      activeSource: 'last_valid',
      errorLine: 3,
      errorColumn: 1,
      errorMessage: 'syntax error',
    });
    await Promise.resolve();
    expect(mockSettingsGet).toHaveBeenCalledTimes(1);

    // Valid status event must call settingsGet() and update memory settings & theme.
    capturedStatusHandler?.({
      valid: true,
      activeSource: 'file',
      errorLine: null,
      errorColumn: null,
      errorMessage: null,
    });
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(mockSettingsGet).toHaveBeenCalledTimes(2);
    expect(store.isSettingsInvalid).toBe(false);
    expect(store.snapshot?.settings.editor.fontSize).toBe(18);
    expect(store.themePreference).toBe('dark');

    store.dispose();
  });

  it('refreshes hotkey selfCheck from pending to ok on window:state and supports switching back to primary hotkey', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    let capturedWindowHandler:
      ((state: { windowId: string; visible: boolean }) => void) | undefined;
    mockOnWindowState.mockImplementationOnce(
      async (handler: (state: { windowId: string; visible: boolean }) => void) => {
        capturedWindowHandler = handler;
        return () => {};
      },
    );

    mockSettingsGet.mockResolvedValueOnce(makeInvalidSettingsSnapshot());
    mockUiStateGet
      .mockResolvedValueOnce(
        makeUiState({
          combo: 'ctrl+alt+space',
          backupCombo: 'ctrl+alt+cmd+space',
          selfCheck: 'pending',
        }),
      )
      .mockResolvedValueOnce(
        makeUiState({
          combo: 'ctrl+alt+space',
          backupCombo: 'ctrl+alt+cmd+space',
          selfCheck: 'ok',
        }),
      );

    const store = useSettingsStore();
    await store.initialize();
    expect(store.hotkeyStatus?.selfCheck).toBe('pending');
    expect(store.isUsingBackupHotkey).toBe(false);

    // Quick Bar opens via hotkey -> window:state refreshes hotkeyStatus to 'ok'.
    capturedWindowHandler?.({ windowId: 'quickbar', visible: true });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(store.hotkeyStatus?.selfCheck).toBe('ok');

    // Switch to backup hotkey -> isUsingBackupHotkey becomes true and primary restore button renders.
    mockHotkeyUpdate.mockResolvedValueOnce({
      combo: 'ctrl+alt+cmd+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'pending',
    });
    await store.switchToBackupHotkey();
    expect(store.isUsingBackupHotkey).toBe(true);

    const viewApp = createSSRApp(SettingsView);
    viewApp.use(pinia);
    const html = await renderToString(viewApp);
    expect(html).toContain('data-testid="switch-primary-hotkey-btn"');
    expect(html).toContain('切回默认主热键 ⌃⌥Space');

    // Switch back to primary hotkey ctrl+alt+space.
    mockHotkeyUpdate.mockResolvedValueOnce({
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'pending',
    });
    const restored = await store.switchToPrimaryHotkey();
    expect(mockHotkeyUpdate).toHaveBeenLastCalledWith({ combo: 'ctrl+alt+space' });
    expect(restored?.combo).toBe('ctrl+alt+space');
    expect(store.isUsingBackupHotkey).toBe(false);

    store.dispose();
  });
});
