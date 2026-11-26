import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SettingsSnapshot, UiState, WindowState } from '@/bindings/types';

const {
  mockUiStateGet,
  mockUiStateSet,
  mockWindowShowMain,
  mockAppSetMenuAccelerators,
  mockSettingsGet,
  mockSettingsUpdate,
  mockOnWindowState,
  mockOnMenuAction,
  mockOnToastAppendUndo,
  mockOnSettingsStatus,
  windowStateListeners,
} = vi.hoisted(() => {
  const listeners: Array<(state: WindowState) => void> = [];
  return {
    mockUiStateGet: vi.fn(),
    mockUiStateSet: vi.fn(),
    mockWindowShowMain: vi.fn(),
    mockAppSetMenuAccelerators: vi.fn(),
    mockSettingsGet: vi.fn(),
    mockSettingsUpdate: vi.fn(),
    mockOnWindowState: vi.fn(),
    mockOnMenuAction: vi.fn(),
    mockOnToastAppendUndo: vi.fn(),
    mockOnSettingsStatus: vi.fn(),
    windowStateListeners: listeners,
  };
});

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: () => false,
  appSetMenuAccelerators: mockAppSetMenuAccelerators,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  windowShowMain: mockWindowShowMain,
  settingsGet: mockSettingsGet,
  settingsUpdate: mockSettingsUpdate,
  settingsExport: vi.fn(),
  settingsImport: vi.fn(),
  libraryGetInfo: vi.fn(),
  notesList: vi.fn().mockResolvedValue([]),
  notesGetTreeAndTags: vi
    .fn()
    .mockResolvedValue({ tree: [], tags: [], recent: [], workspaces: [] }),
  noteRead: vi.fn(),
  noteWrite: vi.fn(),
  notesGetRelated: vi.fn().mockResolvedValue([]),
  commandsSearch: vi.fn().mockResolvedValue([]),
  searchNotesAndFiles: vi.fn().mockResolvedValue([]),
  externalActionsResolve: vi.fn().mockResolvedValue([]),
  terminalListSessions: vi.fn().mockResolvedValue([]),
  terminalSetVisibility: vi.fn().mockResolvedValue(undefined),
  hotkeyUpdate: vi.fn(),
}));

