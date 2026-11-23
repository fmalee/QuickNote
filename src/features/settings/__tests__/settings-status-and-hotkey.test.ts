import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { HotkeyStatus, SettingsSnapshot, UiState } from '@/bindings/types';

const { mockSettingsGet, mockUiStateGet, mockHotkeyUpdate, mockOnSettingsStatus } = vi.hoisted(
  () => ({
    mockSettingsGet: vi.fn(),
    mockUiStateGet: vi.fn(),
    mockHotkeyUpdate: vi.fn(),
    mockOnSettingsStatus: vi.fn(),
  }),
);

vi.mock('@/lib/ipc/commands', () => ({
  settingsGet: mockSettingsGet,
  uiStateGet: mockUiStateGet,
  hotkeyUpdate: mockHotkeyUpdate,
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onSettingsStatus: mockOnSettingsStatus,
  listenEvent: vi.fn(),
  onWindowState: vi.fn(),
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
    panelGroup: { visible: true, activeId: 'outline', width: 260, collapsed: false },
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
});
