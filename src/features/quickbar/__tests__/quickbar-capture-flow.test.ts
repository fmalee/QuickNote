import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp, nextTick } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  AppendResult,
  LibraryInfo,
  NavTreeSnapshot,
  NoteSummary,
  ResolvedTarget,
  SearchResponse,
  SearchSource,
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
  mockSearchNotesAndFiles,
  mockNotesGetTreeAndTags,
  mockNotesList,
  mockWindowShowMain,
  mockWindowHideQuickBar,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockLibraryGetInfo: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockCaptureResolveTarget: vi.fn(),
  mockNoteAppend: vi.fn(),
  mockSearchNotesAndFiles: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockNotesList: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockWindowHideQuickBar: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  libraryGetInfo: mockLibraryGetInfo,
  settingsGet: mockSettingsGet,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  captureResolveTarget: mockCaptureResolveTarget,
  noteAppend: mockNoteAppend,
  searchNotesAndFiles: mockSearchNotesAndFiles,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  notesList: mockNotesList,
  windowShowMain: mockWindowShowMain,
  windowHideQuickBar: mockWindowHideQuickBar,
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onWindowState: mockOnWindowState,
  onSettingsStatus: vi.fn().mockResolvedValue(() => {}),
  onToastAppendUndo: vi.fn().mockResolvedValue(() => {}),
  onFsExternalChange: vi.fn().mockResolvedValue(() => {}),
  listenEvent: vi.fn().mockResolvedValue(() => {}),
}));

import {
  QUICKBAR_DRAFT_STORAGE_KEY,
  QUICKBAR_SEARCH_DEBOUNCE_MS,
  QuickBarView,
  useQuickBarStore,
} from '@/features/quickbar';

function installMemoryLocalStorage(): void {
  const store = new Map<string, string>();
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value);
      },
      removeItem: (key: string) => {
        store.delete(key);
      },
      clear: () => {
        store.clear();
      },
    },
  });
}

function makeLibraryInfo(): LibraryInfo {
  return {
    root: '/Users/demo/QuickNoteVault',
    noteCount: 5,
    lastIndexedAt: 1700000000000,
  };
}

