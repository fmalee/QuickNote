<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import type { WindowMode, WindowsUiState } from '@/bindings/types';
import { QnBanner, QnButton, QnModalShell } from '@/components';
import { CommandBookPalette, useCommandPalette } from '@/features/commands';
import {
  DocHeaderBar,
  EditorConflictBanner,
  EditorMergeView,
  EditorTabBar,
  ExportPdfModal,
  EXTERNAL_ACTION_SPECS,
  InboxBloatBanner,
  InboxExtractModal,
  MarkdownEditor,
  copyMarkdownAsRichText,
  useEditorTabsStore,
  useExternalActions,
} from '@/features/editor';
import { NewNoteTemplateModal, NotesSidebar, useNotesStore } from '@/features/notes';
import { OnboardingView, useOnboardingStore } from '@/features/onboarding';
import { usePanelsStore } from '@/features/panels';
import {
  PENDING_OPEN_FILE_STORAGE_KEY,
  QuickOpenPalette,
  consumePendingOpenFile,
  useQuickOpen,
} from '@/features/quickopen';
import { SettingsErrorBanner, useSettingsStore } from '@/features/settings';
import { TerminalPanel, useTerminalStore } from '@/features/terminal';
import {
  ACTION_KEYBINDINGS_STORAGE_KEY,
  actionRegistry,
  collectNativeMenuAccelerators,
  formatKeyComboDisplay,
  type ActionContext,
  type ActionScope,
} from '@/lib/actions';
import {
  appSetMenuAccelerators,
  uiStateGet,
  uiStateSet,
  windowSetQuickBarPinned,
  windowShowMain,
} from '@/lib/ipc/commands';
import { onMenuAction, onToastAppendUndo, onWindowState, type UnlistenFn } from '@/lib/ipc/events';

const onboardingStore = useOnboardingStore();
const settingsStore = useSettingsStore();
const notesStore = useNotesStore();
const editorTabsStore = useEditorTabsStore();
const externalActions = useExternalActions();
const panelsStore = usePanelsStore();
const terminalStore = useTerminalStore();

const shellRef = ref<HTMLElement | null>(null);
const quickOpenModalOpen = ref(false);
const quickOpenFocusTick = ref(0);
const commandBookModalOpen = ref(false);
const commandBookFocusTick = ref(0);
const exportPdfModalOpen = ref(false);
const mainWindowMode = ref<WindowMode>(
  settingsStore.snapshot?.settings.mainWindow.mode ?? 'standard',
);
panelsStore.setForceOverlayDrawer(mainWindowMode.value === 'auxiliary');
let cachedWindowsUiState: WindowsUiState | null = null;
let windowModePersistTimer: ReturnType<typeof setTimeout> | null = null;
let unlistenToastAppendUndo: UnlistenFn | null = null;
let unlistenMainWindowState: UnlistenFn | null = null;
let unlistenMenuAction: UnlistenFn | null = null;
let crossWindowListenersBound = false;
let viewportResizeObserver: ResizeObserver | null = null;
let pendingOpenInFlight: Promise<unknown> | null = null;
let lastPendingOpenAtMs = 0;
const CROSS_WINDOW_OPEN_GRACE_MS = 2000;
const actionDisposers: Array<() => void> = [];

function resolveActiveScopes(): ActionScope[] {
  const scopes: ActionScope[] = ['main-window', 'global'];
  if (typeof document === 'undefined') {
    return scopes;
  }

  if (
    quickOpenModalOpen.value ||
    commandBookModalOpen.value ||
    document.querySelector('[role="dialog"]')
  ) {
    scopes.unshift('modal');
  }

  const activeEl = document.activeElement;
  if (activeEl instanceof Element) {
    if (activeEl.closest('.xterm, [data-action-scope="terminal"]')) {
      scopes.unshift('terminal');
    } else if (activeEl.closest('.cm-editor, [data-testid="editor-body-stage"]')) {
      scopes.unshift('editor');
    } else if (activeEl.closest('[data-testid="notes-sidebar"], .qn-notes-sidebar')) {
      scopes.unshift('list');
    }
  }

  return scopes;
}

