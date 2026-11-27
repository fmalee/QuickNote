import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type {
  NoteDoc,
  TerminalData,
  TerminalExitPayload,
  TerminalSessionInfo,
} from '@/bindings/types';
import { useEditorTabsStore } from '@/features/editor';
import { useSettingsStore } from '@/features/settings';
import type { UnlistenFn } from '@/lib/ipc/events';
import {
  extractTerminalErrorMessage,
  killTerminalSession,
  listTerminalSessions,
  openExternalTerminalAtDir,
  resizeTerminalPty,
  setTerminalVisibility,
  spawnTerminalSession,
  subscribeTerminalExit,
  subscribeTerminalSessionState,
  writeTerminalInput,
} from './api';
import {
  createStreamingBase64Decoder,
  encodeUtf8ToBase64,
  type StreamingBase64Decoder,
} from './theme';

export type TerminalLaunchSource =
  'doc-terminal-dir' | 'doc-open-dir' | 'doc-parent-dir' | 'library-root' | 'none';

export interface TerminalLaunchContext {
  workspaceKey: string;
  workspaceLabel: string;
  cwd: string | null;
  source: TerminalLaunchSource;
  activeDocPath?: string | null;
}

export interface TerminalSessionBridge {
  write: (text: string) => void;
  clear: () => void;
  getSelection: () => string;
}

