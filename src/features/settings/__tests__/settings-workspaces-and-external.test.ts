import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  DirectoryWorkspaceConfigEntry,
  ExternalBookmark,
  SettingsSnapshot,
  UiState,
} from '@/bindings/types';

const {
  mockSettingsGet,
  mockSettingsUpdate,
  mockUiStateGet,
  mockExternalActionRun,
  mockUpsertExternalBookmark,
  mockRemoveExternalBookmark,
  mockListDirectoryWorkspaceConfigs,
  mockSetDirectoryWorkspaceConfig,
  mockNotesGetTreeAndTags,
  mockIndexRebuild,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockSettingsGet: vi.fn(),
  mockSettingsUpdate: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockExternalActionRun: vi.fn(),
  mockUpsertExternalBookmark: vi.fn(),
  mockRemoveExternalBookmark: vi.fn(),
  mockListDirectoryWorkspaceConfigs: vi.fn(),
  mockSetDirectoryWorkspaceConfig: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockIndexRebuild: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  settingsGet: mockSettingsGet,
  settingsUpdate: mockSettingsUpdate,
  uiStateGet: mockUiStateGet,
  externalActionRun: mockExternalActionRun,
  upsertExternalBookmark: mockUpsertExternalBookmark,
  removeExternalBookmark: mockRemoveExternalBookmark,
  listDirectoryWorkspaceConfigs: mockListDirectoryWorkspaceConfigs,
  setDirectoryWorkspaceConfig: mockSetDirectoryWorkspaceConfig,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  indexRebuild: mockIndexRebuild,
  hotkeyUpdate: vi.fn(),
  librarySetRoot: vi.fn(),
  notesList: vi.fn().mockResolvedValue([]),
  noteCreate: vi.fn(),
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onSettingsStatus: mockOnSettingsStatus,
  onWindowState: mockOnWindowState,
  listenEvent: vi.fn(),
}));

import {
  SETTINGS_READONLY_ERROR,
  SettingsErrorBanner,
  SettingsView,
  useSettingsStore,
} from '@/features/settings';
import { openSettingsYamlForRepair, revealSettingsYamlInFinder } from '@/features/settings/api';
import {
  indexRebuild,
  listDirectoryWorkspaceConfigs,
  removeExternalBookmark,
  setDirectoryWorkspaceConfig,
  upsertExternalBookmark,
} from '@/lib/ipc/commands';

function makeSettingsSnapshot(valid = true): SettingsSnapshot {
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
          width: 360,
          shell: '/bin/zsh',
          idleRecycleMinutes: 30,
          maxConcurrentSessions: 3,
        },
      },
      inbox: { bloatThresholdLines: 500 },
      terminal: { name: 'iTerm2', bundleId: 'com.googlecode.iterm2' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: [],
      maxActiveEditorRenders: 5,
      scanHiddenFiles: true,
      externalBookmarks: [
        {
          path: '/Users/demo/Sites',
          kind: 'directory',
          alias: '本地项目',
          pinned: true,
          lastOpenedAt: null,
        },
        {
          path: '/Users/demo/.zshrc',
          kind: 'file',
          alias: 'Zsh 配置',
          pinned: false,
          lastOpenedAt: null,
        },
      ],
    },
    status: valid
      ? {
          valid: true,
          activeSource: 'settings.yaml',
          errorLine: null,
          errorColumn: null,
          errorMessage: null,
        }
      : {
          valid: false,
          activeSource: 'fallback_default',
          errorLine: 8,
          errorColumn: 5,
          errorMessage: 'invalid yaml syntax',
        },
  };
}

function makeUiState(): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    lastActiveDoc: null,
    hotkey: {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
  };
}

