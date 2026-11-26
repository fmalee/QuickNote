import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SettingsSnapshot, UiState } from '@/bindings/types';

const {
  mockSaveDialog,
  mockOpenDialog,
  mockSettingsGet,
  mockSettingsExport,
  mockSettingsImport,
  mockUiStateGet,
  mockUiStateSet,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockSaveDialog: vi.fn(),
  mockOpenDialog: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockSettingsExport: vi.fn(),
  mockSettingsImport: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: mockSaveDialog,
  open: mockOpenDialog,
}));

vi.mock('@/lib/ipc/commands', async () => {
  const actual = await vi.importActual<typeof import('@/lib/ipc/commands')>('@/lib/ipc/commands');
  return {
    ...actual,
    settingsGet: mockSettingsGet,
    settingsUpdate: vi.fn(),
    settingsExport: mockSettingsExport,
    settingsImport: mockSettingsImport,
    uiStateGet: mockUiStateGet,
    uiStateSet: mockUiStateSet,
    windowShowMain: vi.fn(),
    externalActionRun: vi.fn(),
    hotkeyUpdate: vi.fn(),
    librarySetRoot: vi.fn(),
    notesList: vi.fn().mockResolvedValue([]),
    noteCreate: vi.fn(),
  };
});

vi.mock('@/lib/ipc/events', () => ({
  onSettingsStatus: mockOnSettingsStatus,
  onWindowState: mockOnWindowState,
  listenEvent: vi.fn(),
}));

import { SettingsView, useSettingsStore } from '@/features/settings';

function makeSettingsSnapshot(): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/Users/demo/Documents/QuickNote',
      hotkey: { quickBar: 'ctrl+alt+space', quickBarBackup: 'ctrl+alt+cmd+space' },
      quickBar: {
        defaultTarget: 'last_active_or_inbox',
        lastActiveTtlHours: 24,
        destroyAfterHiddenMinutes: 10,
        autoCreateMissingSection: false,
        pinned: false,
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
        showTabCloseButton: false,
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
      inbox: { bloatThresholdLines: 500 },
      terminal: { name: 'Terminal.app', bundleId: 'com.apple.Terminal' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: ['/Users/demo/Projects'],
      workspaces: {},
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

function makeUiState(overrides: Partial<UiState> = {}): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    selectedWorkspaceKey: null,
    lastActiveDoc: null,
    workspaceModes: {},
    hotkey: {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
    backupPaths: {
      lastExportPath: null,
      lastImportPath: null,
    },
    ...overrides,
  };
}

describe('M5-03b2 backup dialog & remembered paths', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setActivePinia(createPinia());
    mockSettingsGet.mockResolvedValue(makeSettingsSnapshot());
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockOnSettingsStatus.mockResolvedValue(() => undefined);
    mockOnWindowState.mockResolvedValue(() => undefined);
  });

  it('picks save path via dialog, exports settings, and remembers path in uiState', async () => {
    mockSaveDialog.mockResolvedValueOnce('/Users/demo/Desktop/backup.yaml');
    mockSettingsExport.mockResolvedValueOnce(undefined);

    const store = useSettingsStore();
    await store.initialize();

    const ok = await store.pickAndExportSettings();
    expect(ok).toBe(true);
    expect(mockSaveDialog).toHaveBeenCalledWith({
      defaultPath: undefined,
      filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }],
    });
    expect(mockSettingsExport).toHaveBeenCalledWith({
      destPath: '/Users/demo/Desktop/backup.yaml',
    });
    expect(store.exportBackupPathDraft).toBe('/Users/demo/Desktop/backup.yaml');
    expect(store.backupPaths.lastExportPath).toBe('/Users/demo/Desktop/backup.yaml');
    expect(mockUiStateSet).toHaveBeenCalledWith({
      key: 'backupPaths',
      value: {
        lastExportPath: '/Users/demo/Desktop/backup.yaml',
        lastImportPath: null,
      },
    });
    expect(store.backupSuccess).toContain('/Users/demo/Desktop/backup.yaml');
  });

  it('does nothing when save dialog is cancelled', async () => {
    mockSaveDialog.mockResolvedValueOnce(null);

    const store = useSettingsStore();
    await store.initialize();

    const ok = await store.pickAndExportSettings();
    expect(ok).toBe(false);
    expect(mockSettingsExport).not.toHaveBeenCalled();
    expect(mockUiStateSet).not.toHaveBeenCalled();
    expect(store.exportBackupPathDraft).toBe('');
    expect(store.backupError).toBeNull();
    expect(store.backupSuccess).toBeNull();
  });

  it('picks open path via dialog, imports settings, and remembers path in uiState', async () => {
    const initialUi = makeUiState({
      backupPaths: {
        lastExportPath: '/Users/demo/Desktop/prev-export.yaml',
        lastImportPath: '/Users/demo/Desktop/prev-import.yaml',
      },
    });
    mockUiStateGet.mockResolvedValue(initialUi);
    mockOpenDialog.mockResolvedValueOnce('/Users/demo/Desktop/next-import.yml');
    mockSettingsImport.mockResolvedValueOnce(makeSettingsSnapshot());

    const store = useSettingsStore();
    await store.initialize();

    const ok = await store.pickAndImportSettings();
    expect(ok).toBe(true);
    expect(mockOpenDialog).toHaveBeenCalledWith({
      multiple: false,
      directory: false,
      defaultPath: '/Users/demo/Desktop/prev-import.yaml',
      filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }],
    });
    expect(mockSettingsImport).toHaveBeenCalledWith({
      srcPath: '/Users/demo/Desktop/next-import.yml',
    });
    expect(store.importBackupPathDraft).toBe('/Users/demo/Desktop/next-import.yml');
    expect(store.backupPaths.lastImportPath).toBe('/Users/demo/Desktop/next-import.yml');
    expect(mockUiStateSet).toHaveBeenCalledWith({
      key: 'backupPaths',
      value: {
        lastExportPath: '/Users/demo/Desktop/prev-export.yaml',
        lastImportPath: '/Users/demo/Desktop/next-import.yml',
      },
    });
  });

  it('does nothing when open dialog is cancelled', async () => {
    mockOpenDialog.mockResolvedValueOnce(null);

    const store = useSettingsStore();
    await store.initialize();

    const ok = await store.pickAndImportSettings();
    expect(ok).toBe(false);
    expect(mockSettingsImport).not.toHaveBeenCalled();
    expect(mockUiStateSet).not.toHaveBeenCalled();
    expect(store.importBackupPathDraft).toBe('');
    expect(store.backupError).toBeNull();
    expect(store.backupSuccess).toBeNull();
  });

  it('renders remembered backup paths or empty placeholders in SettingsView', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useSettingsStore();
    await store.initialize();

    const emptyApp = createSSRApp(SettingsView);
    emptyApp.use(pinia);
    const emptyHtml = await renderToString(emptyApp);
    expect(emptyHtml).toContain('尚未导出');
    expect(emptyHtml).toContain('尚未导入');

    mockUiStateGet.mockResolvedValueOnce(
      makeUiState({
        backupPaths: {
          lastExportPath: '/Users/demo/Desktop/exported.yaml',
          lastImportPath: '/Users/demo/Desktop/imported.yml',
        },
      }),
    );
    await store.refreshUiState();

    const filledApp = createSSRApp(SettingsView);
    filledApp.use(pinia);
    const filledHtml = await renderToString(filledApp);
    expect(filledHtml).toContain('/Users/demo/Desktop/exported.yaml');
    expect(filledHtml).toContain('/Users/demo/Desktop/imported.yml');
  });
});
