import fs from 'node:fs';
import path from 'node:path';
import * as vueRuntime from 'vue';
import { createRenderer, createSSRApp, ssrContextKey } from 'vue';
import { compileScript, compileTemplate, parse } from 'vue/compiler-sfc';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  CommandEntry,
  ExternalActionState,
  LibraryInfo,
  SettingsSnapshot,
} from '@/bindings/types';

const {
  mockCommandsSearch,
  mockExternalActionsResolve,
  mockExternalActionRun,
  mockLibraryGetInfo,
  mockSettingsGet,
  mockWindowShowMain,
  mockWindowHideQuickBar,
} = vi.hoisted(() => ({
  mockCommandsSearch: vi.fn(),
  mockExternalActionsResolve: vi.fn(),
  mockExternalActionRun: vi.fn(),
  mockLibraryGetInfo: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockWindowHideQuickBar: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  commandsSearch: mockCommandsSearch,
  externalActionsResolve: mockExternalActionsResolve,
  externalActionRun: mockExternalActionRun,
  libraryGetInfo: mockLibraryGetInfo,
  settingsGet: mockSettingsGet,
  windowShowMain: mockWindowShowMain,
  windowHideQuickBar: mockWindowHideQuickBar,
}));

import {
  ALL_SECTIONS_KEY,
  COMMAND_SEARCH_DEBOUNCE_MS,
  CommandBookPalette,
  useCommandPalette,
} from '@/features/commands';

function createSampleCommands(): CommandEntry[] {
  return [
    {
      id: 'cmd-git-undo',
      noteRelPath: 'commands/常用命令.md',
      section: 'Git 操作',
      title: '撤销上次提交',
      description: '保留工作区改动并回退最近一次 commit',
      code: 'git reset --soft HEAD~1',
      language: 'bash',
      keywords: ['git', 'undo'],
      isDanger: false,
      line: 12,
    },
    {
      id: 'cmd-git-clean',
      noteRelPath: 'commands/常用命令.md',
      section: 'Git 操作',
      title: '强制清理未跟踪文件',
      description: '删除所有未跟踪文件与目录',
      code: 'git clean -fd',
      language: 'bash',
      keywords: ['git', 'clean'],
      isDanger: true,
      line: 25,
    },
    {
      id: 'cmd-pnpm-check',
      noteRelPath: 'commands/工程命令.md',
      section: '前端构建',
      title: '全量检查',
      description: '运行全部门禁检查',
      code: 'pnpm check:all',
      language: 'bash',
      keywords: ['pnpm', 'check'],
      isDanger: false,
      line: 8,
    },
  ];
}

function createTerminalState(dirPath: string | null): ExternalActionState[] {
  return [
    {
      id: 'external.terminal',
      enabled: Boolean(dirPath),
      disabledReason: dirPath ? null : '未解析到项目目录',
      resolvedPath: dirPath,
    },
  ];
}