vi.mock('@/lib/ipc/events', () => ({
  onWindowState: mockOnWindowState,
  onMenuAction: mockOnMenuAction,
  onToastAppendUndo: mockOnToastAppendUndo,
  onSettingsStatus: mockOnSettingsStatus,
  onFsExternalChange: vi.fn().mockResolvedValue(() => undefined),
  onIndexProgress: vi.fn().mockResolvedValue(() => undefined),
  onTerminalSessionState: vi.fn().mockResolvedValue(() => undefined),
  onTerminalExit: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock('@/features/notes', () => ({
  NewNoteTemplateModal: { template: '<div />' },
  NotesSidebar: {
    name: 'NotesSidebar',
    props: ['collapsed', 'overlayDrawer'],
    template:
      '<aside data-testid="notes-sidebar" :data-collapsed="String(collapsed)" :data-drawer="String(overlayDrawer)" />',
  },
  useNotesStore: () => ({
    initialize: vi.fn().mockResolvedValue(undefined),
    refreshAll: vi.fn().mockResolvedValue(undefined),
    refreshPinnedExternalFiles: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock('@/features/onboarding', () => ({
  OnboardingView: { template: '<div />' },
  useOnboardingStore: () => ({
    isLocked: false,
    libraryInfo: { root: '/tmp/vault', valid: true },
    refreshLibraryInfo: vi.fn().mockResolvedValue({ root: '/tmp/vault', valid: true }),
  }),
}));

vi.mock('@/features/editor', () => ({
  DocHeaderBar: { template: '<div />' },
  EditorConflictBanner: { template: '<div />' },
  EditorMergeView: { template: '<div />' },
  EditorTabBar: { template: '<div />' },
  ExportPdfModal: { template: '<div />' },
  EXTERNAL_ACTION_SPECS: [],
  InboxBloatBanner: { template: '<div />' },
  InboxExtractModal: { template: '<div />' },
  MarkdownEditor: { template: '<div />' },
  copyMarkdownAsRichText: vi.fn(),
  useEditorTabsStore: () => ({
    activeTab: null,
    activeDoc: null,
    activeTopLine: 1,
    restoreTabsFromUiState: vi.fn().mockResolvedValue(undefined),
    handleMainWindowShown: vi.fn().mockResolvedValue(undefined),
    handleQuickBarAppendEvent: vi.fn().mockResolvedValue(undefined),
    openNoteByAbsPath: vi.fn().mockResolvedValue(undefined),
    createAndOpenNote: vi.fn().mockResolvedValue(undefined),
    saveActiveTab: vi.fn().mockResolvedValue(undefined),
    closeTab: vi.fn(),
    reopenLastClosedTab: vi.fn().mockResolvedValue(undefined),
  }),
  useExternalActions: () => ({
    feedback: null,
    refreshForDoc: vi.fn().mockResolvedValue(undefined),
    isActionEnabled: () => false,
    executeAction: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock('@/features/commands', () => ({
  CommandBookPalette: { template: '<div />' },
  useCommandPalette: () => ({
    refresh: vi.fn(),
  }),
}));

vi.mock('@/features/quickopen', () => ({
  PENDING_OPEN_FILE_STORAGE_KEY: 'quicknote:pending-open-file',
  QuickOpenPalette: { template: '<div />' },
  consumePendingOpenFile: () => null,
  useQuickOpen: () => ({
    refresh: vi.fn(),
  }),
}));

vi.mock('@/features/terminal', () => ({
  TerminalPanel: { template: '<div data-testid="terminal-panel" />' },
  useTerminalStore: () => ({
    hasSelection: false,
    hasAnySession: false,
    syncPanelVisibility: vi.fn().mockResolvedValue(undefined),
    copyActiveSelection: vi.fn().mockResolvedValue(undefined),
  }),
}));

function makeSnapshot(overrides?: Partial<SettingsSnapshot['settings']>): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/tmp/vault',
      hotkey: { quickBar: 'ctrl+alt+space', quickBarBackup: 'ctrl+alt+cmd+space' },
      quickBar: {
        defaultTarget: 'last_active_or_inbox',
        lastActiveTtlHours: 24,
        destroyAfterHiddenMinutes: 10,
        autoCreateMissingSection: false,
        pinned: false,
      },
      mainWindow: {
        mode: 'standard',
        auxiliaryWidthPercent: 45,
      },
      editor: {
        theme: 'system',
        fontSize: 15,
        lineHeight: 1.7,
        maxWidth: 740,
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
      inbox: { bloatThresholdLines: 800 },
      terminal: { name: 'Terminal.app', bundleId: 'com.apple.Terminal' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: [],
      workspaces: {},
      ...overrides,
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

function makeUiState(mode: 'standard' | 'auxiliary' | 'fullscreen' = 'standard'): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1280, height: 800, mode, isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    selectedWorkspaceKey: null,
    hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space', selfCheck: 'ok' },
    lastActiveDoc: null,
    workspaceModes: {},
  };
}

describe('M4-05 auxiliary window mode & settings binding', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    windowStateListeners.length = 0;

    mockUiStateGet.mockResolvedValue(makeUiState('standard'));
    mockUiStateSet.mockResolvedValue(makeUiState('auxiliary'));
    mockAppSetMenuAccelerators.mockResolvedValue(undefined);
    mockWindowShowMain.mockImplementation(async (args?: { mode?: 'standard' | 'auxiliary' }) => ({
      windowId: 'main',
      visible: true,
      mode: args?.mode ?? 'standard',
      fullscreen: false,
      reason: null,
    }));
    mockOnWindowState.mockImplementation(async (cb: (state: WindowState) => void) => {
      windowStateListeners.push(cb);
      return () => undefined;
    });
    mockOnMenuAction.mockResolvedValue(() => undefined);
    mockOnToastAppendUndo.mockResolvedValue(() => undefined);
    mockOnSettingsStatus.mockResolvedValue(() => undefined);

    const initial = makeSnapshot();
    mockSettingsGet.mockResolvedValue(initial);
    mockSettingsUpdate.mockImplementation(async (patch: Record<string, unknown>) => {
      const mainWindowPatch = (patch.mainWindow ?? {}) as Partial<
        SettingsSnapshot['settings']['mainWindow']
      >;
      const quickBarPatch = (patch.quickBar ?? {}) as Partial<
        SettingsSnapshot['settings']['quickBar']
      >;
      return makeSnapshot({
        mainWindow: { ...initial.settings.mainWindow, ...mainWindowPatch },
        quickBar: { ...initial.settings.quickBar, ...quickBarPatch },
      });
    });
  });

  it('1. 收到 auxiliary window:state 时隐藏默认导航栏、显示退出按钮并强制覆盖式抽屉', async () => {
    const { default: MainWorkbenchShell } = await import('@/app/MainWorkbenchShell.vue');
    const { usePanelsStore } = await import('@/features/panels');

    const pinia = createPinia();
    setActivePinia(pinia);

    const standardApp = createSSRApp(MainWorkbenchShell);
    standardApp.use(pinia);
    const standardHtml = await renderToString(standardApp);
    await vi.waitFor(() => {
      expect(windowStateListeners.length).toBeGreaterThan(0);
    });
    await new Promise((resolve) => setTimeout(resolve, 10));

    const panelsStore = usePanelsStore();
    expect(standardHtml).toContain('data-testid="notes-sidebar"');
    expect(standardHtml).not.toContain('data-testid="aux-controls"');
    expect(standardHtml).not.toContain('data-testid="exit-auxiliary-mode-btn"');
    expect(panelsStore.isOverlayDrawer).toBe(false);

    for (const listener of windowStateListeners) {
      listener({
        windowId: 'main',
        visible: true,
        mode: 'auxiliary',
        fullscreen: false,
        reason: null,
      });
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(panelsStore.isOverlayDrawer).toBe(true);

    // 验证从持久化配置('auxiliary') 恢复时的首帧渲染结果
    vi.resetModules();
    windowStateListeners.length = 0;
    mockUiStateGet.mockResolvedValue(makeUiState('auxiliary'));
    mockSettingsGet.mockResolvedValue(
      makeSnapshot({ mainWindow: { mode: 'auxiliary', auxiliaryWidthPercent: 45 } }),
    );
    const { default: AuxShell } = await import('@/app/MainWorkbenchShell.vue');
    const { useSettingsStore: useAuxSettingsStore } = await import('@/features/settings');
    const { usePanelsStore: useAuxPanelsStore } = await import('@/features/panels');
    const auxPinia = createPinia();
    setActivePinia(auxPinia);
    const auxSettings = useAuxSettingsStore();
    await auxSettings.initialize();

    const auxRendered = await renderToString(createSSRApp(AuxShell).use(auxPinia));
    const auxPanels = useAuxPanelsStore(auxPinia);
    expect(auxPanels.isOverlayDrawer).toBe(true);
    expect(auxRendered).toContain('data-testid="aux-controls"');
    expect(auxRendered).toContain('退出侧栏辅助');
    expect(auxRendered).toContain('data-testid="exit-auxiliary-mode-btn"');
    expect(auxRendered).not.toContain('data-testid="notes-sidebar"');
  });

  it('2. 触发 window.toggleAuxiliary 在标准/辅助模式间切换并正确调用 window_show_main', async () => {
    const { default: MainWorkbenchShell } = await import('@/app/MainWorkbenchShell.vue');
    const { actionRegistry } = await import('@/lib/actions');

    const pinia = createPinia();
    setActivePinia(pinia);

    const app = createSSRApp(MainWorkbenchShell);
    app.use(pinia);
    await renderToString(app);
    await vi.waitFor(() => {
      expect(windowStateListeners.length).toBeGreaterThan(0);
    });
    await new Promise((resolve) => setTimeout(resolve, 10));

    // 标准模式下触发动作 -> 切到 auxiliary
    await actionRegistry.executeById('window.toggleAuxiliary', { activeScope: 'main-window' });
    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: 'auxiliary' });

    // 再次触发 -> 切回 standard
    await actionRegistry.executeById('window.toggleAuxiliary', { activeScope: 'main-window' });
    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: 'standard' });
    await new Promise((resolve) => setTimeout(resolve, 230));
  });

  it('3. 设置窗口渲染形态卡片并修改 mainWindow.mode、auxiliaryWidthPercent(夹取30-60) 与 destroyAfterHiddenMinutes', async () => {
    const { SettingsView, useSettingsStore } = await import('@/features/settings');

    const pinia = createPinia();
    setActivePinia(pinia);
    const settingsStore = useSettingsStore();
    await settingsStore.initialize();

    const ssrApp = createSSRApp(SettingsView);
    ssrApp.use(pinia);
    const html = await renderToString(ssrApp);

    expect(html).toContain('data-testid="settings-window-mode-card"');
    expect(html).toContain('data-testid="settings-main-window-mode-select"');
    expect(html).toContain('data-testid="settings-auxiliary-width-input"');
    expect(html).toContain('data-testid="settings-quickbar-destroy-input"');
    expect(html).toContain('data-testid="fullscreen-mission-control-tip"');

    // 修改默认窗口模式
    await settingsStore.setMainWindowMode('auxiliary');
    expect(mockSettingsUpdate).toHaveBeenCalledWith({ mainWindow: { mode: 'auxiliary' } });
    expect(mockUiStateSet).toHaveBeenCalledWith({
      key: 'windows',
      value: expect.objectContaining({
        main: expect.objectContaining({ mode: 'auxiliary' }),
      }),
    });

    // 修改辅助宽度（超出上限 90 -> 夹取为 60；低于下限 10 -> 夹取为 30）
    await settingsStore.setAuxiliaryWidthPercent(90);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({ mainWindow: { auxiliaryWidthPercent: 60 } });

    await settingsStore.setAuxiliaryWidthPercent(10);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({ mainWindow: { auxiliaryWidthPercent: 30 } });

    // 主窗口处于辅助模式时调整宽度应触发实时生效
    for (const listener of windowStateListeners) {
      listener({
        windowId: 'main',
        visible: true,
        mode: 'auxiliary',
        fullscreen: false,
        reason: null,
      });
    }
    await settingsStore.setAuxiliaryWidthPercent(50);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({ mainWindow: { auxiliaryWidthPercent: 50 } });
    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: 'auxiliary' });

    // 修改 Quick Bar 隐藏释放延时
    await settingsStore.setQuickBarDestroyMinutes(15);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({
      quickBar: { destroyAfterHiddenMinutes: 15 },
    });
  });

  it('4. 冷启动恢复 auxiliary 模式时主动调用 windowShowMain({ mode: auxiliary }) 同步原生窗口', async () => {
    mockUiStateGet.mockResolvedValue(makeUiState('auxiliary'));

    const { default: MainWorkbenchShell } = await import('@/app/MainWorkbenchShell.vue');
    const pinia = createPinia();
    setActivePinia(pinia);

    const app = createSSRApp(MainWorkbenchShell);
    app.use(pinia);
    await renderToString(app);

    await vi.waitFor(() => {
      expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: 'auxiliary' });
    });
  });
});
