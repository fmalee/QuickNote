import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ExternalChangeEvent,
  NavTreeSnapshot,
  NoteDoc,
  NoteMeta,
  NoteSummary,
  UiState,
  WriteResult,
} from '@/bindings/types';

const {
  mockNoteRead,
  mockNoteWrite,
  mockNotesList,
  mockNotesGetTreeAndTags,
  mockUiStateGet,
  mockUiStateSet,
  mockOnFsExternalChange,
  mockOnIndexStatus,
} = vi.hoisted(() => ({
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockNotesList: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
  mockOnIndexStatus: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: (err: unknown): err is { code: string; message: string; detail?: string } =>
    typeof err === 'object' && err !== null && 'code' in err && 'message' in err,
  noteRead: mockNoteRead,
  noteWrite: mockNoteWrite,
  notesList: mockNotesList,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: mockOnFsExternalChange,
  onIndexStatus: mockOnIndexStatus,
}));

import { EditorConflictBanner, useEditorTabsStore } from '@/features/editor';
import { useNotesStore } from '@/features/notes';

function makeMeta(overrides: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: '01JTESTNOTE00000000000001',
    title: '测试笔记',
    tags: [],
    project: null,
    docType: 'note',
    openDir: null,
    codeWorkspace: null,
    terminalDir: null,
    created: '2026-11-24T00:00:00Z',
    pinned: false,
    archived: false,
    metaError: null,
    ...overrides,
  };
}

function makeDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  const relPath = overrides.relPath ?? 'notes/demo.md';
  return {
    key: overrides.key ?? '01JTESTNOTE00000000000001',
    relPath,
    absPath: overrides.absPath ?? `/vault/${relPath ?? 'demo.md'}`,
    isInbox: overrides.isInbox ?? false,
    isLibrary: overrides.isLibrary ?? true,
    meta: makeMeta(overrides.meta),
    body: overrides.body ?? '初始正文',
    bodyHash: overrides.bodyHash ?? 'hash-v1',
    mtimeMs: overrides.mtimeMs ?? 1000,
    sizeBytes: overrides.sizeBytes ?? 128,
  };
}

function makeUiState(): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    panelGroup: { visible: true, activeId: 'outline', width: 260, collapsed: false },
    selectedWorkspaceKey: null,
    lastActiveDoc: null,
    workspaceModes: {},
    hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space', selfCheck: 'ok' },
  };
}

