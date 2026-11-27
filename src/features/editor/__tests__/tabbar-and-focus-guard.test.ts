import fs from 'node:fs';
import path from 'node:path';
import { createPinia, setActivePinia } from 'pinia';
import * as vueRuntime from 'vue';
import { createRenderer, nextTick, ssrContextKey } from 'vue';
import { compileScript, compileTemplate, parse } from 'vue/compiler-sfc';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NoteDoc, NoteMeta, UiState } from '@/bindings/types';

const {
  mockNoteRead,
  mockNotesList,
  mockUiStateGet,
  mockUiStateSet,
  mockOnFsExternalChange,
  mockOnWindowFileDrop,
  mockExternalActionResolve,
  mockExternalActionRun,
} = vi.hoisted(() => ({
  mockNoteRead: vi.fn(),
  mockNotesList: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
  mockOnWindowFileDrop: vi.fn(),
  mockExternalActionResolve: vi.fn(),
  mockExternalActionRun: vi.fn(),
}));

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
  externalActionResolve: mockExternalActionResolve,
  externalActionRun: mockExternalActionRun,
  externalActionsResolve: mockExternalActionResolve,
  externalActionsRun: mockExternalActionRun,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: mockOnFsExternalChange,
  onWindowFileDrop: mockOnWindowFileDrop,
}));

import { EditorTabBar, useEditorTabsStore } from '@/features/editor';

function makeMeta(overrides: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: '01JCMD00000000000000000001',
    title: '测试笔记',
    created: null,
    tags: [],
    openDir: null,
    codeWorkspace: null,
    terminalDir: null,
    pinned: false,
    archived: false,
    docType: 'note',
    metaError: null,
    ...overrides,
  };
}

function makeDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  return {
    key: '01JCMD00000000000000000001',
    relPath: 'inbox.md',
    absPath: '/vault/inbox.md',
    isLibrary: true,
    isInbox: true,
    meta: makeMeta({ title: '收集箱' }),
    body: 'inbox body',
    bodyHash: 'hash-inbox',
    mtimeMs: 1,
    sizeBytes: 10,
    ...overrides,
  };
}

