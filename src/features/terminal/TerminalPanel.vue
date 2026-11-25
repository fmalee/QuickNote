<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, watch } from 'vue';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import { QnBanner, QnButton } from '@/components';
import { useSettingsStore } from '@/features/settings';
import { handleTerminalCustomKeyEvent, useTerminalStore } from './store';
import { buildTerminalTheme, observeTerminalThemeChange } from './theme';

interface MountedXtermInstance {
  sessionId: string;
  container: HTMLElement;
  term: Terminal;
  fitAddon: FitAddon;
  resizeObserver: ResizeObserver | null;
  resizeTimer: ReturnType<typeof setTimeout> | null;
  disposeData: { dispose(): void } | null;
  disposeSelection: { dispose(): void } | null;
  unregisterBridge: () => void;
}

const RESIZE_DEBOUNCE_MS = 50;

const terminalStore = useTerminalStore();
const settingsStore = useSettingsStore();

const containerElements = new Map<string, HTMLElement>();
const xtermInstances = new Map<string, MountedXtermInstance>();
let stopThemeObserver: (() => void) | null = null;

const launchContext = computed(() => terminalStore.getCurrentLaunchContext());
const currentDocDir = computed(() => terminalStore.getCurrentDocDir());
const activeSession = computed(() => terminalStore.activeSession);
const maxConcurrentSessions = computed<number>(
  () => settingsStore.snapshot?.settings.panels.terminal.maxConcurrentSessions ?? 3,
);
const canCreateNewSession = computed<boolean>(() => {
  if (terminalStore.sessionList.length >= maxConcurrentSessions.value) {
    return false;
  }
  const targetKey = launchContext.value.workspaceKey;
  const hasSameWorkspace = terminalStore.sessionList.some(
    (session) => session.workspaceKey === targetKey,
  );
  return !hasSameWorkspace && Boolean(launchContext.value.cwd);
});

function setContainerRef(sessionId: string, el: unknown): void {
  if (el instanceof HTMLElement) {
    containerElements.set(sessionId, el);
    ensureInstanceMounted(sessionId, el);
  } else {
    containerElements.delete(sessionId);
  }
}

function scheduleFitAndResize(instance: MountedXtermInstance): void {
  if (!terminalStore.isPanelVisible || terminalStore.activeSessionId !== instance.sessionId) {
    return;
  }
  if (instance.resizeTimer !== null) {
    clearTimeout(instance.resizeTimer);
  }
  instance.resizeTimer = setTimeout(() => {
    instance.resizeTimer = null;
    const rect = instance.container.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      return;
    }
    try {
      instance.fitAddon.fit();
      void terminalStore.resizeSession(instance.sessionId, instance.term.cols, instance.term.rows);
    } catch {
      // jsdom 或隐藏态容错
    }
  }, RESIZE_DEBOUNCE_MS);
}

function ensureInstanceMounted(sessionId: string, container: HTMLElement): void {
  const existing = xtermInstances.get(sessionId);
  if (existing) {
    if (existing.container !== container) {
      disposeInstance(sessionId);
    } else {
      return;
    }
  }

  let term: Terminal;
  let fitAddon: FitAddon;
  try {
    term = new Terminal({
      cursorBlink: true,
      scrollback: 5000,
      fontSize: 12,
      fontFamily: 'var(--qn-font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)',
      theme: buildTerminalTheme(),
      allowProposedApi: false,
    });
    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(container);
  } catch {
    // 无头测试环境（无 Canvas 支持）优雅降级
    const unregisterBridge = terminalStore.registerSessionBridge(sessionId, {
      write: () => {},
      clear: () => {},
      getSelection: () => '',
    });
    xtermInstances.set(sessionId, {
      sessionId,
      container,
      term: {} as Terminal,
      fitAddon: {} as FitAddon,
      resizeObserver: null,
      resizeTimer: null,
      disposeData: null,
      disposeSelection: null,
      unregisterBridge,
    });
    return;
  }

  term.attachCustomKeyEventHandler((event: KeyboardEvent) =>
    handleTerminalCustomKeyEvent(event, term.hasSelection(), {
      copySelection: () => {
        void terminalStore.copyActiveSelection();
      },
    }),
  );

  const disposeData = term.onData((data) => {
    void terminalStore.sendInput(sessionId, data);
  });

  const disposeSelection = term.onSelectionChange(() => {
    if (terminalStore.activeSessionId === sessionId) {
      terminalStore.setHasSelection(term.hasSelection());
    }
  });

  const unregisterBridge = terminalStore.registerSessionBridge(sessionId, {
    write: (text) => {
      term.write(text);
    },
    clear: () => {
      term.clear();
    },
    getSelection: () => term.getSelection(),
  });

  let resizeObserver: ResizeObserver | null = null;
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => {
      const current = xtermInstances.get(sessionId);
      if (current) {
        scheduleFitAndResize(current);
      }
    });
    resizeObserver.observe(container);
  }

  const instance: MountedXtermInstance = {
    sessionId,
    container,
    term,
    fitAddon,
    resizeObserver,
    resizeTimer: null,
    disposeData,
    disposeSelection,
    unregisterBridge,
  };
  xtermInstances.set(sessionId, instance);
  scheduleFitAndResize(instance);
}