function buildActionContext(): ActionContext {
  const activeScopes = resolveActiveScopes();
  const activeTab = editorTabsStore.activeTab;
  const activeDoc = editorTabsStore.activeDoc;
  const relPath = activeDoc?.relPath ?? '';
  const pathForKind = activeDoc?.absPath ?? relPath;
  return {
    activeScope: activeScopes[0] ?? 'main-window',
    activeScopes,
    hasActiveDoc: Boolean(activeDoc),
    hasSelection: Boolean(activeTab?.hasSelection),
    hasTerminalSelection: terminalStore.hasSelection,
    isMarkdown: /\.(md|markdown)$/i.test(pathForKind),
    isInbox: relPath === 'inbox.md',
  };
}

const mainQuickOpenEngine = useQuickOpen({
  getActionItems: () => {
    const ctx = buildActionContext();
    return actionRegistry
      .getAll()
      .filter((action) => action.scope !== 'quickbar')
      .map((action) => ({
        id: action.id,
        title: action.title,
        subtitle: action.id,
        shortcutLabel: formatKeyComboDisplay(actionRegistry.getEffectiveKeys(action.id)),
        disabled: action.when ? !action.when(ctx) : false,
      }));
  },
  onOpenItem: async (item, openMode) => {
    quickOpenModalOpen.value = false;
    if (!item.absPath) {
      return;
    }
    await editorTabsStore.openNoteByAbsPath(item.absPath, {
      preview: openMode !== 'newTab',
    });
  },
  onExecuteAction: async (actionId) => {
    quickOpenModalOpen.value = false;
    await actionRegistry.executeById(actionId, buildActionContext());
  },
  onClose: () => {
    quickOpenModalOpen.value = false;
  },
});

function openMainQuickOpen(): void {
  quickOpenModalOpen.value = true;
  mainQuickOpenEngine.refresh(true);
  quickOpenFocusTick.value += 1;
}

const mainCommandPaletteEngine = useCommandPalette({
  getContext: () => ({
    absPath: editorTabsStore.activeTab?.absPath ?? null,
    workspaceKey: editorTabsStore.activeTab?.doc?.meta.project ?? null,
  }),
  onOpenSource: async (absPath, line) => {
    commandBookModalOpen.value = false;
    await editorTabsStore.openNoteByAbsPath(absPath, {
      preview: false,
      line,
    });
  },
  onRequestClose: () => {
    commandBookModalOpen.value = false;
  },
});

function openMainCommandBook(): void {
  commandBookModalOpen.value = true;
  void mainCommandPaletteEngine.refresh(true);
  commandBookFocusTick.value += 1;
}

function consumeAndOpenPendingFile(): boolean {
  const pending = consumePendingOpenFile();
  if (!pending) {
    return false;
  }
  lastPendingOpenAtMs = Date.now();
  const openPromise = editorTabsStore
    .openNoteByAbsPath(pending.absPath, {
      preview: !pending.newTab,
      line: pending.line ?? null,
    })
    .finally(() => {
      if (pendingOpenInFlight === openPromise) {
        pendingOpenInFlight = null;
      }
    });
  pendingOpenInFlight = openPromise;
  return true;
}