describe('command-book-palette', () => {
  const originalNavigator = globalThis.navigator;
  const mockClipboardWrite = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    mockClipboardWrite.mockResolvedValue(undefined);
    Object.defineProperty(globalThis, 'navigator', {
      configurable: true,
      value: {
        clipboard: {
          writeText: mockClipboardWrite,
        },
      },
    });

    mockCommandsSearch.mockResolvedValue(createSampleCommands());
    mockExternalActionsResolve.mockResolvedValue(
      createTerminalState('/Users/Joey/Sites/Projects/QuickNote'),
    );
    mockExternalActionRun.mockResolvedValue(undefined);
    mockLibraryGetInfo.mockResolvedValue({
      root: '/Users/Joey/Notes/QuickNote',
    } as LibraryInfo);
    mockSettingsGet.mockResolvedValue({
      settings: {
        workspaces: {
          quicknote: {
            name: 'QuickNote',
            key: 'quicknote',
            root: '/Users/Joey/Sites/Projects/QuickNote',
            openDir: '/Users/Joey/Sites/Projects/QuickNote',
            codeWorkspace: null,
            terminalDir: '/Users/Joey/Sites/Projects/QuickNote',
            defaultEditor: 'code',
            defaultEntryDoc: null,
          },
        },
      },
    } as unknown as SettingsSnapshot);
    mockWindowShowMain.mockResolvedValue({});
    mockWindowHideQuickBar.mockResolvedValue({});
  });

  afterEach(() => {
    vi.useRealTimers();
    Object.defineProperty(globalThis, 'navigator', {
      configurable: true,
      value: originalNavigator,
    });
  });

  it('debounces queries by 80ms and discards out-of-order responses', async () => {
    let resolveFirst!: (rows: CommandEntry[]) => void;
    let resolveSecond!: (rows: CommandEntry[]) => void;

    mockCommandsSearch
      .mockImplementationOnce(
        () =>
          new Promise<CommandEntry[]>((resolve) => {
            resolveFirst = resolve;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise<CommandEntry[]>((resolve) => {
            resolveSecond = resolve;
          }),
      );

    const engine = useCommandPalette();
    engine.setQuery('git');
    expect(mockCommandsSearch).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(COMMAND_SEARCH_DEBOUNCE_MS);
    expect(mockCommandsSearch).toHaveBeenCalledTimes(1);
    expect(mockCommandsSearch).toHaveBeenLastCalledWith({ query: 'git' });

    engine.setQuery('pnpm');
    await vi.advanceTimersByTimeAsync(COMMAND_SEARCH_DEBOUNCE_MS);
    expect(mockCommandsSearch).toHaveBeenCalledTimes(2);
    expect(mockCommandsSearch).toHaveBeenLastCalledWith({ query: 'pnpm' });

    const samples = createSampleCommands();
    const secondRows = [samples[2]!];
    resolveSecond(secondRows);
    await Promise.resolve();
    await Promise.resolve();
    expect(engine.entries.value).toEqual(secondRows);

    // 较早请求的乱序响应后到达，必须被丢弃
    resolveFirst([samples[0]!]);
    await Promise.resolve();
    await Promise.resolve();
    expect(engine.entries.value).toEqual(secondRows);

    engine.dispose();
  });

  it('renders danger badge and supports section filtering', async () => {
    const engine = useCommandPalette();
    await engine.refresh(true);

    expect(engine.sections.value).toEqual([
      { name: 'Git 操作', count: 2 },
      { name: '前端构建', count: 1 },
    ]);

    const compactApp = createSSRApp(CommandBookPalette, {
      engine,
      variant: 'compact',
    });
    const compactHtml = await renderToString(compactApp);
    expect(compactHtml).toContain('cmd-danger-badge');
    expect(compactHtml).toContain('⚠ 危险');
    expect(compactHtml).toContain('将在 /Users/Joey/Sites/Projects/QuickNote 打开外部终端');

    engine.selectSection('前端构建');
    expect(engine.filteredEntries.value).toHaveLength(1);
    expect(engine.filteredEntries.value[0]?.id).toBe('cmd-pnpm-check');

    engine.selectSection(ALL_SECTIONS_KEY);
    expect(engine.filteredEntries.value).toHaveLength(3);

    const largeApp = createSSRApp(CommandBookPalette, {
      engine,
      variant: 'large',
    });
    const largeHtml = await renderToString(largeApp);
    expect(largeHtml).toContain('cmd-section-sidebar');
    expect(largeHtml).toContain('cmd-detail-pane');

    engine.dispose();
  });

  it('copies command on Enter without closing or running externalActionRun', async () => {
    const onRequestClose = vi.fn();
    const engine = useCommandPalette({ onRequestClose });
    await engine.refresh(true);

    const copied = await engine.copySelected();
    expect(copied).toBe(true);
    expect(mockClipboardWrite).toHaveBeenCalledWith('git reset --soft HEAD~1');
    expect(engine.feedbackMessage.value).toBe('已复制命令');
    expect(mockExternalActionRun).not.toHaveBeenCalled();
    expect(onRequestClose).not.toHaveBeenCalled();

    engine.dispose();
  });

  it('copies and opens external terminal with dirPath only when resolved, then requests close', async () => {
    const onRequestClose = vi.fn();
    const engine = useCommandPalette({ onRequestClose });
    await engine.refresh(true);

    const opened = await engine.copyAndOpenTerminal();
    expect(opened).toBe(true);
    expect(mockClipboardWrite).toHaveBeenCalledWith('git reset --soft HEAD~1');
    expect(mockExternalActionRun).toHaveBeenCalledTimes(1);
    expect(mockExternalActionRun).toHaveBeenCalledWith({
      id: 'external.terminal',
      absPath: null,
      dirPath: '/Users/Joey/Sites/Projects/QuickNote',
    });
    expect(onRequestClose).toHaveBeenCalledTimes(1);

    engine.dispose();
  });

  it('opens manual dir picker when external.terminal is disabled and never silently falls back', async () => {
    mockExternalActionsResolve.mockResolvedValue(createTerminalState(null));
    const onRequestClose = vi.fn();
    const engine = useCommandPalette({ onRequestClose });
    await engine.refresh(true);

    const opened = await engine.copyAndOpenTerminal();
    expect(opened).toBe(false);
    expect(mockClipboardWrite).toHaveBeenCalledWith('git reset --soft HEAD~1');
    expect(mockExternalActionRun).not.toHaveBeenCalled();
    expect(engine.dirPickerOpen.value).toBe(true);
    expect(engine.workspaceOptions.value).toHaveLength(0);
    expect(onRequestClose).not.toHaveBeenCalled();

    // 空输入不允许提交
    const emptyConfirm = await engine.confirmManualDir('   ');
    expect(emptyConfirm).toBe(false);
    expect(mockExternalActionRun).not.toHaveBeenCalled();

    // 选择工作区目录后才调用 externalActionRun
    const confirmed = await engine.confirmManualDir('/Users/Joey/Sites/Projects/QuickNote');
    expect(confirmed).toBe(true);
    expect(mockExternalActionRun).toHaveBeenCalledWith({
      id: 'external.terminal',
      absPath: null,
      dirPath: '/Users/Joey/Sites/Projects/QuickNote',
    });
    expect(engine.dirPickerOpen.value).toBe(false);
    expect(onRequestClose).toHaveBeenCalledTimes(1);

    engine.dispose();
  });

  it('opens source file with exact absPath and entry.line on Cmd+O', async () => {
    const onOpenSource = vi.fn();
    const engine = useCommandPalette({ onOpenSource });
    await engine.refresh(true);
    engine.selectIndex(1);

    const opened = await engine.openSourceFile();
    expect(opened).toBe(true);
    expect(onOpenSource).toHaveBeenCalledWith(
      '/Users/Joey/Notes/QuickNote/commands/常用命令.md',
      25,
    );

    engine.dispose();
  });

  it('closes large command book palette on Escape when dirPicker is closed', async () => {
    const onRequestClose = vi.fn();
    const engine = useCommandPalette({ onRequestClose });
    await engine.refresh(true);

    class FakeNode {
      parentNode: FakeNode | null = null;
      childNodes: FakeNode[] = [];
      listeners = new Map<string, Set<(e: unknown) => void>>();

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

      addEventListener(type: string, fn: (e: unknown) => void): void {
        const set = this.listeners.get(type) ?? new Set();
        set.add(fn);
        this.listeners.set(type, set);
      }

      removeEventListener(type: string, fn: (e: unknown) => void): void {
        this.listeners.get(type)?.delete(fn);
      }

      dispatchEvent(event: { type: string; bubbles?: boolean; cancelBubble?: boolean }): void {
        for (const fn of Array.from(this.listeners.get(event.type) ?? [])) {
          fn(event);
        }
        if (event.bubbles && !event.cancelBubble && this.parentNode) {
          this.parentNode.dispatchEvent(event);
        }
      }
    }

    class FakeEl extends FakeNode {
      tag: string;
      attrs = new Map<string, string>();
      constructor(tag: string) {
        super();
        this.tag = tag;
      }
      setAttribute(k: string, v: string) {
        this.attrs.set(k, v);
      }
      removeAttribute(k: string) {
        this.attrs.delete(k);
      }
      focus() {}
      select() {}
      findByTestId(id: string): FakeEl | null {
        if (this.attrs.get('data-testid') === id) return this;
        for (const c of this.childNodes) {
          if (c instanceof FakeEl) {
            const found = c.findByTestId(id);
            if (found) return found;
          }
        }
        return null;
      }
    }

    function withDomRender<T extends object>(comp: T, relPath: string): T {
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
      return Object.assign(Object.create(Object.getPrototypeOf(comp)), comp, {
        ssrRender: undefined,
        render: renderFn,
      }) as T;
    }

    const { createApp } = createRenderer<FakeNode, FakeEl>({
      createElement: (tag) => new FakeEl(tag),
      createText: () => new FakeNode(),
      createComment: () => new FakeNode(),
      setText: () => undefined,
      setElementText: () => undefined,
      insert: (child, parent, anchor) => parent.insertBefore(child, anchor ?? null),
      remove: (child) => child.parentNode?.removeChild(child),
      parentNode: (node) => node.parentNode as FakeEl | null,
      nextSibling: (node) => node.nextSibling,
      patchProp: (el, key, prev, next) => {
        if (key.startsWith('on')) {
          const ev = key.slice(2).toLowerCase();
          if (prev) el.removeEventListener(ev, prev as (e: unknown) => void);
          if (next) el.addEventListener(ev, next as (e: unknown) => void);
        } else if (next == null || next === false) {
          el.removeAttribute(key);
        } else {
          el.setAttribute(key, String(next === true ? '' : next));
        }
      },
    });

    const DomPalette = withDomRender(
      CommandBookPalette,
      'src/features/commands/CommandBookPalette.vue',
    );
    const root = new FakeEl('div');
    const app = createApp(DomPalette, {
      engine,
      variant: 'large',
    });
    app.provide(ssrContextKey, { modules: new Set() });

    try {
      app.mount(root);
      const paletteEl = root.findByTestId('command-book-palette');
      expect(paletteEl).not.toBeNull();

      // When dirPickerOpen is true, Esc closes dirPicker instead of palette
      await engine.openDirPicker();
      expect(engine.dirPickerOpen.value).toBe(true);
      const escInPicker = {
        type: 'keydown',
        key: 'Escape',
        bubbles: true,
        isComposing: false,
        keyCode: 27,
        cancelBubble: false,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(function (this: { cancelBubble: boolean }) {
          this.cancelBubble = true;
        }),
      };
      paletteEl!.dispatchEvent(escInPicker);
      expect(escInPicker.preventDefault).toHaveBeenCalledTimes(1);
      expect(escInPicker.stopPropagation).toHaveBeenCalledTimes(1);
      expect(engine.dirPickerOpen.value).toBe(false);
      expect(onRequestClose).not.toHaveBeenCalled();

      // When dirPickerOpen is false, Esc closes the large palette
      const escPalette = {
        type: 'keydown',
        key: 'Escape',
        bubbles: true,
        isComposing: false,
        keyCode: 27,
        cancelBubble: false,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(function (this: { cancelBubble: boolean }) {
          this.cancelBubble = true;
        }),
      };
      paletteEl!.dispatchEvent(escPalette);
      expect(escPalette.preventDefault).toHaveBeenCalledTimes(1);
      expect(escPalette.stopPropagation).toHaveBeenCalledTimes(1);
      expect(onRequestClose).toHaveBeenCalledTimes(1);
    } finally {
      app.unmount();
      engine.dispose();
    }
  });
});
