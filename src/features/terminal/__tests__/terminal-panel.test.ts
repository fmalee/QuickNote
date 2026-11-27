import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NoteDoc, TerminalExitPayload, TerminalSessionInfo } from '@/bindings/types';

const {
  mockExternalActionRun,
  mockTerminalSpawn,
  mockTerminalWrite,
  mockTerminalResize,
  mockTerminalSetVisibility,
  mockTerminalKill,
  mockTerminalListSessions,
  mockOnTerminalSessionState,
  mockOnTerminalExit,
  sessionStateListeners,
  exitListeners,
} = vi.hoisted(() => {
  const stateHandlers: Array<(info: TerminalSessionInfo) => void> = [];
  const exitHandlers: Array<(payload: TerminalExitPayload) => void> = [];
  return {
    mockExternalActionRun: vi.fn(),
    mockTerminalSpawn: vi.fn(),
    mockTerminalWrite: vi.fn(),
    mockTerminalResize: vi.fn(),
    mockTerminalSetVisibility: vi.fn(),
    mockTerminalKill: vi.fn(),
    mockTerminalListSessions: vi.fn(),
    mockOnTerminalSessionState: vi.fn(async (handler: (info: TerminalSessionInfo) => void) => {
      stateHandlers.push(handler);
      return () => {};
    }),
    mockOnTerminalExit: vi.fn(async (handler: (payload: TerminalExitPayload) => void) => {
      exitHandlers.push(handler);
      return () => {};
    }),
    sessionStateListeners: stateHandlers,
    exitListeners: exitHandlers,
  };
});

vi.mock('@/lib/ipc/commands', () => ({
  Channel: class MockChannel<T> {
    onmessage: ((msg: T) => void) | null = null;
  },
  isAppError: (err: unknown): err is { code: string; message: string } =>
    typeof err === 'object' && err !== null && 'code' in err && 'message' in err,
  externalActionRun: mockExternalActionRun,
  terminalSpawn: mockTerminalSpawn,
  terminalWrite: mockTerminalWrite,
  terminalResize: mockTerminalResize,
  terminalSetVisibility: mockTerminalSetVisibility,
  terminalKill: mockTerminalKill,
  terminalListSessions: mockTerminalListSessions,
}));

vi.mock('@/lib/ipc/events', () => ({
  onTerminalSessionState: mockOnTerminalSessionState,
  onTerminalExit: mockOnTerminalExit,
}));

import {
  TerminalPanel,
  createStreamingBase64Decoder,
  encodeUtf8ToBase64,
  evaluateTerminalKeyEvent,
  resolveTerminalLaunchContext,
  useTerminalStore,
} from '@/features/terminal';
import { handleTerminalCustomKeyEvent } from '../store';
import { createActionRegistry, registerDefaultActionCatalog } from '@/lib/actions';

function makeDoc(
  overrides?: Omit<Partial<NoteDoc>, 'meta'> & { meta?: Partial<NoteDoc['meta']> },
): NoteDoc {
  return {
    key: '01JDOC00000000000000000001',
    relPath: 'notes/demo.md',
    absPath: '/vault/notes/demo.md',
    isInbox: false,
    isLibrary: true,
    body: '# Demo',
    bodyHash: 'hash-1',
    mtimeMs: 1000,
    sizeBytes: 64,
    ...overrides,
    meta: {
      id: '01JDOC00000000000000000001',
      title: 'Demo',
      tags: [],
      docType: 'note',
      openDir: null,
      codeWorkspace: null,
      terminalDir: null,
      created: '2026-11-24T00:00:00Z',
      pinned: false,
      archived: false,
      metaError: null,
      ...overrides?.meta,
    },
  };
}

async function renderTerminalPanelHtml(): Promise<string> {
  const app = createSSRApp(TerminalPanel);
  return renderToString(app);
}

