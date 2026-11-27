import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  NavTreeSnapshot,
  NoteDoc,
  NoteMeta,
  NoteSummary,
  UiState,
  WriteResult,
} from '@/bindings/types';

const mockNoteRead = vi.fn();
const mockNoteWrite = vi.fn();
const mockNoteUpdateMeta = vi.fn();
const mockNotesList = vi.fn();
const mockNotesGetTreeAndTags = vi.fn();
const mockUiStateGet = vi.fn();
const mockUiStateSet = vi.fn();
const mockOnFsExternalChange = vi.fn();
const mockOnIndexStatus = vi.fn();
const mockResolveExternalActions = vi.fn();
const mockExternalBookmarksList = vi.fn();

vi.mock('@/lib/ipc/commands', () => ({
  noteRead: (...args: unknown[]) => mockNoteRead(...args),
  noteWrite: (...args: unknown[]) => mockNoteWrite(...args),
  noteUpdateMeta: (...args: unknown[]) => mockNoteUpdateMeta(...args),
  notesList: (...args: unknown[]) => mockNotesList(...args),
  notesGetTreeAndTags: (...args: unknown[]) => mockNotesGetTreeAndTags(...args),
  uiStateGet: (...args: unknown[]) => mockUiStateGet(...args),
  uiStateSet: (...args: unknown[]) => mockUiStateSet(...args),
  externalActionsResolve: (...args: unknown[]) => mockResolveExternalActions(...args),
  externalBookmarksList: (...args: unknown[]) => mockExternalBookmarksList(...args),
  searchNotesAndFiles: vi.fn().mockResolvedValue({ requestId: 1, items: [], truncated: false }),
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: (...args: unknown[]) => mockOnFsExternalChange(...args),
  onIndexStatus: (...args: unknown[]) => mockOnIndexStatus(...args),
}));

import { DocHeaderBar, EditorTabBar, useEditorTabsStore } from '@/features/editor';
import { deriveTabTitle } from '@/features/editor/core/tabModel';
import { useNotesStore } from '@/features/notes';
import { deriveFallbackTitleFromPath, isMarkdownPath } from '@/lib/utils/docTitle';

function makeMeta(overrides: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: '01JTITLE000000000000000001',
    title: '初始一级标题',
    docType: 'note',
    tags: ['dev'],
    openDir: null,
    codeWorkspace: null,
    terminalDir: null,
    created: '2026-11-27T15:00:00+08:00',
    pinned: false,
    archived: false,
    metaError: null,
    ...overrides,
  };
}

function makeDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  return {
    key: '01JTITLE000000000000000001',
    relPath: 'notes/demo.md',
    absPath: '/vault/notes/demo.md',
    isLibrary: true,
    isInbox: false,
    meta: makeMeta(),
    body: '# 初始一级标题\n\n正文内容',
    bodyHash: 'hash-v1',
    mtimeMs: 1000,
    sizeBytes: 32,
    ...overrides,
  };
}

function makeSummary(overrides: Partial<NoteSummary> = {}): NoteSummary {
  return {
    key: '01JTITLE000000000000000001',
    relPath: 'notes/demo.md',
    absPath: '/vault/notes/demo.md',
    title: '初始一级标题',
    docType: 'note',
    tags: ['dev'],
    excerpt: '正文内容',
    pinned: false,
    archived: false,
    isInbox: false,
    mtimeMs: 1000,
    metaError: null,
    ...overrides,
  };
}