function bindMainWindowActions(): void {
  actionDisposers.push(
    actionRegistry.bindHandler('window.quickOpen', () => {
      openMainQuickOpen();
    }),
    actionRegistry.bindHandler('window.commandBook', () => {
      openMainCommandBook();
    }),
    actionRegistry.bindHandler('window.save', () => editorTabsStore.flushSave()),
    actionRegistry.bindHandler('window.newNote', () => notesStore.openNewNoteModal()),
    actionRegistry.bindHandler(
      'editor.moveSelectionToNewNote',
      () => editorTabsStore.openInboxExtractModal(),
      {
        when: (ctx) => Boolean(ctx.hasActiveDoc && ctx.isInbox && ctx.hasSelection),
      },
    ),
    actionRegistry.bindHandler(
      'editor.copyRichText',
      async () => {
        const currentDoc = editorTabsStore.activeDoc;
        if (!currentDoc) {
          return;
        }
        const body = editorTabsStore.activeTab?.draftBody ?? currentDoc.body;
        await copyMarkdownAsRichText(body);
      },
      {
        when: (ctx) => Boolean(ctx.hasActiveDoc),
      },
    ),
    actionRegistry.bindHandler(
      'editor.exportPdf',
      () => {
        if (!editorTabsStore.activeDoc) {
          return;
        }
        exportPdfModalOpen.value = true;
      },
      {
        when: (ctx) => Boolean(ctx.hasActiveDoc),
      },
    ),
    actionRegistry.bindHandler('window.closeTab', () => {
      const current = editorTabsStore.activeTab;
      if (current) {
        editorTabsStore.closeTab(current.key);
      }
    }),
    actionRegistry.bindHandler('window.reopenTab', () => {
      void editorTabsStore.reopenLastClosedTab();
    }),
    actionRegistry.bindHandler('window.toggleAuxiliary', () => {
      void toggleAuxiliaryMode();
    }),
    actionRegistry.bindHandler('window.pinQuickBar', () => {
      void windowSetQuickBarPinned(!(settingsStore.snapshot?.settings.quickBar.pinned ?? false));
    }),
    actionRegistry.bindHandler('panel.toggle.outline', () => {
      panelsStore.toggleLeftView('outline');
    }),
    actionRegistry.bindHandler('panel.toggle.terminal', () => {
      panelsStore.toggleTerminalPanel();
    }),
    actionRegistry.bindHandler('panel.toggle.related', () => {
      panelsStore.toggleLeftView('related');
    }),
    actionRegistry.bindHandler(
      'terminal.copySelection',
      async () => {
        await terminalStore.copyActiveSelection();
      },
      {
        when: (ctx) => Boolean(ctx.hasTerminalSelection),
      },
    ),
    ...EXTERNAL_ACTION_SPECS.map((spec) =>
      actionRegistry.bindHandler(
        spec.id,
        async () => {
          await externalActions.executeAction(spec.id, {
            pinned: editorTabsStore.activeDoc?.meta.pinned,
          });
        },
        {
          when: (ctx) => Boolean(ctx.hasActiveDoc && externalActions.isActionEnabled(spec.id)),
        },
      ),
    ),
  );
}

watch(
  () => {
    const doc = editorTabsStore.activeDoc;
    return [
      doc?.absPath ?? null,
      doc?.meta.project ?? null,
      doc?.meta.openDir ?? null,
      doc?.meta.terminalDir ?? null,
      doc?.meta.codeWorkspace ?? null,
      settingsStore.lastSavedAt ?? null,
    ] as const;
  },
  () => {
    void externalActions.refreshForDoc(editorTabsStore.activeDoc);
  },
  { immediate: true },
);

watch(
  () => panelsStore.isTerminalPanelShown,
  (terminalVisible) => {
    void terminalStore.syncPanelVisibility(terminalVisible);
  },
  { immediate: true },
);

let terminalResizing = false;
let terminalResizeStartY = 0;
let terminalResizeStartHeight = 240;

function onTerminalResizeMove(event: PointerEvent): void {
  if (!terminalResizing) {
    return;
  }
  const nextHeight = terminalResizeStartHeight + (terminalResizeStartY - event.clientY);
  panelsStore.setTerminalPanelHeight(nextHeight);
}

function stopTerminalResize(): void {
  if (!terminalResizing) {
    return;
  }
  terminalResizing = false;
  if (typeof window !== 'undefined') {
    window.removeEventListener('pointermove', onTerminalResizeMove);
    window.removeEventListener('pointerup', stopTerminalResize);
    window.removeEventListener('pointercancel', stopTerminalResize);
  }
  panelsStore.commitTerminalPanelHeight();
}

function startTerminalResize(event: PointerEvent): void {
  if (event.button !== 0) {
    return;
  }
  event.preventDefault();
  terminalResizing = true;
  terminalResizeStartY = event.clientY;
  terminalResizeStartHeight = panelsStore.terminalPanel.height;
  if (typeof window !== 'undefined') {
    window.addEventListener('pointermove', onTerminalResizeMove);
    window.addEventListener('pointerup', stopTerminalResize);
    window.addEventListener('pointercancel', stopTerminalResize);
  }
}

watch(
  () => externalActions.feedback,
  (feedback) => {
    if (feedback) {
      void notesStore.refreshPinnedExternalFiles();
    }
  },
);