function disposeInstance(sessionId: string): void {
  const instance = xtermInstances.get(sessionId);
  if (!instance) {
    return;
  }
  if (instance.resizeTimer !== null) {
    clearTimeout(instance.resizeTimer);
    instance.resizeTimer = null;
  }
  instance.resizeObserver?.disconnect();
  instance.disposeData?.dispose();
  instance.disposeSelection?.dispose();
  instance.unregisterBridge();
  if (typeof instance.term.dispose === 'function') {
    instance.term.dispose();
  }
  xtermInstances.delete(sessionId);
}

async function handleStartSession(): Promise<void> {
  await terminalStore.spawnForCurrentContext();
}

async function handleCreateNewSession(): Promise<void> {
  await terminalStore.spawnForCurrentContext();
}

async function handleCloseActiveSession(): Promise<void> {
  if (!terminalStore.activeSessionId) {
    return;
  }
  await terminalStore.closeSession(terminalStore.activeSessionId);
}

async function handleResumePausedOutput(): Promise<void> {
  if (!terminalStore.activeSessionId) {
    return;
  }
  await terminalStore.resumePausedOutput(terminalStore.activeSessionId);
}

async function handleSwitchToCurrentDocDir(): Promise<void> {
  await terminalStore.switchToCurrentDocDir();
}

async function openExternalTerminal(): Promise<void> {
  await terminalStore.openCurrentCwdInExternalTerminal();
}

watch(
  () => terminalStore.sessionList.map((s) => s.sessionId),
  (activeIds) => {
    const activeSet = new Set(activeIds);
    for (const existingId of Array.from(xtermInstances.keys())) {
      if (!activeSet.has(existingId)) {
        disposeInstance(existingId);
      }
    }
  },
);

watch(
  () => [terminalStore.isPanelVisible, terminalStore.activeSessionId] as const,
  async ([visible, sessionId]) => {
    if (visible && sessionId) {
      await nextTick();
      const instance = xtermInstances.get(sessionId);
      if (instance) {
        scheduleFitAndResize(instance);
      }
    }
    if (
      visible &&
      !terminalStore.hasAnySession &&
      !terminalStore.hasAutoStartedOnce &&
      !terminalStore.spawning &&
      !terminalStore.recycledNotice &&
      launchContext.value.cwd
    ) {
      await terminalStore.spawnForCurrentContext();
    }
  },
);

onMounted(() => {
  void terminalStore.initListeners();
  stopThemeObserver = observeTerminalThemeChange((theme) => {
    for (const instance of xtermInstances.values()) {
      if (instance.term && instance.term.options) {
        instance.term.options.theme = theme;
      }
    }
  });
  if (
    terminalStore.isPanelVisible &&
    !terminalStore.hasAnySession &&
    !terminalStore.hasAutoStartedOnce &&
    !terminalStore.spawning &&
    !terminalStore.recycledNotice &&
    launchContext.value.cwd
  ) {
    void terminalStore.spawnForCurrentContext();
  }
});

onUnmounted(() => {
  stopThemeObserver?.();
  stopThemeObserver = null;
  for (const sessionId of Array.from(xtermInstances.keys())) {
    disposeInstance(sessionId);
  }
});
</script>

