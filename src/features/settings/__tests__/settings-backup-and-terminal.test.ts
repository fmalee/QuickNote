import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SettingsSnapshot, UiState } from '@/bindings/types';

const {
  mockSettingsGet,
  mockSettingsUpdate,
  mockSettingsExport,
  mockSettingsImport,
  mockUiStateGet,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockSettingsGet: vi.fn(),
  mockSettingsUpdate: vi.fn(),
  mockSettingsExport: vi.fn(),
  mockSettingsImport: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  settingsGet: mockSettingsGet,
  settingsUpdate: mockSettingsUpdate,
  settingsExport: mockSettingsExport,
  settingsImport: mockSettingsImport,
  uiStateGet: mockUiStateGet,
  externalActionRun: vi.fn(),
  hotkeyUpdate: vi.fn(),
  librarySetRoot: vi.fn(),
  notesList: vi.fn().mockResolvedValue([]),
  noteCreate: vi.fn(),
  isAppError: (err: unknown): err is { code: string; message: string; detail?: string | null } =>
    typeof err === 'object' && err !== null && 'code' in err && 'message' in err,
}));

vi.mock('@/lib/ipc/events', () => ({
  onSettingsStatus: mockOnSettingsStatus,
  onWindowState: mockOnWindowState,
  listenEvent: vi.fn(),
}));

import { SettingsExternalPanel, SettingsView, useSettingsStore } from '@/features/settings';

function makeSettingsSnapshot(
  overrides: Partial<SettingsSnapshot['settings']> = {},
): SettingsSnapshot {
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
          width: 380,
          shell: '/bin/zsh',
          idleRecycleMinutes: 30,
          maxConcurrentSessions: 3,
        },
      },
      inbox: {
        bloatThresholdLines: 500,
      },
      terminal: {
        name: 'Terminal.app',
        bundleId: 'com.apple.Terminal',
      },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: ['/Users/demo/Projects'],
      workspaces: {},
      ...overrides,
    },
    status: {
      valid: true,
      activeSource: 'user_file',
      errorLine: null,
      errorColumn: null,
      errorMessage: null,
    },
  };
}

function makeUiState(): UiState {
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
    hotkey: {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
  };
}

describe('M4-04 settings backup export/import & embedded terminal panel config', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setActivePinia(createPinia());
    mockSettingsGet.mockResolvedValue(makeSettingsSnapshot());
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockOnSettingsStatus.mockResolvedValue(() => undefined);
    mockOnWindowState.mockResolvedValue(() => undefined);
  });

  it('exports settings via settings_export and shows success feedback', async () => {
    mockSettingsExport.mockResolvedValue(undefined);
    const store = useSettingsStore();
    await store.initialize();

    const ok = await store.exportSettings('~/Desktop/quicknote-settings-backup.yaml');
    expect(ok).toBe(true);
    expect(mockSettingsExport).toHaveBeenCalledWith({
      destPath: '~/Desktop/quicknote-settings-backup.yaml',
    });
    expect(store.backupError).toBeNull();
    expect(store.backupSuccess).toContain('~/Desktop/quicknote-settings-backup.yaml');

    const app = createSSRApp(SettingsView);
    app.use(createPinia());
    const html = await renderToString(app);
    expect(html).toContain('data-testid="settings-backup-card"');
    expect(html).toContain('data-testid="settings-export-btn"');
    expect(html).toContain('data-testid="settings-import-btn"');
  });

  it('imports valid settings to hot-update store and preserves current settings on invalid YAML error', async () => {
    const store = useSettingsStore();
    await store.initialize();
    expect(store.snapshot?.settings.panels.terminal.shell).toBe('/bin/zsh');

    // 1. Invalid import returns error with line/col detail and does not corrupt current snapshot
    mockSettingsImport.mockRejectedValueOnce({
      code: 'validation',
      message: '配置文件校验失败',
      detail: 'line 12, col 4: unknown field `fooBar`',
    });

    const failed = await store.importSettings('/tmp/broken.yaml');
    expect(failed).toBe(false);
    expect(store.backupError).toContain('line 12, col 4: unknown field `fooBar`');
    expect(store.backupSuccess).toBeNull();
    expect(store.snapshot?.settings.panels.terminal.shell).toBe('/bin/zsh');

    // 2. Valid import hot-updates store snapshot and clears error
    const updatedSnapshot = makeSettingsSnapshot({
      panels: {
        default: 'outline',
        width: 260,
        terminal: {
          visibleByDefault: true,
          width: 420,
          shell: '/bin/bash',
          idleRecycleMinutes: 15,
          maxConcurrentSessions: 5,
        },
      },
    });
    mockSettingsImport.mockResolvedValueOnce(updatedSnapshot);

    const succeeded = await store.importSettings('/tmp/valid-backup.yaml');
    expect(succeeded).toBe(true);
    expect(store.backupError).toBeNull();
    expect(store.backupSuccess).toContain('导入');
    expect(store.snapshot?.settings.panels.terminal.shell).toBe('/bin/bash');
    expect(store.snapshot?.settings.panels.terminal.maxConcurrentSessions).toBe(5);
  });

  it('renders embedded terminal panel config card and triggers settings_update on patch', async () => {
    const updatedSnapshot = makeSettingsSnapshot({
      panels: {
        default: 'outline',
        width: 260,
        terminal: {
          visibleByDefault: false,
          width: 380,
          shell: '/opt/homebrew/bin/fish',
          idleRecycleMinutes: 0,
          maxConcurrentSessions: 4,
        },
      },
    });
    mockSettingsUpdate.mockResolvedValue(updatedSnapshot);

    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useSettingsStore();
    await store.initialize();

    const app = createSSRApp(SettingsExternalPanel);
    app.use(pinia);
    const html = await renderToString(app);
    expect(html).toContain('data-testid="settings-terminal-panel-card"');
    expect(html).toContain('data-testid="embedded-terminal-shell-input"');
    expect(html).toContain('data-testid="embedded-terminal-idle-input"');
    expect(html).toContain('data-testid="embedded-terminal-max-sessions-input"');

    const ok = await store.patchSettings({
      panels: {
        terminal: {
          shell: '/opt/homebrew/bin/fish',
          idleRecycleMinutes: 0,
          maxConcurrentSessions: 4,
        },
      },
    });
    expect(ok).toBe(true);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({
      panels: {
        terminal: {
          shell: '/opt/homebrew/bin/fish',
          idleRecycleMinutes: 0,
          maxConcurrentSessions: 4,
        },
      },
    });
    expect(store.snapshot?.settings.panels.terminal.shell).toBe('/opt/homebrew/bin/fish');
    expect(store.snapshot?.settings.panels.terminal.idleRecycleMinutes).toBe(0);
    expect(store.snapshot?.settings.panels.terminal.maxConcurrentSessions).toBe(4);
  });
});
