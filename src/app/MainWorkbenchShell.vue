<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { QnBadge, QnBanner, QnButton, QnModalShell } from '@/components';
import {
  EditorConflictBanner,
  EditorTabBar,
  InboxBloatBanner,
  MarkdownEditor,
  useEditorTabsStore,
} from '@/features/editor';
import { NotesSidebar, useNotesStore } from '@/features/notes';
import { OnboardingView, useOnboardingStore } from '@/features/onboarding';
import { OutlinePanel, usePanelsStore } from '@/features/panels';
import { SettingsErrorBanner, VisualBaselinePreview, useSettingsStore } from '@/features/settings';
import {
  ACTION_KEYBINDINGS_STORAGE_KEY,
  actionRegistry,
  type ActionContext,
  type ActionScope,
} from '@/lib/actions';
import { onMenuAction, onToastAppendUndo, onWindowState, type UnlistenFn } from '@/lib/ipc/events';

const onboardingStore = useOnboardingStore();
const settingsStore = useSettingsStore();
const notesStore = useNotesStore();
const editorTabsStore = useEditorTabsStore();
const panelsStore = usePanelsStore();

const baselinePreviewOpen = ref(false);
let unlistenToastAppendUndo: UnlistenFn | null = null;
let unlistenMainWindowState: UnlistenFn | null = null;
let unlistenMenuAction: UnlistenFn | null = null;
let crossWindowListenersBound = false;
const actionDisposers: Array<() => void> = [];

function resolveActiveScopes(): ActionScope[] {
  const scopes: ActionScope[] = ['main-window', 'global'];
  if (typeof document === 'undefined') {
    return scopes;
  }

  if (baselinePreviewOpen.value || document.querySelector('[role="dialog"]')) {
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
  const activeDoc = editorTabsStore.activeDoc;
  const relPath = activeDoc?.relPath ?? '';
  return {
    activeScope: activeScopes[0] ?? 'main-window',
    activeScopes,
    hasActiveDoc: Boolean(activeDoc),
    isMarkdown: relPath.toLowerCase().endsWith('.md'),
    isInbox: relPath === 'inbox.md',
  };
}

function bindMainWindowActions(): void {
  actionDisposers.push(
    actionRegistry.bindHandler('window.save', () => editorTabsStore.flushSave()),
    actionRegistry.bindHandler('window.closeTab', () => {
      const current = editorTabsStore.activeTab;
      if (current) {
        editorTabsStore.closeTab(current.key);
      }
    }),
    actionRegistry.bindHandler('window.reopenTab', () => {
      void editorTabsStore.reopenLastClosedTab();
    }),
    actionRegistry.bindHandler('panel.toggle.outline', () => {
      panelsStore.toggleOutlinePanel();
    }),
    actionRegistry.bindHandler('panel.collapseGroup', () => {
      panelsStore.toggleCollapseAll();
    }),
  );
}

function handleWindowKeyDown(event: KeyboardEvent): void {
  if (onboardingStore.isLocked) {
    return;
  }
  actionRegistry.dispatchKeyEvent(event, buildActionContext());
}

function handleWindowStorage(event: StorageEvent): void {
  if (event.key === ACTION_KEYBINDINGS_STORAGE_KEY || event.key === null) {
    actionRegistry.syncFromStorage();
  }
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
      if (winState.windowId === 'main' && winState.visible) {
        void panelsStore.initialize().then(() => editorTabsStore.handleMainWindowShown());
      }
    });
    unlistenMenuAction = await onMenuAction((actionId) => {
      void actionRegistry.executeById(actionId, buildActionContext());
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
}

if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
  ensureMainWindowActionBindings();
}

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
        ensureCrossWindowListeners(),
      ]);
    }
  },
  { immediate: true },
);

onUnmounted(() => {
  if (typeof window !== 'undefined' && typeof window.removeEventListener === 'function') {
    window.removeEventListener('keydown', handleWindowKeyDown);
    window.removeEventListener('storage', handleWindowStorage);
  }
  while (actionDisposers.length > 0) {
    const dispose = actionDisposers.pop();
    dispose?.();
  }
  mainWindowActionsBound = false;
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
  notesStore.dispose();
  editorTabsStore.dispose();
  panelsStore.dispose();
});
</script>