function makeUiState(lastRelPath: string | null = 'inbox.md'): UiState {
  return {
    windows: {
      main: { x: 0, y: 0, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'recent', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    lastActiveDoc: lastRelPath ? { relPath: lastRelPath, touchedAtMs: 100 } : null,
    hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space', selfCheck: 'ok' },
  };
}

describe('tabbar-and-focus-guard (M3-qa)', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mockUiStateGet.mockResolvedValue(makeUiState('inbox.md'));
    mockUiStateSet.mockResolvedValue(undefined);
    mockNotesList.mockResolvedValue([]);
    mockOnFsExternalChange.mockResolvedValue(() => undefined);
    mockOnWindowFileDrop.mockResolvedValue(() => undefined);
    mockExternalActionResolve.mockResolvedValue([]);
    mockExternalActionRun.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('does not let handleMainWindowShown overwrite an explicitly opened library or external tab with old lastActiveDoc', async () => {
    const store = useEditorTabsStore();

    const inboxDoc = makeDoc();
    const libraryNoteDoc = makeDoc({
      key: '01JCMD00000000000000000002',
      relPath: 'notes/target.md',
      absPath: '/vault/notes/target.md',
      isInbox: false,
      meta: makeMeta({ id: '01JCMD00000000000000000002', title: '目标笔记' }),
      body: 'target body',
      bodyHash: 'hash-target',
    });
    const externalDoc = makeDoc({
      key: 'path:/tmp/nginx.conf',
      relPath: null,
      absPath: '/tmp/nginx.conf',
      isLibrary: false,
      isInbox: false,
      meta: makeMeta({ id: null, title: 'nginx.conf' }),
      body: 'server {}',
      bodyHash: 'hash-ext',
    });

    mockNotesList.mockResolvedValue([
      {
        key: inboxDoc.key,
        relPath: inboxDoc.relPath!,
        absPath: inboxDoc.absPath,
        title: '收集箱',
        excerpt: '',
        tags: [],
        docType: 'note',
        pinned: false,
        archived: false,
        mtimeMs: 1,
        metaError: null,
        isInbox: true,
      },
    ]);
    mockNoteRead.mockImplementation(async ({ absPath }: { absPath: string }) => {
      if (absPath === '/vault/inbox.md') return inboxDoc;
      if (absPath === '/vault/notes/target.md') return libraryNoteDoc;
      if (absPath === '/tmp/nginx.conf') return externalDoc;
      throw new Error(`unexpected absPath: ${absPath}`);
    });

    // 1. When tabs is empty, handleMainWindowShown opens lastActiveDoc
    await store.handleMainWindowShown();
    expect(store.activeTab?.relPath).toBe('inbox.md');

    // 2. Quick Bar opens a library note -> handleMainWindowShown must keep it active
    await store.openNoteByAbsPath('/vault/notes/target.md', { preview: true });
    expect(store.activeTab?.absPath).toBe('/vault/notes/target.md');

    await store.handleMainWindowShown();
    expect(store.activeTab?.absPath).toBe('/vault/notes/target.md');

    // 3. Quick Bar opens an external file -> handleMainWindowShown must keep it active
    await store.openNoteByAbsPath('/tmp/nginx.conf', { preview: true });
    expect(store.activeTab?.absPath).toBe('/tmp/nginx.conf');

    await store.handleMainWindowShown();
    expect(store.activeTab?.absPath).toBe('/tmp/nginx.conf');

    // 4. Explicit Quick Bar append event still syncs and activates the appended note
    await store.handleQuickBarAppendEvent({
      relPath: 'inbox.md',
      title: '收集箱',
      addedLines: 1,
      sectionHeading: null,
      bodyHash: 'hash-inbox-next',
      mtimeMs: 2,
      undoAvailable: true,
    });
    await store.handleMainWindowShown();
    expect(store.activeTab?.relPath).toBe('inbox.md');

    store.dispose();
  });

  it('auto-hides tabbar scrolling class after 600ms, translates wheel deltaY to scrollLeft, and scrolls active tab into view', async () => {
    vi.useFakeTimers();

    type Listener = (event: unknown) => void;
    const scrollIntoViewCalls: Array<{ testId: string | null; options: unknown }> = [];

    class FakeNode {
      parentNode: FakeNode | null = null;
      childNodes: FakeNode[] = [];
      listeners = new Map<string, Set<Listener>>();

      insertBefore(child: FakeNode, anchor: FakeNode | null): FakeNode {
        if (child.parentNode) {
          child.parentNode.removeChild(child);
        }
        child.parentNode = this;
        if (!anchor) {
          this.childNodes.push(child);
        } else {
          const idx = this.childNodes.indexOf(anchor);
          if (idx >= 0) {
            this.childNodes.splice(idx, 0, child);
          } else {
            this.childNodes.push(child);
          }
        }
        return child;
      }

      removeChild(child: FakeNode): FakeNode {
        const idx = this.childNodes.indexOf(child);
        if (idx >= 0) {
          this.childNodes.splice(idx, 1);
          child.parentNode = null;
        }
        return child;
      }

      get nextSibling(): FakeNode | null {
        if (!this.parentNode) return null;
        const siblings = this.parentNode.childNodes;
        const idx = siblings.indexOf(this);
        return idx >= 0 && idx + 1 < siblings.length ? (siblings[idx + 1] ?? null) : null;
      }

      addEventListener(type: string, fn: Listener): void {
        let set = this.listeners.get(type);
        if (!set) {
          set = new Set();
          this.listeners.set(type, set);
        }
        set.add(fn);
      }

      removeEventListener(type: string, fn: Listener): void {
        this.listeners.get(type)?.delete(fn);
      }

      dispatchEvent(event: Record<string, unknown>): void {
        const set = this.listeners.get(String(event.type));
        if (set) {
          for (const fn of Array.from(set)) {
            fn(event);
          }
        }
      }
    }

    class FakeElement extends FakeNode {
      tagName: string;
      attributes = new Map<string, string>();
      style: Record<string, string> = {};
      textContent = '';
      scrollWidth = 600;
      clientWidth = 200;
      scrollLeft = 0;

      constructor(tagName: string) {
        super();
        this.tagName = tagName.toUpperCase();
      }

      setAttribute(name: string, val: string): void {
        this.attributes.set(name, String(val));
      }

      getAttribute(name: string): string | null {
        return this.attributes.get(name) ?? null;
      }

      removeAttribute(name: string): void {
        this.attributes.delete(name);
      }

      getBoundingClientRect(): { left: number; top: number; width: number; height: number } {
        return { left: 0, top: 0, width: 220, height: 200 };
      }

      scrollIntoView(options?: unknown): void {
        scrollIntoViewCalls.push({
          testId: this.getAttribute('data-testid'),
          options,
        });
      }

      querySelector(selector: string): FakeElement | null {
        const visit = (node: FakeNode): FakeElement | null => {
          if (node instanceof FakeElement) {
            if (selector.startsWith('.')) {
              const cls = selector.slice(1);
              const classes = (node.getAttribute('class') ?? '').split(/\s+/);
              if (classes.includes(cls)) {
                return node;
              }
            } else {
              const attrMatch = /^\[([a-z0-9-]+)="([^"]+)"\]$/.exec(selector);
              if (
                attrMatch?.[1] &&
                attrMatch[2] &&
                node.getAttribute(attrMatch[1]) === attrMatch[2]
              ) {
                return node;
              }
            }
          }
          for (const child of node.childNodes) {
            const found = visit(child);
            if (found) return found;
          }
          return null;
        };
        return visit(this);
      }
    }

    const restorers: Array<() => void> = [];
    function patchDomRender(comp: Record<string, unknown>, relPath: string): void {
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
      const prevRender = comp.render;
      const prevSsrRender = comp.ssrRender;
      comp.render = renderFn;
      comp.ssrRender = undefined;
      restorers.push(() => {
        comp.render = prevRender;
        comp.ssrRender = prevSsrRender;
      });
    }

    const host = new FakeElement('div');
    const { createApp } = createRenderer<FakeNode, FakeElement>({
      createElement: (tag) => new FakeElement(tag),
      createText: (text) => {
        const node = new FakeNode();
        (node as unknown as { nodeValue: string }).nodeValue = text;
        return node;
      },
      createComment: () => new FakeNode(),
      setText: (node, text) => {
        (node as unknown as { nodeValue: string }).nodeValue = text;
      },
      setElementText: (el, text) => {
        el.textContent = text;
      },
      insert: (child, parent, anchor) => parent.insertBefore(child, anchor ?? null),
      remove: (child) => child.parentNode?.removeChild(child),
      parentNode: (node) => node.parentNode as FakeElement | null,
      nextSibling: (node) => node?.nextSibling ?? null,
      querySelector: (selector) => (selector === 'body' ? host : null),
      patchProp: (el, key, prev, next) => {
        if (key.startsWith('on')) {
          const ev = key.slice(2).toLowerCase();
          if (prev) el.removeEventListener(ev, prev as Listener);
          if (next) el.addEventListener(ev, next as Listener);
        } else if (key === 'class') {
          const val = Array.isArray(next)
            ? next
                .flatMap((entry) => {
                  if (typeof entry === 'string') return [entry];
                  if (entry && typeof entry === 'object') {
                    return Object.entries(entry)
                      .filter(([, enabled]) => Boolean(enabled))
                      .map(([cls]) => cls);
                  }
                  return [];
                })
                .join(' ')
            : String(next ?? '');
          el.setAttribute('class', val);
        } else if (next == null || next === false) {
          el.removeAttribute(key);
        } else {
          el.setAttribute(key, String(next === true ? '' : next));
        }
      },
    });

    const componentsMod = await import('@/components');
    patchDomRender(
      componentsMod.QnBadge as unknown as Record<string, unknown>,
      'src/components/QnBadge.vue',
    );
    patchDomRender(
      componentsMod.QnContextMenu as unknown as Record<string, unknown>,
      'src/components/QnContextMenu.vue',
    );
    patchDomRender(
      EditorTabBar as unknown as Record<string, unknown>,
      'src/features/editor/EditorTabBar.vue',
    );

    const pinia = createPinia();
    setActivePinia(pinia);
    const tabsStore = useEditorTabsStore();

    mockNoteRead.mockImplementation(async ({ absPath }: { absPath: string }) => {
      if (absPath === '/vault/a.md') {
        return makeDoc({ key: 'k-a', relPath: 'a.md', absPath: '/vault/a.md', isInbox: false });
      }
      if (absPath === '/vault/inbox.md') {
        return makeDoc({
          key: 'k-inbox',
          relPath: 'inbox.md',
          absPath: '/vault/inbox.md',
          isInbox: true,
        });
      }
      return makeDoc({ key: 'k-b', relPath: 'b.md', absPath: '/vault/b.md', isInbox: false });
    });

    await tabsStore.openNoteByAbsPath('/vault/a.md', { preview: false });
    await tabsStore.openNoteByAbsPath('/vault/b.md', { preview: false });

    const app = createApp(EditorTabBar);
    app.use(pinia);
    app.provide(ssrContextKey, { modules: new Set() });

    try {
      app.mount(host);
      await nextTick();

      const listEl = host.querySelector('[data-testid="editor-tabbar-list"]');
      expect(listEl).not.toBeNull();
      expect(listEl!.getAttribute('class')).not.toContain('qn-tabbar__list--scrolling');

      // 1. Wheel vertical deltaY drives scrollLeft and activates scrolling class
      const preventDefault = vi.fn();
      listEl!.dispatchEvent({
        type: 'wheel',
        deltaY: 48,
        deltaX: 0,
        preventDefault,
      });
      await nextTick();

      expect(preventDefault).toHaveBeenCalledTimes(1);
      expect(listEl!.scrollLeft).toBe(48);
      expect(listEl!.getAttribute('class')).toContain('qn-tabbar__list--scrolling');

      // 2. Scrolling class auto-removes after 600ms
      await vi.advanceTimersByTimeAsync(599);
      await nextTick();
      expect(listEl!.getAttribute('class')).toContain('qn-tabbar__list--scrolling');

      await vi.advanceTimersByTimeAsync(1);
      await nextTick();
      expect(listEl!.getAttribute('class')).not.toContain('qn-tabbar__list--scrolling');

      // 3. Switching activeKey triggers scrollIntoView on the newly active tab after nextTick
      await tabsStore.activateTab('k-a');
      await nextTick();
      await nextTick();

      expect(scrollIntoViewCalls).toContainEqual({
        testId: 'editor-tab-a.md',
        options: { behavior: 'smooth', block: 'nearest', inline: 'nearest' },
      });

      // 4. R24: 不再渲染「关闭其他」按钮
      expect(host.querySelector('[data-testid="editor-tab-close-others"]')).toBeNull();

      // 5. R25: 普通笔记标签右键菜单包含 tab.deleteFile 菜单项且未禁用；inbox.md 标签上禁用
      const tabAEl = host.querySelector('[data-testid="editor-tab-a.md"]');
      expect(tabAEl).not.toBeNull();
      tabAEl!.dispatchEvent({
        type: 'contextmenu',
        clientX: 50,
        clientY: 20,
        preventDefault: vi.fn(),
      });
      await nextTick();

      const deleteItemNormal = host.querySelector('[data-menu-id="tab.deleteFile"]');
      expect(deleteItemNormal).not.toBeNull();
      expect(deleteItemNormal!.getAttribute('disabled')).toBeNull();

      await tabsStore.openNoteByAbsPath('/vault/inbox.md', { preview: false });
      await nextTick();
      const tabInboxEl = host.querySelector('[data-testid="editor-tab-inbox.md"]');
      expect(tabInboxEl).not.toBeNull();
      tabInboxEl!.dispatchEvent({
        type: 'contextmenu',
        clientX: 50,
        clientY: 20,
        preventDefault: vi.fn(),
      });
      await nextTick();

      const deleteItemInbox = host.querySelector('[data-menu-id="tab.deleteFile"]');
      expect(deleteItemInbox).not.toBeNull();
      expect(deleteItemInbox!.getAttribute('disabled')).not.toBeNull();
    } finally {
      app.unmount();
      tabsStore.dispose();
      while (restorers.length > 0) {
        restorers.pop()?.();
      }
    }
  });

  it('reorders tabs via reorderTab and ignores nonexistent keys', async () => {
    const store = useEditorTabsStore();

    mockNoteRead.mockImplementation(async ({ absPath }: { absPath: string }) => {
      if (absPath === '/vault/a.md') {
        return makeDoc({ key: 'a', relPath: 'a.md', absPath: '/vault/a.md', isInbox: false });
      }
      if (absPath === '/vault/b.md') {
        return makeDoc({ key: 'b', relPath: 'b.md', absPath: '/vault/b.md', isInbox: false });
      }
      return makeDoc({ key: 'c', relPath: 'c.md', absPath: '/vault/c.md', isInbox: false });
    });

    await store.openNoteByAbsPath('/vault/a.md', { preview: false });
    await store.openNoteByAbsPath('/vault/b.md', { preview: false });
    await store.openNoteByAbsPath('/vault/c.md', { preview: false });

    expect(store.tabs.map((t) => t.key)).toEqual(['a', 'b', 'c']);

    store.reorderTab('a', 'c', 'after');
    expect(store.tabs.map((t) => t.key)).toEqual(['b', 'c', 'a']);

    store.reorderTab('missing', 'c', 'before');
    store.reorderTab('a', 'missing', 'before');
    expect(store.tabs.map((t) => t.key)).toEqual(['b', 'c', 'a']);

    store.dispose();
  });
});
