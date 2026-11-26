<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type { ExternalActionState } from '@/bindings/types';
import { QnBadge, QnContextMenu, type ContextMenuEntry } from '@/components';
import { useNotesStore } from '@/features/notes';
import { useSettingsStore } from '@/features/settings';
import { actionRegistry } from '@/lib/actions';
import { type EditorTab, useEditorTabsStore } from './store';
import {
  buildExternalContextMenuItems,
  isExternalActionId,
  useExternalActions,
} from './useExternalActions';

const tabsStore = useEditorTabsStore();
const notesStore = useNotesStore();
const settingsStore = useSettingsStore();
const externalActions = useExternalActions();

const SCROLL_INDICATOR_HIDE_MS = 600;

const tabListRef = ref<HTMLElement | null>(null);
const isScrolling = ref(false);
const draggingKey = ref<string | null>(null);
const dropTarget = ref<{ key: string; position: 'before' | 'after' } | null>(null);
const contextMenuOpen = ref(false);
const contextMenuPos = ref({ x: 0, y: 0 });
const contextTab = ref<EditorTab | null>(null);
const contextTabStates = ref<ExternalActionState[]>([]);
let tabMenuSeq = 0;
let scrollHideTimer: ReturnType<typeof setTimeout> | null = null;
const actionDisposers: Array<() => void> = [];

function nextIndex(tabs: readonly EditorTab[], currentIndex: number, step: 1 | -1): number {
  if (tabs.length === 0 || currentIndex < 0 || currentIndex >= tabs.length) {
    return 0;
  }
  return (currentIndex + step + tabs.length) % tabs.length;
}

function markScrolling(): void {
  isScrolling.value = true;
  if (scrollHideTimer !== null) {
    clearTimeout(scrollHideTimer);
  }
  scrollHideTimer = setTimeout(() => {
    scrollHideTimer = null;
    isScrolling.value = false;
  }, SCROLL_INDICATOR_HIDE_MS);
}

function handleTabListScroll(): void {
  markScrolling();
}

function handleTabListWheel(event: WheelEvent): void {
  const el = tabListRef.value;
  if (!el) {
    return;
  }
  const maxScrollLeft = el.scrollWidth - el.clientWidth;
  if (maxScrollLeft <= 0) {
    return;
  }
  const delta = Math.abs(event.deltaY) >= Math.abs(event.deltaX) ? event.deltaY : event.deltaX;
  if (delta === 0) {
    return;
  }
  if (Math.abs(event.deltaY) > Math.abs(event.deltaX)) {
    event.preventDefault();
    el.scrollLeft = Math.max(0, Math.min(maxScrollLeft, el.scrollLeft + event.deltaY));
  }
  markScrolling();
}

watch(
  () => tabsStore.activeKey,
  async (activeKey) => {
    if (!activeKey) {
      return;
    }
    await nextTick();
    const listEl = tabListRef.value;
    if (!listEl || typeof listEl.querySelector !== 'function') {
      return;
    }
    const activeEl = listEl.querySelector('.qn-tabbar__item--active');
    if (activeEl && typeof (activeEl as HTMLElement).scrollIntoView === 'function') {
      (activeEl as HTMLElement).scrollIntoView({
        behavior: 'smooth',
        block: 'nearest',
        inline: 'nearest',
      });
    }
  },
);

function canDeleteFile(tab: EditorTab | null): boolean {
  return Boolean(tab?.relPath && tab.relPath !== 'inbox.md');
}

const contextMenuItems = computed<ContextMenuEntry[]>(() => [
  ...buildExternalContextMenuItems(contextTabStates.value),
  { type: 'separator', id: 'tab-sep' },
  { type: 'item', id: 'tab.close', title: '关闭标签' },
  {
    type: 'item',
    id: 'tab.closeOthers',
    title: '关闭其他标签',
    disabled: tabsStore.tabs.length <= 1,
  },
  {
    type: 'submenu',
    id: 'tab.delete',
    title: '删除',
    disabled: !canDeleteFile(contextTab.value),
    items: [
      {
        type: 'item',
        id: 'tab.deleteFile',
        title: '删除文件（移到废纸篓）',
        danger: true,
      },
    ],
  },
]);

async function openTabContextMenu(event: MouseEvent, tab: EditorTab): Promise<void> {
  const reqSeq = ++tabMenuSeq;
  contextTab.value = tab;
  contextMenuPos.value = { x: event.clientX, y: event.clientY };

  if (
    tab.absPath &&
    tab.absPath === externalActions.resolvedAbsPath &&
    externalActions.states.length > 0
  ) {
    contextTabStates.value = externalActions.states;
    contextMenuOpen.value = true;
    return;
  }

  contextTabStates.value = [];
  contextMenuOpen.value = true;
  const resolved = await externalActions.resolveForTarget({
    absPath: tab.absPath || null,
    workspaceKey: tab.doc?.meta.project ?? null,
  });
  if (reqSeq === tabMenuSeq) {
    contextTabStates.value = resolved;
  }
}