function makeUiState(
  items: UiState['tabs']['items'] = [],
  activeKey: string | null = null,
): UiState {
  return {
    filters: {
      activeTab: 'projects',
      selectedDir: null,
      selectedTags: [],
      listScrollTop: 0,
    },
    tabs: {
      items,
      activeKey,
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

describe('unified doc title & DocHeaderBar tag bar replacement', () => {
  let pinia = createPinia();

  beforeEach(() => {
    pinia = createPinia();
    setActivePinia(pinia);
    vi.clearAllMocks();
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([]);
    mockNotesGetTreeAndTags.mockResolvedValue({ dirs: [], tags: [] } satisfies NavTreeSnapshot);
    mockOnFsExternalChange.mockResolvedValue(() => undefined);
    mockOnIndexStatus.mockResolvedValue(() => undefined);
    mockResolveExternalActions.mockResolvedValue([]);
    mockExternalBookmarksList.mockResolvedValue([]);
  });

  it('derives fallback titles uniformly for Markdown vs non-Markdown paths', () => {
    expect(isMarkdownPath('notes/hello.md')).toBe(true);
    expect(isMarkdownPath('notes/HELLO.MARKDOWN')).toBe(true);
    expect(isMarkdownPath('/etc/nginx/nginx.conf')).toBe(false);

    expect(deriveFallbackTitleFromPath('notes/hello.md')).toBe('hello');
    expect(deriveFallbackTitleFromPath('/tmp/guide.markdown')).toBe('guide');
    expect(deriveFallbackTitleFromPath('/etc/nginx/nginx.conf')).toBe('nginx.conf');
    expect(deriveFallbackTitleFromPath('/repo/package.json')).toBe('package.json');
    expect(deriveFallbackTitleFromPath('/repo/Makefile')).toBe('Makefile');

    expect(
      deriveTabTitle({
        doc: null,
        cachedTitle: null,
        relPath: 'notes/plain.md',
        absPath: '/vault/notes/plain.md',
      }),
    ).toBe('plain');

    expect(
      deriveTabTitle({
        doc: null,
        cachedTitle: null,
        relPath: null,
        absPath: '/repo/config.yaml',
      }),
    ).toBe('config.yaml');
  });

  it('preserves unified title across cold restore and LRU eviction without falling back to .md', async () => {
    const docA = makeDoc({
      key: 'k-a',
      relPath: 'notes/a.md',
      absPath: '/vault/notes/a.md',
      meta: makeMeta({ id: 'k-a', title: '文档 A 的 H1 标题' }),
    });
    const summaryB = makeSummary({
      key: 'k-b',
      relPath: 'notes/b.md',
      absPath: '/vault/notes/b.md',
      title: '文档 B 的 H1 标题',
    });

    mockUiStateGet.mockResolvedValueOnce(
      makeUiState(
        [
          {
            key: 'k-a',
            relPath: 'notes/a.md',
            absPath: '/vault/notes/a.md',
            isPinned: true,
            isPreview: false,
            scrollTop: 0,
            cursorPos: 0,
          },
          {
            key: 'k-b',
            relPath: 'notes/b.md',
            absPath: '/vault/notes/b.md',
            isPinned: true,
            isPreview: false,
            scrollTop: 0,
            cursorPos: 0,
          },
        ],
        'k-a',
      ),
    );
    mockNotesList.mockResolvedValueOnce([
      makeSummary({
        key: 'k-a',
        relPath: 'notes/a.md',
        absPath: '/vault/notes/a.md',
        title: '文档 A 的 H1 标题',
      }),
      summaryB,
    ]);
    mockNoteRead.mockResolvedValueOnce(docA);

    const tabsStore = useEditorTabsStore();
    await tabsStore.restoreTabsFromUiState();

    // 未激活的 tab B 尚未 hydrate（doc === null），应展示索引中的 H1 标题而非 b.md
    const tabB = tabsStore.tabs.find((t) => t.key === 'k-b');
    expect(tabB?.doc).toBeNull();
    expect(deriveTabTitle(tabB!)).toBe('文档 B 的 H1 标题');

    // 验证 EditorTabBar 渲染一致，不出现 b.md
    const tabbarApp = createSSRApp(EditorTabBar);
    tabbarApp.use(pinia);
    const html = await renderToString(tabbarApp);
    expect(html).toContain('文档 A 的 H1 标题');
    expect(html).toContain('文档 B 的 H1 标题');
    expect(html).not.toContain('>b.md<');
  });

  it('updates tab title and refreshes sidebar together only after save completes', async () => {
    const initialDoc = makeDoc();
    mockNoteRead.mockResolvedValueOnce(initialDoc);

    const tabsStore = useEditorTabsStore();
    const notesStore = useNotesStore();
    const refreshSpy = vi.spyOn(notesStore, 'refreshAll').mockResolvedValue();

    await tabsStore.openNote('notes/demo.md', { preview: false });
    expect(deriveTabTitle(tabsStore.activeTab!)).toBe('初始一级标题');

    // 编辑正文修改一级标题但尚未保存：标题保持稳定
    tabsStore.updateTabBody(initialDoc.key, '# 保存后的新标题\n\n正文内容', 10, 1, 10);
    expect(deriveTabTitle(tabsStore.activeTab!)).toBe('初始一级标题');
    expect(refreshSpy).not.toHaveBeenCalled();

    // 落盘保存后，后端返回 updatedMeta，标签页与左侧栏同时刷新
    const writeRes: WriteResult = {
      bodyHash: 'hash-v2',
      mtimeMs: 2000,
      updatedMeta: makeMeta({ title: '保存后的新标题' }),
    };
    mockNoteWrite.mockResolvedValueOnce(writeRes);

    await tabsStore.flushSave(initialDoc.key);
    expect(deriveTabTitle(tabsStore.activeTab!)).toBe('保存后的新标题');
    expect(tabsStore.activeTab?.cachedTitle).toBe('保存后的新标题');
    expect(refreshSpy).toHaveBeenCalledTimes(1);
  });

  it('renders tag editor in DocHeaderBar top row and removes title input', async () => {
    const doc = makeDoc({ meta: makeMeta({ tags: ['rust', 'ui'] }) });
    mockNoteRead.mockResolvedValueOnce(doc);

    const tabsStore = useEditorTabsStore();
    await tabsStore.openNote('notes/demo.md', { preview: false });

    const app = createSSRApp(DocHeaderBar);
    app.use(pinia);
    const html = await renderToString(app);

    expect(html).not.toContain('data-testid="doc-header-title-input"');
    expect(html).not.toContain('data-testid="doc-header-title-readonly"');
    expect(html).toContain('data-testid="doc-header-tags"');
    expect(html).toContain('#rust');
    expect(html).toContain('#ui');
    expect(html).toContain('data-testid="doc-header-tag-input"');
  });
});
