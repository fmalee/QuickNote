import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  NotesFilterArgs,
  SettingsSnapshot,
  UiState,
} from '@/bindings/types';

const {
  mockNotesGetTreeAndTags,
  mockNotesList,
  mockNoteRead,
  mockNoteCreate,
  mockNoteRename,
  mockNoteDelete,
  mockNoteUpdateMeta,
  mockUiStateGet,
  mockUiStateSet,
  mockSettingsGet,
  mockLibraryGetInfo,
  mockOnIndexStatus,
  mockOnSettingsStatus,
  mockOnWindowState,
  mockOnFsExternalChange,
} = vi.hoisted(() => ({
  mockNotesGetTreeAndTags: vi.fn(),
  mockNotesList: vi.fn(),
  mockNoteRead: vi.fn(),
  mockNoteCreate: vi.fn(),
  mockNoteRename: vi.fn(),
  mockNoteDelete: vi.fn(),
  mockNoteUpdateMeta: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockLibraryGetInfo: vi.fn(),
  mockOnIndexStatus: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  notesList: mockNotesList,
  noteRead: mockNoteRead,
  noteCreate: mockNoteCreate,
  noteRename: mockNoteRename,
  noteDelete: mockNoteDelete,
  noteUpdateMeta: mockNoteUpdateMeta,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  settingsGet: mockSettingsGet,
  libraryGetInfo: mockLibraryGetInfo,
  librarySetRoot: vi.fn(),
  hotkeyUpdate: vi.fn(),
  windowShowMain: vi.fn(),
  windowHideQuickBar: vi.fn(),
  isAppError: (err: unknown) =>
    typeof err === 'object' &&
    err !== null &&
    'code' in err &&
    'message' in err &&
    typeof (err as { code: unknown }).code === 'string',
}));

vi.mock('@/lib/ipc/events', () => ({
  onIndexStatus: mockOnIndexStatus,
  onSettingsStatus: mockOnSettingsStatus,
  onWindowState: mockOnWindowState,
  onFsExternalChange: mockOnFsExternalChange,
  listenEvent: vi.fn(),
}));

import { InboxBloatBanner, useEditorTabsStore } from '@/features/editor';
import { NotesSidebar, useNotesStore } from '@/features/notes';
import { useSettingsStore } from '@/features/settings';
import { normalizeNavTab } from '../store';

function makeSummary(overrides: Partial<NoteSummary> = {}): NoteSummary {
  const relPath = overrides.relPath === undefined ? 'projects/alpha.md' : overrides.relPath;
  const absPath = overrides.absPath ?? `/vault/${relPath ?? 'external.md'}`;
  return {
    key: overrides.key ?? '01JTESTKEY0000000000000001',
    relPath,
    absPath,
    title: overrides.title ?? 'Alpha Note',
    docType: overrides.docType ?? 'note',
    tags: overrides.tags ?? ['dev'],
    project: overrides.project ?? null,
    excerpt: overrides.excerpt ?? '摘要内容',
    pinned: overrides.pinned ?? false,
    archived: overrides.archived ?? false,
    isInbox: overrides.isInbox ?? false,
    mtimeMs: overrides.mtimeMs ?? 1_700_000_000_000,
    metaError: overrides.metaError ?? null,
  };
}

function makeDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  const relPath = overrides.relPath === undefined ? 'projects/alpha.md' : overrides.relPath;
  const absPath = overrides.absPath ?? `/vault/${relPath ?? 'external.md'}`;
  return {
    key: overrides.key ?? '01JTESTKEY0000000000000001',
    relPath,
    absPath,
    isLibrary: overrides.isLibrary ?? true,
    isInbox: overrides.isInbox ?? false,
    meta: {
      id: overrides.key ?? '01JTESTKEY0000000000000001',
      title: overrides.meta?.title ?? 'Alpha Note',
      docType: overrides.meta?.docType ?? 'note',
      tags: overrides.meta?.tags ?? ['dev'],
      project: overrides.meta?.project ?? null,
      openDir: overrides.meta?.openDir ?? null,
      codeWorkspace: overrides.meta?.codeWorkspace ?? null,
      terminalDir: overrides.meta?.terminalDir ?? null,
      created: '2026-11-24T00:00:00Z',
      pinned: false,
      archived: false,
      metaError: overrides.meta?.metaError ?? null,
    },
    body: overrides.body ?? '# Alpha\nhello world',
    bodyHash: overrides.bodyHash ?? 'hash-1',
    mtimeMs: overrides.mtimeMs ?? 1_700_000_000_000,
    sizeBytes: overrides.sizeBytes ?? 128,
  };
}

