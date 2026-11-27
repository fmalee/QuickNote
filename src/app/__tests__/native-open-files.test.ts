import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SettingsSnapshot, UiState } from '@/bindings/types';

const {
  mockUiStateGet,
  mockUiStateSet,
  mockWindowShowMain,
  mockAppSetMenuAccelerators,
  mockAppTakePendingOpenFiles,
  mockPickLocalFilesToOpen,
  mockSettingsGet,
  mockOpenNoteByAbsPath,
  mockRestoreTabsFromUiState,
  menuActionListeners,
  pendingOpenFilesListeners,
} = vi.hoisted(() => ({
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockAppSetMenuAccelerators: vi.fn(),
  mockAppTakePendingOpenFiles: vi.fn(),
  mockPickLocalFilesToOpen: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockOpenNoteByAbsPath: vi.fn(),
  mockRestoreTabsFromUiState: vi.fn(),
  menuActionListeners: [] as Array<(actionId: string) => void>,
  pendingOpenFilesListeners: [] as Array<() => void>,
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: () => false,
  appSetMenuAccelerators: mockAppSetMenuAccelerators,
  appTakePendingOpenFiles: mockAppTakePendingOpenFiles,
  pickLocalFilesToOpen: mockPickLocalFilesToOpen,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  windowShowMain: mockWindowShowMain,
  windowSetQuickBarPinned: vi.fn().mockResolvedValue(undefined),
  settingsGet: mockSettingsGet,
  settingsUpdate: vi.fn(),
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
  onWindowState: vi.fn().mockResolvedValue(() => undefined),
  onMenuAction: vi.fn().mockImplementation((handler: (actionId: string) => void) => {
    menuActionListeners.push(handler);
    return Promise.resolve(() => undefined);
  }),
  onPendingOpenFiles: vi.fn().mockImplementation((handler: () => void) => {
    pendingOpenFilesListeners.push(handler);
    return Promise.resolve(() => undefined);
  }),
  onToastAppendUndo: vi.fn().mockResolvedValue(() => undefined),
  onSettingsStatus: vi.fn().mockResolvedValue(() => undefined),
  onFsExternalChange: vi.fn().mockResolvedValue(() => undefined),
  onIndexProgress: vi.fn().mockResolvedValue(() => undefined),
  onTerminalSessionState: vi.fn().mockResolvedValue(() => undefined),
  onTerminalExit: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock('@/features/notes', () => ({
  NewNoteTemplateModal: { template: '<div />' },
  NotesSidebar: { template: '<aside />' },
  useNotesStore: () => ({
    initialize: vi.fn().mockResolvedValue(undefined),
    refreshAll: vi.fn().mockResolvedValue(undefined),
    refreshPinnedExternalFiles: vi.fn().mockResolvedValue(undefined),
    dispose: vi.fn(),
  }),
}));

vi.mock('@/features/onboarding', () => ({
  OnboardingView: { template: '<div />' },
  useOnboardingStore: () => ({
    libraryInfo: { root: '/tmp/vault', noteCount: 1, initialized: true },
    loading: false,
    errorMessage: null,
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
    tabs: [],
    activeKey: null,
    activeTab: null,
    activeDoc: null,
    errorMessage: null,
    restoreTabsFromUiState: mockRestoreTabsFromUiState,
    openNoteByAbsPath: mockOpenNoteByAbsPath,
    flushSave: vi.fn().mockResolvedValue(undefined),
    handleMainWindowShown: vi.fn().mockResolvedValue(undefined),
    handleQuickBarAppendEvent: vi.fn().mockResolvedValue(undefined),
    dispose: vi.fn(),
  }),
  useExternalActions: () => ({
    refreshForDoc: vi.fn().mockResolvedValue(undefined),
    executeAction: vi.fn().mockResolvedValue(undefined),
    isActionEnabled: vi.fn().mockReturnValue(false),
  }),
}));

vi.mock('@/features/panels', () => ({
  usePanelsStore: () => ({
    leftSidebarCollapsed: false,
    leftSidebarWidth: 240,
    leftSidebarView: 'files',
    leftSidebar: { collapsed: false, width: 240, view: 'files' },
    terminalPanel: { visible: false, height: 240 },
    isTerminalPanelShown: false,
    terminalPanelHeight: 240,
    isLeftSidebarShown: true,
    useOverlayLeftDrawer: false,
    initialize: vi.fn().mockResolvedValue(undefined),
    setForceOverlayDrawer: vi.fn(),
    applyViewportWidth: vi.fn(),
    toggleLeftView: vi.fn(),
    toggleTerminalPanel: vi.fn(),
    setTerminalPanelHeight: vi.fn(),
    dispose: vi.fn(),
  }),
}));

vi.mock('@/features/terminal', () => ({
  TerminalPanel: { template: '<div />' },
  useTerminalStore: () => ({
    hasSelection: false,
    syncPanelVisibility: vi.fn().mockResolvedValue(undefined),
    copyActiveSelection: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock('@/features/commands', () => ({
  CommandBookPalette: { template: '<div />' },
  useCommandPalette: () => ({
    query: { value: '' },
    results: { value: [] },
    selectedIndex: { value: 0 },
    loading: { value: false },
    refresh: vi.fn(),
    dispose: vi.fn(),
  }),
}));

vi.mock('@/features/quickopen', () => ({
  PENDING_OPEN_FILE_STORAGE_KEY: 'quicknote:pending-open-file',
  QuickOpenPalette: { template: '<div />' },
  consumePendingOpenFile: () => null,
  useQuickOpen: () => ({
    query: { value: '' },
    results: { value: [] },
    selectedIndex: { value: 0 },
    loading: { value: false },
    refresh: vi.fn(),
    dispose: vi.fn(),
  }),
}));

vi.mock('@/features/settings', () => ({
  SettingsErrorBanner: { template: '<div />' },
  useSettingsStore: () => ({
    snapshot: {
      settings: {
        mainWindow: { mode: 'standard', auxiliaryWidthPercent: 45 },
        quickBar: { pinned: false },
      },
      status: { ok: true, parseError: null },
    } as unknown as SettingsSnapshot,
    lastSavedAt: null,
  }),
}));

import MainWorkbenchShell from '@/app/MainWorkbenchShell.vue';
import { NATIVE_MENU_ACTION_IDS, actionRegistry } from '@/lib/actions';

function makeUiState(): UiState {
  return {
    openTabs: [],
    activeTabKey: null,
    windows: {
      main: { mode: 'standard', x: null, y: null, width: 1200, height: 800 },
    },
    leftSidebar: { collapsed: false, width: 240, view: 'files' },
    terminalPanel: { visible: false, height: 240 },
    backupPaths: { lastExportPath: null, lastImportPath: null },
  } as unknown as UiState;
}

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe('MainWorkbenchShell native file open flows', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    menuActionListeners.length = 0;
    pendingOpenFilesListeners.length = 0;
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockAppSetMenuAccelerators.mockResolvedValue(undefined);
    mockAppTakePendingOpenFiles.mockResolvedValue([]);
    mockPickLocalFilesToOpen.mockResolvedValue([]);
    mockOpenNoteByAbsPath.mockResolvedValue(undefined);
    mockRestoreTabsFromUiState.mockResolvedValue(undefined);
  });

  it('registers window.openFile with cmd+o and syncs to native menu', () => {
    expect(NATIVE_MENU_ACTION_IDS).toContain('window.openFile');
    expect(actionRegistry.getEffectiveKeys('window.openFile')).toBe('cmd+o');
  });

  it('drains cold-start pending files, handles hot-start signal, and opens multi-select from menu', async () => {
    mockAppTakePendingOpenFiles
      .mockResolvedValueOnce(['/tmp/cold-start.md', '/tmp/Dockerfile'])
      .mockResolvedValueOnce(['/tmp/hot-drop.json']);

    mockPickLocalFilesToOpen.mockResolvedValueOnce(['/tmp/bad.png', '/tmp/good.txt']);
    mockOpenNoteByAbsPath.mockImplementation(async (absPath: string) => {
      if (absPath === '/tmp/bad.png') {
        throw new Error('binary rejected');
      }
    });

    const app = createSSRApp(MainWorkbenchShell);
    app.use(createPinia());
    await renderToString(app);
    await flushMicrotasks();

    // 1. Cold-start drain opened both files as non-preview tabs.
    expect(mockAppTakePendingOpenFiles).toHaveBeenCalledTimes(1);
    expect(mockOpenNoteByAbsPath).toHaveBeenNthCalledWith(1, '/tmp/cold-start.md', {
      preview: false,
    });
    expect(mockOpenNoteByAbsPath).toHaveBeenNthCalledWith(2, '/tmp/Dockerfile', {
      preview: false,
    });

    // 2. Hot-start Dock drop / Finder Open With signal triggers another drain.
    expect(pendingOpenFilesListeners.length).toBeGreaterThan(0);
    pendingOpenFilesListeners[0]?.();
    await flushMicrotasks();

    expect(mockAppTakePendingOpenFiles).toHaveBeenCalledTimes(2);
    expect(mockOpenNoteByAbsPath).toHaveBeenNthCalledWith(3, '/tmp/hot-drop.json', {
      preview: false,
    });

    // 3. Menu File -> Open... picks multiple files; one failure does not block subsequent files.
    expect(menuActionListeners.length).toBeGreaterThan(0);
    menuActionListeners[0]?.('window.openFile');
    await flushMicrotasks();

    expect(mockPickLocalFilesToOpen).toHaveBeenCalledWith('/tmp/vault');
    expect(mockOpenNoteByAbsPath).toHaveBeenNthCalledWith(4, '/tmp/bad.png', {
      preview: false,
    });
    expect(mockOpenNoteByAbsPath).toHaveBeenNthCalledWith(5, '/tmp/good.txt', {
      preview: false,
    });
  });

  it('does not drain pending open files before restoreTabsFromUiState completes', async () => {
    let resolveRestore!: () => void;
    mockRestoreTabsFromUiState.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveRestore = resolve;
        }),
    );
    mockAppTakePendingOpenFiles.mockResolvedValueOnce(['/tmp/cold-raced.md']);

    const app = createSSRApp(MainWorkbenchShell);
    app.use(createPinia());
    await renderToString(app);
    await flushMicrotasks();

    // Even if backend emits app:pending-open-files while restoreTabsFromUiState is still in flight,
    // frontend must NOT drain the queue yet (otherwise restoreTabsFromUiState would overwrite tabs).
    expect(pendingOpenFilesListeners.length).toBeGreaterThan(0);
    pendingOpenFilesListeners[0]?.();
    await flushMicrotasks();

    expect(mockAppTakePendingOpenFiles).not.toHaveBeenCalled();
    expect(mockOpenNoteByAbsPath).not.toHaveBeenCalled();

    // Once restoreTabsFromUiState finishes, the queue is drained and opened on top of restored tabs.
    resolveRestore();
    await flushMicrotasks();

    expect(mockAppTakePendingOpenFiles).toHaveBeenCalledTimes(1);
    expect(mockOpenNoteByAbsPath).toHaveBeenCalledWith('/tmp/cold-raced.md', {
      preview: false,
    });
  });
});