function normalizeNonEmpty(value: string | null | undefined): string | null {
  if (typeof value !== 'string') {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function extractParentDir(absPath: string): string | null {
  const normalized = absPath.replace(/[\\/]+$/, '');
  const lastSep = Math.max(normalized.lastIndexOf('/'), normalized.lastIndexOf('\\'));
  if (lastSep <= 0) {
    return lastSep === 0 ? '/' : null;
  }
  return normalized.slice(0, lastSep);
}

function quoteShellPath(dirPath: string): string {
  return `'${dirPath.replace(/'/g, `'\\''`)}'`;
}

export function resolveTerminalLaunchContext(
  activeDoc: NoteDoc | null,
  libraryRoot: string | null,
): TerminalLaunchContext {
  const activeDocPath = normalizeNonEmpty(activeDoc?.absPath);
  const fallbackKey = 'default';
  const fallbackLabel = '默认工作区';

  // 外部文件终端默认 cwd 必须取该文件所在父目录
  if (activeDoc && !activeDoc.isLibrary && activeDocPath) {
    const parent = extractParentDir(activeDocPath);
    if (parent) {
      return {
        workspaceKey: fallbackKey,
        workspaceLabel: fallbackLabel,
        cwd: parent,
        source: 'doc-parent-dir',
        activeDocPath,
      };
    }
  }

  const docTerminalDir = normalizeNonEmpty(activeDoc?.meta.terminalDir);
  if (docTerminalDir) {
    return {
      workspaceKey: fallbackKey,
      workspaceLabel: fallbackLabel,
      cwd: docTerminalDir,
      source: 'doc-terminal-dir',
      activeDocPath,
    };
  }

  const docOpenDir = normalizeNonEmpty(activeDoc?.meta.openDir);
  if (docOpenDir) {
    return {
      workspaceKey: fallbackKey,
      workspaceLabel: fallbackLabel,
      cwd: docOpenDir,
      source: 'doc-open-dir',
      activeDocPath,
    };
  }

  if (activeDocPath) {
    const parent = extractParentDir(activeDocPath);
    if (parent) {
      return {
        workspaceKey: fallbackKey,
        workspaceLabel: fallbackLabel,
        cwd: parent,
        source: 'doc-parent-dir',
        activeDocPath,
      };
    }
  }

  const rootDir = normalizeNonEmpty(libraryRoot);
  if (rootDir) {
    return {
      workspaceKey: fallbackKey,
      workspaceLabel: fallbackLabel,
      cwd: rootDir,
      source: 'library-root',
      activeDocPath,
    };
  }

  return {
    workspaceKey: fallbackKey,
    workspaceLabel: fallbackLabel,
    cwd: null,
    source: 'none',
    activeDocPath,
  };
}

export const useTerminalStore = defineStore('terminal', () => {
  const sessions = ref<Record<string, TerminalSessionInfo>>({});
  const activeSessionId = ref<string | null>(null);
  const hasSelection = ref<boolean>(false);
  const spawning = ref<boolean>(false);
  const errorMessage = ref<string | null>(null);
  const recycledNotice = ref<string | null>(null);
  const isPanelVisible = ref<boolean>(false);
  const hasAutoStartedOnce = ref<boolean>(false);

  const decoders = new Map<string, StreamingBase64Decoder>();
  const pendingOutput = new Map<string, string[]>();
  const bridges = new Map<string, TerminalSessionBridge>();

  let listenersBound = false;
  let unlistenSessionState: UnlistenFn | null = null;
  let unlistenExit: UnlistenFn | null = null;

  const sessionList = computed<TerminalSessionInfo[]>(() => Object.values(sessions.value));

  const activeSession = computed<TerminalSessionInfo | null>(() => {
    if (!activeSessionId.value) {
      return null;
    }
    return sessions.value[activeSessionId.value] ?? null;
  });

  const hasAnySession = computed<boolean>(() => sessionList.value.length > 0);

  const hasAnyActiveForegroundChild = computed<boolean>(() =>
    sessionList.value.some((session) => session.hasActiveForegroundChild),
  );

  function getCurrentLaunchContext(): TerminalLaunchContext {
    const editorTabsStore = useEditorTabsStore();
    const settingsStore = useSettingsStore();

    const libraryRoot = settingsStore.snapshot?.settings.libraryRoot ?? null;
    return resolveTerminalLaunchContext(editorTabsStore.activeDoc, libraryRoot);
  }

  function getOrCreateDecoder(sessionId: string): StreamingBase64Decoder {
    let decoder = decoders.get(sessionId);
    if (!decoder) {
      decoder = createStreamingBase64Decoder();
      decoders.set(sessionId, decoder);
    }
    return decoder;
  }

  function handleIncomingData(chunk: TerminalData): void {
    const decoder = getOrCreateDecoder(chunk.sessionId);
    const decoded = decoder.decode(chunk.dataBase64);
    if (!decoded) {
      return;
    }
    const bridge = bridges.get(chunk.sessionId);
    if (bridge) {
      bridge.write(decoded);
      return;
    }
    const queue = pendingOutput.get(chunk.sessionId) ?? [];
    queue.push(decoded);
    pendingOutput.set(chunk.sessionId, queue);
  }

  function registerSessionBridge(sessionId: string, bridge: TerminalSessionBridge): () => void {
    bridges.set(sessionId, bridge);
    const queued = pendingOutput.get(sessionId);
    if (queued && queued.length > 0) {
      for (const text of queued) {
        bridge.write(text);
      }
      pendingOutput.delete(sessionId);
    }
    return () => {
      if (bridges.get(sessionId) === bridge) {
        bridges.delete(sessionId);
      }
    };
  }

  function cleanupSessionResources(sessionId: string): void {
    decoders.delete(sessionId);
    pendingOutput.delete(sessionId);
    bridges.delete(sessionId);
  }

  function handleSessionStateEvent(info: TerminalSessionInfo): void {
    if (info.state === 'recycled') {
      return;
    }
    sessions.value = {
      ...sessions.value,
      [info.sessionId]: info,
    };
  }

  function handleExitEvent(payload: TerminalExitPayload): void {
    if (sessions.value[payload.sessionId]) {
      const next = { ...sessions.value };
      delete next[payload.sessionId];
      sessions.value = next;
    }
    cleanupSessionResources(payload.sessionId);

    if (activeSessionId.value === payload.sessionId) {
      const remaining = Object.keys(sessions.value);
      activeSessionId.value = remaining[0] ?? null;
      hasSelection.value = false;
    }

    if (payload.reason === 'idle_recycled') {
      recycledNotice.value = '终端因闲置 30 分钟已自动回收，点击重新启动';
    } else if (payload.reason === 'process_exit') {
      recycledNotice.value =
        payload.exitCode !== null
          ? `终端进程已退出（退出码 ${payload.exitCode}），点击重新启动`
          : '终端进程已退出，点击重新启动';
    } else {
      recycledNotice.value = null;
    }
  }

  async function initListeners(): Promise<void> {
    if (listenersBound) {
      return;
    }
    listenersBound = true;
    try {
      unlistenSessionState = await subscribeTerminalSessionState((info) => {
        handleSessionStateEvent(info);
      });
      unlistenExit = await subscribeTerminalExit((payload) => {
        handleExitEvent(payload);
      });
    } catch {
      // 非 Tauri 测试环境容错
    }
  }

  function disposeListeners(): void {
    unlistenSessionState?.();
    unlistenSessionState = null;
    unlistenExit?.();
    unlistenExit = null;
    listenersBound = false;
  }

  async function refreshSessions(): Promise<void> {
    try {
      const list = await listTerminalSessions();
      const map: Record<string, TerminalSessionInfo> = {};
      for (const item of list) {
        map[item.sessionId] = item;
      }
      sessions.value = map;
      if (activeSessionId.value && !map[activeSessionId.value]) {
        activeSessionId.value = list[0]?.sessionId ?? null;
      } else if (!activeSessionId.value && list.length > 0) {
        activeSessionId.value = list[0]?.sessionId ?? null;
      }
    } catch {
      // 非 Tauri 环境容错
    }
  }

  async function spawnOrFocusContext(
    context: TerminalLaunchContext,
    options?: { cols?: number; rows?: number },
  ): Promise<TerminalSessionInfo | null> {
    const existing = sessionList.value.find(
      (session) => session.workspaceKey === context.workspaceKey,
    );
    if (existing) {
      errorMessage.value = null;
      recycledNotice.value = null;
      await selectSession(existing.sessionId);
      return existing;
    }

    const spawnTarget = normalizeNonEmpty(context.activeDocPath) ?? normalizeNonEmpty(context.cwd);
    if (!spawnTarget) {
      errorMessage.value = '当前未找到可用的工作区目录，请先在设置中配置工作区或打开笔记目录';
      return null;
    }

    spawning.value = true;
    errorMessage.value = null;
    recycledNotice.value = null;

    const settingsStore = useSettingsStore();
    const configuredShell = normalizeNonEmpty(
      settingsStore.snapshot?.settings.panels.terminal.shell ?? null,
    );
    const prevId = activeSessionId.value;

    try {
      const info = await spawnTerminalSession(
        {
          workspaceKey: context.workspaceKey,
          cwd: spawnTarget,
          cols: Math.max(10, Math.floor(options?.cols ?? 80)),
          rows: Math.max(4, Math.floor(options?.rows ?? 24)),
          shell: configuredShell,
        },
        handleIncomingData,
      );
      const nextSessions = {
        ...sessions.value,
        [info.sessionId]: !isPanelVisible.value
          ? { ...info, state: 'hidden_alive' as const }
          : info,
      };
      if (prevId && prevId !== info.sessionId && nextSessions[prevId]) {
        nextSessions[prevId] = {
          ...nextSessions[prevId],
          state: 'hidden_alive',
        };
        void setTerminalVisibility({ sessionId: prevId, visible: false }).catch(() => undefined);
      }
      if (!isPanelVisible.value) {
        void setTerminalVisibility({ sessionId: info.sessionId, visible: false }).catch(
          () => undefined,
        );
      }
      sessions.value = nextSessions;
      activeSessionId.value = info.sessionId;
      hasAutoStartedOnce.value = true;
      return nextSessions[info.sessionId] ?? info;
    } catch (error) {
      errorMessage.value = extractTerminalErrorMessage(error);
      return null;
    } finally {
      spawning.value = false;
    }
  }

  async function spawnForCurrentContext(options?: {
    cols?: number;
    rows?: number;
  }): Promise<TerminalSessionInfo | null> {
    const context = getCurrentLaunchContext();
    return spawnOrFocusContext(context, options);
  }

  async function selectSession(sessionId: string): Promise<void> {
    if (!sessions.value[sessionId]) {
      return;
    }
    const prevId = activeSessionId.value;
    activeSessionId.value = sessionId;
    hasSelection.value = Boolean(bridges.get(sessionId)?.getSelection());

    if (isPanelVisible.value) {
      if (prevId && prevId !== sessionId && sessions.value[prevId]) {
        void setTerminalVisibility({ sessionId: prevId, visible: false }).catch(() => {});
      }
      await setTerminalVisibility({ sessionId, visible: true }).catch(() => {});
    }
  }

  async function syncPanelVisibility(visible: boolean): Promise<void> {
    isPanelVisible.value = visible;
    const currentSessions = sessionList.value;
    if (currentSessions.length === 0) {
      return;
    }

    if (!visible) {
      await Promise.all(
        currentSessions.map(async (session) => {
          sessions.value = {
            ...sessions.value,
            [session.sessionId]: { ...session, state: 'hidden_alive' },
          };
          await setTerminalVisibility({ sessionId: session.sessionId, visible: false }).catch(
            () => {},
          );
        }),
      );
      return;
    }

    const targetId = activeSessionId.value ?? currentSessions[0]?.sessionId ?? null;
    if (!targetId) {
      return;
    }
    activeSessionId.value = targetId;
    const target = sessions.value[targetId];
    if (target) {
      sessions.value = {
        ...sessions.value,
        [targetId]: { ...target, state: 'active' },
      };
    }
    await setTerminalVisibility({ sessionId: targetId, visible: true }).catch(() => {});
  }

  async function resumePausedOutput(sessionId: string): Promise<void> {
    bridges.get(sessionId)?.clear();
    pendingOutput.delete(sessionId);
    const current = sessions.value[sessionId];
    if (current) {
      sessions.value = {
        ...sessions.value,
        [sessionId]: { ...current, outputPaused: false },
      };
    }
    await setTerminalVisibility({ sessionId, visible: true }).catch(() => {});
  }

  async function closeSession(sessionId: string): Promise<void> {
    try {
      await killTerminalSession({ sessionId });
    } catch {
      // 若后端已退出则继续清理本地状态
    }
    if (sessions.value[sessionId]) {
      const next = { ...sessions.value };
      delete next[sessionId];
      sessions.value = next;
    }
    cleanupSessionResources(sessionId);
    if (activeSessionId.value === sessionId) {
      const remaining = Object.keys(sessions.value);
      activeSessionId.value = remaining[0] ?? null;
      hasSelection.value = false;
    }
  }

  async function sendInput(sessionId: string, text: string): Promise<void> {
    if (!text) {
      return;
    }
    const dataBase64 = encodeUtf8ToBase64(text);
    await writeTerminalInput({ sessionId, dataBase64 });
  }

  async function resizeSession(sessionId: string, cols: number, rows: number): Promise<void> {
    if (cols <= 0 || rows <= 0) {
      return;
    }
    await resizeTerminalPty({ sessionId, cols, rows }).catch(() => {});
  }

  function setHasSelection(selected: boolean): void {
    hasSelection.value = selected;
  }

  async function copyActiveSelection(): Promise<boolean> {
    if (!activeSessionId.value) {
      return false;
    }
    const text = bridges.get(activeSessionId.value)?.getSelection() ?? '';
    if (!text) {
      return false;
    }
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
    return false;
  }

  async function pasteFromClipboardToSession(sessionId: string): Promise<boolean> {
    if (typeof navigator === 'undefined' || !navigator.clipboard?.readText) {
      return false;
    }
    const text = await navigator.clipboard.readText();
    if (!text) {
      return false;
    }
    await sendInput(sessionId, text);
    return true;
  }

  function getCurrentDocDir(): string | null {
    const editorTabsStore = useEditorTabsStore();
    const docPath = normalizeNonEmpty(editorTabsStore.activeDoc?.absPath);
    if (docPath) {
      const parent = extractParentDir(docPath);
      if (parent) {
        return parent;
      }
    }
    return normalizeNonEmpty(getCurrentLaunchContext().cwd);
  }

  async function switchToCurrentDocDir(): Promise<boolean> {
    const targetDir = getCurrentDocDir();
    if (!targetDir) {
      errorMessage.value = '当前未找到可切换的文档目录，请先打开本地文档或配置工作区';
      return false;
    }
    errorMessage.value = null;

    const sid = activeSessionId.value;
    if (!sid || !sessions.value[sid]) {
      const spawned = await spawnForCurrentContext();
      return Boolean(spawned);
    }

    try {
      await sendInput(sid, `cd ${quoteShellPath(targetDir)}\n`);
      const current = sessions.value[sid];
      if (current) {
        sessions.value = {
          ...sessions.value,
          [sid]: {
            ...current,
            cwd: targetDir,
          },
        };
      }
      return true;
    } catch (error) {
      errorMessage.value = extractTerminalErrorMessage(error);
      return false;
    }
  }

  async function openCurrentCwdInExternalTerminal(): Promise<boolean> {
    const targetDir =
      normalizeNonEmpty(activeSession.value?.cwd) ??
      normalizeNonEmpty(getCurrentLaunchContext().cwd);
    if (!targetDir) {
      errorMessage.value = '未解析到可用目录，请先在设置中配置工作区或为文档配置 open_dir';
      return false;
    }
    errorMessage.value = null;
    try {
      await openExternalTerminalAtDir(targetDir);
      return true;
    } catch (error) {
      errorMessage.value = extractTerminalErrorMessage(error);
      return false;
    }
  }

  async function openExternalTerminalForActiveSession(): Promise<boolean> {
    return openCurrentCwdInExternalTerminal();
  }

  function clearNotice(): void {
    errorMessage.value = null;
    recycledNotice.value = null;
  }

  return {
    sessions,
    activeSessionId,
    sessionList,
    activeSession,
    hasAnySession,
    hasAnyActiveForegroundChild,
    hasSelection,
    spawning,
    errorMessage,
    recycledNotice,
    isPanelVisible,
    hasAutoStartedOnce,
    getCurrentLaunchContext,
    getCurrentDocDir,
    registerSessionBridge,
    handleIncomingData,
    handleSessionStateEvent,
    handleExitEvent,
    initListeners,
    disposeListeners,
    refreshSessions,
    spawnOrFocusContext,
    spawnForCurrentContext,
    switchToCurrentDocDir,
    selectSession,
    syncPanelVisibility,
    resumePausedOutput,
    closeSession,
    sendInput,
    resizeSession,
    setHasSelection,
    copyActiveSelection,
    pasteFromClipboardToSession,
    openCurrentCwdInExternalTerminal,
    openExternalTerminalForActiveSession,
    clearNotice,
  };
});

export type TerminalKeyAction =
  | 'pass_to_xterm'
  | 'swallow_alt_cmd_c'
  | 'copy_selection'
  | 'pass_cmd_c_no_selection'
  | 'paste_clipboard'
  | 'pass_to_outer_actions';

export interface TerminalKeyEventLike {
  type: string;
  key: string;
  keyCode?: number;
  isComposing?: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export function evaluateTerminalKeyEvent(
  event: TerminalKeyEventLike,
  hasSelection: boolean,
): TerminalKeyAction {
  if (event.isComposing || event.keyCode === 229) {
    return 'pass_to_xterm';
  }

  const key = event.key.toLowerCase();

  // ⌥⌘C：显式吞掉，严禁穿透触发编辑器 editor.codeBlock，也不发给 PTY
  if (event.altKey && event.metaKey && !event.ctrlKey && key === 'c') {
    return 'swallow_alt_cmd_c';
  }

  // ⌘C：有选区时复制并拦截；无选区时放行给系统/外层默认行为，不写入 PTY
  if (event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey && key === 'c') {
    return hasSelection ? 'copy_selection' : 'pass_cmd_c_no_selection';
  }

  // ⌘V：交由浏览器/WKWebView 原生 paste 事件直贴 xterm，不触发 navigator.clipboard.readText()
  if (event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey && key === 'v') {
    return 'paste_clipboard';
  }

  // ⌃C / ⌃D / ⌃Z：必须直传 PTY
  if (
    event.ctrlKey &&
    !event.metaKey &&
    !event.altKey &&
    (key === 'c' || key === 'd' || key === 'z')
  ) {
    return 'pass_to_xterm';
  }

  // 其他带 ⌘ 的组合键（如 ⌃⌘T、⌃⌘O、⌃⌘0、⌘K 等）放行给外层 actionRegistry
  if (event.metaKey) {
    return 'pass_to_outer_actions';
  }

  return 'pass_to_xterm';
}

export interface TerminalCustomKeyEventInput extends TerminalKeyEventLike {
  preventDefault: () => void;
  stopPropagation: () => void;
}

export interface TerminalCustomKeyCallbacks {
  copySelection: () => void;
}

export function handleTerminalCustomKeyEvent(
  event: TerminalCustomKeyEventInput,
  hasSelection: boolean,
  callbacks: TerminalCustomKeyCallbacks,
): boolean {
  const decision = evaluateTerminalKeyEvent(event, hasSelection);
  if (decision === 'swallow_alt_cmd_c') {
    if (event.type === 'keydown') {
      event.preventDefault();
      event.stopPropagation();
    }
    return false;
  }
  if (decision === 'copy_selection') {
    if (event.type === 'keydown') {
      event.preventDefault();
      event.stopPropagation();
      callbacks.copySelection();
    }
    return false;
  }
  if (
    decision === 'pass_cmd_c_no_selection' ||
    decision === 'paste_clipboard' ||
    decision === 'pass_to_outer_actions'
  ) {
    return false;
  }
  return true;
}
