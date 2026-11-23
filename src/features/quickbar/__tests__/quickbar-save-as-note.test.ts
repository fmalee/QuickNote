import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  LibraryInfo,
  NavTreeSnapshot,
  NoteDoc,
  NoteMeta,
  NoteSummary,
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
  mockNoteCreate,
  mockNoteRead,
  mockSearchNotesAndFiles,
  mockNotesGetTreeAndTags,
  mockNotesList,
  mockWindowShowMain,
  mockWindowHideQuickBar,
  mockOnWindowState,
  mockOnIndexStatus,
  mockOnFsExternalChange,
} = vi.hoisted(() => ({
  mockLibraryGetInfo: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockCaptureResolveTarget: vi.fn(),
  mockNoteAppend: vi.fn(),
  mockNoteCreate: vi.fn(),
  mockNoteRead: vi.fn(),
  mockSearchNotesAndFiles: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockNotesList: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockWindowHideQuickBar: vi.fn(),
  mockOnWindowState: vi.fn(),
  mockOnIndexStatus: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  libraryGetInfo: mockLibraryGetInfo,
  settingsGet: mockSettingsGet,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  captureResolveTarget: mockCaptureResolveTarget,
  noteAppend: mockNoteAppend,
  noteCreate: mockNoteCreate,
  noteRead: mockNoteRead,
  searchNotesAndFiles: mockSearchNotesAndFiles,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  notesList: mockNotesList,
  windowShowMain: mockWindowShowMain,
  windowHideQuickBar: mockWindowHideQuickBar,
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onWindowState: mockOnWindowState,
  onIndexStatus: mockOnIndexStatus,
  onFsExternalChange: mockOnFsExternalChange,
}));

import { deriveTitleFromContent, inferDefaultTemplateName, useNotesStore } from '@/features/notes';
import { QUICKBAR_DRAFT_STORAGE_KEY, useQuickBarStore } from '@/features/quickbar/store';

function makeMeta(overrides: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: '01JTESTNOTE00000000000001',
    title: '新笔记',
    tags: [],
    project: null,
    docType: 'note',
    openDir: null,
    codeWorkspace: null,
    terminalDir: null,
    created: '2026-11-24T00:00:00+08:00',
    pinned: false,
    archived: false,
    metaError: null,
    ...overrides,
  };
}

function makeDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  return {
    key: 'projects/quicknote/todo-note.md',
    relPath: 'projects/quicknote/todo-note.md',
    absPath: '/vault/projects/quicknote/todo-note.md',
    isLibrary: true,
    isInbox: false,
    meta: makeMeta(),
    body: '- [ ] 完成 M2-04 验收',
    bodyHash: 'hash-1',
    mtimeMs: 1000,
    sizeBytes: 32,
    ...overrides,
  };
}

function makeUiState(): UiState {
  return {
    selectedWorkspaceKey: null,
    workspaceModes: {},
    filters: {
      activeTab: 'inbox',
      selectedDir: null,
      selectedTags: [],
      listScrollTop: 0,
    },
    tabs: {
      items: [],
      activeKey: null,
      closedStack: [],
    },
    lastActiveDoc: null,
    panelGroup: {
      visible: true,
      activeId: 'outline',
      width: 260,
      collapsed: false,
    },
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
  };
}

