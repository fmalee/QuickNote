import fs from 'node:fs';
import path from 'node:path';
import { createPinia, setActivePinia } from 'pinia';
import * as vueRuntime from 'vue';
import { createRenderer, nextTick, ssrContextKey } from 'vue';
import { compileScript, compileTemplate, parse } from 'vue/compiler-sfc';
import { EditorState } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NoteDoc, NoteMeta, UiState } from '@/bindings/types';

const {
  mockDispatchedTargets,
  mockViewInstances,
  mockNoteRead,
  mockNotesList,
  mockUiStateGet,
  mockUiStateSet,
  mockOnFsExternalChange,
  mockOnWindowFileDrop,
} = vi.hoisted(() => ({
  mockDispatchedTargets: [] as Array<{ anchor: number; scrollEffectsCount: number }>,
  mockViewInstances: [] as Array<{
    state: EditorState;
    focused: boolean;
  }>,
  mockNoteRead: vi.fn(),
  mockNotesList: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
  mockOnWindowFileDrop: vi.fn(),
}));

vi.mock('@codemirror/view', () => {
  class MockEditorView {
    state: EditorState;
    scrollDOM = { scrollTop: 0 };
    dom = { getBoundingClientRect: () => ({ left: 0, right: 0, top: 0, bottom: 0 }) };
    focused = false;
    composing = false;

    static lineWrapping = [];
    static decorations = { from: () => [] };
    static theme = () => [];
    static updateListener = { of: () => [] };
    static domEventHandlers = () => [];
    static scrollIntoView = (anchor: number, options?: unknown) => ({
      type: 'scrollIntoView',
      anchor,
      options,
    });

    constructor(config: { state: EditorState }) {
      this.state = config.state;
      mockViewInstances.push(this);
    }

    setState(next: EditorState): void {
      this.state = next;
    }

    dispatch(spec: { selection?: { anchor: number }; effects?: unknown }): void {
      const anchor = spec.selection?.anchor ?? this.state.selection.main.anchor;
      this.state = this.state.update({
        selection: { anchor },
      }).state;
      const effectsList = Array.isArray(spec.effects)
        ? spec.effects
        : spec.effects
          ? [spec.effects]
          : [];
      mockDispatchedTargets.push({
        anchor,
        scrollEffectsCount: effectsList.length,
      });
    }

    focus(): void {
      this.focused = true;
    }

    destroy(): void {}
  }

  return {
    EditorView: MockEditorView,
    keymap: { of: () => [] },
    Decoration: {
      mark: () => ({ range: () => ({}) }),
      replace: () => ({ range: () => ({}) }),
      set: () => ({}),
    },
    ViewPlugin: { fromClass: () => [] },
    WidgetType: class {},
  };
});

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: () => false,
  noteRead: mockNoteRead,
  noteWrite: vi.fn(),
  notesList: mockNotesList,
  notesGetTreeAndTags: vi.fn(),
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  appOpenExternalUrl: vi.fn(),
  noteSaveImage: vi.fn(),
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: mockOnFsExternalChange,
  onWindowFileDrop: mockOnWindowFileDrop,
}));

import { MarkdownEditor, useEditorTabsStore } from '@/features/editor';
import { resolveScrollTargetAnchor } from '../extensions/scrollTarget';

function makeMeta(overrides: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: '01JCMD00000000000000000001',
    title: '常用命令',
    tags: [],
    project: null,
    docType: 'command-book',
    openDir: null,
    codeWorkspace: null,
    terminalDir: null,
    created: null,
    pinned: false,
    archived: false,
    metaError: null,
    ...overrides,
  };
}

function makeDoc(body: string): NoteDoc {
  return {
    key: '01JCMD00000000000000000001',
    relPath: 'commands/常用命令.md',
    absPath: '/vault/commands/常用命令.md',
    isInbox: false,
    isLibrary: true,
    meta: makeMeta(),
    body,
    bodyHash: 'hash-cmd',
    mtimeMs: 1000,
    sizeBytes: body.length,
  };
}