function makeSettingsSnapshot(): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/Users/demo/QuickNoteVault',
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
      terminal: { name: 'Terminal.app', bundleId: 'com.apple.Terminal' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: [],
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

function makeUiState(overrides: Partial<UiState> = {}): UiState {
  return {
    filters: { activeTab: 'recent', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    tabs: { items: [], activeKey: null, closedStack: [] },
    lastActiveDoc: null,
    windows: {
      main: {
        isOpen: false,
        mode: 'standard',
        x: 0,
        y: 0,
        width: 1200,
        height: 800,
      },
    },
    hotkey: {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
    ...overrides,
  };
}

function makeResolvedTarget(overrides: Partial<ResolvedTarget> = {}): ResolvedTarget {
  return {
    scenario: 'S3',
    kind: 'note',
    targetRelPath: 'work-a/plan.md',
    targetTitle: '项目规划',
    sectionHint: null,
    recommendedMode: 'list',
    chipLabel: '项目规划 · work-a/plan.md',
    ...overrides,
  };
}

describe('quickbar-capture-flow (M1-05)', () => {
  let windowStateListeners: Array<(state: WindowState) => void> = [];

  beforeEach(() => {
    vi.clearAllMocks();
    installMemoryLocalStorage();
    windowStateListeners = [];
    mockLibraryGetInfo.mockResolvedValue(makeLibraryInfo());
    mockSettingsGet.mockResolvedValue(makeSettingsSnapshot());
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockImplementation(async () => makeUiState());
    mockCaptureResolveTarget.mockResolvedValue(makeResolvedTarget());
    mockOnWindowState.mockImplementation(async (handler: (state: WindowState) => void) => {
      windowStateListeners.push(handler);
      return () => {};
    });
    mockWindowHideQuickBar.mockResolvedValue({
      windowId: 'quickbar',
      visible: false,
      fullscreen: false,
      mode: null,
      reason: 'esc',
    });
    mockWindowShowMain.mockResolvedValue({
      windowId: 'main',
      visible: true,
      fullscreen: false,
      mode: 'standard',
      reason: null,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('1. 渲染 S1-S4 目标 Chip、未选库 warning 与“将追加到文末”提示', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    mockCaptureResolveTarget.mockResolvedValueOnce(
      makeResolvedTarget({
        scenario: 'S3',
        targetRelPath: 'work-a/plan.md',
        targetTitle: '项目规划',
        sectionHint: '将追加到文末',
        chipLabel: '项目规划 · 列表',
      }),
    );

    const store = useQuickBarStore();
    await store.initialize();

    const app1 = createSSRApp(QuickBarView);
    app1.use(pinia);
    const html1 = await renderToString(app1);
    expect(html1).toContain('data-testid="quickbar-target-chip"');
    expect(html1).toContain('项目规划 · 列表');
    expect(html1).toContain('将追加到文末');
    expect(store.targetChipTone).toBe('neutral');

    mockLibraryGetInfo.mockResolvedValueOnce({ ...makeLibraryInfo(), root: null });
    await store.refreshState();
    expect(store.targetChipLabel).toBe('未选库 · S1');
    expect(store.targetChipTone).toBe('warning');
  });

  it('2. ⌘1/⌘2/⌘3 模式切换更新状态并写入本地草稿（不再持久化 workspaceModes）', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    mockCaptureResolveTarget.mockResolvedValueOnce(
      makeResolvedTarget({
        targetRelPath: 'work-a/note.md',
        recommendedMode: 'list',
      }),
    );

    const store = useQuickBarStore();
    await store.initialize();

    store.setCaptureMode('todo');
    expect(store.captureMode).toBe('todo');
    expect(mockUiStateSet).not.toHaveBeenCalled();

    store.selectExplicitDoc({ relPath: 'inbox.md', title: '收件箱' });
    store.setCaptureMode('prose');
    expect(store.captureMode).toBe('prose');
    expect(mockUiStateSet).not.toHaveBeenCalled();
  });

  it('3. 相关笔记 >= 2 字符 80ms 防抖、丢弃过期 requestId、⌘↑/⌘↓ 改投', async () => {
    vi.useFakeTimers();
    const pinia = createPinia();
    setActivePinia(pinia);

    const store = useQuickBarStore();
    await store.initialize();

    // < 2 字符不触发
    store.draft = 'a';
    await nextTick();
    vi.advanceTimersByTime(QUICKBAR_SEARCH_DEBOUNCE_MS + 20);
    expect(mockSearchNotesAndFiles).not.toHaveBeenCalled();

    // 竞态：先发 req1，再发 req2；req1 晚返回应被丢弃
    let resolveReq1!: (val: SearchResponse) => void;
    let resolveReq2!: (val: SearchResponse) => void;
    mockSearchNotesAndFiles
      .mockImplementationOnce(
        () =>
          new Promise<SearchResponse>((resolve) => {
            resolveReq1 = resolve;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise<SearchResponse>((resolve) => {
            resolveReq2 = resolve;
          }),
      );

    store.draft = '设计';
    await nextTick();
    vi.advanceTimersByTime(QUICKBAR_SEARCH_DEBOUNCE_MS);
    const req1Id = mockSearchNotesAndFiles.mock.calls[0]?.[0]?.requestId as number;
    expect(mockSearchNotesAndFiles).toHaveBeenNthCalledWith(1, {
      query: '设计',
      limit: 3,
      requestId: req1Id,
    });

    store.draft = '设计方案';
    await nextTick();
    vi.advanceTimersByTime(QUICKBAR_SEARCH_DEBOUNCE_MS);
    const req2Id = mockSearchNotesAndFiles.mock.calls[1]?.[0]?.requestId as number;
    expect(req2Id).toBeGreaterThan(req1Id);
    expect(mockSearchNotesAndFiles).toHaveBeenNthCalledWith(2, {
      query: '设计方案',
      limit: 3,
      requestId: req2Id,
    });

    // req2 先返回
    resolveReq2({
      requestId: req2Id,
      truncated: false,
      items: [
        {
          kind: 'note',
          key: 'k2',
          title: '新方案文档',
          subtitle: 'notes/new-plan.md',
          relPath: 'notes/new-plan.md',
          absPath: '/Users/demo/QuickNoteVault/notes/new-plan.md',
          score: 99,
          source: 'library' as SearchSource,
        },
      ],
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(store.relatedNotes.map((n) => n.title)).toEqual(['新方案文档']);

    // req1 晚返回，必须被丢弃
    resolveReq1({
      requestId: req1Id,
      truncated: false,
      items: [
        {
          kind: 'note',
          key: 'k1',
          title: '过期旧文档',
          subtitle: 'notes/old.md',
          relPath: 'notes/old.md',
          absPath: '/Users/demo/QuickNoteVault/notes/old.md',
          score: 50,
          source: 'library' as SearchSource,
        },
      ],
    });
    await Promise.resolve();
    expect(store.relatedNotes.map((n) => n.title)).toEqual(['新方案文档']);

    // ⌘↓ 选中第一条相关笔记并验证提交改投
    store.cycleRelatedTarget('next');
    expect(store.selectedRelatedIndex).toBe(0);
    expect(store.effectiveTargetRelPath).toBe('notes/new-plan.md');

    const appendRes: AppendResult = {
      relPath: 'notes/new-plan.md',
      title: '新方案文档',
      addedLines: 1,
      sectionHeading: null,
      bodyHash: 'hash-1',
      mtimeMs: 1700000001000,
      undoAvailable: true,
    };
    mockNoteAppend.mockResolvedValueOnce(appendRes);

    await store.submitAppend(false);
    expect(mockNoteAppend).toHaveBeenCalledWith({
      targetRelPath: 'notes/new-plan.md',
      content: '设计方案',
      mode: 'list',
    });
  });

  it('4. ⌘T 目标选择器显式选文档与 Esc 分层回退', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const navTree: NavTreeSnapshot = {
      dirs: [],
      tags: [{ tag: '架构', count: 2, children: [] }],
    };
    const notes: NoteSummary[] = [
      {
        key: 'k_b',
        relPath: 'work-b/spec.md',
        absPath: '/Users/demo/QuickNoteVault/work-b/spec.md',
        title: 'B 区规格',
        docType: 'note',
        tags: ['架构'],
        excerpt: '',
        pinned: false,
        archived: false,
        isInbox: false,
        mtimeMs: 1700000000,
        metaError: null,
      },
    ];
    mockNotesGetTreeAndTags.mockResolvedValue(navTree);
    mockNotesList.mockResolvedValue(notes);

    const store = useQuickBarStore();
    await store.initialize();

    store.openTargetPicker();
    expect(store.targetPickerOpen).toBe(true);

    // Esc 第一层：先关闭选择器，不隐藏窗口
    await store.handleEscape();
    expect(store.targetPickerOpen).toBe(false);
    expect(mockWindowHideQuickBar).not.toHaveBeenCalled();

    // 显式选文档
    store.selectExplicitDoc({
      relPath: 'work-b/spec.md',
      title: 'B 区规格',
    });
    expect(store.effectiveTargetRelPath).toBe('work-b/spec.md');
    expect(store.targetChipLabel).toContain('B 区规格');

    // Esc 在 open 模式下直接隐藏 Quick Bar 并将模式复位为 capture
    store.mode = 'open';
    await store.handleEscape();
    expect(store.mode).toBe('capture');
    expect(mockWindowHideQuickBar).toHaveBeenCalledWith({ reason: 'esc' });
  });

  it('5. 草稿在 Esc / space_changed 隐藏后无损恢复，⌘↵ 与 ⇧⌘↵ 清空草稿并协同主窗口', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const store1 = useQuickBarStore();
    await store1.initialize();
    store1.draft = '未提交的灵感草稿';
    store1.setCaptureMode('todo');
    await nextTick();

    // 模拟切 Space 隐藏
    windowStateListeners.forEach((fn) =>
      fn({
        windowId: 'quickbar',
        visible: false,
        fullscreen: false,
        mode: null,
        reason: 'space_changed',
      }),
    );
    expect(globalThis.localStorage.getItem(QUICKBAR_DRAFT_STORAGE_KEY)).not.toBeNull();

    // 重新初始化（新 Pinia 实例模拟重新呼出）
    const pinia2 = createPinia();
    setActivePinia(pinia2);
    const store2 = useQuickBarStore();
    await store2.initialize();
    expect(store2.draft).toBe('未提交的灵感草稿');
    expect(store2.captureMode).toBe('todo');

    // ⌘↵ 提交
    mockNoteAppend.mockResolvedValueOnce({
      relPath: 'work-a/plan.md',
      title: '项目规划',
      addedLines: 1,
      sectionHeading: null,
      bodyHash: 'h1',
      mtimeMs: 1,
      undoAvailable: true,
    });
    await store2.submitAppend(false);
    expect(store2.draft).toBe('');
    expect(globalThis.localStorage.getItem(QUICKBAR_DRAFT_STORAGE_KEY)).toBeNull();
    expect(mockWindowHideQuickBar).toHaveBeenCalledWith({ reason: 'esc' });
    expect(mockWindowShowMain).not.toHaveBeenCalled();

    // ⇧⌘↵ 提交并升级主窗口
    store2.draft = '升级到主窗口继续编辑';
    await nextTick();
    mockNoteAppend.mockResolvedValueOnce({
      relPath: 'work-a/plan.md',
      title: '项目规划',
      addedLines: 1,
      sectionHeading: null,
      bodyHash: 'h2',
      mtimeMs: 2,
      undoAvailable: true,
    });
    await store2.submitAppend(true);
    expect(store2.draft).toBe('');
    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: null });
  });
});