function handleWindowKeyDown(event: KeyboardEvent): void {
  if (event.isComposing || event.keyCode === 229) {
    return;
  }
  if (onboardingStore.isLocked) {
    return;
  }

  const target = event.target instanceof Element ? event.target : document.activeElement;
  if (
    target instanceof Element &&
    !target.closest('.cm-content') &&
    target.matches('input, textarea, select')
  ) {
    const hasModifier = event.metaKey || event.ctrlKey || event.altKey;
    if (!hasModifier && event.key !== 'Escape') {
      return;
    }
  }

  actionRegistry.dispatchKeyEvent(event, buildActionContext());
}

async function syncNativeMenuAccelerators(): Promise<void> {
  try {
    await appSetMenuAccelerators({
      entries: collectNativeMenuAccelerators(actionRegistry),
    });
  } catch {
    // 非 Tauri 测试环境容错
  }
}

function handleWindowStorage(event: StorageEvent): void {
  if (event.key === ACTION_KEYBINDINGS_STORAGE_KEY || event.key === null) {
    actionRegistry.syncFromStorage();
    void syncNativeMenuAccelerators();
  }
  if (event.key === PENDING_OPEN_FILE_STORAGE_KEY || event.key === null) {
    consumeAndOpenPendingFile();
  }
}

function schedulePersistMainWindowMode(mode: WindowMode): void {
  if (windowModePersistTimer !== null) {
    clearTimeout(windowModePersistTimer);
  }
  windowModePersistTimer = setTimeout(() => {
    windowModePersistTimer = null;
    const baseMain = cachedWindowsUiState?.main ?? {
      x: 0,
      y: 0,
      width: 1280,
      height: 800,
      mode,
      isOpen: true,
    };
    const nextWindows: WindowsUiState = {
      ...(cachedWindowsUiState ?? { main: baseMain }),
      main: {
        ...baseMain,
        mode,
      },
    };
    cachedWindowsUiState = nextWindows;
    void uiStateSet({ key: 'windows', value: nextWindows }).catch(() => undefined);
  }, 200);
}

function applyMainWindowMode(mode: WindowMode): void {
  mainWindowMode.value = mode;
  panelsStore.setForceOverlayDrawer(mode === 'auxiliary');
}

async function restoreMainWindowMode(): Promise<void> {
  let resolvedMode: WindowMode = 'standard';
  try {
    const uiState = await uiStateGet();
    cachedWindowsUiState = uiState.windows;
    const uiMode = uiState.windows?.main?.mode;
    const settingsMode = settingsStore.snapshot?.settings.mainWindow.mode;
    resolvedMode = uiMode ?? settingsMode ?? 'standard';
  } catch {
    resolvedMode = settingsStore.snapshot?.settings.mainWindow.mode ?? 'standard';
  }
  applyMainWindowMode(resolvedMode);
  if (resolvedMode === 'auxiliary') {
    try {
      await windowShowMain({ mode: 'auxiliary' });
    } catch {
      // 非 Tauri 测试环境容错
    }
  }
}

async function toggleAuxiliaryMode(): Promise<void> {
  const targetMode: WindowMode = mainWindowMode.value === 'auxiliary' ? 'standard' : 'auxiliary';
  try {
    const nextState = await windowShowMain({ mode: targetMode });
    if (nextState.mode !== null) {
      applyMainWindowMode(nextState.mode);
      schedulePersistMainWindowMode(nextState.mode);
    }
  } catch {
    // 非 Tauri 测试环境容错
  }
}

async function exitAuxiliaryMode(): Promise<void> {
  if (mainWindowMode.value !== 'auxiliary') {
    return;
  }
  await toggleAuxiliaryMode();
}

