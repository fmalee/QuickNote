import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SettingsSnapshot, UiState, WorkspaceConfig } from '@/bindings/types';

const {
  mockSettingsGet,
  mockSettingsUpdate,
  mockUiStateGet,
  mockExternalActionRun,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockSettingsGet: vi.fn(),
  mockSettingsUpdate: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockExternalActionRun: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  settingsGet: mockSettingsGet,
  settingsUpdate: mockSettingsUpdate,
  uiStateGet: mockUiStateGet,
  externalActionRun: mockExternalActionRun,
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
      terminal: { name: 'iTerm2', bundleId: 'com.googlecode.iterm2' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: ['/Users/demo/Sites'],
      workspaces: {
        quicknote: {
          key: 'quicknote',
          name: 'QuickNote',
          root: '/Users/demo/Sites/QuickNote',
          openDir: '/Users/demo/Sites/QuickNote',
          codeWorkspace: null,
          terminalDir: null,
          defaultEditor: 'vscode',
          defaultEntryDoc: 'projects/quicknote.md',
        },
      },
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

describe('settings-workspaces-and-external', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnSettingsStatus.mockResolvedValue(() => {});
    mockOnWindowState.mockResolvedValue(() => {});
    mockUiStateGet.mockResolvedValue(makeUiState());
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

  it('renders workspaces and external panels in SettingsView and patches settings payloads', async () => {
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
    expect(wsHtml).toContain('data-testid="workspace-row-quicknote"');
    expect(wsHtml).toContain('data-testid="workspace-new-btn"');

    store.activeSection = 'external';
    const extApp = createSSRApp(SettingsView);
    extApp.use(pinia);
    const extHtml = await renderToString(extApp);
    expect(extHtml).toContain('data-testid="external-panel"');
    expect(extHtml).toContain('data-testid="terminal-preset-ghostty"');
    expect(extHtml).toContain('data-testid="editor-vscode-bundle-id-input"');
    expect(extHtml).toContain('data-testid="search-root-item-0"');

    // Patch workspaces (add + update + delete)
    const nextWs: Record<string, WorkspaceConfig> = {
      ...snapshot.settings.workspaces,
      demo: {
        key: 'demo',
        name: 'Demo Repo',
        root: '/Users/demo/Sites/Demo',
        openDir: '/Users/demo/Sites/Demo',
        codeWorkspace: '/Users/demo/Sites/Demo/demo.code-workspace',
        terminalDir: null,
        defaultEditor: 'sublime',
        defaultEntryDoc: null,
      },
    };
    const updatedSnapshot: SettingsSnapshot = {
      ...snapshot,
      settings: {
        ...snapshot.settings,
        workspaces: nextWs,
        terminal: { name: 'Ghostty', bundleId: 'com.mitchellh.ghostty' },
        searchRoots: ['/Users/demo/Sites', '/Users/demo/Projects'],
      },
    };
    mockSettingsUpdate.mockResolvedValue(updatedSnapshot);

    const wsOk = await store.patchSettings({ workspaces: nextWs });
    expect(wsOk).toBe(true);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({ workspaces: nextWs });

    const termOk = await store.patchSettings({
      terminal: { name: 'Ghostty', bundleId: 'com.mitchellh.ghostty' },
    });
    expect(termOk).toBe(true);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({
      terminal: { name: 'Ghostty', bundleId: 'com.mitchellh.ghostty' },
    });

    const rootsOk = await store.patchSettings({
      searchRoots: ['/Users/demo/Sites', '/Users/demo/Projects'],
    });
    expect(rootsOk).toBe(true);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({
      searchRoots: ['/Users/demo/Sites', '/Users/demo/Projects'],
    });
  });

  it('blocks saving workspaces and external settings when settings.yaml is invalid', async () => {
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