async function handleContextMenuSelect(id: string): Promise<void> {
  const tab = contextTab.value;
  if (!tab) {
    return;
  }
  if (isExternalActionId(id)) {
    await externalActions.executeAction(id, {
      overrideStates: contextTabStates.value,
      pinned: tab.doc?.meta.pinned,
    });
    return;
  }
  if (id === 'tab.close') {
    tabsStore.closeTab(tab.key);
  } else if (id === 'tab.closeOthers') {
    tabsStore.closeOtherTabs(tab.key);
  } else if (id === 'tab.deleteFile' && tab.relPath && tab.relPath !== 'inbox.md') {
    await notesStore.deleteNoteItem(tab.relPath);
  }
}

function handleTabDragStart(event: DragEvent, tab: EditorTab): void {
  draggingKey.value = tab.key;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', tab.key);
  }
}

function handleTabDragOver(event: DragEvent, tab: EditorTab): void {
  if (!draggingKey.value || draggingKey.value === tab.key) {
    dropTarget.value = null;
    return;
  }
  const el = event.currentTarget as HTMLElement | null;
  if (!el) {
    return;
  }
  const rect = el.getBoundingClientRect();
  const position: 'before' | 'after' =
    event.clientX < rect.left + rect.width / 2 ? 'before' : 'after';
  dropTarget.value = { key: tab.key, position };
  if (event.dataTransfer) {
    event.dataTransfer.dropEffect = 'move';
  }
}

function handleTabDrop(tab: EditorTab): void {
  if (draggingKey.value && dropTarget.value && dropTarget.value.key === tab.key) {
    tabsStore.reorderTab(draggingKey.value, tab.key, dropTarget.value.position);
  }
  draggingKey.value = null;
  dropTarget.value = null;
}

function handleTabDragEnd(): void {
  draggingKey.value = null;
  dropTarget.value = null;
}

function getTabTitle(tab: EditorTab): string {
  if (tab.doc?.meta.title) {
    return tab.doc.meta.title;
  }
  const path = tab.relPath ?? tab.absPath;
  const parts = path.split('/');
  return parts[parts.length - 1] ?? path;
}

function isExternalTab(tab: EditorTab): boolean {
  if (tab.doc) {
    return !tab.doc.isLibrary;
  }
  return tab.relPath === null;
}

async function handleTogglePinExternalTab(tab: EditorTab): Promise<void> {
  const states =
    tab.absPath &&
    tab.absPath === externalActions.resolvedAbsPath &&
    externalActions.states.length > 0
      ? externalActions.states
      : await externalActions.resolveForTarget({
          absPath: tab.absPath || null,
          workspaceKey: tab.doc?.meta.project ?? null,
        });
  await externalActions.executeAction('external.pin', {
    overrideStates: states,
    pinned: tab.doc?.meta.pinned,
  });
}

function handleKeydown(event: KeyboardEvent): void {
  if (!event.metaKey || event.ctrlKey || event.altKey) {
    return;
  }

  if (event.shiftKey && event.key.toLowerCase() === 't') {
    event.preventDefault();
    void tabsStore.reopenLastClosedTab();
    return;
  }

  if (!event.shiftKey && event.key >= '1' && event.key <= '9') {
    const index = Number(event.key) - 1;
    if (index < tabsStore.tabs.length) {
      event.preventDefault();
      void tabsStore.activateTabByIndex(index);
    }
  }
}

onMounted(() => {
  actionDisposers.push(
    actionRegistry.bindHandler('window.nextTab', () => {
      const currentIndex = tabsStore.tabs.findIndex((tab) => tab.key === tabsStore.activeKey);
      void tabsStore.activateTabByIndex(nextIndex(tabsStore.tabs, currentIndex, 1));
    }),
    actionRegistry.bindHandler('window.prevTab', () => {
      const currentIndex = tabsStore.tabs.findIndex((tab) => tab.key === tabsStore.activeKey);
      void tabsStore.activateTabByIndex(nextIndex(tabsStore.tabs, currentIndex, -1));
    }),
  );
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleKeydown);
  }
});

onUnmounted(() => {
  while (actionDisposers.length > 0) {
    actionDisposers.pop()?.();
  }
  if (scrollHideTimer !== null) {
    clearTimeout(scrollHideTimer);
    scrollHideTimer = null;
  }
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', handleKeydown);
  }
});
</script>

