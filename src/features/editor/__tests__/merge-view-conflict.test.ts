import { createPinia, setActivePinia } from 'pinia';
import { createRenderer, createSSRApp, h, nextTick, ssrContextKey } from 'vue';
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

interface MockMergeConfig {
  a: { doc: string; extensions: unknown[] };
  b: { doc: string; extensions: unknown[] };
  revertControls?: string;
  highlightChanges?: boolean;
  gutter?: boolean;
  collapseUnchanged?: { margin?: number; minSize?: number };
}

interface MockMergeInstance {
  config: MockMergeConfig;
  aDoc: string;
  bDoc: string;
  destroyed: boolean;
  a: {
    state: { doc: { toString: () => string } };
    dispatch: (tr: {
      changes?: { from: number; to: number; insert: string };
      effects?: unknown;
    }) => void;
  };
  b: {
    state: { doc: { toString: () => string } };
    dispatch: (tr: {
      changes?: { from: number; to: number; insert: string };
      effects?: unknown;
    }) => void;
  };
  destroy: () => void;
}

const {
  mockNoteRead,
  mockNoteWrite,
  mockNotesList,
  mockNotesGetTreeAndTags,
  mockUiStateGet,
  mockUiStateSet,
  mockOnFsExternalChange,
  mockOnIndexStatus,
  mergeInstances,
} = vi.hoisted(() => ({
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockNotesList: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
  mockOnIndexStatus: vi.fn(),
  mergeInstances: [] as MockMergeInstance[],
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

vi.mock('@codemirror/merge', () => {
  class MockMergeView {
    config: MockMergeConfig;
    aDoc: string;
    bDoc: string;
    destroyed = false;
    a: MockMergeInstance['a'];
    b: MockMergeInstance['b'];

    constructor(config: MockMergeConfig) {
      this.config = config;
      this.aDoc = config.a.doc;
      this.bDoc = config.b.doc;

      this.a = {
        state: {
          doc: {
            toString: () => this.aDoc,
          },
        },
        dispatch: (tr) => {
          if (tr.changes) {
            const { from, to, insert } = tr.changes;
            this.aDoc = this.aDoc.slice(0, from) + insert + this.aDoc.slice(to);
          }
        },
      };

      this.b = {
        state: {
          doc: {
            toString: () => this.bDoc,
          },
        },
        dispatch: (tr) => {
          if (tr.changes) {
            const { from, to, insert } = tr.changes;
            this.bDoc = this.bDoc.slice(0, from) + insert + this.bDoc.slice(to);
          }
        },
      };

      mergeInstances.push(this);
    }

    destroy(): void {
      this.destroyed = true;
    }
  }

  return { MergeView: MockMergeView };
});

import { EditorConflictBanner, EditorMergeView, useEditorTabsStore } from '@/features/editor';

function makeMeta(overrides: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: '01JTESTMERGE00000000000001',
    title: '冲突合并测试',
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
  const relPath = overrides.relPath ?? 'notes/merge-demo.md';
  return {
    key: overrides.key ?? '01JTESTMERGE00000000000001',
    relPath,
    absPath: overrides.absPath ?? `/vault/${relPath}`,
    isInbox: overrides.isInbox ?? false,
    isLibrary: overrides.isLibrary ?? true,
    meta: makeMeta(overrides.meta),
    body: overrides.body ?? '初始内容',
    bodyHash: overrides.bodyHash ?? 'hash-v1',
    mtimeMs: overrides.mtimeMs ?? 1000,
    sizeBytes: overrides.sizeBytes ?? 64,
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

interface FakeNode {
  tag: string;
  props: Record<string, unknown>;
  children: FakeNode[];
  text?: string;
}

function createFakeHost(): FakeNode {
  return { tag: 'div', props: {}, children: [] };
}

function mountComponentForLifecycle(
  component: typeof EditorMergeView,
  pinia: ReturnType<typeof createPinia>,
): { unmount: () => void; host: FakeNode } {
  const { createApp } = createRenderer<FakeNode, FakeNode>({
    patchProp(el, key, _prevValue, nextValue) {
      el.props[key] = nextValue;
    },
    insert(child, parent) {
      parent.children.push(child);
    },
    remove(child) {
      void child;
    },
    createElement(type) {
      return { tag: type, props: {}, children: [] };
    },
    createText(text) {
      return { tag: '#text', props: {}, children: [], text };
    },
    createComment(text) {
      return { tag: '#comment', props: {}, children: [], text };
    },
    setText(node, text) {
      node.text = text;
    },
    setElementText(node, text) {
      node.text = text;
    },
    parentNode() {
      return null;
    },
    nextSibling() {
      return null;
    },
  });

  const comp = component as unknown as {
    render?: () => unknown;
    ssrRender?: unknown;
  };
  const prevRender = comp.render;
  const prevSsrRender = comp.ssrRender;
  comp.render = () => h('div', { ref: 'containerRef', class: 'qn-merge-view__container' });
  comp.ssrRender = undefined;

  const host = createFakeHost();
  const app = createApp(component);
  app.provide(ssrContextKey, { modules: new Set() });
  app.use(pinia);
  app.mount(host);
  return {
    host,
    unmount: () => {
      app.unmount();
      comp.render = prevRender;
      comp.ssrRender = prevSsrRender;
    },
  };
}

describe('CM6 MergeView conflict resolution', () => {
  let externalChangeHandler: ((event: ExternalChangeEvent) => void) | null = null;
  let pinia: ReturnType<typeof createPinia>;

  beforeEach(() => {
    vi.useFakeTimers();
    pinia = createPinia();
    setActivePinia(pinia);
    externalChangeHandler = null;
    mergeInstances.length = 0;

    mockNoteRead.mockReset();
    mockNoteWrite.mockReset();
    mockNotesList.mockReset();
    mockNotesGetTreeAndTags.mockReset();
    mockUiStateGet.mockReset();
    mockUiStateSet.mockReset();
    mockOnFsExternalChange.mockReset();
    mockOnIndexStatus.mockReset();

    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(undefined);
    mockNotesList.mockResolvedValue([] satisfies NoteSummary[]);
    mockNotesGetTreeAndTags.mockResolvedValue({
      dirs: [],
      tags: [],
      workspaces: [],
      inboxCount: 0,
    } satisfies NavTreeSnapshot);
    mockOnFsExternalChange.mockImplementation((cb: (event: ExternalChangeEvent) => void) => {
      externalChangeHandler = cb;
      return Promise.resolve(() => {});
    });
    mockOnIndexStatus.mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders 4-button conflict banner, hides it in mergeMode, and initializes MergeView panes', async () => {
    const store = useEditorTabsStore();
    await store.ensureExternalChangeListener();

    const doc = makeDoc({
      body: '第一行\n第二行\n',
      bodyHash: 'hash-v1',
    });
    mockNoteRead.mockResolvedValue(doc);
    await store.openNote(doc.key, { preview: false });

    store.updateTabBody(doc.key, '第一行\n本地修改第二行\n');
    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: '第一行\n磁盘外部修改第二行\n',
      newBodyHash: 'hash-disk-2',
      newMeta: null,
    });

    const bannerApp = createSSRApp(EditorConflictBanner);
    bannerApp.use(pinia);
    const bannerHtml = await renderToString(bannerApp);
    expect(bannerHtml).toContain('data-testid="conflict-reload-btn"');
    expect(bannerHtml).toContain('data-testid="conflict-overwrite-btn"');
    expect(bannerHtml).toContain('data-testid="conflict-merge-btn"');
    expect(bannerHtml).toContain('data-testid="conflict-dismiss-btn"');

    // 点击并排对比进入 mergeMode，常规横幅隐藏
    store.enterConflictMerge(doc.key);
    expect(store.activeTab?.conflictState).toMatchObject({
      kind: 'modified',
      mergeMode: true,
      bannerDismissed: false,
    });

    const hiddenBannerApp = createSSRApp(EditorConflictBanner);
    hiddenBannerApp.use(pinia);
    const hiddenBannerHtml = await renderToString(hiddenBannerApp);
    expect(hiddenBannerHtml).not.toContain('data-testid="editor-conflict-banner"');

    // 渲染 EditorMergeView 顶部工具栏与初始化 MergeView 双栏
    const mergeSsrApp = createSSRApp(EditorMergeView);
    mergeSsrApp.use(pinia);
    const mergeHtml = await renderToString(mergeSsrApp);
    expect(mergeHtml).toContain('data-testid="merge-left-badge"');
    expect(mergeHtml).toContain('磁盘最新版本（只读）');
    expect(mergeHtml).toContain('data-testid="merge-right-badge"');
    expect(mergeHtml).toContain('本地未保存版本（可编辑）');
    expect(mergeHtml).toContain('data-testid="merge-accept-all-disk-btn"');
    expect(mergeHtml).toContain('data-testid="merge-cancel-btn"');
    expect(mergeHtml).toContain('data-testid="merge-commit-btn"');

    const { unmount } = mountComponentForLifecycle(EditorMergeView, pinia);
    expect(mergeInstances).toHaveLength(1);
    const instance = mergeInstances[0]!;
    expect(instance.config.revertControls).toBe('a-to-b');
    expect(instance.config.highlightChanges).toBe(true);
    expect(instance.config.gutter).toBe(true);
    expect(instance.config.collapseUnchanged).toEqual({ margin: 3, minSize: 4 });
    expect(instance.a.state.doc.toString()).toBe('第一行\n磁盘外部修改第二行\n');
    expect(instance.b.state.doc.toString()).toBe('第一行\n本地修改第二行\n');

    unmount();
    expect(instance.destroyed).toBe(true);
  });

  it('commits merged body with latest diskBodyHash and exits mergeMode on success', async () => {
    const store = useEditorTabsStore();
    await store.ensureExternalChangeListener();

    const doc = makeDoc({ body: '原文', bodyHash: 'hash-v1' });
    mockNoteRead.mockResolvedValue(doc);
    await store.openNote(doc.key, { preview: false });

    store.updateTabBody(doc.key, '本地编辑版');
    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: '磁盘修改版',
      newBodyHash: 'hash-disk-2',
      newMeta: null,
    });

    store.enterConflictMerge(doc.key);
    const prevSyncSeq = store.activeTab!.syncSeq;
    store.updateMergeDraftBody(doc.key, '人工合并最终版');
    expect(store.activeTab?.draftBody).toBe('人工合并最终版');

    mockNoteWrite.mockResolvedValue({
      bodyHash: 'hash-merged-3',
      mtimeMs: 3000,
      updatedMeta: null,
    } satisfies WriteResult);

    const ok = await store.commitConflictMerge(doc.key, '人工合并最终版');
    expect(ok).toBe(true);
    expect(mockNoteWrite).toHaveBeenCalledWith({
      absPath: doc.absPath,
      body: '人工合并最终版',
      baseBodyHash: 'hash-disk-2',
    });
    expect(store.activeTab?.conflictState).toBeNull();
    expect(store.activeTab?.isDirty).toBe(false);
    expect(store.activeTab?.saveStatus).toBe('saved');
    expect(store.activeTab?.baseBodyHash).toBe('hash-merged-3');
    expect(store.activeTab?.doc?.body).toBe('人工合并最终版');
    expect(store.activeTab?.syncSeq).toBe(prevSyncSeq + 1);
  });

  it('refreshes left disk baseline and preserves right merged draft on secondary Conflict error', async () => {
    const store = useEditorTabsStore();
    await store.ensureExternalChangeListener();

    const doc = makeDoc({ body: 'v1', bodyHash: 'hash-v1' });
    mockNoteRead.mockResolvedValue(doc);
    await store.openNote(doc.key, { preview: false });

    store.updateTabBody(doc.key, '我的合并草稿');
    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: '磁盘版 v2',
      newBodyHash: 'hash-disk-2',
      newMeta: null,
    });
    store.enterConflictMerge(doc.key);

    const { unmount } = mountComponentForLifecycle(EditorMergeView, pinia);
    const instance = mergeInstances[0]!;

    mockNoteWrite.mockRejectedValueOnce({
      code: 'conflict',
      message: 'concurrent modification',
      detail: JSON.stringify({
        currentBody: '磁盘版 v3（二次外部修改）',
        currentBodyHash: 'hash-disk-3',
      }),
    });

    const ok = await store.commitConflictMerge(doc.key, '我的合并草稿-已调好');
    expect(ok).toBe(false);
    await nextTick();

    expect(store.activeTab?.draftBody).toBe('我的合并草稿-已调好');
    expect(store.activeTab?.conflictState).toMatchObject({
      kind: 'modified',
      mergeMode: true,
      diskBody: '磁盘版 v3（二次外部修改）',
      diskBodyHash: 'hash-disk-3',
      mergeSaving: false,
      mergeError: '合并保存期间磁盘又有新修改，已刷新左栏基线，请复核后再次完成合并',
    });

    // 左栏已平滑刷新为磁盘版 v3，右栏保持不重置
    expect(instance.a.state.doc.toString()).toBe('磁盘版 v3（二次外部修改）');
    expect(instance.b.state.doc.toString()).toBe('我的合并草稿');

    // 外部再次推送 modified 事件时，处于 mergeMode 应仅刷新左栏基线不退出 mergeMode
    externalChangeHandler!({
      absPath: doc.absPath,
      kind: 'modified',
      newBody: '磁盘版 v4',
      newBodyHash: 'hash-disk-4',
      newMeta: null,
    });
    await nextTick();
    expect(store.activeTab?.conflictState).toMatchObject({
      kind: 'modified',
      mergeMode: true,
      diskBody: '磁盘版 v4',
      diskBodyHash: 'hash-disk-4',
    });
    expect(store.activeTab?.draftBody).toBe('我的合并草稿-已调好');
    expect(instance.a.state.doc.toString()).toBe('磁盘版 v4');

    unmount();

    // SSR 验证错误提示横幅可见（此时已恢复 ssrRender）
    const errApp = createSSRApp(EditorMergeView);
    errApp.use(pinia);
    const errHtml = await renderToString(errApp);
    expect(errHtml).toContain('data-testid="merge-error-banner"');
    expect(errHtml).toContain('合并保存期间磁盘又有新修改');

    // 复核后以最新 hash-disk-4 再次提交成功
    mockNoteWrite.mockResolvedValueOnce({
      bodyHash: 'hash-merged-5',
      mtimeMs: 5000,
      updatedMeta: null,
    } satisfies WriteResult);
    const retryOk = await store.commitConflictMerge(doc.key, '我的合并草稿-已调好');
    expect(retryOk).toBe(true);
    expect(mockNoteWrite).toHaveBeenLastCalledWith({
      absPath: doc.absPath,
      body: '我的合并草稿-已调好',
      baseBodyHash: 'hash-disk-4',
    });
    expect(store.activeTab?.conflictState).toBeNull();
  });

  it('cancels mergeMode without losing local draft and keeps multi-tab isolation', async () => {
    const store = useEditorTabsStore();
    await store.ensureExternalChangeListener();

    const docA = makeDoc({
      key: '01JTESTMERGE0000000000000A',
      relPath: 'notes/a.md',
      absPath: '/vault/notes/a.md',
      body: 'A 原文',
      bodyHash: 'hash-a1',
    });
    const docB = makeDoc({
      key: '01JTESTMERGE0000000000000B',
      relPath: 'notes/b.md',
      absPath: '/vault/notes/b.md',
      body: 'B 原文',
      bodyHash: 'hash-b1',
    });

    mockNoteRead.mockResolvedValueOnce(docA).mockResolvedValueOnce(docB);
    await store.openNote(docA.key, { preview: false });
    await store.openNote(docB.key, { preview: false });

    await store.activateTab(docA.key);
    store.updateTabBody(docA.key, 'A 本地草稿');
    externalChangeHandler!({
      absPath: docA.absPath,
      kind: 'modified',
      newBody: 'A 磁盘修改',
      newBodyHash: 'hash-a2',
      newMeta: null,
    });
    store.enterConflictMerge(docA.key);

    // 切换到正常标签 B，B 不处于 mergeMode
    await store.activateTab(docB.key);
    expect(store.activeTab?.key).toBe(docB.key);
    expect(store.activeTab?.conflictState).toBeNull();

    // 切回 A，保留 mergeMode 与本地草稿
    await store.activateTab(docA.key);
    expect(store.activeTab?.conflictState).toMatchObject({
      kind: 'modified',
      mergeMode: true,
    });
    expect(store.activeTab?.draftBody).toBe('A 本地草稿');

    // 取消返回：退出 mergeMode，保留本地草稿和冲突横幅
    store.cancelConflictMerge(docA.key);
    expect(store.activeTab?.conflictState).toMatchObject({
      kind: 'modified',
      mergeMode: false,
      bannerDismissed: false,
    });
    expect(store.activeTab?.draftBody).toBe('A 本地草稿');
  });
});