async function ensureCrossWindowListeners(): Promise<void> {
  if (crossWindowListenersBound) {
    return;
  }
  crossWindowListenersBound = true;
  try {
    unlistenToastAppendUndo = await onToastAppendUndo((appendResult) => {
      void editorTabsStore.handleQuickBarAppendEvent(appendResult);
    });
    unlistenMainWindowState = await onWindowState((winState) => {
      if (winState.windowId === 'main' && winState.mode !== null) {
        applyMainWindowMode(winState.mode);
        schedulePersistMainWindowMode(winState.mode);
      }
      if (winState.windowId === 'main' && winState.visible) {
        void notesStore.refreshAll();
        const openedNow = consumeAndOpenPendingFile();
        const hasActiveOrRecentPendingOpen =
          openedNow ||
          pendingOpenInFlight !== null ||
          Date.now() - lastPendingOpenAtMs < CROSS_WINDOW_OPEN_GRACE_MS;
        if (hasActiveOrRecentPendingOpen) {
          lastPendingOpenAtMs = 0;
        }
        void panelsStore.initialize().then(() => {
          if (!hasActiveOrRecentPendingOpen) {
            void editorTabsStore.handleMainWindowShown();
          }
        });
      }
    });
    unlistenMenuAction = await onMenuAction((actionId) => {
      let mappedActionId = actionId;
      if (actionId === 'menu_quick_open') {
        mappedActionId = 'window.quickOpen';
      } else if (actionId === 'menu_command_book' || actionId === 'menu_tools_commands') {
        mappedActionId = 'window.commandBook';
      } else if (actionId === 'menu_view_terminal') {
        mappedActionId = 'panel.toggle.terminal';
      }
      const ctx = buildActionContext();
      if (
        (ctx.activeScope === 'terminal' || ctx.activeScopes?.includes('terminal')) &&
        mappedActionId.startsWith('editor.')
      ) {
        return;
      }
      void actionRegistry.executeById(mappedActionId, ctx);
    });
  } catch {
    // 非 Tauri 测试环境容错
  }
}

let mainWindowActionsBound = false;

function ensureMainWindowActionBindings(): void {
  if (mainWindowActionsBound) {
    return;
  }
  mainWindowActionsBound = true;
  bindMainWindowActions();
  if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
    window.addEventListener('keydown', handleWindowKeyDown);
    window.addEventListener('storage', handleWindowStorage);
  }
  void syncNativeMenuAccelerators();
}

ensureMainWindowActionBindings();

const saveStatusLabel = computed<string>(() => {
  const tab = editorTabsStore.activeTab;
  if (!tab) {
    return '就绪';
  }
  switch (tab.saveStatus) {
    case 'saved':
      return '已保存';
    case 'dirty':
      return '未保存';
    case 'saving':
      return '保存中…';
    case 'conflict':
      return '存在冲突';
    case 'deleted':
      return '磁盘文件已丢失';
    case 'external-synced':
      return '已同步外部修改';
    case 'error':
      return '保存失败';
    default:
      return '就绪';
  }
});

const charCount = computed<number>(() => editorTabsStore.activeTab?.draftBody.length ?? 0);

watch(
  () => onboardingStore.libraryInfo?.root ?? null,
  (nextRoot) => {
    if (nextRoot) {
      void Promise.all([
        notesStore.initialize(),
        editorTabsStore.restoreTabsFromUiState(),
        panelsStore.initialize(),
        restoreMainWindowMode(),
        ensureCrossWindowListeners(),
      ]).then(() => {
        consumeAndOpenPendingFile();
      });
    }
  },
  { immediate: true },
);

function syncViewportWidth(): void {
  const rectWidth = shellRef.value?.getBoundingClientRect().width ?? 0;
  const width = rectWidth > 0 ? rectWidth : typeof window !== 'undefined' ? window.innerWidth : 0;
  if (width > 0) {
    panelsStore.applyViewportWidth(width);
  }
}

if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
  window.addEventListener('resize', syncViewportWidth);
  syncViewportWidth();
}

watch(shellRef, (el) => {
  if (viewportResizeObserver) {
    viewportResizeObserver.disconnect();
    viewportResizeObserver = null;
  }
  if (!el) {
    return;
  }
  syncViewportWidth();
  if (typeof ResizeObserver !== 'undefined') {
    viewportResizeObserver = new ResizeObserver((entries) => {
      const entryWidth = entries[0]?.contentRect.width;
      if (entryWidth && entryWidth > 0) {
        panelsStore.applyViewportWidth(entryWidth);
      }
    });
    viewportResizeObserver.observe(el);
  }
});