<template>
  <div class="qn-tabbar" role="tablist" aria-label="已打开的笔记标签" data-testid="editor-tabbar">
    <div
      ref="tabListRef"
      :class="['qn-tabbar__list', { 'qn-tabbar__list--scrolling': isScrolling }]"
      data-testid="editor-tabbar-list"
      @scroll="handleTabListScroll"
      @wheel="handleTabListWheel"
    >
      <div
        v-for="tab in tabsStore.tabs"
        :key="tab.key"
        role="tab"
        draggable="true"
        :aria-selected="tab.key === tabsStore.activeKey"
        :data-testid="`editor-tab-${tab.relPath ?? tab.absPath}`"
        :class="[
          'qn-tabbar__item',
          {
            'qn-tabbar__item--active': tab.key === tabsStore.activeKey,
            'qn-tabbar__item--preview': tab.isPreview,
            'qn-tabbar__item--missing': tab.isMissing || tab.conflictState?.kind === 'deleted',
            'qn-tabbar__item--drop-before':
              dropTarget?.key === tab.key && dropTarget.position === 'before',
            'qn-tabbar__item--drop-after':
              dropTarget?.key === tab.key && dropTarget.position === 'after',
          },
        ]"
        :title="tab.absPath || tab.relPath || undefined"
        @click="tabsStore.activateTab(tab.key)"
        @dblclick="tabsStore.promoteTabToPinned(tab.key)"
        @mousedown.middle.prevent="tabsStore.closeTab(tab.key)"
        @contextmenu.prevent="openTabContextMenu($event, tab)"
        @dragstart="handleTabDragStart($event, tab)"
        @dragover.prevent="handleTabDragOver($event, tab)"
        @drop.prevent="handleTabDrop(tab)"
        @dragend="handleTabDragEnd"
      >
        <span class="qn-tabbar__title">{{ getTabTitle(tab) }}</span>
        <QnBadge
          v-if="isExternalTab(tab)"
          tone="accent"
          size="sm"
          data-testid="editor-tab-external-badge"
        >
          外部
        </QnBadge>
        <button
          v-if="isExternalTab(tab)"
          type="button"
          class="qn-tabbar__pin-btn"
          :title="tab.doc?.meta.pinned ? '取消固定常用文件' : '固定到常用文件'"
          :aria-label="tab.doc?.meta.pinned ? '取消固定常用文件' : '固定到常用文件'"
          :data-testid="`editor-tab-pin-${tab.absPath}`"
          @click.stop="handleTogglePinExternalTab(tab)"
        >
          📌
        </button>
        <QnBadge v-if="tab.isMissing" tone="danger">失联</QnBadge>
        <QnBadge v-else-if="tab.conflictState?.kind === 'deleted'" tone="danger">已删除</QnBadge>
        <QnBadge v-else-if="tab.conflictState?.kind === 'modified'" tone="warning">冲突</QnBadge>
        <span v-else-if="tab.isDirty" class="qn-tabbar__dirty" aria-label="未保存" />
        <button
          v-if="settingsStore.showTabCloseButton"
          type="button"
          class="qn-tabbar__close"
          :aria-label="`关闭 ${getTabTitle(tab)}`"
          :data-testid="`editor-tab-close-${tab.relPath ?? tab.absPath}`"
          @click.stop="tabsStore.closeTab(tab.key)"
        >
          ×
        </button>
      </div>
    </div>

    <QnContextMenu
      :open="contextMenuOpen"
      :x="contextMenuPos.x"
      :y="contextMenuPos.y"
      :items="contextMenuItems"
      @select="handleContextMenuSelect"
      @close="contextMenuOpen = false"
    />
  </div>
</template>

<style scoped>
.qn-tabbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  height: 36px;
  padding: 0 var(--qn-space-2);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-tabbar__list {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  flex: 1;
  min-width: 0;
  padding: 4px 0;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
}

.qn-tabbar__list::-webkit-scrollbar {
  height: 2px;
}

.qn-tabbar__list::-webkit-scrollbar-track {
  background: transparent;
}

.qn-tabbar__list::-webkit-scrollbar-thumb {
  background-color: transparent;
  border-radius: var(--qn-radius-full);
  transition: background-color 0.15s ease;
}

.qn-tabbar__list--scrolling::-webkit-scrollbar-thumb,
.qn-tabbar__list:hover::-webkit-scrollbar-thumb {
  background-color: var(--qn-border-strong);
}

.qn-tabbar__item {
  display: inline-flex;
  align-items: center;
  gap: var(--qn-space-1);
  max-width: 200px;
  height: 28px;
  padding: 0 var(--qn-space-2);
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  cursor: pointer;
  user-select: none;
  flex-shrink: 0;
  transition:
    background-color 0.12s ease,
    color 0.12s ease,
    border-color 0.12s ease;
}

.qn-tabbar__item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-tabbar__item--active {
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  border-color: var(--qn-border-subtle);
}

.qn-tabbar__item--preview .qn-tabbar__title {
  font-style: italic;
}

.qn-tabbar__item--missing {
  color: var(--qn-danger);
}

.qn-tabbar__item--drop-before {
  box-shadow: inset 2px 0 0 var(--qn-accent);
}

.qn-tabbar__item--drop-after {
  box-shadow: inset -2px 0 0 var(--qn-accent);
}

.qn-tabbar__title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-tabbar__dirty {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: var(--qn-accent);
  flex-shrink: 0;
}

.qn-tabbar__pin-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 2px;
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  line-height: 1;
  cursor: pointer;
}

.qn-tabbar__pin-btn:hover {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-tabbar__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-sm);
  line-height: 1;
  cursor: pointer;
}

.qn-tabbar__close:hover {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}
</style>