describe('editor autosave, ULID backfill and external change handling', () => {
  let externalChangeHandler: ((event: ExternalChangeEvent) => void) | null = null;

  beforeEach(() => {
    vi.useFakeTimers();
    setActivePinia(createPinia());
    externalChangeHandler = null;

    mockNoteRead.mockReset();
    mockNoteWrite.mockReset();
    mockNotesList.mockReset();
    mockNotesGetTreeAndTags.mockReset();
    mockUiStateGet.mockReset();
    mockUiStateSet.mockReset();
    mockOnFsExternalChange.mockReset();
    mockOnIndexStatus.mockReset();

    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([] satisfies NoteSummary[]);
    mockNotesGetTreeAndTags.mockResolvedValue({
      dirs: [],
      tags: [],
      workspaces: [],
      inboxCount: 0,
    } satisfies NavTreeSnapshot);
    mockOnIndexStatus.mockResolvedValue(() => {});
    mockOnFsExternalChange.mockImplementation(async (cb: (event: ExternalChangeEvent) => void) => {
      externalChangeHandler = cb;
      return () => {
        externalChangeHandler = null;
      };
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('promotes preview tab to pinned when editing body', async () => {
    const doc = makeDoc();
    mockNoteRead.mockResolvedValue(doc);

    const store = useEditorTabsStore();
    await store.openNote(doc.relPath!, { preview: true, absPath: doc.absPath });

    expect(store.activeTab?.isPreview).toBe(true);
    expect(store.activeTab?.isPinned).toBe(false);
    expect(store.activeTab?.isDirty).toBe(false);

    store.updateTabBody(doc.key, '修改后的内容', 6, 1, 7);

    expect(store.activeTab?.isPreview).toBe(false);
    expect(store.activeTab?.isPinned).toBe(true);
    expect(store.activeTab?.isDirty).toBe(true);
    expect(store.activeTab?.saveStatus).toBe('dirty');
    expect(store.activeTab?.cursorLine).toBe(1);
    expect(store.activeTab?.cursorCol).toBe(7);
  });

  it('auto-saves after 600ms debounce, forces save at 2000ms maxWait, and backfills ULID', async () => {
    const legacyDoc = makeDoc({
      key: 'path:notes/legacy.md',
      relPath: 'notes/legacy.md',
      absPath: '/vault/notes/legacy.md',
      meta: makeMeta({ id: '' }),
    });
    mockNoteRead.mockResolvedValue(legacyDoc);

    const newUlid = '01JBACKFILLEDULID000000001';
    const writeRes: WriteResult = {
      bodyHash: 'hash-v2',
      mtimeMs: 2000,
      updatedMeta: makeMeta({ id: newUlid, title: '旧笔记回填' }),
    };
    mockNoteWrite.mockResolvedValue(writeRes);

    const notesStore = useNotesStore();
    const refreshSpy = vi.spyOn(notesStore, 'refreshAll');
    const store = useEditorTabsStore();

    await store.openNote(legacyDoc.relPath!, { preview: false, absPath: legacyDoc.absPath });

    // 持续高频输入（每 400ms 输入一次，持续 5 次 = 2000ms），验证 maxWait 2000ms 强制保存
    for (let i = 1; i <= 5; i += 1) {
      store.updateTabBody(store.activeTab!.key, `内容-${i}`);
      await vi.advanceTimersByTimeAsync(400);
      if (i < 5) {
        expect(mockNoteWrite).not.toHaveBeenCalled();
      }
    }

    expect(mockNoteWrite).toHaveBeenCalledTimes(1);
    expect(mockNoteWrite).toHaveBeenCalledWith({
      absPath: '/vault/notes/legacy.md',
      body: '内容-5',
      baseBodyHash: 'hash-v1',
    });

    // 验证 ULID 回填更新了 key、meta 并刷新了左侧列表
    expect(store.activeKey).toBe(newUlid);
    expect(store.activeTab?.key).toBe(newUlid);
    expect(store.activeTab?.doc?.meta.id).toBe(newUlid);
    expect(store.activeTab?.baseBodyHash).toBe('hash-v2');
    expect(store.activeTab?.isDirty).toBe(false);
    expect(store.activeTab?.saveStatus).toBe('saved');
    expect(refreshSpy).toHaveBeenCalled();

    // 验证单次输入后 600ms 防抖触发保存
    mockNoteWrite.mockResolvedValue({
      bodyHash: 'hash-v3',
      mtimeMs: 3000,
      updatedMeta: null,
    });
    store.updateTabBody(newUlid, '单次防抖内容');
    await vi.advanceTimersByTimeAsync(599);
    expect(mockNoteWrite).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(mockNoteWrite).toHaveBeenCalledTimes(2);
    expect(store.activeTab?.baseBodyHash).toBe('hash-v3');
    expect(store.activeTab?.saveStatus).toBe('saved');
  });

  it('handles 3-state external changes: clean silent sync, dirty conflict, and deleted preservation', async () => {
    const doc = makeDoc();
    mockNoteRead.mockResolvedValue(doc);

    const store = useEditorTabsStore();
    await store.ensureExternalChangeListener();
    await store.openNote(doc.relPath!, { preview: false, absPath: doc.absPath });
    expect(externalChangeHandler).not.toBeNull();

    // 1. 干净态收到外部修改 -> 静默更新
    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: '外部同步的正文',
      newBodyHash: 'hash-ext-1',
      newMeta: makeMeta({ title: '外部新标题' }),
    });

    expect(store.activeTab?.draftBody).toBe('外部同步的正文');
    expect(store.activeTab?.baseBodyHash).toBe('hash-ext-1');
    expect(store.activeTab?.isDirty).toBe(false);
    expect(store.activeTab?.saveStatus).toBe('external-synced');
    expect(store.activeTab?.doc?.meta.title).toBe('外部新标题');

    // 2. 脏态收到外部修改 -> 立即暂停自动保存并进入冲突态
    store.updateTabBody(doc.key, '本地未保存草稿');
    expect(store.activeTab?.isDirty).toBe(true);

    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: '磁盘冲突版本',
      newBodyHash: 'hash-disk-2',
      newMeta: null,
    });

    expect(store.activeTab?.conflictState).toEqual({
      kind: 'modified',
      diskBody: '磁盘冲突版本',
      diskBodyHash: 'hash-disk-2',
      diskMeta: null,
    });
    expect(store.activeTab?.saveStatus).toBe('conflict');

    // 推进 5000ms 也不得自动保存覆盖磁盘
    await vi.advanceTimersByTimeAsync(5000);
    expect(mockNoteWrite).not.toHaveBeenCalled();

    // 渲染 ConflictBanner 验证 UI 按钮存在
    const pinia = createPinia();
    setActivePinia(pinia);
    const uiStore = useEditorTabsStore();
    uiStore.tabs = [...store.tabs];
    uiStore.activeKey = store.activeKey;
    const conflictApp = createSSRApp(EditorConflictBanner);
    conflictApp.use(pinia);
    const conflictHtml = await renderToString(conflictApp);
    expect(conflictHtml).toContain('data-testid="editor-conflict-banner"');
    expect(conflictHtml).toContain('重新加载磁盘版本');
    expect(conflictHtml).toContain('备份并覆盖磁盘');

    // 测试“备份并覆盖磁盘”
    mockNoteWrite.mockResolvedValue({
      bodyHash: 'hash-overwrite-3',
      mtimeMs: 4000,
      updatedMeta: null,
    });
    const okOverwrite = await store.resolveConflictOverwrite(doc.key);
    expect(okOverwrite).toBe(true);
    expect(mockNoteWrite).toHaveBeenCalledWith({
      absPath: doc.absPath,
      body: '本地未保存草稿',
      baseBodyHash: 'hash-disk-2',
    });
    expect(store.activeTab?.conflictState).toBeNull();
    expect(store.activeTab?.saveStatus).toBe('saved');

    // 再测一次“重新加载磁盘版本”
    store.updateTabBody(doc.key, '准备放弃的本地修改');
    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: '磁盘最新内容',
      newBodyHash: 'hash-disk-4',
      newMeta: null,
    });
    store.resolveConflictReload(doc.key);
    expect(store.activeTab?.draftBody).toBe('磁盘最新内容');
    expect(store.activeTab?.baseBodyHash).toBe('hash-disk-4');
    expect(store.activeTab?.isDirty).toBe(false);
    expect(store.activeTab?.conflictState).toBeNull();

    // 3. 收到外部删除 -> 不关闭标签、保留 draftBody 并支持写回原路径
    store.updateTabBody(doc.key, '绝对不能丢失的正文');
    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'deleted',
      newBody: null,
      newBodyHash: null,
      newMeta: null,
    });

    expect(store.tabs.length).toBe(1);
    expect(store.activeTab?.draftBody).toBe('绝对不能丢失的正文');
    expect(store.activeTab?.conflictState).toEqual({ kind: 'deleted' });
    expect(store.activeTab?.saveStatus).toBe('deleted');

    await vi.advanceTimersByTimeAsync(5000);
    expect(mockNoteWrite).toHaveBeenCalledTimes(1);

    // 写回原路径
    mockNoteWrite.mockResolvedValue({
      bodyHash: 'hash-restored-5',
      mtimeMs: 5000,
      updatedMeta: null,
    });
    const restored = await store.restoreDeletedTabToDisk(doc.key);
    expect(restored).toBe(true);
    expect(store.activeTab?.conflictState).toBeNull();
    expect(store.activeTab?.saveStatus).toBe('saved');
    expect(store.activeTab?.baseBodyHash).toBe('hash-restored-5');
  });

  it('flushSave 正在保存时防重入，不并发发出携带旧 baseBodyHash 的 writeNote', async () => {
    const doc = makeDoc();
    mockNoteRead.mockResolvedValue(doc);

    let resolveWrite: ((value: WriteResult) => void) | null = null;
    mockNoteWrite.mockImplementation(
      () =>
        new Promise<WriteResult>((resolve) => {
          resolveWrite = resolve;
        }),
    );

    const store = useEditorTabsStore();
    await store.openNote(doc.relPath!, { preview: false, absPath: doc.absPath });
    store.updateTabBody(doc.key, '第一版待保存正文');

    const firstFlush = store.flushSave(doc.key);
    expect(store.activeTab?.saveStatus).toBe('saving');
    expect(mockNoteWrite).toHaveBeenCalledTimes(1);

    // 在首次 writeNote 尚未返回前再次触发 flushSave，绝不能并发请求
    const secondFlush = store.flushSave(doc.key);
    expect(mockNoteWrite).toHaveBeenCalledTimes(1);

    resolveWrite!({
      bodyHash: 'hash-v2',
      mtimeMs: 2000,
      updatedMeta: null,
    });
    await Promise.all([firstFlush, secondFlush]);

    expect(mockNoteWrite).toHaveBeenCalledTimes(1);
    expect(store.activeTab?.saveStatus).toBe('saved');
    expect(store.activeTab?.baseBodyHash).toBe('hash-v2');
  });

  it('启动恢复：激活标签 hydrate 后 activeDoc 响应式更新（R03）', async () => {
    const mk = (key: string, relPath: string) => ({
      key,
      relPath,
      absPath: `/vault/${relPath}`,
      title: relPath,
      docType: 'note' as const,
      tags: [],
      project: null,
      excerpt: '',
      pinned: false,
      archived: false,
      isInbox: false,
      mtimeMs: 1000,
      metaError: null,
    });
    const a = mk('KEY_A', 'notes/a.md');
    const b = mk('KEY_B', 'notes/b.md');
    const item = (s: typeof a) => ({
      key: s.key,
      relPath: s.relPath,
      absPath: s.absPath,
      isPinned: true,
      isPreview: false,
      scrollTop: 0,
      cursorPos: 0,
    });
    const ui = makeUiState();
    ui.tabs = { items: [item(a), item(b)], activeKey: a.key, closedStack: [] };
    mockUiStateGet.mockResolvedValue(ui);
    mockNotesList.mockResolvedValue([a, b] satisfies NoteSummary[]);

    let resolveRead!: (doc: NoteDoc) => void;
    mockNoteRead.mockReturnValue(
      new Promise<NoteDoc>((resolve) => {
        resolveRead = resolve;
      }),
    );

    const store = useEditorTabsStore();
    const restoring = store.restoreTabsFromUiState();
    for (let i = 0; i < 20; i += 1) {
      await Promise.resolve();
    }
    expect(store.activeDoc).toBeNull();

    resolveRead(
      makeDoc({ key: a.key, relPath: a.relPath, absPath: a.absPath, body: '恢复后的正文' }),
    );
    await restoring;

    expect(store.activeDoc?.body).toBe('恢复后的正文');
    expect(store.activeTab?.key).toBe(a.key);
    expect(store.activeTab?.evicted).toBe(false);
  });

  it('回声被忽略：脏态下哈希与元数据均一致的 modified 事件不弹冲突', async () => {
    const doc = makeDoc();
    mockNoteRead.mockResolvedValue(doc);
    const store = useEditorTabsStore();
    await store.ensureExternalChangeListener();
    await store.openNote(doc.relPath!, { preview: false, absPath: doc.absPath });

    store.updateTabBody(doc.key, '本地继续输入的草稿');
    expect(store.activeTab?.isDirty).toBe(true);

    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: doc.body,
      newBodyHash: store.activeTab!.baseBodyHash,
      newMeta: makeMeta(),
    });

    expect(store.activeTab?.conflictState).toBeNull();
    expect(store.activeTab?.saveStatus).not.toBe('conflict');
    expect(store.activeTab?.draftBody).toBe('本地继续输入的草稿');
  });

  it('真实外部修改仍冲突：哈希不同时脏态弹冲突', async () => {
    const doc = makeDoc();
    mockNoteRead.mockResolvedValue(doc);
    const store = useEditorTabsStore();
    await store.ensureExternalChangeListener();
    await store.openNote(doc.relPath!, { preview: false, absPath: doc.absPath });

    store.updateTabBody(doc.key, '本地继续输入的草稿');
    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: '外部编辑器改过的正文',
      newBodyHash: 'hash-real-external',
      newMeta: makeMeta(),
    });

    expect(store.activeTab?.conflictState?.kind).toBe('modified');
    expect(store.activeTab?.saveStatus).toBe('conflict');
  });

  it('组合态不排保存：compositionend 后才调度自动保存', async () => {
    const doc = makeDoc();
    mockNoteRead.mockResolvedValue(doc);
    mockNoteWrite.mockResolvedValue({ bodyHash: 'hash-v2', mtimeMs: 2000, updatedMeta: null });
    const store = useEditorTabsStore();
    await store.openNote(doc.relPath!, { preview: false, absPath: doc.absPath });

    store.updateTabBody(doc.key, '新正文', 0, 1, 1, true);
    await vi.advanceTimersByTimeAsync(5000);
    expect(mockNoteWrite).not.toHaveBeenCalled();

    store.scheduleAutoSaveIfDirty(doc.key);
    await vi.advanceTimersByTimeAsync(5000);
    expect(mockNoteWrite).toHaveBeenCalledTimes(1);
  });
});
