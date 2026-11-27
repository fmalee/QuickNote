import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NavTreeSnapshot, NoteDoc, NoteMeta, UiState, WriteResult } from '@/bindings/types';

const {
  mockNoteRead,
  mockNoteWrite,
  mockNoteUpdateMeta,
  mockInboxExtractToNote,
  mockNotesList,
  mockNotesGetTreeAndTags,
  mockUiStateGet,
  mockUiStateSet,
  mockOnFsExternalChange,
  mockOnIndexStatus,
} = vi.hoisted(() => ({
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockNoteUpdateMeta: vi.fn(),
  mockInboxExtractToNote: vi.fn(),
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
  noteUpdateMeta: mockNoteUpdateMeta,
  inboxExtractToNote: mockInboxExtractToNote,
  notesList: mockNotesList,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: mockOnFsExternalChange,
  onIndexStatus: mockOnIndexStatus,
}));

import {
  DocHeaderBar,
  DocPropertiesPopover,
  InboxExtractModal,
  needsExternalFrontmatterConfirm,
  useEditorTabsStore,
} from '@/features/editor';

function makeMeta(overrides: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: '01JTESTNOTE00000000000001',
    title: '测试笔记',
    tags: [],
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
    key: 'notes/demo.md',
    relPath: 'notes/demo.md',
    absPath: '/vault/notes/demo.md',
    isLibrary: true,
    isInbox: false,
    meta: makeMeta(),
    body: '原始正文内容',
    bodyHash: 'hash-v1',
    mtimeMs: 1000,
    sizeBytes: 32,
    ...overrides,
  };
}