function makeUiState(): UiState {
  return {
    windows: {
      main: { x: 0, y: 0, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'recent', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    selectedWorkspaceKey: null,
    lastActiveDoc: null,
    workspaceModes: {},
    hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space', selfCheck: 'ok' },
  };
}

describe('editor scrollTarget line resolution & initial mount consumption', () => {
  beforeEach(() => {
    mockDispatchedTargets.length = 0;
    mockViewInstances.length = 0;
    vi.clearAllMocks();
    mockNotesList.mockResolvedValue([]);
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockOnFsExternalChange.mockResolvedValue(() => undefined);
    mockOnWindowFileDrop.mockResolvedValue(() => undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('resolves 1-based line offset when target.from <= 0 and clamps out-of-range lines', () => {
    const body = ['第一行', '第二行', '第三行', '### 目标命令', '```bash', 'echo hi', '```'].join(
      '\n',
    );
    const state = EditorState.create({ doc: body });

    const line4 = resolveScrollTargetAnchor(state.doc, { line: 4, from: 0 });
    expect(line4.line).toBe(4);
    expect(line4.anchor).toBe(state.doc.line(4).from);
    expect(state.doc.sliceString(line4.anchor, line4.anchor + '### 目标命令'.length)).toBe(
      '### 目标命令',
    );

    const clamped = resolveScrollTargetAnchor(state.doc, { line: 999, from: 0 });
    expect(clamped.line).toBe(7);
    expect(clamped.anchor).toBe(state.doc.line(7).from);
  });

  it('consumes pre-existing scrollTarget on initial MarkdownEditor mount (cross-window Cmd+O with no prior open tab)', async () => {
    const body = [
      '',
      '## Git',
      '',
      '### 撤销最近一次提交，但保留改动',
      '',
      '```bash',
      'git reset --soft HEAD~1',
      '```',
      '',
      '### 强制清理未跟踪文件与目录',
    ].join('\n');
    const doc = makeDoc(body);
    mockNoteRead.mockResolvedValueOnce(doc);

    const pinia = createPinia();
    setActivePinia(pinia);
    const tabsStore = useEditorTabsStore();

    // Open note by absPath with target line 10 while MarkdownEditor is NOT mounted yet
    const opened = await tabsStore.openNoteByAbsPath(doc.absPath, {
      preview: false,
      line: 10,
    });
    expect(opened).not.toBeNull();
    expect(tabsStore.scrollTarget).toMatchObject({
      key: doc.key,
      line: 10,
      from: 0,
    });

    class FakeNode {
      parentNode: FakeNode | null = null;
      childNodes: FakeNode[] = [];
      insertBefore(child: FakeNode, anchor: FakeNode | null): FakeNode {
        child.parentNode = this;
        if (!anchor) {
          this.childNodes.push(child);
        } else {
          const idx = this.childNodes.indexOf(anchor);
          this.childNodes.splice(idx >= 0 ? idx : this.childNodes.length, 0, child);
        }
        return child;
      }
      removeChild(child: FakeNode): void {
        const idx = this.childNodes.indexOf(child);
        if (idx >= 0) {
          this.childNodes.splice(idx, 1);
          child.parentNode = null;
        }
      }
      get nextSibling(): FakeNode | null {
        if (!this.parentNode) return null;
        const idx = this.parentNode.childNodes.indexOf(this);
        return idx >= 0 ? (this.parentNode.childNodes[idx + 1] ?? null) : null;
      }
    }

    class FakeEl extends FakeNode {}

    const relPath = 'src/features/editor/MarkdownEditor.vue';
    const fullPath = path.resolve(process.cwd(), relPath);
    const { descriptor } = parse(fs.readFileSync(fullPath, 'utf8'));
    const script = compileScript(descriptor, { id: relPath });
    const tpl = compileTemplate({
      source: descriptor.template!.content,
      id: relPath,
      filename: relPath,
      compilerOptions: { mode: 'function', bindingMetadata: script.bindings },
    });
    const renderFn = new Function('Vue', tpl.code)(vueRuntime) as unknown;
    const DomMarkdownEditor = Object.assign(
      Object.create(Object.getPrototypeOf(MarkdownEditor)),
      MarkdownEditor,
      { ssrRender: undefined, render: renderFn },
    );

    const { createApp } = createRenderer<FakeNode, FakeEl>({
      createElement: () => new FakeEl(),
      createText: () => new FakeNode(),
      createComment: () => new FakeNode(),
      setText: () => undefined,
      setElementText: () => undefined,
      insert: (child, parent, anchor) => parent.insertBefore(child, anchor ?? null),
      remove: (child) => child.parentNode?.removeChild(child),
      parentNode: (node) => node.parentNode as FakeEl | null,
      nextSibling: (node) => node.nextSibling,
      patchProp: () => undefined,
    });

    const app = createApp(DomMarkdownEditor);
    app.use(pinia);
    app.provide(ssrContextKey, { modules: new Set() });

    try {
      app.mount(new FakeEl());
      await nextTick();

      const expectedOffset = EditorState.create({ doc: body }).doc.line(10).from;
      expect(mockViewInstances[0]?.state.selection.main.anchor).toBe(expectedOffset);
      expect(mockDispatchedTargets[mockDispatchedTargets.length - 1]?.anchor).toBe(expectedOffset);
      expect(mockDispatchedTargets[mockDispatchedTargets.length - 1]?.scrollEffectsCount).toBe(1);
      expect(tabsStore.activeTopLine).toBe(10);
      expect(tabsStore.scrollTarget).toBeNull();

      // Subsequent requestScrollToLine(4, 0) also jumps accurately
      tabsStore.requestScrollToLine(4, 0);
      await nextTick();
      const line4Offset = EditorState.create({ doc: body }).doc.line(4).from;
      expect(mockViewInstances[0]?.state.selection.main.anchor).toBe(line4Offset);
      expect(mockDispatchedTargets[mockDispatchedTargets.length - 1]?.anchor).toBe(line4Offset);
      expect(tabsStore.activeTopLine).toBe(4);
    } finally {
      app.unmount();
      tabsStore.dispose();
    }
  });
});