onUnmounted(() => {
  stopTerminalResize();
  if (viewportResizeObserver) {
    viewportResizeObserver.disconnect();
    viewportResizeObserver = null;
  }
  if (typeof window !== 'undefined' && typeof window.removeEventListener === 'function') {
    window.removeEventListener('resize', syncViewportWidth);
    window.removeEventListener('keydown', handleWindowKeyDown);
    window.removeEventListener('storage', handleWindowStorage);
  }
  while (actionDisposers.length > 0) {
    const dispose = actionDisposers.pop();
    dispose?.();
  }
  mainWindowActionsBound = false;
  if (windowModePersistTimer !== null) {
    clearTimeout(windowModePersistTimer);
    windowModePersistTimer = null;
  }
  if (unlistenToastAppendUndo) {
    unlistenToastAppendUndo();
    unlistenToastAppendUndo = null;
  }
  if (unlistenMainWindowState) {
    unlistenMainWindowState();
    unlistenMainWindowState = null;
  }
  if (unlistenMenuAction) {
    unlistenMenuAction();
    unlistenMenuAction = null;
  }
  crossWindowListenersBound = false;
  pendingOpenInFlight = null;
  lastPendingOpenAtMs = 0;
  mainQuickOpenEngine.dispose();
  mainCommandPaletteEngine.dispose();
  notesStore.dispose();
  editorTabsStore.dispose();
  panelsStore.dispose();
});
</script>