<template>
  <div class="qn-main-shell" data-testid="main-workbench-shell">
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

    <div v-else class="qn-main-shell__workbench" data-testid="main-workbench-unlocked">
      <header class="qn-main-shell__titlebar">
        <div class="qn-main-shell__brand">
          <span class="qn-main-shell__title">QuickNote</span>
          <QnBadge tone="neutral">{{ onboardingStore.libraryInfo?.root }}</QnBadge>
        </div>
        <div class="qn-main-shell__titlebar-actions">
          <QnButton variant="ghost" size="sm" @click="baselinePreviewOpen = true">
            视觉基线预览
          </QnButton>
          <QnBadge tone="accent">⌃⌥Space 捕捉</QnBadge>
        </div>
      </header>

      <div class="qn-main-shell__body">
        <NotesSidebar />

        <section class="qn-main-shell__editor-col">
          <EditorTabBar />
          <InboxBloatBanner />
          <EditorConflictBanner />

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
                <header class="qn-main-shell__doc-header">
                  <h1 class="qn-main-shell__doc-title" data-testid="editor-doc-title">
                    {{ editorTabsStore.activeDoc.meta.title || editorTabsStore.activeDoc.relPath }}
                  </h1>
                  <div class="qn-main-shell__doc-meta">
                    <QnBadge v-if="editorTabsStore.activeDoc.meta.docType" tone="neutral">
                      {{ editorTabsStore.activeDoc.meta.docType }}
                    </QnBadge>
                    <QnBadge
                      v-for="tag in editorTabsStore.activeDoc.meta.tags"
                      :key="tag"
                      tone="accent"
                    >
                      #{{ tag }}
                    </QnBadge>
                    <QnBadge
                      v-if="editorTabsStore.activeDoc.meta.metaError"
                      tone="warning"
                      data-testid="editor-doc-meta-error"
                    >
                      Frontmatter 异常：{{ editorTabsStore.activeDoc.meta.metaError }}
                    </QnBadge>
                  </div>
                </header>

                <div class="qn-main-shell__body-stage" data-testid="editor-body-stage">
                  <MarkdownEditor />
                </div>
              </div>

              <div v-else class="qn-main-shell__editor-paper">
                <p class="qn-main-shell__placeholder-text">从左侧选择或新建一篇笔记。</p>
              </div>
            </div>
            <OutlinePanel />
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
        </div>
        <span>热键：{{ settingsStore.hotkeyStatus?.combo ?? 'ctrl+alt+space' }}</span>
      </footer>
    </div>

    <QnModalShell
      :open="baselinePreviewOpen"
      title="视觉基线预览"
      max-width="lg"
      @close="baselinePreviewOpen = false"
    >
      <div class="qn-main-shell__baseline-scroll">
        <VisualBaselinePreview />
      </div>
      <template #footer>
        <QnButton variant="primary" size="sm" @click="baselinePreviewOpen = false">关闭</QnButton>
      </template>
    </QnModalShell>
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

.qn-main-shell__titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  height: 44px;
  padding: 0 var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-main-shell__brand {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  min-width: 0;
}

.qn-main-shell__title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-main-shell__titlebar-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-main-shell__body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.qn-main-shell__editor-col {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.qn-main-shell__editor-banner {
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-base);
}

.qn-main-shell__workspace {
  flex: 1;
  min-height: 0;
  display: flex;
}

.qn-main-shell__editor-stage {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: var(--qn-space-6);
  display: flex;
  justify-content: center;
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

.qn-main-shell__right-panel {
  width: 260px;
  flex-shrink: 0;
  border-left: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  padding: var(--qn-space-3);
}

.qn-main-shell__panel-header {
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-secondary);
  margin-bottom: var(--qn-space-2);
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
  height: 26px;
  padding: 0 var(--qn-space-4);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-main-shell__status-left {
  display: flex;
  align-items: center;
  gap: var(--qn-space-3);
}

.qn-main-shell__baseline-scroll {
  max-height: 70vh;
  overflow-y: auto;
}
</style>