<template>
  <div class="qn-terminal-panel" data-testid="terminal-panel" data-action-scope="terminal">
    <!-- 顶部控制栏 -->
    <header class="qn-terminal-panel__header">
      <div class="qn-terminal-panel__meta">
        <span class="qn-terminal-panel__workspace">
          {{ activeSession ? activeSession.workspaceKey : launchContext.workspaceLabel }}
        </span>
        <span
          class="qn-terminal-panel__cwd"
          :title="activeSession?.cwd ?? launchContext.cwd ?? '未选择目录'"
        >
          {{ activeSession?.cwd ?? launchContext.cwd ?? '未配置工作区目录' }}
        </span>
      </div>

      <div class="qn-terminal-panel__actions">
        <button
          v-if="canCreateNewSession && terminalStore.hasAnySession"
          type="button"
          class="qn-terminal-panel__icon-btn"
          title="新建当前工作区终端会话"
          data-testid="terminal-new-session-btn"
          @click="handleCreateNewSession"
        >
          + 新建
        </button>
        <button
          type="button"
          class="qn-terminal-panel__icon-btn"
          :title="currentDocDir ? `切换至当前文档目录 (${currentDocDir})` : '切换至当前文档目录'"
          :disabled="!currentDocDir"
          data-testid="terminal-cd-current-doc-btn"
          @click="handleSwitchToCurrentDocDir"
        >
          切换至当前文档目录
        </button>
        <button
          type="button"
          class="qn-terminal-panel__icon-btn"
          title="在外部终端中打开 (⌥⌘T)"
          data-testid="terminal-open-external-btn"
          @click="openExternalTerminal"
        >
          在外部终端中打开
        </button>
        <button
          v-if="activeSession"
          type="button"
          class="qn-terminal-panel__icon-btn qn-terminal-panel__icon-btn--danger"
          title="关闭当前终端会话"
          data-testid="terminal-close-session-btn"
          @click="handleCloseActiveSession"
        >
          × 关闭会话
        </button>
      </div>
    </header>

    <!-- 多会话切换条（仅在多于 1 个会话时显示） -->
    <div
      v-if="terminalStore.sessionList.length > 1"
      class="qn-terminal-panel__session-tabs"
      data-testid="terminal-session-tabs"
    >
      <button
        v-for="session in terminalStore.sessionList"
        :key="session.sessionId"
        type="button"
        :class="[
          'qn-terminal-panel__session-chip',
          {
            'qn-terminal-panel__session-chip--active':
              session.sessionId === terminalStore.activeSessionId,
          },
        ]"
        :data-testid="`terminal-session-chip-${session.sessionId}`"
        @click="terminalStore.selectSession(session.sessionId)"
      >
        <span
          v-if="session.hasActiveForegroundChild"
          class="qn-terminal-panel__running-dot"
          aria-label="有前台进程正在运行"
        />
        <span>{{ session.workspaceKey }}</span>
      </button>
    </div>

    <!-- 错误提示横幅 -->
    <QnBanner
      v-if="terminalStore.errorMessage"
      tone="danger"
      class="qn-terminal-panel__banner"
      data-testid="terminal-error-banner"
    >
      <div class="qn-terminal-panel__banner-row">
        <span>{{ terminalStore.errorMessage }}</span>
        <QnButton size="sm" variant="ghost" @click="terminalStore.clearNotice()">知道了</QnButton>
      </div>
    </QnBanner>

    <!-- 10MB 背压暂停横幅 -->
    <QnBanner
      v-if="activeSession?.outputPaused"
      tone="warning"
      class="qn-terminal-panel__banner"
      data-testid="terminal-output-paused-banner"
    >
      <div class="qn-terminal-panel__banner-row">
        <span>输出过大，已暂停显示</span>
        <QnButton
          size="sm"
          variant="secondary"
          data-testid="terminal-resume-output-btn"
          @click="handleResumePausedOutput"
        >
          清空并恢复显示
        </QnButton>
      </div>
    </QnBanner>

    <!-- 主体：空态引导卡片 或 xterm.js 容器列表（v-show 保活） -->
    <div class="qn-terminal-panel__stage">
      <div
        v-if="!terminalStore.hasAnySession"
        class="qn-terminal-panel__empty"
        data-testid="terminal-empty-state"
      >
        <div class="qn-terminal-panel__empty-card">
          <div class="qn-terminal-panel__empty-title">
            {{ terminalStore.recycledNotice ? '终端会话已结束' : '内嵌终端' }}
          </div>
          <p class="qn-terminal-panel__empty-desc">
            {{
              terminalStore.recycledNotice ??
              (launchContext.cwd
                ? `将在目录 ${launchContext.cwd} 启动登录 Shell。`
                : '当前未解析到有效目录，请先在设置中配置工作区或打开本地笔记。')
            }}
          </p>
          <QnButton
            variant="primary"
            size="sm"
            :disabled="!launchContext.cwd || terminalStore.spawning"
            data-testid="terminal-start-btn"
            @click="handleStartSession"
          >
            {{
              terminalStore.spawning
                ? '正在启动…'
                : terminalStore.recycledNotice
                  ? '重新启动终端'
                  : '启动内嵌终端'
            }}
          </QnButton>
        </div>
      </div>

      <div
        v-for="session in terminalStore.sessionList"
        v-show="session.sessionId === terminalStore.activeSessionId"
        :key="session.sessionId"
        :ref="(el) => setContainerRef(session.sessionId, el)"
        class="qn-terminal-panel__xterm-host"
        :data-testid="`terminal-xterm-host-${session.sessionId}`"
      />
    </div>
  </div>
