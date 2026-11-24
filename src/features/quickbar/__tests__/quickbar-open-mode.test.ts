import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ExternalActionState,
  LibraryInfo,
  NavTreeSnapshot,
  NoteDoc,
  ResolvedTarget,
  SearchResponse,
  SettingsSnapshot,
  UiState,
  WindowState,
} from '@/bindings/types';

const {
  mockLibraryGetInfo,
  mockSettingsGet,
  mockUiStateGet,
  mockUiStateSet,
  mockCaptureResolveTarget,
  mockNoteAppend,
  mockNoteRead,
  mockNoteWrite,
  mockSearchNotesAndFiles,
  mockSearchSpotlight,
  mockPathComplete,
  mockNotesGetTreeAndTags,
  mockNotesList,
  mockExternalActionsResolve,
  mockExternalActionRun,
  mockWindowShowMain,
  mockWindowHideQuickBar,
  mockOnWindowState,
  mockOnExternalChange,
  mockOnIndexStatus,
} = vi.hoisted(() => ({
  mockLibraryGetInfo: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockCaptureResolveTarget: vi.fn(),
  mockNoteAppend: vi.fn(),
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockSearchNotesAndFiles: vi.fn(),
  mockSearchSpotlight: vi.fn(),
  mockPathComplete: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockNotesList: vi.fn(),
  mockExternalActionsResolve: vi.fn(),
  mockExternalActionRun: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockWindowHideQuickBar: vi.fn(),
  mockOnWindowState: vi.fn(),
  mockOnExternalChange: vi.fn(),
  mockOnIndexStatus: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: () => false,
  libraryGetInfo: mockLibraryGetInfo,
  settingsGet: mockSettingsGet,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  captureResolveTarget: mockCaptureResolveTarget,
  noteAppend: mockNoteAppend,
  noteRead: mockNoteRead,
  noteWrite: mockNoteWrite,
  searchNotesAndFiles: mockSearchNotesAndFiles,
  searchSpotlight: mockSearchSpotlight,
  pathComplete: mockPathComplete,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  notesList: mockNotesList,
  externalActionsResolve: mockExternalActionsResolve,
  externalActionRun: mockExternalActionRun,
  windowShowMain: mockWindowShowMain,
  windowHideQuickBar: mockWindowHideQuickBar,
}));

vi.mock('@/lib/ipc/events', () => ({
  onWindowState: mockOnWindowState,
  onExternalChange: mockOnExternalChange,
  onIndexStatus: mockOnIndexStatus,
}));

import { EditorTabBar, useEditorTabsStore } from '@/features/editor';
import { NotesSidebar, useNotesStore } from '@/features/notes';
import { PENDING_OPEN_FILE_STORAGE_KEY, consumePendingOpenFile } from '@/features/quickopen';
import { actionRegistry } from '@/lib/actions';
import QuickBarView from '../QuickBarView.vue';
import { useQuickBarStore } from '../store';

function makeLibraryInfo(): LibraryInfo {
  return {
    root: '/Users/demo/Vault',
    noteCount: 2,
    lastIndexedAt: null,
  };
}

function makeSettingsSnapshot(): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/Users/demo/Vault',
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
      valid: true,
      activeSource: 'default',
      errorLine: null,
      errorColumn: null,
      errorMessage: null,
    },
  };
}

