import fs from 'node:fs';
import path from 'node:path';
import { createPinia, setActivePinia } from 'pinia';
import * as vueRuntime from 'vue';
import { createRenderer, createSSRApp, nextTick, ssrContextKey } from 'vue';
import { compileScript, compileTemplate, parse } from 'vue/compiler-sfc';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ExternalActionState,
  ExternalDirectoryListing,
  ExternalPathProbeResult,
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
  mockProbeExternalPath,
  mockListExternalDirectory,
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
  mockProbeExternalPath: vi.fn(),
  mockListExternalDirectory: vi.fn(),
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
  probeExternalPath: mockProbeExternalPath,
  listExternalDirectory: mockListExternalDirectory,
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
    mockSearchNotesAndFiles.mockImplementation(
      async (args: { requestId: number }) =>
        ({
          requestId: args.requestId,
          items: [],
          truncated: false,
        }) satisfies SearchResponse,
    );
    mockProbeExternalPath.mockResolvedValue({
      rawInput: '',
      resolvedPath: '',
      exists: false,
      kind: 'not_found',
      readable: false,
      size: null,
      isText: false,
      isUtf8: false,
      isHidden: false,
    } satisfies ExternalPathProbeResult);
    mockListExternalDirectory.mockResolvedValue({
      rootPath: '',
      entries: [],
      truncated: false,
      visitedCount: 0,
    } satisfies ExternalDirectoryListing);
    mockNotesGetTreeAndTags.mockResolvedValue({
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

  it('stops Esc and Cmd+Enter bubbling from QuickOpenPalette to QuickBarView and guards capture actions', async () => {
    interface Listener {
      (event: unknown): void;
    }

    class FakeNode {
      parentNode: FakeNode | null = null;
      childNodes: FakeNode[] = [];
      private listeners = new Map<string, Set<Listener>>();

      appendChild(child: FakeNode): FakeNode {
        child.parentNode = this;
        this.childNodes.push(child);
        return child;
      }

      insertBefore(child: FakeNode, anchor: FakeNode | null): FakeNode {
        child.parentNode = this;
        if (!anchor) {
          this.childNodes.push(child);
          return child;
        }
        const idx = this.childNodes.indexOf(anchor);
        if (idx >= 0) {
          this.childNodes.splice(idx, 0, child);
        } else {
          this.childNodes.push(child);
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
        if (!this.parentNode) {
          return null;
        }
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

      dispatchEvent(event: FakeKeyboardEvent): boolean {
        if (!event.target) {
          event.target = this;
        }
        event.currentTarget = this;
        const set = this.listeners.get(event.type);
        if (set) {
          for (const fn of Array.from(set)) {
            fn(event);
          }
        }
        if (event.bubbles && !event.cancelBubble && this.parentNode) {
          this.parentNode.dispatchEvent(event);
        }
        return !event.defaultPrevented;
      }
    }

    class FakeElement extends FakeNode {
      tagName: string;
      attributes = new Map<string, string>();
      style: Record<string, string> & {
        setProperty: (k: string, v: string) => void;
        removeProperty: (k: string) => void;
      };
      textContent = '';
      value = '';
      disabled = false;

      constructor(tagName: string) {
        super();
        this.tagName = tagName.toUpperCase();
        this.style = Object.assign(Object.create(null) as Record<string, string>, {
          setProperty: () => undefined,
          removeProperty: () => undefined,
        }) as FakeElement['style'];
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

      focus(): void {}
      select(): void {}

      querySelector(selector: string): FakeElement | null {
        const testIdMatch = /^\[data-testid="([^"]+)"\]$/.exec(selector);
        const targetTestId = testIdMatch?.[1];
        const visit = (node: FakeNode): FakeElement | null => {
          if (node instanceof FakeElement && targetTestId) {
            if (node.getAttribute('data-testid') === targetTestId) {
              return node;
            }
          }
          for (const child of node.childNodes) {
            const found = visit(child);
            if (found) {
              return found;
            }
          }
          return null;
        };
        return visit(this);
      }
    }

    class FakeKeyboardEvent {
      type: string;
      key: string;
      code: string;
      metaKey: boolean;
      ctrlKey: boolean;
      altKey: boolean;
      shiftKey: boolean;
      bubbles: boolean;
      cancelable: boolean;
      isComposing = false;
      keyCode: number;
      defaultPrevented = false;
      cancelBubble = false;
      target: FakeNode | null = null;
      currentTarget: FakeNode | null = null;

      constructor(
        type: string,
        init: {
          key: string;
          code?: string;
          metaKey?: boolean;
          ctrlKey?: boolean;
          altKey?: boolean;
          shiftKey?: boolean;
          bubbles?: boolean;
          cancelable?: boolean;
          keyCode?: number;
        },
      ) {
        this.type = type;
        this.key = init.key;
        this.code = init.code ?? '';
        this.metaKey = Boolean(init.metaKey);
        this.ctrlKey = Boolean(init.ctrlKey);
        this.altKey = Boolean(init.altKey);
        this.shiftKey = Boolean(init.shiftKey);
        this.bubbles = Boolean(init.bubbles);
        this.cancelable = Boolean(init.cancelable);
        this.keyCode = init.keyCode ?? 0;
      }

      preventDefault(): void {
        this.defaultPrevented = true;
      }

      stopPropagation(): void {
        this.cancelBubble = true;
      }
    }

    const body = new FakeElement('body');

    const origKeyboardEvent = (globalThis as { KeyboardEvent?: unknown }).KeyboardEvent;
    const origElement = (globalThis as { Element?: unknown }).Element;

    Object.defineProperty(globalThis, 'KeyboardEvent', {
      value: FakeKeyboardEvent,
      configurable: true,
      writable: true,
    });
    Object.defineProperty(globalThis, 'Element', {
      value: FakeElement,
      configurable: true,
      writable: true,
    });

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
      nextSibling: (node) => node.nextSibling,
      patchProp: (el, key, prev, next) => {
        if (key.startsWith('on')) {
          const ev = key.slice(2).toLowerCase();
          if (prev) el.removeEventListener(ev, prev as Listener);
          if (next) el.addEventListener(ev, next as Listener);
        } else if (key === 'value') {
          el.value = String(next ?? '');
        } else if (next == null || next === false) {
          el.removeAttribute(key);
        } else {
          el.setAttribute(key, String(next === true ? '' : next));
        }
      },
    });

    const quickOpenMod = await import('@/features/quickopen');
    const targetPickerMod = await import('../QuickBarTargetPicker.vue');
    const componentsMod = await import('@/components');

    patchDomRender(
      quickOpenMod.QuickOpenPalette as unknown as Record<string, unknown>,
      'src/features/quickopen/QuickOpenPalette.vue',
    );
    patchDomRender(
      targetPickerMod.default as unknown as Record<string, unknown>,
      'src/features/quickbar/QuickBarTargetPicker.vue',
    );
    patchDomRender(
      componentsMod.QnBadge as unknown as Record<string, unknown>,
      'src/components/QnBadge.vue',
    );
    patchDomRender(
      componentsMod.QnBanner as unknown as Record<string, unknown>,
      'src/components/QnBanner.vue',
    );
    patchDomRender(
      componentsMod.QnButton as unknown as Record<string, unknown>,
      'src/components/QnButton.vue',
    );
    patchDomRender(
      QuickBarView as unknown as Record<string, unknown>,
      'src/features/quickbar/QuickBarView.vue',
    );

    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useQuickBarStore();
    await store.initialize();
    store.draft = '尚未提交的草稿';

    const host = new FakeElement('div');
    body.appendChild(host);
    const app = createApp(QuickBarView);
    app.use(pinia);
    app.provide(ssrContextKey, { modules: new Set() });

    try {
      app.mount(host);
      store.setMode('open');
      await nextTick();
      await nextTick();

      const inputEl = host.querySelector('[data-testid="quickopen-input"]');
      expect(inputEl).not.toBeNull();

      // 1. In open mode, Cmd+Enter with bubbles: true must NOT trigger quickbar.submitAppend
      const cmdEnter = new FakeKeyboardEvent('keydown', {
        key: 'Enter',
        code: 'Enter',
        metaKey: true,
        bubbles: true,
        cancelable: true,
        keyCode: 13,
      });
      inputEl!.dispatchEvent(cmdEnter);
      expect(cmdEnter.cancelBubble).toBe(true);
      expect(mockNoteAppend).not.toHaveBeenCalled();
      expect(store.draft).toBe('尚未提交的草稿');

      // Also verify direct dispatch on actionRegistry in open mode does NOT trigger capture actions
      const directCmdEnter = new FakeKeyboardEvent('keydown', {
        key: 'Enter',
        code: 'Enter',
        metaKey: true,
        bubbles: true,
        cancelable: true,
        keyCode: 13,
      });
      const dispatchRes = actionRegistry.dispatchKeyEvent(
        directCmdEnter as unknown as KeyboardEvent,
        {
          activeScope: 'quickbar',
          activeScopes: ['quickbar', 'global'],
        },
      );
      expect(dispatchRes.handled).toBe(false);
      expect(mockNoteAppend).not.toHaveBeenCalled();

      // 2. In open mode, one Esc with bubbles: true returns to capture mode and must NOT hide Quick Bar
      const escEvent = new FakeKeyboardEvent('keydown', {
        key: 'Escape',
        code: 'Escape',
        bubbles: true,
        cancelable: true,
        keyCode: 27,
      });
      inputEl!.dispatchEvent(escEvent);
      expect(escEvent.cancelBubble).toBe(true);
      expect(store.mode).toBe('capture');
      expect(mockWindowHideQuickBar).not.toHaveBeenCalled();
    } finally {
      app.unmount();
      store.dispose();
      while (restorers.length > 0) {
        restorers.pop()?.();
      }
      Object.defineProperty(globalThis, 'KeyboardEvent', {
        value: origKeyboardEvent,
        configurable: true,
        writable: true,
      });
      Object.defineProperty(globalThis, 'Element', {
        value: origElement,
        configurable: true,
        writable: true,
      });
    }
  });

  it('resets Quick Bar mode back to capture on hide or re-show without explicit mode (v4 §3.5.4)', async () => {
    let windowStateListener: ((state: WindowState) => void) | undefined;
    mockOnWindowState.mockImplementationOnce(async (cb: (state: WindowState) => void) => {
      windowStateListener = cb;
      return () => undefined;
    });

    const store = useQuickBarStore();
    await store.initialize();
    store.draft = '保留草稿';

    // 1. Hide via openFileInMain resets mode to capture
    store.setMode('open');
    await store.openFileInMain('/etc/nginx/nginx.conf', false);
    expect(store.mode).toBe('capture');
    expect(store.draft).toBe('保留草稿');

    // 2. Hide via hotkey / window:state visible=false resets mode to capture
    store.setMode('command');
    windowStateListener?.({
      windowId: 'quickbar',
      visible: false,
      fullscreen: false,
      mode: null,
      reason: 'hotkey',
    });
    expect(store.mode).toBe('capture');
    expect(store.draft).toBe('保留草稿');

    // 3. Re-show via window:state visible=true with mode=null resets mode to capture
    store.setMode('open');
    windowStateListener?.({
      windowId: 'quickbar',
      visible: true,
      fullscreen: false,
      mode: null,
      reason: null,
    });
    expect(store.mode).toBe('capture');

    store.dispose();
  });

  it('probes absolute and ~/ hidden paths directly and opens readable files in a new tab without Spotlight', async () => {
    const store = useQuickBarStore();
    await store.initialize();
    store.setMode('open');

    // 1. Absolute system path: /usr/local/etc/nginx/nginx.conf
    mockProbeExternalPath.mockResolvedValueOnce({
      rawInput: '/usr/local/etc/nginx/nginx.conf',
      resolvedPath: '/usr/local/etc/nginx/nginx.conf',
      exists: true,
      kind: 'file',
      readable: true,
      size: 512,
      isText: true,
      isUtf8: true,
      isHidden: false,
    } satisfies ExternalPathProbeResult);

    store.openQuery = '/usr/local/etc/nginx/nginx.conf';
    await store.refreshOpenMode(false);

    expect(mockProbeExternalPath).toHaveBeenCalledWith('/usr/local/etc/nginx/nginx.conf');
    expect(mockSearchSpotlight).not.toHaveBeenCalled();
    expect(store.openGroups).toHaveLength(1);
    expect(store.openGroups[0]?.id).toBe('path_direct');
    expect(store.openGroups[0]?.items[0]?.badgeLabel).toBe('路径直开');
    expect(store.openGroups[0]?.items[0]?.title).toContain('直接打开外部文件');

    await store.confirmOpenSelection('newTab');
    expect(consumePendingOpenFile()).toMatchObject({
      kind: 'file',
      absPath: '/usr/local/etc/nginx/nginx.conf',
      newTab: true,
      line: null,
    });
    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: null });

    // 2. Home hidden file path: ~/.zshrc (default Enter opens as preview tab)
    store.setMode('open');
    mockProbeExternalPath.mockResolvedValueOnce({
      rawInput: '~/.zshrc',
      resolvedPath: '/Users/demo/.zshrc',
      exists: true,
      kind: 'file',
      readable: true,
      size: 1024,
      isText: true,
      isUtf8: true,
      isHidden: true,
    } satisfies ExternalPathProbeResult);

    store.openQuery = '~/.zshrc';
    await store.refreshOpenMode(false);
    expect(store.openGroups[0]?.items[0]?.absPath).toBe('/Users/demo/.zshrc');

    await store.confirmOpenSelection();
    expect(consumePendingOpenFile()).toMatchObject({
      kind: 'file',
      absPath: '/Users/demo/.zshrc',
      newTab: false,
    });

    // 3. Probing a directory lists entries including hidden files
    store.setMode('open');
    mockProbeExternalPath.mockResolvedValueOnce({
      rawInput: '/usr/local/etc/nginx',
      resolvedPath: '/usr/local/etc/nginx',
      exists: true,
      kind: 'directory',
      readable: true,
      size: null,
      isText: false,
      isUtf8: false,
      isHidden: false,
    } satisfies ExternalPathProbeResult);
    mockListExternalDirectory.mockResolvedValueOnce({
      rootPath: '/usr/local/etc/nginx',
      entries: [
        {
          name: '.env.local',
          path: '/usr/local/etc/nginx/.env.local',
          relPath: '.env.local',
          kind: 'file',
          depth: 0,
          size: 42,
          isHidden: true,
          matchScore: 100,
        },
        {
          name: 'nginx.conf',
          path: '/usr/local/etc/nginx/nginx.conf',
          relPath: 'nginx.conf',
          kind: 'file',
          depth: 0,
          size: 512,
          isHidden: false,
          matchScore: 90,
        },
      ],
      truncated: false,
      visitedCount: 2,
    } satisfies ExternalDirectoryListing);

    store.openQuery = '/usr/local/etc/nginx';
    await store.refreshOpenMode(false);
    expect(mockListExternalDirectory).toHaveBeenCalledWith({
      dirPath: '/usr/local/etc/nginx',
      query: null,
      includeHidden: true,
    });
    expect(store.openFlatItems.map((i) => i.title)).toEqual(['.env.local', 'nginx.conf']);

    store.dispose();
  });

  it('groups vault notes, file bookmarks, and dir bookmarks, and supports dir bookmark drill-down with Backspace return', async () => {
    const snapshot = makeSettingsSnapshot();
    snapshot.settings.externalBookmarks = [
      {
        path: '/Users/demo/.zshrc',
        kind: 'file',
        alias: 'Zsh 配置',
        pinned: true,
        lastOpenedAt: 200,
      },
      {
        path: '/usr/local/etc/nginx',
        kind: 'directory',
        alias: 'Nginx 目录',
        pinned: true,
        lastOpenedAt: 100,
      },
    ];
    mockSettingsGet.mockResolvedValue(snapshot);
    mockSearchNotesAndFiles.mockImplementation(
      async (args: { requestId: number }) =>
        ({
          requestId: args.requestId,
          items: [
            {
              kind: 'note',
              key: 'n1',
              title: '架构设计笔记',
              subtitle: 'docs/arch.md',
              relPath: 'docs/arch.md',
              absPath: '/Users/demo/Vault/docs/arch.md',
              score: 100,
              source: 'library',
            },
            {
              kind: 'file',
              key: 'sp1',
              title: 'legacy-spotlight.txt',
              subtitle: '/tmp/legacy-spotlight.txt',
              relPath: null,
              absPath: '/tmp/legacy-spotlight.txt',
              score: 50,
              source: 'spotlight',
            },
          ],
          truncated: false,
        }) satisfies SearchResponse,
    );

    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useQuickBarStore();
    await store.initialize();
    store.setMode('open');
    await store.refreshOpenMode(true);

    // Spotlight result must be excluded; three groups rendered in order
    expect(store.openGroups.map((g) => g.id)).toEqual(['note', 'file_bookmark', 'dir_bookmark']);
    expect(store.openGroups[0]?.items[0]?.badgeLabel).toBe('笔记');
    expect(store.openGroups[1]?.items[0]?.badgeLabel).toBe('文件书签');
    expect(store.openGroups[2]?.items[0]?.badgeLabel).toBe('目录书签');

    // SSR check verifies group labels and badges in QuickBarView
    const app = createSSRApp(QuickBarView);
    app.use(pinia);
    const html = await renderToString(app);
    expect(html).toContain('笔记库文档');
    expect(html).toContain('外部文件书签');
    expect(html).toContain('外部目录书签');
    expect(html).toContain('文件书签');
    expect(html).toContain('目录书签');

    // Drill down into directory bookmark
    mockListExternalDirectory.mockResolvedValueOnce({
      rootPath: '/usr/local/etc/nginx',
      entries: [
        {
          name: '.htpasswd',
          path: '/usr/local/etc/nginx/.htpasswd',
          relPath: '.htpasswd',
          kind: 'file',
          depth: 0,
          size: 32,
          isHidden: true,
          matchScore: 100,
        },
        {
          name: 'servers',
          path: '/usr/local/etc/nginx/servers',
          relPath: 'servers',
          kind: 'directory',
          depth: 0,
          size: null,
          isHidden: false,
          matchScore: 80,
        },
      ],
      truncated: false,
      visitedCount: 2,
    } satisfies ExternalDirectoryListing);

    const dirBookmark = store.openGroups[2]!.items[0]!;
    await store.selectOpenItem(dirBookmark);
    expect(store.activeOpenDirectoryScope).toEqual({
      dirPath: '/usr/local/etc/nginx',
      title: 'Nginx 目录',
    });
    expect(mockListExternalDirectory).toHaveBeenCalledWith({
      dirPath: '/usr/local/etc/nginx',
      query: null,
      includeHidden: true,
    });
    expect(store.openFlatItems.map((i) => i.title)).toEqual(['.htpasswd', 'servers']);

    // Filter inside directory scope
    mockListExternalDirectory.mockResolvedValueOnce({
      rootPath: '/usr/local/etc/nginx',
      entries: [
        {
          name: '.htpasswd',
          path: '/usr/local/etc/nginx/.htpasswd',
          relPath: '.htpasswd',
          kind: 'file',
          depth: 0,
          size: 32,
          isHidden: true,
          matchScore: 100,
        },
      ],
      truncated: false,
      visitedCount: 1,
    } satisfies ExternalDirectoryListing);
    store.openQuery = 'htp';
    await store.refreshOpenMode(false);
    expect(mockListExternalDirectory).toHaveBeenLastCalledWith({
      dirPath: '/usr/local/etc/nginx',
      query: 'htp',
      includeHidden: true,
    });
    expect(store.openFlatItems).toHaveLength(1);

    // Backspace on empty input returns to parent group view
    store.openQuery = '';
    const exited = await store.exitOpenDirectory();
    expect(exited).toBe(true);
    expect(store.activeOpenDirectoryScope).toBeNull();
    expect(store.openGroups.map((g) => g.id)).toEqual(['note', 'file_bookmark', 'dir_bookmark']);

    store.dispose();
  });

  it('lists tag group on @ta prefix and dispatches pending tag payload on confirm (M5-06b)', async () => {
    mockNotesGetTreeAndTags.mockResolvedValueOnce({
      dirs: [],
      tags: [
        { tag: 'task', count: 3, children: [] },
        { tag: 'other', count: 1, children: [] },
      ],
      workspaces: [],
    } satisfies NavTreeSnapshot);

    const store = useQuickBarStore();
    await store.initialize();
    store.setMode('open');
    store.openQuery = '@ta';
    await store.refreshOpenMode(false);

    expect(store.openGroups.map((g) => g.id)).toEqual(['tag']);
    expect(store.openGroups[0]?.items[0]).toMatchObject({
      group: 'tag',
      badgeLabel: '标签',
      title: '#task',
      tagName: 'task',
    });

    await store.confirmOpenSelection();
    expect(consumePendingOpenFile()).toMatchObject({
      kind: 'tag',
      tag: 'task',
    });
    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: null });
    expect(mockWindowHideQuickBar).toHaveBeenCalledWith({ reason: 'esc' });

    store.dispose();
  });

  it('lists action group on > prefix and executes matched quickbar action (M5-06b)', async () => {
    const handler = vi.fn();
    const disposeAction = actionRegistry.bindHandler('quickbar.togglePin', handler);

    const store = useQuickBarStore();
    await store.initialize();
    store.setMode('open');
    store.setOpenQuery('>pin', true);

    expect(store.openGroups.map((g) => g.id)).toEqual(['action']);
    expect(store.openGroups[0]?.items[0]).toMatchObject({
      group: 'action',
      badgeLabel: '动作',
      actionId: 'quickbar.togglePin',
    });

    await store.confirmOpenSelection();
    expect(handler).toHaveBeenCalledTimes(1);

    disposeAction();
    store.dispose();
  });

  it('triggers Tab completion in path mode via handleOpenKeydown (M5-06b)', async () => {
    const store = useQuickBarStore();
    await store.initialize();
    store.setMode('open');

    mockProbeExternalPath.mockResolvedValueOnce({
      rawInput: '/usr/lo',
      resolvedPath: '/usr/lo',
      exists: false,
      kind: 'not_found',
      readable: false,
      size: null,
      isText: false,
      isUtf8: false,
      isHidden: false,
    } satisfies ExternalPathProbeResult);
    mockListExternalDirectory.mockResolvedValueOnce({
      rootPath: '/usr',
      entries: [
        {
          name: 'local',
          path: '/usr/local',
          relPath: 'local',
          kind: 'directory',
          depth: 0,
          size: null,
          isHidden: false,
          matchScore: 100,
        },
      ],
      truncated: false,
      visitedCount: 1,
    } satisfies ExternalDirectoryListing);

    store.openQuery = '/usr/lo';
    await store.refreshOpenMode(false);

    mockProbeExternalPath.mockResolvedValueOnce({
      rawInput: '/usr/local/',
      resolvedPath: '/usr/local',
      exists: true,
      kind: 'directory',
      readable: true,
      size: null,
      isText: false,
      isUtf8: false,
      isHidden: false,
    } satisfies ExternalPathProbeResult);
    mockListExternalDirectory.mockResolvedValueOnce({
      rootPath: '/usr/local',
      entries: [],
      truncated: false,
      visitedCount: 0,
    } satisfies ExternalDirectoryListing);

    const preventDefault = vi.fn();
    const stopPropagation = vi.fn();
    const handled = store.handleOpenKeydown({
      key: 'Tab',
      isComposing: false,
      keyCode: 9,
      altKey: false,
      metaKey: false,
      preventDefault,
      stopPropagation,
    } as unknown as KeyboardEvent);

    expect(handled).toBe(true);
    expect(preventDefault).toHaveBeenCalled();
    await Promise.resolve();
    expect(store.openQuery).toBe('/usr/local/');

    store.dispose();
  });
});