function makeUiState(): UiState {
  return {
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
    windows: {
      main: {
        isOpen: true,
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

describe('doc meta editing, external frontmatter guard & inbox extract (M2-04)', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();

    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([]);
    mockNotesGetTreeAndTags.mockResolvedValue({
      dirs: [],
      tags: [],
    } satisfies NavTreeSnapshot);
    mockOnFsExternalChange.mockResolvedValue(() => undefined);
    mockOnIndexStatus.mockResolvedValue(() => undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('updateActiveMeta 调用 noteUpdateMeta 更新全部属性并保留未保存的 draftBody (D-0006)', async () => {
    const initialDoc = makeDoc();
    mockNoteRead.mockResolvedValue(initialDoc);

    const store = useEditorTabsStore();
    await store.openNote('notes/demo.md', { preview: false });

    // 模拟用户在编辑器中修改正文但尚未保存
    store.updateTabBody(initialDoc.key, '用户未保存的正文草稿\n第二行', 5, 2, 1);
    expect(store.activeTab?.isDirty).toBe(true);

    const updatedMeta = makeMeta({
      docType: 'doc',
      tags: ['dev/m2', 'rust'],
      openDir: '/Users/Joey/Sites/Projects/QuickNote',
      codeWorkspace: '/Users/Joey/QuickNote.code-workspace',
      terminalDir: '/Users/Joey/Sites/Projects/QuickNote/src-tauri',
      pinned: true,
      archived: true,
    });
    mockNoteUpdateMeta.mockResolvedValue(
      makeDoc({
        meta: updatedMeta,
        body: '磁盘旧正文',
        mtimeMs: 2500,
      }),
    );

    const result = await store.updateActiveMeta({
      docType: 'doc',
      tags: ['dev/m2', 'rust'],
      openDir: '/Users/Joey/Sites/Projects/QuickNote',
      codeWorkspace: '/Users/Joey/QuickNote.code-workspace',
      terminalDir: '/Users/Joey/Sites/Projects/QuickNote/src-tauri',
      pinned: true,
      archived: true,
    });

    expect(mockNoteUpdateMeta).toHaveBeenCalledWith({
      absPath: '/vault/notes/demo.md',
      patch: {
        docType: 'doc',
        tags: ['dev/m2', 'rust'],
        openDir: '/Users/Joey/Sites/Projects/QuickNote',
        codeWorkspace: '/Users/Joey/QuickNote.code-workspace',
        terminalDir: '/Users/Joey/Sites/Projects/QuickNote/src-tauri',
        pinned: true,
        archived: true,
      },
    });
    expect(result?.meta).toEqual(updatedMeta);
    expect(store.activeDoc?.meta.tags).toEqual(['dev/m2', 'rust']);
    expect(store.activeDoc?.meta.pinned).toBe(true);
    expect(store.activeDoc?.mtimeMs).toBe(2500);
    // D-0006 核心红线：绝不覆写编辑器中未保存的 draftBody
    expect(store.activeTab?.draftBody).toBe('用户未保存的正文草稿\n第二行');
    expect(store.activeTab?.isDirty).toBe(true);
  });

  it('needsExternalFrontmatterConfirm 正确识别外部无 frontmatter 文件', () => {
    const externalNoFm = makeDoc({
      key: '/tmp/readme.md',
      relPath: null,
      absPath: '/tmp/readme.md',
      isLibrary: false,
      meta: makeMeta({
        id: null,
        created: null,
        tags: [],
        openDir: null,
        codeWorkspace: null,
        terminalDir: null,
        pinned: false,
        archived: false,
        docType: 'note',
        metaError: null,
      }),
    });
    expect(needsExternalFrontmatterConfirm(externalNoFm)).toBe(true);

    const libraryDoc = makeDoc({
      isLibrary: true,
      meta: externalNoFm.meta,
    });
    expect(needsExternalFrontmatterConfirm(libraryDoc)).toBe(false);

    const externalWithId = makeDoc({
      ...externalNoFm,
      meta: makeMeta({ id: '01JEXT000000000000000001' }),
    });
    expect(needsExternalFrontmatterConfirm(externalWithId)).toBe(false);

    const externalWithTag = makeDoc({
      ...externalNoFm,
      meta: makeMeta({ id: null, created: null, tags: ['work'] }),
    });
    expect(needsExternalFrontmatterConfirm(externalWithTag)).toBe(false);
  });

  it('extractInboxSelectionToNote 先 flushSave 再提取并刷新 inbox.md 且打开新笔记', async () => {
    const inboxDoc = makeDoc({
      key: 'inbox.md',
      relPath: 'inbox.md',
      absPath: '/vault/inbox.md',
      isInbox: true,
      body: '- 10:00 第一条\n- 10:05 重要想法\n- 10:10 后续任务',
      bodyHash: 'inbox-hash-1',
    });
    mockNoteRead.mockResolvedValueOnce(inboxDoc);

    const store = useEditorTabsStore();
    await store.openNote('inbox.md', { preview: false });

    // 修改 inbox 并设置选区为第 2-3 行
    store.updateTabBody(
      'inbox.md',
      '- 10:00 第一条\n- 10:05 重要想法\n- 10:10 后续任务\n',
      1,
      1,
      1,
    );
    store.updateTabSelection('inbox.md', 2, 3, true);
    expect(store.activeTab?.isDirty).toBe(true);
    expect(store.activeTab?.hasSelection).toBe(true);

    const writeResult: WriteResult = {
      bodyHash: 'inbox-hash-2',
      mtimeMs: 1500,
      updatedMeta: null,
    };
    mockNoteWrite.mockResolvedValueOnce(writeResult);

    const extractedDoc = makeDoc({
      key: 'projects/new-idea.md',
      relPath: 'projects/new-idea.md',
      absPath: '/vault/projects/new-idea.md',
      meta: makeMeta({ title: '重要想法', docType: 'doc' }),
      body: '- 10:05 重要想法\n- 10:10 后续任务',
      bodyHash: 'new-doc-hash',
    });
    mockInboxExtractToNote.mockResolvedValueOnce(extractedDoc);

    const refreshedInbox = makeDoc({
      ...inboxDoc,
      body: '- 10:00 第一条\n',
      bodyHash: 'inbox-hash-3',
      mtimeMs: 1600,
    });
    mockNoteRead.mockResolvedValueOnce(refreshedInbox);
    mockNoteRead.mockResolvedValueOnce(extractedDoc);

    const result = await store.extractInboxSelectionToNote({
      newTitle: '重要想法',
      targetDirRelPath: 'projects',
      templateName: 'doc',
    });

    expect(mockNoteWrite).toHaveBeenCalledTimes(1);
    expect(mockInboxExtractToNote).toHaveBeenCalledWith({
      fromLine: 2,
      toLine: 3,
      newTitle: '重要想法',
      targetDirRelPath: 'projects',
      templateName: 'doc',
    });
    expect(result?.relPath).toBe('projects/new-idea.md');

    const inboxTab = store.tabs.find((t) => t.key === 'inbox.md');
    expect(inboxTab?.draftBody).toBe('- 10:00 第一条\n');
    expect(inboxTab?.syncSeq).toBe(1);
    expect(inboxTab?.hasSelection).toBe(false);

    expect(store.activeKey).toBe('projects/new-idea.md');
    expect(store.activeTab?.isPinned).toBe(true);
    expect(store.activeTab?.isPreview).toBe(false);
  });

  it('DocHeaderBar / DocPropertiesPopover / InboxExtractModal SSR 渲染无异常且文档类型位于属性表单', async () => {
    const doc = makeDoc({
      isInbox: true,
      relPath: 'inbox.md',
      key: 'inbox.md',
      meta: makeMeta({ docType: 'command-book' }),
    });
    mockNoteRead.mockResolvedValueOnce(doc);
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useEditorTabsStore();
    await store.openNote('inbox.md', { preview: false });
    store.updateTabSelection('inbox.md', 1, 2, true);
    store.openInboxExtractModal();

    const app = createSSRApp({
      components: { DocHeaderBar, DocPropertiesPopover, InboxExtractModal },
      setup() {
        return { meta: doc.meta };
      },
      template:
        '<div><DocHeaderBar /><DocPropertiesPopover :open="true" :meta="meta" /><InboxExtractModal /></div>',
    });
    app.use(pinia);
    const html = await renderToString(app);
    expect(html).toContain('<div>');
    expect(html).not.toContain('data-testid="doc-header-type-select"');
    expect(html).toContain('data-testid="doc-prop-type-select"');
    expect(html).toContain('value="command-book"');
    expect(html).toContain('>command-book</option>');
  });
});