describe('M4-02 embedded terminal panel & key isolation', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    sessionStateListeners.length = 0;
    exitListeners.length = 0;
    vi.clearAllMocks();

    mockExternalActionRun.mockResolvedValue(undefined);
    mockTerminalSpawn.mockResolvedValue({
      sessionId: 'term-1-1',
      workspaceKey: 'quicknote',
      cwd: '/repos/quicknote/scripts',
      hasActiveForegroundChild: false,
      outputPaused: false,
      state: 'active',
    } satisfies TerminalSessionInfo);
    mockTerminalWrite.mockResolvedValue(undefined);
    mockTerminalResize.mockResolvedValue(undefined);
    mockTerminalSetVisibility.mockResolvedValue(undefined);
    mockTerminalKill.mockResolvedValue(undefined);
    mockTerminalListSessions.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('1. 按优先级解析工作区 cwd（terminalDir > openDir > 文档父目录 > 库根）', () => {
    // 1. 文档 frontmatter terminalDir 最高优先
    const docWithTermDir = makeDoc({
      meta: { terminalDir: '/custom/term', openDir: '/custom/open' },
    });
    expect(resolveTerminalLaunchContext(docWithTermDir, '/vault')).toMatchObject({
      workspaceKey: 'default',
      cwd: '/custom/term',
      source: 'doc-terminal-dir',
    });

    // 2. 文档 frontmatter openDir 次之
    const docWithOpenDir = makeDoc({
      meta: { terminalDir: null, openDir: '/custom/open' },
    });
    expect(resolveTerminalLaunchContext(docWithOpenDir, '/vault')).toMatchObject({
      workspaceKey: 'default',
      cwd: '/custom/open',
      source: 'doc-open-dir',
    });

    // 3. 库内文档无显式目录时回退到文档父目录，会话分组键统一回退 default
    const docParentFallback = makeDoc({});
    expect(resolveTerminalLaunchContext(docParentFallback, '/vault')).toMatchObject({
      workspaceKey: 'default',
      cwd: '/vault/notes',
      source: 'doc-parent-dir',
    });

    // 4. 库外文件父目录
    const externalDoc = makeDoc({
      isLibrary: false,
      relPath: null,
      absPath: '/Users/dev/notes/external-readme.md',
    });
    expect(resolveTerminalLaunchContext(externalDoc, '/vault')).toMatchObject({
      workspaceKey: 'default',
      cwd: '/Users/dev/notes',
      source: 'doc-parent-dir',
    });

    // 5. 回退到库根
    expect(resolveTerminalLaunchContext(null, '/vault')).toMatchObject({
      workspaceKey: 'default',
      cwd: '/vault',
      source: 'library-root',
    });
  });

  it('2. 折叠保活：隐藏/切走触发 terminal_set_visibility(false) 且不销毁会话；切回触发 true，同工作区不重复 spawn', async () => {
    const store = useTerminalStore();
    await store.initListeners();

    const session = await store.spawnOrFocusContext({
      workspaceKey: 'quicknote',
      workspaceLabel: 'QuickNote',
      cwd: '/repos/quicknote',
      source: 'doc-open-dir',
    });
    expect(session?.sessionId).toBe('term-1-1');
    expect(mockTerminalSpawn).toHaveBeenCalledTimes(1);

    // 同一工作区再次请求启动，直接复用而不重复 spawn
    const reused = await store.spawnOrFocusContext({
      workspaceKey: 'quicknote',
      workspaceLabel: 'QuickNote',
      cwd: '/repos/quicknote',
      source: 'doc-open-dir',
    });
    expect(reused?.sessionId).toBe('term-1-1');
    expect(mockTerminalSpawn).toHaveBeenCalledTimes(1);

    // 切换到 outline 或折叠面板 -> visible = false，会话仍保留
    await store.syncPanelVisibility(false);
    expect(mockTerminalSetVisibility).toHaveBeenCalledWith({
      sessionId: 'term-1-1',
      visible: false,
    });
    expect(store.hasAnySession).toBe(true);
    expect(store.sessions['term-1-1']?.state).toBe('hidden_alive');

    // 切回终端面板 -> visible = true
    await store.syncPanelVisibility(true);
    expect(mockTerminalSetVisibility).toHaveBeenCalledWith({
      sessionId: 'term-1-1',
      visible: true,
    });
    expect(store.sessions['term-1-1']?.state).toBe('active');
  });

  it('3. 收到 terminal:session-state(hasActiveForegroundChild=true) 更新运行指示状态', async () => {
    const store = useTerminalStore();
    await store.initListeners();
    await store.spawnOrFocusContext({
      workspaceKey: 'quicknote',
      workspaceLabel: 'QuickNote',
      cwd: '/repos/quicknote',
      source: 'doc-open-dir',
    });

    expect(store.hasAnyActiveForegroundChild).toBe(false);

    for (const listener of sessionStateListeners) {
      listener({
        sessionId: 'term-1-1',
        workspaceKey: 'quicknote',
        cwd: '/repos/quicknote',
        hasActiveForegroundChild: true,
        outputPaused: false,
        state: 'active',
      });
    }

    expect(store.hasAnyActiveForegroundChild).toBe(true);
  });

  it('4. 收到 outputPaused=true 时渲染 10MB 背压横幅，点击恢复触发清空并 setVisibility(true)', async () => {
    const store = useTerminalStore();
    await store.initListeners();
    await store.spawnOrFocusContext({
      workspaceKey: 'quicknote',
      workspaceLabel: 'QuickNote',
      cwd: '/repos/quicknote',
      source: 'doc-open-dir',
    });

    const clearSpy = vi.fn();
    store.registerSessionBridge('term-1-1', {
      write: vi.fn(),
      clear: clearSpy,
      getSelection: () => '',
    });

    for (const listener of sessionStateListeners) {
      listener({
        sessionId: 'term-1-1',
        workspaceKey: 'quicknote',
        cwd: '/repos/quicknote',
        hasActiveForegroundChild: true,
        outputPaused: true,
        state: 'active',
      });
    }

    const pausedHtml = await renderTerminalPanelHtml();
    expect(pausedHtml).toContain('data-testid="terminal-output-paused-banner"');
    expect(pausedHtml).toContain('qn-banner--warning');
    expect(pausedHtml).toContain('输出过大，已暂停显示');
    expect(pausedHtml).toContain('data-testid="terminal-resume-output-btn"');

    await store.resumePausedOutput('term-1-1');
    expect(clearSpy).toHaveBeenCalledTimes(1);
    expect(store.sessions['term-1-1']?.outputPaused).toBe(false);
    expect(mockTerminalSetVisibility).toHaveBeenCalledWith({
      sessionId: 'term-1-1',
      visible: true,
    });
  });

  it('5. 收到 terminal:exit(reason=idle_recycled) 重置会话态并显示闲置回收提示', async () => {
    const store = useTerminalStore();
    await store.initListeners();
    await store.spawnOrFocusContext({
      workspaceKey: 'quicknote',
      workspaceLabel: 'QuickNote',
      cwd: '/repos/quicknote',
      source: 'doc-open-dir',
    });

    expect(store.hasAnySession).toBe(true);

    for (const listener of exitListeners) {
      listener({
        sessionId: 'term-1-1',
        exitCode: null,
        reason: 'idle_recycled',
      });
    }

    expect(store.hasAnySession).toBe(false);
    expect(store.activeSessionId).toBeNull();
    expect(store.recycledNotice).toContain('闲置 30 分钟已自动回收');

    const emptyHtml = await renderTerminalPanelHtml();
    expect(emptyHtml).toContain('data-testid="terminal-empty-state"');
    expect(emptyHtml).toContain('闲置 30 分钟已自动回收');
    expect(emptyHtml).toContain('重新启动终端');
  });

  it('6. 按键隔离：⌥⌘C 吞掉不触发 editor.codeBlock；⌘C 有选区复制、无选区放行；⌃C/⌃D/⌃Z 直传 PTY', async () => {
    // 1. xterm custom key evaluator
    expect(
      evaluateTerminalKeyEvent(
        {
          type: 'keydown',
          key: 'c',
          metaKey: true,
          altKey: true,
          ctrlKey: false,
          shiftKey: false,
        },
        false,
      ),
    ).toBe('swallow_alt_cmd_c');

    expect(
      evaluateTerminalKeyEvent(
        {
          type: 'keydown',
          key: 'c',
          metaKey: true,
          altKey: false,
          ctrlKey: false,
          shiftKey: false,
        },
        true,
      ),
    ).toBe('copy_selection');

    expect(
      evaluateTerminalKeyEvent(
        {
          type: 'keydown',
          key: 'c',
          metaKey: true,
          altKey: false,
          ctrlKey: false,
          shiftKey: false,
        },
        false,
      ),
    ).toBe('pass_cmd_c_no_selection');

    for (const ctrlKey of ['c', 'd', 'z']) {
      expect(
        evaluateTerminalKeyEvent(
          {
            type: 'keydown',
            key: ctrlKey,
            metaKey: false,
            altKey: false,
            ctrlKey: true,
            shiftKey: false,
          },
          false,
        ),
      ).toBe('pass_to_xterm');
    }

    expect(
      evaluateTerminalKeyEvent(
        {
          type: 'keydown',
          key: 't',
          metaKey: true,
          altKey: false,
          ctrlKey: false,
          shiftKey: true,
        },
        false,
      ),
    ).toBe('pass_to_outer_actions');

    // 2. 外层 actionRegistry 在 terminal 作用域下验证 ⌥⌘C 不会触发 editor.codeBlock
    const registry = createActionRegistry({ storage: null });
    registerDefaultActionCatalog(registry);
    const codeBlockRun = vi.fn();
    registry.bindHandler('editor.codeBlock', codeBlockRun);

    const preventDefault = vi.fn();
    const stopPropagation = vi.fn();
    const result = registry.dispatchKeyEvent(
      {
        key: 'c',
        code: 'KeyC',
        metaKey: true,
        altKey: true,
        ctrlKey: false,
        shiftKey: false,
        isComposing: false,
        preventDefault,
        stopPropagation,
      } as unknown as KeyboardEvent,
      {
        activeScope: 'terminal',
        activeScopes: ['terminal', 'main-window', 'global'],
        hasActiveDoc: true,
        hasSelection: true,
        hasTerminalSelection: false,
        isMarkdown: true,
        isInbox: false,
      },
    );

    expect(result).toEqual({
      handled: false,
      swallowed: true,
      actionId: 'terminal.swallowCodeBlock',
    });
    expect(codeBlockRun).not.toHaveBeenCalled();
    expect(preventDefault).toHaveBeenCalledTimes(1);
    expect(mockTerminalWrite).not.toHaveBeenCalled();
  });

  it('7. 流式 UTF-8 base64 解码：分片中文字节流解码不乱码', () => {
    const decoder = createStreamingBase64Decoder();
    // "你好世界" UTF-8 编码共 12 字节，故意拆成非对齐的 4 字节和 8 字节分片
    const fullBytes = new TextEncoder().encode('你好世界');
    const part1 = btoa(String.fromCharCode(...fullBytes.subarray(0, 4)));
    const part2 = btoa(String.fromCharCode(...fullBytes.subarray(4)));

    const out = decoder.decode(part1) + decoder.decode(part2) + decoder.flush();
    expect(out).toBe('你好世界');

    const roundtrip = encodeUtf8ToBase64('ls -la\n');
    expect(atob(roundtrip)).toBe('ls -la\n');
  });

  it('8. 多工作区新建终端会话时旧活跃会话自动触发 terminal_set_visibility(prevId, false) 并转入 hidden_alive', async () => {
    const store = useTerminalStore();
    await store.syncPanelVisibility(true);

    const sessionA: TerminalSessionInfo = {
      sessionId: 'term-ws-a',
      workspaceKey: 'ws-a',
      cwd: '/tmp/ws-a',
      state: 'active',
      hasActiveForegroundChild: false,
      outputPaused: false,
    };
    const sessionB: TerminalSessionInfo = {
      sessionId: 'term-ws-b',
      workspaceKey: 'ws-b',
      cwd: '/tmp/ws-b',
      state: 'active',
      hasActiveForegroundChild: false,
      outputPaused: false,
    };

    mockTerminalSpawn.mockResolvedValueOnce(sessionA).mockResolvedValueOnce(sessionB);

    await store.spawnOrFocusContext({
      workspaceKey: 'ws-a',
      workspaceLabel: 'ws-a',
      cwd: '/tmp/ws-a',
      source: 'doc-open-dir',
    });
    expect(store.activeSessionId).toBe('term-ws-a');
    expect(mockTerminalSetVisibility).not.toHaveBeenCalled();

    await store.spawnOrFocusContext({
      workspaceKey: 'ws-b',
      workspaceLabel: 'ws-b',
      cwd: '/tmp/ws-b',
      source: 'doc-open-dir',
    });
    expect(store.activeSessionId).toBe('term-ws-b');
    expect(mockTerminalSetVisibility).toHaveBeenCalledWith({
      sessionId: 'term-ws-a',
      visible: false,
    });
    expect(store.sessions['term-ws-a']?.state).toBe('hidden_alive');
    expect(store.sessions['term-ws-b']?.state).toBe('active');
  });

  it('9. Cmd+V 交由原生 paste 链路：不阻止默认事件且不调用 navigator.clipboard.readText()', () => {
    const readTextSpy = vi.fn();
    vi.stubGlobal('navigator', { clipboard: { readText: readTextSpy } });
    try {
      const preventDefault = vi.fn();
      const stopPropagation = vi.fn();
      const copySelection = vi.fn();

      const event = {
        type: 'keydown',
        key: 'v',
        metaKey: true,
        altKey: false,
        ctrlKey: false,
        shiftKey: false,
        preventDefault,
        stopPropagation,
      };

      expect(evaluateTerminalKeyEvent(event, false)).toBe('paste_clipboard');
      const passToXterm = handleTerminalCustomKeyEvent(event, false, { copySelection });

      expect(passToXterm).toBe(false);
      expect(preventDefault).not.toHaveBeenCalled();
      expect(stopPropagation).not.toHaveBeenCalled();
      expect(copySelection).not.toHaveBeenCalled();
      expect(readTextSpy).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('10. 外部终端按钮：活跃会话直接携带 cwd 调用 external_action_run，失败时在 danger 横幅展示提示', async () => {
    const store = useTerminalStore();

    // 无活跃会话且无可用 cwd 时，写入明确错误提示而非静默无反应
    const noDirOk = await store.openExternalTerminalForActiveSession();
    expect(noDirOk).toBe(false);
    expect(store.errorMessage).toContain('未解析到可用目录');
    expect(mockExternalActionRun).not.toHaveBeenCalled();

    // 启动活跃会话后，直接携带 activeSession.cwd 调用 external_action_run
    await store.spawnOrFocusContext({
      workspaceKey: 'quicknote',
      workspaceLabel: 'QuickNote',
      cwd: '/repos/quicknote/scripts',
      source: 'doc-terminal-dir',
    });

    const ok = await store.openExternalTerminalForActiveSession();
    expect(ok).toBe(true);
    expect(store.errorMessage).toBeNull();
    expect(mockExternalActionRun).toHaveBeenCalledWith({
      id: 'external.terminal',
      absPath: null,
      dirPath: '/repos/quicknote/scripts',
    });

    // 模拟未安装 iTerm2 等外部终端缺失错误，验证顶部 danger 横幅展示可读提示
    mockExternalActionRun.mockRejectedValueOnce({
      code: 'externalAppMissing',
      message: 'iTerm',
    });
    const failedOk = await store.openExternalTerminalForActiveSession();
    expect(failedOk).toBe(false);
    expect(store.errorMessage).toContain('iTerm');
    expect(store.errorMessage).toContain('设置 > 外部与搜索 > 外部终端');

    const errorHtml = await renderTerminalPanelHtml();
    expect(errorHtml).toContain('data-testid="terminal-error-banner"');
    expect(errorHtml).toContain('qn-banner--danger');
    expect(errorHtml).toContain('设置 &gt; 外部与搜索 &gt; 外部终端');
  });
});