<template>
  <div ref="shellRef" class="qn-main-shell" data-testid="main-workbench-shell">
    <div v-if="settingsStore.isSettingsInvalid" class="qn-main-shell__top-banner">
      <SettingsErrorBanner :status="settingsStore.loadStatus" />
    </div>

    <div
      v-if="onboardingStore.isLocked"
      class="qn-main-shell__locked"
      data-testid="main-onboarding-lock"
    >
      <OnboardingView />
    </div>

    <div
      v-else
      :class="[
        'qn-main-shell__workbench',
        { 'qn-main-shell__workbench--auxiliary': mainWindowMode === 'auxiliary' },
      ]"
      data-testid="main-workbench-unlocked"
    >
      <div class="qn-main-shell__body">
        <div
          v-if="panelsStore.isOverlayDrawer && !panelsStore.leftSidebarCollapsed"
          class="qn-main-shell__left-backdrop"
          data-testid="left-sidebar-drawer-backdrop"
          @click="panelsStore.toggleLeftSidebar()"
        />
        <NotesSidebar
          v-if="mainWindowMode !== 'auxiliary' || !panelsStore.leftSidebarCollapsed"
          :collapsed="panelsStore.leftSidebarCollapsed"
          :overlay-drawer="panelsStore.isOverlayDrawer"
          @toggle-collapse="panelsStore.toggleLeftSidebar()"
        />

        <section class="qn-main-shell__editor-col">
          <EditorTabBar />
          <InboxBloatBanner />
          <EditorConflictBanner />

          <!-- 45% 侧栏辅助模式专用的两个悬浮按钮（标准模式下不渲染） -->
          <div
            v-if="mainWindowMode === 'auxiliary'"
            class="qn-main-shell__aux-controls"
            data-testid="aux-controls"
          >
            <QnButton
              variant="ghost"
              size="sm"
              data-testid="auxiliary-nav-drawer-btn"
              @click="panelsStore.toggleLeftSidebar()"
            >
              {{ panelsStore.leftSidebarCollapsed ? '拉出导航' : '收起导航' }}
            </QnButton>
            <QnButton
              variant="secondary"
              size="sm"
              data-testid="exit-auxiliary-mode-btn"
              @click="exitAuxiliaryMode"
            >
              退出侧栏辅助
            </QnButton>
          </div>

          <div class="qn-main-shell__workspace">
            <div class="qn-main-shell__editor-stage">
              <div
                v-if="editorTabsStore.activeTab?.isMissing"
                class="qn-main-shell__editor-paper"
                data-testid="editor-missing-doc"
              >
                <QnBanner tone="danger" title="文件已移走或失联">
                  未在当前笔记库或磁盘路径中找到：{{ editorTabsStore.activeTab.relPath }}
                  <template #actions>
                    <QnButton
                      size="sm"
                      variant="secondary"
                      @click="editorTabsStore.closeTab(editorTabsStore.activeTab.key)"
                    >
                      关闭标签
                    </QnButton>
                  </template>
                </QnBanner>
              </div>

              <div
                v-else-if="editorTabsStore.activeDoc"
                class="qn-main-shell__editor-paper"
                data-testid="editor-doc-stage"
              >
                <DocHeaderBar />

                <div class="qn-main-shell__body-stage" data-testid="editor-body-stage">
                  <EditorMergeView
                    v-if="
                      editorTabsStore.activeTab?.conflictState?.kind === 'modified' &&
                      editorTabsStore.activeTab.conflictState.mergeMode
                    "
                  />
                  <MarkdownEditor v-else />
                </div>
              </div>

              <div v-else class="qn-main-shell__editor-paper">
                <p class="qn-main-shell__placeholder-text">从左侧选择或新建一篇笔记。</p>
              </div>
            </div>

            <!-- 底部终端：有存活会话时折叠也保持挂载（D-0005 折叠保活） -->
            <aside
              v-show="panelsStore.terminalPanel.visible"
              class="qn-terminal-dock"
              :class="{ 'qn-terminal-dock--collapsed': !panelsStore.isTerminalPanelShown }"
              data-testid="terminal-dock"
            >
              <div class="qn-terminal-dock__header">
                <button
                  type="button"
                  class="qn-terminal-dock__toggle"
                  :title="panelsStore.isTerminalPanelShown ? '折叠终端' : '展开终端'"
                  data-testid="terminal-dock-toggle"
                  @click="panelsStore.toggleTerminalCollapse()"
                >
                  {{ panelsStore.isTerminalPanelShown ? '▾' : '▴' }} 终端
                </button>
                <button
                  type="button"
                  class="qn-terminal-dock__close"
                  title="隐藏终端（会话保留）"
                  data-testid="terminal-dock-close"
                  @click="panelsStore.toggleTerminalPanel()"
                >
                  ×
                </button>
              </div>
              <div
                v-show="panelsStore.isTerminalPanelShown"
                class="qn-terminal-dock__body"
                :style="{ height: `${panelsStore.terminalPanel.height}px` }"
              >
                <div
                  class="qn-terminal-dock__resizer"
                  role="separator"
                  aria-orientation="horizontal"
                  aria-label="调整终端高度"
                  data-testid="terminal-dock-resizer"
                  @pointerdown="startTerminalResize"
                />
                <TerminalPanel
                  v-if="panelsStore.terminalPanel.visible || terminalStore.hasAnySession"
                />
              </div>
            </aside>
          </div>
        </section>
      </div>

      <footer class="qn-main-shell__statusbar" data-testid="workbench-statusbar">
        <div class="qn-main-shell__status-left">
          <span data-testid="statusbar-save-status">{{ saveStatusLabel }}</span>
          <span v-if="editorTabsStore.activeTab" data-testid="statusbar-char-count">
            {{ charCount }} 字
          </span>
          <span v-if="editorTabsStore.activeTab" data-testid="statusbar-cursor">
            Ln {{ editorTabsStore.activeTab.cursorLine }}, Col
            {{ editorTabsStore.activeTab.cursorCol }}
          </span>
          <span
            v-if="externalActions.feedback"
            :class="[
              'qn-main-shell__status-feedback',
              `qn-main-shell__status-feedback--${externalActions.feedback.tone}`,
            ]"
            data-testid="external-action-feedback"
          >
            {{ externalActions.feedback.message }}
          </span>
        </div>
        <div class="qn-main-shell__status-right">
          <span v-if="editorTabsStore.editorStatus?.docKindLabel" data-testid="statusbar-doc-kind">
            {{ editorTabsStore.editorStatus.docKindLabel }}
          </span>
          <span v-if="editorTabsStore.editorStatus?.indentLabel" data-testid="statusbar-indent">
            {{ editorTabsStore.editorStatus.indentLabel }}
          </span>
          <span
            v-if="editorTabsStore.editorStatus?.diagnosticsLabel"
            class="qn-main-shell__status-diagnostics"
            data-testid="statusbar-diagnostics"
            :title="editorTabsStore.editorStatus.diagnosticsLabel"
          >
            {{ editorTabsStore.editorStatus.diagnosticsLabel }}
          </span>
        </div>
      </footer>
    </div>

    <QnModalShell
      :open="quickOpenModalOpen"
      title="快速打开"
      max-width="lg"
      @close="quickOpenModalOpen = false"
    >
      <QuickOpenPalette
        :engine="mainQuickOpenEngine"
        variant="large"
        :focus-trigger="quickOpenFocusTick"
      />
    </QnModalShell>
    <QnModalShell
      :open="commandBookModalOpen"
      title="命令库"
      max-width="lg"
      @close="commandBookModalOpen = false"
    >
      <CommandBookPalette
        :engine="mainCommandPaletteEngine"
        variant="large"
        :focus-trigger="commandBookFocusTick"
        @close="commandBookModalOpen = false"
      />
    </QnModalShell>
    <NewNoteTemplateModal />
    <InboxExtractModal />
    <ExportPdfModal
      :open="exportPdfModalOpen"
      :doc="editorTabsStore.activeDoc"
      :body="editorTabsStore.activeTab?.draftBody ?? editorTabsStore.activeDoc?.body ?? ''"
      @close="exportPdfModalOpen = false"
    />
  </div>