function makeUiState(overrides: Partial<UiState> = {}): UiState {
  return {
    windows: overrides.windows ?? {
      main: { x: 100, y: 100, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: overrides.tabs ?? {
      items: [],
      activeKey: null,
      closedStack: [],
    },
    filters: overrides.filters ?? {
      activeTab: 'projects',
      selectedDir: null,
      selectedTags: [],
      listScrollTop: 0,
    },
    selectedWorkspaceKey: overrides.selectedWorkspaceKey ?? null,
    lastActiveDoc: overrides.lastActiveDoc ?? null,
    workspaceModes: overrides.workspaceModes ?? {},
    hotkey: overrides.hotkey ?? {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
  };
}

function makeSettingsSnapshot(bloatThresholdLines = 500): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/vault',
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
      inbox: { bloatThresholdLines },
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

function makeDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('M1-04a 导航树、笔记列表与多标签状态机', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();

    const defaultTree: NavTreeSnapshot = {
      dirs: [{ relPath: 'projects', name: 'projects', count: 2, children: [] }],
      tags: [
        {
          tag: 'dev',
          count: 2,
          children: [{ tag: 'dev/quicknote', count: 1, children: [] }],
        },
      ],
      workspaces: [{ key: 'qn', name: 'QuickNote', root: '/repo/qn', openDir: '/repo/qn' }],
    };

    mockNotesGetTreeAndTags.mockResolvedValue(defaultTree);
    mockNotesList.mockResolvedValue([]);
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockSettingsGet.mockResolvedValue(makeSettingsSnapshot(500));
    mockLibraryGetInfo.mockResolvedValue({
      root: '/vault',
      exists: true,
      hasInbox: true,
      hasSettings: true,
    });
    mockOnIndexStatus.mockResolvedValue(() => undefined);
    mockOnSettingsStatus.mockResolvedValue(() => undefined);
    mockOnWindowState.mockResolvedValue(() => undefined);
    mockOnFsExternalChange.mockResolvedValue(() => undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('支持 5 个导航分段切换、多标签 AND 过滤与 80ms 防抖防竞态', async () => {
    vi.useFakeTimers();
    const notesStore = useNotesStore();

    await notesStore.setActiveTab('projects');
    await notesStore.selectDir('projects');
    expect(mockNotesList).toHaveBeenLastCalledWith({
      tab: 'projects',
      dirRelPath: 'projects',
      tags: [],
      workspaceKey: null,
      filterText: null,
    });

    await notesStore.setActiveTab('tags');
    await notesStore.toggleTag('dev');
    await notesStore.toggleTag('dev/quicknote');
    expect(mockNotesList).toHaveBeenLastCalledWith({
      tab: 'tags',
      dirRelPath: null,
      tags: ['dev', 'dev/quicknote'],
      workspaceKey: null,
      filterText: null,
    });

    await notesStore.clearTags();
    expect(notesStore.selectedTags).toEqual([]);

    await notesStore.setActiveTab('workspaces');
    await notesStore.selectWorkspace('qn');
    expect(mockUiStateSet).toHaveBeenCalledWith({
      key: 'selectedWorkspace',
      value: 'qn',
    });
    expect(mockNotesList).toHaveBeenLastCalledWith({
      tab: 'workspaces',
      dirRelPath: null,
      tags: [],
      workspaceKey: 'qn',
      filterText: null,
    });

    await notesStore.setActiveTab('recent');
    expect(mockNotesList).toHaveBeenLastCalledWith({
      tab: 'recent',
      dirRelPath: null,
      tags: [],
      workspaceKey: null,
      filterText: null,
    });

    // 80ms 防抖与过期响应丢弃
    mockNotesList.mockClear();
    const firstReq = makeDeferred<NoteSummary[]>();
    const secondReq = makeDeferred<NoteSummary[]>();
    mockNotesList
      .mockImplementationOnce((args: NotesFilterArgs) => {
        expect(args.filterText).toBe('old');
        return firstReq.promise;
      })
      .mockImplementationOnce((args: NotesFilterArgs) => {
        expect(args.filterText).toBe('new');
        return secondReq.promise;
      });

    notesStore.setFilterText('old');
    await vi.advanceTimersByTimeAsync(79);
    expect(mockNotesList).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(mockNotesList).toHaveBeenCalledTimes(1);

    notesStore.setFilterText('new');
    await vi.advanceTimersByTimeAsync(80);
    expect(mockNotesList).toHaveBeenCalledTimes(2);

    const freshSummary = makeSummary({ key: 'K_NEW', title: 'Fresh Result' });
    const staleSummary = makeSummary({ key: 'K_OLD', title: 'Stale Result' });

    // 后发请求先返回
    secondReq.resolve([freshSummary]);
    await Promise.resolve();
    await Promise.resolve();
    expect(notesStore.notes.map((n) => n.key)).toEqual(['K_NEW']);

    // 先发请求晚返回，必须丢弃
    firstReq.resolve([staleSummary]);
    await Promise.resolve();
    await Promise.resolve();
    expect(notesStore.notes.map((n) => n.key)).toEqual(['K_NEW']);

    notesStore.dispose();
  });

  it('预览标签互斥替换、双击晋升固定与关闭栈 ⌘⇧T 恢复', async () => {
    const editorTabsStore = useEditorTabsStore();

    const docA = makeDoc({
      key: 'KEY_A',
      relPath: 'a.md',
      absPath: '/vault/a.md',
      meta: {
        id: 'KEY_A',
        title: 'A',
        docType: 'note',
        tags: [],
        project: null,
        openDir: null,
        codeWorkspace: null,
        terminalDir: null,
        created: '',
        pinned: false,
        archived: false,
        metaError: null,
      },
    });
    const docB = makeDoc({
      key: 'KEY_B',
      relPath: 'b.md',
      absPath: '/vault/b.md',
      meta: {
        id: 'KEY_B',
        title: 'B',
        docType: 'note',
        tags: [],
        project: null,
        openDir: null,
        codeWorkspace: null,
        terminalDir: null,
        created: '',
        pinned: false,
        archived: false,
        metaError: null,
      },
    });

    mockNoteRead.mockImplementation(async ({ absPath }: { absPath: string }) => {
      if (absPath === '/vault/a.md') return docA;
      if (absPath === '/vault/b.md') return docB;
      throw { code: 'notFound', message: 'not found' };
    });

    // 单击打开 A -> 预览标签
    await editorTabsStore.openNote('a.md', { preview: true, absPath: '/vault/a.md' });
    expect(editorTabsStore.tabs).toHaveLength(1);
    expect(editorTabsStore.tabs[0]?.isPreview).toBe(true);
    expect(editorTabsStore.tabs[0]?.isPinned).toBe(false);

    // 单击打开 B -> 替换同一预览标签
    await editorTabsStore.openNote('b.md', { preview: true, absPath: '/vault/b.md' });
    expect(editorTabsStore.tabs).toHaveLength(1);
    expect(editorTabsStore.tabs[0]?.key).toBe('KEY_B');
    expect(editorTabsStore.tabs[0]?.isPreview).toBe(true);

    // 双击或 promoteTabToPinned -> 晋升为固定标签
    editorTabsStore.promoteTabToPinned('KEY_B');
    expect(editorTabsStore.tabs[0]?.isPreview).toBe(false);
    expect(editorTabsStore.tabs[0]?.isPinned).toBe(true);

    // 再单击 A -> 新增 1 个预览标签，此时共 2 个标签
    await editorTabsStore.openNote('a.md', { preview: true, absPath: '/vault/a.md' });
    expect(editorTabsStore.tabs).toHaveLength(2);
    expect(editorTabsStore.activeKey).toBe('KEY_A');

    // 关闭 A -> 推入 closedStack 并激活 B
    editorTabsStore.closeTab('KEY_A');
    expect(editorTabsStore.tabs).toHaveLength(1);
    expect(editorTabsStore.activeKey).toBe('KEY_B');
    expect(editorTabsStore.closedStack).toEqual(['/vault/a.md']);

    // ⌘⇧T 重开最近关闭的标签（作为固定标签打开）
    const reopened = await editorTabsStore.reopenLastClosedTab();
    expect(reopened?.key).toBe('KEY_A');
    expect(reopened?.isPinned).toBe(true);
    expect(reopened?.isPreview).toBe(false);
    expect(editorTabsStore.closedStack).toEqual([]);

    editorTabsStore.dispose();
  });

  it('重命名联动更新已打开标签与启动/搬库 ULID 兜底及失联标记', async () => {
    const notesStore = useNotesStore();
    const editorTabsStore = useEditorTabsStore();

    const docOld = makeDoc({
      key: 'KEY_RENAME',
      relPath: 'projects/old.md',
      absPath: '/vault/projects/old.md',
      meta: {
        id: 'KEY_RENAME',
        title: 'Old Title',
        docType: 'note',
        tags: [],
        project: null,
        openDir: null,
        codeWorkspace: null,
        terminalDir: null,
        created: '',
        pinned: false,
        archived: false,
        metaError: null,
      },
    });
    const docRenamed = makeDoc({
      key: 'KEY_RENAME',
      relPath: 'projects/new.md',
      absPath: '/vault/projects/new.md',
      meta: {
        id: 'KEY_RENAME',
        title: 'New Title',
        docType: 'note',
        tags: [],
        project: null,
        openDir: null,
        codeWorkspace: null,
        terminalDir: null,
        created: '',
        pinned: false,
        archived: false,
        metaError: null,
      },
    });

    mockNoteRead.mockResolvedValueOnce(docOld);
    await editorTabsStore.openNote('projects/old.md', {
      preview: false,
      absPath: '/vault/projects/old.md',
    });

    // inbox.md 禁止重命名与删除
    expect(await notesStore.renameNoteItem('inbox.md', 'other')).toBeNull();
    expect(await notesStore.deleteNoteItem('inbox.md')).toBe(false);

    mockNoteRename.mockResolvedValueOnce(docRenamed);
    const updated = await notesStore.renameNoteItem('projects/old.md', 'new');
    expect(updated?.relPath).toBe('projects/new.md');
    expect(editorTabsStore.tabs[0]?.relPath).toBe('projects/new.md');
    expect(editorTabsStore.tabs[0]?.absPath).toBe('/vault/projects/new.md');
    expect(editorTabsStore.tabs[0]?.doc?.meta.title).toBe('New Title');

    // 启动/搬库恢复：relPath 已变更为 moved/target.md，通过 key (ULID) 兜底重定位；另一个彻底缺失标记 isMissing
    mockUiStateGet.mockResolvedValueOnce(
      makeUiState({
        tabs: {
          items: [
            {
              key: 'KEY_MOVED',
              relPath: 'old/path.md',
              absPath: '/old-vault/old/path.md',
              isPinned: true,
              isPreview: false,
              scrollTop: 10,
              cursorPos: 2,
            },
            {
              key: 'KEY_GONE',
              relPath: 'gone.md',
              absPath: '/old-vault/gone.md',
              isPinned: true,
              isPreview: false,
              scrollTop: 0,
              cursorPos: 0,
            },
          ],
          activeKey: 'KEY_MOVED',
          closedStack: [],
        },
      }),
    );

    const movedSummary = makeSummary({
      key: 'KEY_MOVED',
      relPath: 'moved/target.md',
      absPath: '/new-vault/moved/target.md',
      title: 'Moved Note',
    });
    const movedDoc = makeDoc({
      key: 'KEY_MOVED',
      relPath: 'moved/target.md',
      absPath: '/new-vault/moved/target.md',
    });

    mockNotesList.mockResolvedValueOnce([movedSummary]);
    mockNoteRead.mockImplementation(async ({ absPath }: { absPath: string }) => {
      if (absPath === '/new-vault/moved/target.md') return movedDoc;
      throw { code: 'notFound', message: 'not found' };
    });

    await editorTabsStore.restoreTabsFromUiState();
    expect(editorTabsStore.tabs).toHaveLength(2);
    expect(editorTabsStore.tabs[0]?.relPath).toBe('moved/target.md');
    expect(editorTabsStore.tabs[0]?.absPath).toBe('/new-vault/moved/target.md');
    expect(editorTabsStore.tabs[0]?.isMissing).toBe(false);
    expect(editorTabsStore.tabs[1]?.key).toBe('KEY_GONE');
    expect(editorTabsStore.tabs[1]?.isMissing).toBe(true);

    notesStore.dispose();
    editorTabsStore.dispose();
  });

  it('收件箱超过 500 行时展示膨胀提醒横幅，未超过时不显示', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const settingsStore = useSettingsStore();
    settingsStore.snapshot = makeSettingsSnapshot(500);

    const editorTabsStore = useEditorTabsStore();

    // 500 行（未超过阈值）
    const okLines = Array.from({ length: 500 }, (_, i) => `line ${i + 1}`).join('\n');
    const inboxOkDoc = makeDoc({
      key: 'KEY_INBOX',
      relPath: 'inbox.md',
      absPath: '/vault/inbox.md',
      isInbox: true,
      body: okLines,
    });
    mockNoteRead.mockResolvedValueOnce(inboxOkDoc);
    await editorTabsStore.openNote('inbox.md', { preview: false, absPath: '/vault/inbox.md' });
    expect(editorTabsStore.isInboxBloated).toBe(false);

    const appOk = createSSRApp(InboxBloatBanner);
    appOk.use(pinia);
    const htmlOk = await renderToString(appOk);
    expect(htmlOk).not.toContain('inbox-bloat-banner');

    // 501 行（超过 500 行阈值）
    const bloatedLines = Array.from({ length: 501 }, (_, i) => `line ${i + 1}`).join('\n');
    const inboxBloatedDoc = makeDoc({
      key: 'KEY_INBOX',
      relPath: 'inbox.md',
      absPath: '/vault/inbox.md',
      isInbox: true,
      body: bloatedLines,
    });
    editorTabsStore.handleNoteRenamed('inbox.md', inboxBloatedDoc);
    expect(editorTabsStore.isInboxBloated).toBe(true);

    const appBloated = createSSRApp(InboxBloatBanner);
    appBloated.use(pinia);
    const htmlBloated = await renderToString(appBloated);
    expect(htmlBloated).toContain('inbox-bloat-banner');
    expect(htmlBloated).toContain('收件箱已超过 500 行，建议整理');

    editorTabsStore.dispose();
  });

  it('R21：updateActiveMeta 改标签后列表与标签树立即同步', async () => {
    setActivePinia(createPinia());
    const notesStore = useNotesStore();
    const editorTabsStore = useEditorTabsStore();

    mockNoteRead.mockResolvedValueOnce(makeDoc());
    await editorTabsStore.openNote('projects/alpha.md', {
      preview: false,
      absPath: '/vault/projects/alpha.md',
    });

    const updated = makeDoc({ meta: { ...makeDoc().meta, tags: ['x'] } });
    mockNoteUpdateMeta.mockResolvedValueOnce(updated);
    mockNotesList.mockResolvedValue([makeSummary({ tags: ['x'] })]);
    mockNotesGetTreeAndTags.mockResolvedValue({
      dirs: [],
      tags: [{ tag: 'x', count: 1, children: [] }],
      workspaces: [],
    });

    await editorTabsStore.updateActiveMeta({ tags: ['x'] });
    await vi.waitFor(() => {
      expect(notesStore.notes[0]?.tags).toEqual(['x']);
      expect(notesStore.navTree?.tags[0]?.tag).toBe('x');
    });

    editorTabsStore.dispose();
  });

  it('M5-04c：默认 activeTab 为 projects，且旧持久化值 inbox / workspaces 归一化为 projects', () => {
    const notesStore = useNotesStore();
    expect(notesStore.activeTab).toBe('projects');
    expect(normalizeNavTab('inbox')).toBe('projects');
    expect(normalizeNavTab('workspaces')).toBe('projects');
    expect(normalizeNavTab('unknown')).toBe('projects');
    expect(normalizeNavTab('tags')).toBe('tags');
    expect(normalizeNavTab('recent')).toBe('recent');
  });

  it('M5-04c：左侧导航渲染目录/标签/最近/相关文档/大纲，不含收件箱与工作区且无「项目」文案', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const notesStore = useNotesStore();
    notesStore.navTree = {
      dirs: [{ relPath: 'alpha', name: 'alpha', count: 1, children: [] }],
      tags: [{ tag: 'dev', count: 2, children: [] }],
      workspaces: [],
    };
    notesStore.notes = [makeSummary({ relPath: 'inbox.md', isInbox: true, title: 'Inbox' })];

    const app = createSSRApp(NotesSidebar);
    app.use(pinia);
    const html = await renderToString(app);

    expect(html).toContain('data-testid="nav-tab-projects"');
    expect(html).toContain('data-testid="nav-tab-tags"');
    expect(html).toContain('data-testid="nav-tab-recent"');
    expect(html).toContain('data-testid="nav-tab-related"');
    expect(html).toContain('data-testid="nav-tab-outline"');
    expect(html).not.toContain('data-testid="nav-tab-inbox"');
    expect(html).not.toContain('data-testid="nav-tab-workspaces"');
    expect(html).not.toContain('inbox-count-badge');
    expect(html).not.toContain('项目');
    expect(html).not.toContain('note-rename-btn-');
    expect(html).not.toContain('note-delete-btn-');
    expect(html).toContain('data-testid="left-sidebar-resizer"');
  });

  it('M5-04c：标签云渲染 tag-cloud，点击标签后 notesList 参数 tags 含该标签', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const notesStore = useNotesStore();
    notesStore.navTree = {
      dirs: [],
      tags: [
        {
          tag: 'dev',
          count: 4,
          children: [{ tag: 'dev/rust', count: 1, children: [] }],
        },
      ],
      workspaces: [],
    };
    await notesStore.setActiveTab('tags');

    const app = createSSRApp(NotesSidebar);
    app.use(pinia);
    const html = await renderToString(app);

    expect(html).toContain('data-testid="tag-cloud"');
    expect(html).toContain('data-testid="tag-cloud-item-dev"');
    expect(html).toContain('data-testid="tag-cloud-item-dev/rust"');

    await notesStore.toggleTag('dev');
    expect(mockNotesList).toHaveBeenLastCalledWith({
      tab: 'tags',
      dirRelPath: null,
      tags: ['dev'],
      workspaceKey: null,
      filterText: null,
    });
  });
});