</template>

<style scoped>
.qn-terminal-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: var(--qn-bg-base);
}

.qn-terminal-panel__header {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-terminal-panel__meta {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-terminal-panel__workspace {
  flex-shrink: 0;
  padding: 1px 6px;
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
  font-weight: 600;
}

.qn-terminal-panel__cwd {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
}

.qn-terminal-panel__actions {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-terminal-panel__icon-btn {
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-secondary);
  padding: 2px 6px;
  font-size: var(--qn-text-xs);
  cursor: pointer;
  white-space: nowrap;
}

.qn-terminal-panel__icon-btn:hover:not(:disabled) {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-terminal-panel__icon-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.qn-terminal-panel__icon-btn--danger:hover:not(:disabled) {
  color: var(--qn-danger);
  border-color: var(--qn-danger);
}

.qn-terminal-panel__session-tabs {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  overflow-x: auto;
  scrollbar-width: none;
}

.qn-terminal-panel__session-tabs::-webkit-scrollbar {
  display: none;
}

.qn-terminal-panel__session-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--qn-space-1);
  padding: 2px 8px;
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
  white-space: nowrap;
}

.qn-terminal-panel__session-chip--active {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
  font-weight: 600;
}

.qn-terminal-panel__running-dot {
  width: 6px;
  height: 6px;
  border-radius: var(--qn-radius-full);
  background-color: var(--qn-success);
  flex-shrink: 0;
}

.qn-terminal-panel__banner {
  flex-shrink: 0;
  margin: var(--qn-space-2) var(--qn-space-2) 0;
}

.qn-terminal-panel__banner-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  width: 100%;
}

.qn-terminal-panel__stage {
  flex: 1;
  min-height: 0;
  position: relative;
  display: flex;
  flex-direction: column;
}

.qn-terminal-panel__empty {
  flex: 1;
  min-height: 0;
  padding: var(--qn-space-3);
  display: flex;
  flex-direction: column;
}

.qn-terminal-panel__empty-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--qn-space-2);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-sidebar);
}

.qn-terminal-panel__empty-title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-terminal-panel__empty-desc {
  margin: 0;
  font-size: var(--qn-text-xs);
  line-height: 1.5;
  color: var(--qn-text-secondary);
  word-break: break-all;
}

.qn-terminal-panel__xterm-host {
  flex: 1;
  min-height: 0;
  padding: var(--qn-space-1) var(--qn-space-2);
  overflow: hidden;
}

.qn-terminal-panel__xterm-host :deep(.xterm-viewport) {
  scrollbar-width: none;
  overscroll-behavior: contain;
}

.qn-terminal-panel__xterm-host :deep(.xterm-viewport::-webkit-scrollbar) {
  display: none;
}

.qn-terminal-panel__xterm-host :deep(.xterm-scrollable-element > .scrollbar),
.qn-terminal-panel__xterm-host :deep(.xterm-scrollable-element > .shadow) {
  display: none !important;
  opacity: 0 !important;
  pointer-events: none !important;
}
</style>