</template>

<style scoped>
.qn-main-shell {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--qn-bg-base);
}

.qn-main-shell__top-banner {
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-main-shell__locked {
  flex: 1;
  min-height: 0;
}

.qn-main-shell__workbench {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.qn-main-shell__body {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
}

.qn-main-shell__left-backdrop {
  position: absolute;
  inset: 0;
  background-color: var(--qn-bg-backdrop);
  z-index: var(--qn-z-drawer);
}

.qn-main-shell__editor-col {
  position: relative;
  flex: 1;
  min-width: min(480px, 100%);
  display: flex;
  flex-direction: column;
}

.qn-main-shell__aux-controls {
  position: absolute;
  top: var(--qn-space-2);
  right: var(--qn-space-3);
  z-index: var(--qn-z-drawer);
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-main-shell__editor-banner {
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-base);
}

.qn-main-shell__workspace {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.qn-main-shell__editor-stage {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: var(--qn-space-6);
  display: flex;
  justify-content: center;
}

.qn-terminal-dock {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-terminal-dock__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 28px;
  padding: 0 var(--qn-space-3);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-terminal-dock--collapsed .qn-terminal-dock__header {
  border-bottom: none;
}

.qn-terminal-dock__toggle,
.qn-terminal-dock__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  border-radius: var(--qn-radius-sm);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
  padding: 2px var(--qn-space-2);
}

.qn-terminal-dock__toggle:hover,
.qn-terminal-dock__close:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-terminal-dock__body {
  position: relative;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.qn-terminal-dock__resizer {
  position: absolute;
  top: -3px;
  left: 0;
  right: 0;
  height: 6px;
  cursor: row-resize;
  z-index: var(--qn-z-base);
}

.qn-terminal-dock__resizer:hover {
  background-color: var(--qn-accent-soft);
}

.qn-main-shell__editor-paper {
  width: 100%;
  max-width: var(--qn-editor-max-width);
}

.qn-main-shell__doc-header {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
  margin-bottom: var(--qn-space-4);
  padding-bottom: var(--qn-space-3);
  border-bottom: 1px solid var(--qn-border-subtle);
}

.qn-main-shell__doc-title {
  margin: 0;
  font-size: var(--qn-text-lg);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-main-shell__doc-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-main-shell__body-stage {
  min-height: 240px;
}

.qn-main-shell__body-preview {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--qn-font-sans);
  font-size: var(--qn-text-base);
  line-height: 1.7;
  color: var(--qn-text-primary);
}

.qn-main-shell__placeholder-text {
  margin: 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
}

.qn-main-shell__statusbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  height: 26px;
  padding: 0 var(--qn-space-4);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
  overflow: hidden;
}

.qn-main-shell__status-left {
  display: flex;
  align-items: center;
  gap: var(--qn-space-3);
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
}

.qn-main-shell__status-right {
  display: flex;
  align-items: center;
  gap: var(--qn-space-3);
  min-width: 0;
  flex-shrink: 1;
  white-space: nowrap;
  overflow: hidden;
}

.qn-main-shell__status-right > span {
  flex-shrink: 0;
  white-space: nowrap;
}

.qn-main-shell__status-right > .qn-main-shell__status-diagnostics {
  flex-shrink: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.qn-main-shell__status-feedback--success {
  color: var(--qn-success);
}

.qn-main-shell__status-feedback--warning {
  color: var(--qn-warning);
}

.qn-main-shell__status-feedback--danger {
  color: var(--qn-danger);
}
</style>