describe('template utils & Quick Bar save-as-note (M2-04)', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    if (typeof globalThis.localStorage !== 'undefined') {
      globalThis.localStorage.removeItem(QUICKBAR_DRAFT_STORAGE_KEY);
    }

    mockLibraryGetInfo.mockResolvedValue({
      root: '/vault',
      noteCount: 1,
      lastIndexedAt: 1000,
    } satisfies LibraryInfo);
    mockSettingsGet.mockResolvedValue({
      loadStatus: 'ok',
      errorDetail: null,
      settings: {
        library: { root: '/vault' },
        hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space' },
        inbox: { bloatThresholdLines: 500 },
        editor: {
          autoSaveDebounceMs: 600,
          autoSaveMaxWaitMs: 2000,
          maxWidth: 900,
        },
        quickbar: {
          defaultMode: 'list',
          relatedDebounceMs: 80,
        },
        terminal: {
          idleRecycleMinutes: 30,
        },
        workspaces: {},
      },
    } as unknown as SettingsSnapshot);
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
      inboxCount: 0,
      dirs: [
        { relPath: 'projects', name: 'projects', count: 1, children: [] },
        { relPath: 'commands', name: 'commands', count: 1, children: [] },
      ],
      tags: [],
      workspaces: [],
    } satisfies NavTreeSnapshot);
    mockNotesList.mockResolvedValue([
      {
        key: '_templates/doc.md',
        relPath: '_templates/doc.md',
        absPath: '/vault/_templates/doc.md',
        title: '项目文档模板',
        docType: 'template',
        tags: [],
        project: null,
        excerpt: '',
        pinned: false,
        archived: false,
        isInbox: false,
        mtimeMs: 1000,
        metaError: null,
      } satisfies NoteSummary,
    ]);
    mockWindowHideQuickBar.mockResolvedValue({
      windowId: 'quickbar',
      visible: false,
    } as WindowState);
    mockWindowShowMain.mockResolvedValue({
      windowId: 'main',
      visible: true,
    } as WindowState);
    mockOnWindowState.mockResolvedValue(() => undefined);
    mockOnIndexStatus.mockResolvedValue(() => undefined);
    mockOnFsExternalChange.mockResolvedValue(() => undefined);
  });

  it('inferDefaultTemplateName 与 deriveTitleFromContent 遵循契约推导规则', () => {
    expect(inferDefaultTemplateName('commands')).toBe('command-book');
    expect(inferDefaultTemplateName('commands/git')).toBe('command-book');
    expect(inferDefaultTemplateName('projects')).toBe('doc');
    expect(inferDefaultTemplateName('projects/quicknote')).toBe('doc');
    expect(inferDefaultTemplateName('')).toBe('note');
    expect(inferDefaultTemplateName(null)).toBe('note');
    expect(inferDefaultTemplateName('notes/daily')).toBe('note');

    expect(deriveTitleFromContent('- 14:30 完成 QuickNote 设计评审')).toBe(
      '完成 QuickNote 设计评审',
    );
    expect(deriveTitleFromContent('## 实现文档头属性弹窗')).toBe('实现文档头属性弹窗');
    expect(deriveTitleFromContent('- [ ] 补充单元测试用例')).toBe('补充单元测试用例');
    expect(deriveTitleFromContent('   \n   ')).toBe('未命名笔记');
    expect(deriveTitleFromContent('12345678901234567890123456789012345').length).toBe(30);
  });

  it('Quick Bar openSaveAsNoteDialog + confirmSaveAsNote 创建新笔记、清空草稿并激活主窗口', async () => {
    const store = useQuickBarStore();
    await store.initialize();

    store.draft = '完成 M2-04 验收';
    store.setCaptureMode('todo');
    store.openSaveAsNoteDialog();

    expect(store.saveAsModalOpen).toBe(true);
    expect(store.saveAsTitle).toBe('完成 M2-04 验收');
    expect(store.saveAsTemplate).toBe('note');

    store.saveAsDir = 'projects/quicknote';
    store.saveAsTemplate = 'doc';

    const createdDoc = makeDoc();
    mockNoteCreate.mockResolvedValueOnce(createdDoc);

    const doc = await store.confirmSaveAsNote();

    expect(mockNoteCreate).toHaveBeenCalledWith({
      title: '完成 M2-04 验收',
      dirRelPath: 'projects/quicknote',
      templateName: 'doc',
      body: '- [ ] 完成 M2-04 验收',
    });
    expect(doc?.relPath).toBe('projects/quicknote/todo-note.md');
    expect(store.draft).toBe('');
    expect(store.saveAsModalOpen).toBe(false);
    expect(mockUiStateSet).toHaveBeenCalledWith({
      key: 'lastActiveDoc',
      value: {
        key: createdDoc.key,
        relPath: createdDoc.relPath,
        absPath: createdDoc.absPath,
      },
    });
    expect(mockWindowHideQuickBar).toHaveBeenCalledWith({ reason: 'esc' });
    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: null });
  });

  it('notesStore.createNoteAndOpen 传递 templateName 并在新固定标签打开', async () => {
    const createdDoc = makeDoc({
      key: 'commands/git-tips.md',
      relPath: 'commands/git-tips.md',
      absPath: '/vault/commands/git-tips.md',
      meta: makeMeta({ title: 'Git 常用命令', docType: 'command-book' }),
    });
    mockNoteCreate.mockResolvedValueOnce(createdDoc);
    mockNoteRead.mockResolvedValueOnce(createdDoc);

    const notesStore = useNotesStore();
    const doc = await notesStore.createNoteAndOpen({
      title: 'Git 常用命令',
      dirRelPath: 'commands',
      templateName: 'command-book',
    });

    expect(mockNoteCreate).toHaveBeenCalledWith({
      title: 'Git 常用命令',
      dirRelPath: 'commands',
      templateName: 'command-book',
      body: null,
    });
    expect(doc?.relPath).toBe('commands/git-tips.md');
  });
});