describe('settings-workspaces-and-external', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnSettingsStatus.mockResolvedValue(() => {});
    mockOnWindowState.mockResolvedValue(() => {});
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockListDirectoryWorkspaceConfigs.mockResolvedValue([]);
    mockNotesGetTreeAndTags.mockResolvedValue({
      inboxCount: 0,
      dirs: [],
      tags: [],
    });
  });

  it('renders repair & reveal buttons in SettingsErrorBanner when invalid and calls externalActionRun', async () => {
    const invalid = makeSettingsSnapshot(false);
    const html = await renderToString(
      createSSRApp(SettingsErrorBanner, {
        status: invalid.status,
      }),
    );

    expect(html).toContain('data-testid="settings-error-banner"');
    expect(html).toContain('data-testid="open-settings-yaml-btn"');
    expect(html).toContain('打开 settings.yaml 修复');
    expect(html).toContain('data-testid="reveal-settings-yaml-btn"');
    expect(html).toContain('在 Finder 中显示');

    mockExternalActionRun.mockResolvedValue(undefined);
    await openSettingsYamlForRepair();
    expect(mockExternalActionRun).toHaveBeenCalledWith({
      id: 'external.default',
      absPath: '$SETTINGS_YAML',
      dirPath: null,
    });

    await revealSettingsYamlInFinder();
    expect(mockExternalActionRun).toHaveBeenCalledWith({
      id: 'external.reveal',
      absPath: '$SETTINGS_YAML',
      dirPath: null,
    });
  });

  it('renders directory workspaces, external bookmarks, and library memory/cache controls', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const snapshot = makeSettingsSnapshot(true);
    mockSettingsGet.mockResolvedValueOnce(snapshot);

    const store = useSettingsStore();
    await store.initialize();

    store.activeSection = 'workspaces';
    const wsApp = createSSRApp(SettingsView);
    wsApp.use(pinia);
    const wsHtml = await renderToString(wsApp);
    expect(wsHtml).toContain('data-testid="workspaces-panel"');
    expect(wsHtml).toContain('data-testid="workspace-new-btn"');
    expect(wsHtml).toContain('.quicknote.yaml');
    expect(wsHtml).not.toContain('新增工作区');

    store.activeSection = 'external';
    const extApp = createSSRApp(SettingsView);
    extApp.use(pinia);
    const extHtml = await renderToString(extApp);
    expect(extHtml).toContain('data-testid="external-panel"');
    expect(extHtml).toContain('data-testid="terminal-preset-ghostty"');
    expect(extHtml).toContain('data-testid="editor-vscode-bundle-id-input"');
    expect(extHtml).toContain('data-testid="external-bookmarks-card"');
    expect(extHtml).toContain('data-testid="external-scan-hidden-checkbox"');
    expect(extHtml).toContain('data-testid="external-bookmark-item-0"');
    expect(extHtml).toContain('data-testid="external-bookmark-item-1"');
    expect(extHtml).not.toContain('Spotlight');

    store.activeSection = 'library';
    const libApp = createSSRApp(SettingsView);
    libApp.use(pinia);
    const libHtml = await renderToString(libApp);
    expect(libHtml).toContain('data-testid="library-memory-and-cache-card"');
    expect(libHtml).toContain('data-testid="max-active-editor-renders-input"');
    expect(libHtml).toContain('data-testid="rebuild-index-btn"');

    // Directory workspace IPC calls (.quicknote.yaml)
    const dirEntries: DirectoryWorkspaceConfigEntry[] = [
      {
        vaultRelDir: 'projects/quicknote',
        config: {
          projectRoot: '/Users/demo/Sites/QuickNote',
          vscodeWorkspacePath: '/Users/demo/Sites/QuickNote/app.code-workspace',
          defaultTerminalCwd: '/Users/demo/Sites/QuickNote',
        },
        mtimeMs: 1700000000000,
        parseError: null,
      },
    ];
    mockListDirectoryWorkspaceConfigs.mockResolvedValueOnce(dirEntries);
    const listed = await listDirectoryWorkspaceConfigs();
    expect(listed).toEqual(dirEntries);

    mockSetDirectoryWorkspaceConfig.mockResolvedValue(undefined);
    await setDirectoryWorkspaceConfig({
      vaultRelDir: 'projects/quicknote',
      config: {
        projectRoot: '/Users/demo/Sites/QuickNote',
        vscodeWorkspacePath: '/Users/demo/Sites/QuickNote/app.code-workspace',
        defaultTerminalCwd: '/Users/demo/Sites/QuickNote',
      },
    });
    expect(mockSetDirectoryWorkspaceConfig).toHaveBeenCalledWith({
      vaultRelDir: 'projects/quicknote',
      config: {
        projectRoot: '/Users/demo/Sites/QuickNote',
        vscodeWorkspacePath: '/Users/demo/Sites/QuickNote/app.code-workspace',
        defaultTerminalCwd: '/Users/demo/Sites/QuickNote',
      },
    });

    await setDirectoryWorkspaceConfig({
      vaultRelDir: 'projects/quicknote',
      config: null,
    });
    expect(mockSetDirectoryWorkspaceConfig).toHaveBeenCalledWith({
      vaultRelDir: 'projects/quicknote',
      config: null,
    });

    // External bookmark IPC calls
    const newBookmark: ExternalBookmark = {
      path: '/opt/homebrew/etc',
      kind: 'directory',
      alias: 'Homebrew',
      pinned: false,
      lastOpenedAt: null,
    };
    const nextBookmarks: ExternalBookmark[] = [
      ...(snapshot.settings.externalBookmarks ?? []),
      newBookmark,
    ];
    mockUpsertExternalBookmark.mockResolvedValueOnce(nextBookmarks);
    const added = await upsertExternalBookmark(newBookmark);
    expect(added).toHaveLength(3);

    mockRemoveExternalBookmark.mockResolvedValueOnce(snapshot.settings.externalBookmarks ?? []);
    const removed = await removeExternalBookmark('/opt/homebrew/etc');
    expect(removed).toHaveLength(2);

    // Patch scanHiddenFiles & maxActiveEditorRenders & indexRebuild
    const updatedSnapshot: SettingsSnapshot = {
      ...snapshot,
      settings: {
        ...snapshot.settings,
        terminal: { name: 'Ghostty', bundleId: 'com.mitchellh.ghostty' },
        scanHiddenFiles: false,
        maxActiveEditorRenders: 10,
      },
    };
    mockSettingsUpdate.mockResolvedValue(updatedSnapshot);

    const hiddenOk = await store.patchSettings({ scanHiddenFiles: false });
    expect(hiddenOk).toBe(true);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({ scanHiddenFiles: false });

    const quotaOk = await store.patchSettings({ maxActiveEditorRenders: 10 });
    expect(quotaOk).toBe(true);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({ maxActiveEditorRenders: 10 });

    mockIndexRebuild.mockResolvedValueOnce({
      phase: 'ready',
      notesCount: 42,
      ftsDegraded: false,
      lastCompletedAt: 1700000000000,
    });
    const rebuilt = await indexRebuild();
    expect(rebuilt.notesCount).toBe(42);
  });

  it('blocks saving external settings when settings.yaml is invalid', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    mockSettingsGet.mockResolvedValueOnce(makeSettingsSnapshot(false));
    const store = useSettingsStore();
    await store.initialize();

    expect(store.isSettingsInvalid).toBe(true);

    store.activeSection = 'workspaces';
    const wsApp = createSSRApp(SettingsView);
    wsApp.use(pinia);
    const wsHtml = await renderToString(wsApp);
    expect(wsHtml).toContain('data-testid="workspaces-readonly-banner"');

    store.activeSection = 'external';
    const extApp = createSSRApp(SettingsView);
    extApp.use(pinia);
    const extHtml = await renderToString(extApp);
    expect(extHtml).toContain('data-testid="external-readonly-banner"');

    const ok = await store.patchSettings({
      terminal: { name: 'Ghostty', bundleId: 'com.mitchellh.ghostty' },
    });
    expect(ok).toBe(false);
    expect(store.saveError).toBe(SETTINGS_READONLY_ERROR);
    expect(mockSettingsUpdate).not.toHaveBeenCalled();
  });
});