function makeUiState(): UiState {
  return {
    selectedWorkspaceKey: null,
    workspaceModes: {},
    filters: { activeTab: 'recent', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    tabs: { items: [], activeKey: null, closedStack: [] },
    lastActiveDoc: null,
    panelGroup: { visible: true, activeId: 'outline', width: 260, collapsed: false },
    windows: { main: { isOpen: true, mode: 'standard', x: 0, y: 0, width: 1200, height: 800 } },
    hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space', selfCheck: 'ok' },
  };
}

function makeMemoryStorage(): Storage {
  const map = new Map<string, string>();
  return {
    get length() {
      return map.size;
    },
    clear() {
      map.clear();
    },
    getItem(key: string) {
      return map.get(key) ?? null;
    },
    key(index: number) {
      return Array.from(map.keys())[index] ?? null;
    },
    removeItem(key: string) {
      map.delete(key);
    },
    setItem(key: string, value: string) {
      map.set(key, value);
    },
  };
}

describe('quickbar-open-mode & external tabs (M3-03)', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    Object.defineProperty(globalThis, 'localStorage', {
      value: makeMemoryStorage(),
      configurable: true,
      writable: true,
    });

    mockLibraryGetInfo.mockResolvedValue(makeLibraryInfo());
    mockSettingsGet.mockResolvedValue(makeSettingsSnapshot());
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockCaptureResolveTarget.mockResolvedValue({
      scenario: 'S8',
      kind: 'inbox',
      targetRelPath: 'inbox.md',
      targetTitle: '收件箱',
      sectionHint: null,
      workspaceKey: null,
      workspaceName: null,
      isCrossWorkspaceFallback: false,
      recommendedMode: 'list',
      chipLabel: '收件箱 · inbox.md',
    } satisfies ResolvedTarget);
    mockSearchNotesAndFiles.mockResolvedValue({
      requestId: 1,
      items: [],
      truncated: false,
    } satisfies SearchResponse);
    mockNotesGetTreeAndTags.mockResolvedValue({
      inboxCount: 1,
      dirs: [],
      tags: [],
      workspaces: [],
    } satisfies NavTreeSnapshot);
    mockNotesList.mockResolvedValue([]);
    mockWindowShowMain.mockResolvedValue({
      windowId: 'main',
      visible: true,
      fullscreen: false,
      mode: 'standard',
      reason: null,
    } satisfies WindowState);
    mockWindowHideQuickBar.mockResolvedValue({
      windowId: 'quickbar',
      visible: false,
      fullscreen: false,
      mode: null,
      reason: 'esc',
    } satisfies WindowState);
    mockOnWindowState.mockResolvedValue(() => undefined);
    mockOnExternalChange.mockResolvedValue(() => undefined);
    mockOnIndexStatus.mockResolvedValue(() => undefined);
    mockExternalActionRun.mockResolvedValue(undefined);
    mockExternalActionsResolve.mockResolvedValue([
      {
        id: 'external.pin',
        enabled: true,
        disabledReason: null,
        resolvedPath: '/etc/nginx/nginx.conf',
      } satisfies ExternalActionState,
    ]);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('switches to open mode on Cmd+K, returns to capture on first Esc, and hides on second Esc', async () => {
    const store = useQuickBarStore();
    await store.initialize();
    expect(store.mode).toBe('capture');

    const dispose = actionRegistry.bindHandler('quickbar.mode.open', () => {
      store.setMode('open');
    });

    const dispatched = actionRegistry.dispatchKeyEvent(
      {
        key: 'k',
        metaKey: true,
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        isComposing: false,
        keyCode: 75,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as KeyboardEvent,
      {
        activeScope: 'quickbar',
        activeScopes: ['quickbar', 'global'],
      },
    );
    expect(dispatched.handled).toBe(true);
    expect(store.mode).toBe('open');

    // First Esc returns to capture mode without hiding Quick Bar
    await store.handleEscape();
    expect(store.mode).toBe('capture');
    expect(mockWindowHideQuickBar).not.toHaveBeenCalled();

    // Second Esc hides Quick Bar
    await store.handleEscape();
    expect(mockWindowHideQuickBar).toHaveBeenCalledWith({ reason: 'esc' });

    dispose();
    store.dispose();
  });

  it('writes pending open payload, shows main window, and hides Quick Bar when opening an item', async () => {
    const store = useQuickBarStore();
    await store.initialize();
    store.setMode('open');

    await store.openFileInMain('/etc/nginx/nginx.conf', true);

    const raw = globalThis.localStorage.getItem(PENDING_OPEN_FILE_STORAGE_KEY);
    expect(raw).not.toBeNull();
    const consumed = consumePendingOpenFile();
    expect(consumed).toMatchObject({
      absPath: '/etc/nginx/nginx.conf',
      newTab: true,
      line: null,
    });
    expect(globalThis.localStorage.getItem(PENDING_OPEN_FILE_STORAGE_KEY)).toBeNull();

    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: null });
    expect(mockWindowHideQuickBar).toHaveBeenCalledWith({ reason: 'esc' });

    // SSR smoke check for QuickBarView in open mode
    const app = createSSRApp(QuickBarView);
    app.use(createPinia());
    const html = await renderToString(app);
    expect(html).toContain('qn-quickbar');

    store.dispose();
  });

  it('opens external files in editor tabs with external badge and pins to sidebar recent external files', async () => {
    const externalDoc: NoteDoc = {
      key: 'path:/etc/nginx/nginx.conf',
      relPath: null,
      absPath: '/etc/nginx/nginx.conf',
      isLibrary: false,
      isInbox: false,
      meta: {
        id: null,
        title: 'nginx.conf',
        created: null,
        tags: [],
        project: null,
        openDir: null,
        codeWorkspace: null,
        terminalDir: null,
        pinned: false,
        archived: false,
        docType: 'note',
        metaError: null,
      },
      body: 'worker_processes 1;\n',
      bodyHash: 'h1',
      mtimeMs: 1000,
      sizeBytes: 20,
    };
    mockNoteRead.mockResolvedValueOnce(externalDoc);
    mockSearchNotesAndFiles.mockResolvedValueOnce({
      requestId: 2,
      items: [
        {
          kind: 'file',
          key: 'abs:/etc/nginx/nginx.conf',
          title: 'nginx.conf',
          subtitle: '[已固定] /etc/nginx/nginx.conf',
          relPath: null,
          absPath: '/etc/nginx/nginx.conf',
          score: 1000,
          source: 'recent',
        },
      ],
      truncated: false,
    } satisfies SearchResponse);

    const pinia = createPinia();
    setActivePinia(pinia);

    const tabsStore = useEditorTabsStore();
    const opened = await tabsStore.openNoteByAbsPath('/etc/nginx/nginx.conf', { preview: false });
    expect(opened).not.toBeNull();
    expect(opened?.doc?.isLibrary).toBe(false);
    expect(opened?.absPath).toBe('/etc/nginx/nginx.conf');

    const notesStore = useNotesStore();
    await notesStore.loadPinnedExternalFiles();
    expect(notesStore.pinnedExternalFiles).toHaveLength(1);
    expect(notesStore.pinnedExternalFiles[0]?.absPath).toBe('/etc/nginx/nginx.conf');

    // Render EditorTabBar and verify external badge + pin button
    const tabBarApp = createSSRApp(EditorTabBar);
    tabBarApp.use(pinia);
    const tabBarHtml = await renderToString(tabBarApp);
    expect(tabBarHtml).toContain('editor-tab-external-badge');
    expect(tabBarHtml).toContain('外部');
    expect(tabBarHtml).toContain('editor-tab-pin-/etc/nginx/nginx.conf');

    // Render NotesSidebar and verify pinned external files section
    const sidebarApp = createSSRApp(NotesSidebar);
    sidebarApp.use(pinia);
    const sidebarHtml = await renderToString(sidebarApp);
    expect(sidebarHtml).toContain('notes-pinned-external');
    expect(sidebarHtml).toContain('常用外部文件');
    expect(sidebarHtml).toContain('nginx.conf');

    tabsStore.dispose();
    notesStore.dispose();
  });
});
