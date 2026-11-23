<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { QnBadge } from '@/components';
import { type EditorTab, useEditorTabsStore } from './store';

const tabsStore = useEditorTabsStore();

function getTabTitle(tab: EditorTab): string {
  if (tab.doc?.meta.title) {
    return tab.doc.meta.title;
  }
  const path = tab.relPath ?? tab.absPath;
  const parts = path.split('/');
  return parts[parts.length - 1] ?? path;
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
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleKeydown);
  }
});

onUnmounted(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', handleKeydown);
  }
});
</script>

<template>
  <div class="qn-tabbar" role="tablist" aria-label="已打开的笔记标签" data-testid="editor-tabbar">
    <div class="qn-tabbar__list">
      <div
        v-for="tab in tabsStore.tabs"
        :key="tab.key"
        role="tab"
        :aria-selected="tab.key === tabsStore.activeKey"
        :data-testid="`editor-tab-${tab.relPath}`"
        :class="[
          'qn-tabbar__item',
          {
            'qn-tabbar__item--active': tab.key === tabsStore.activeKey,
            'qn-tabbar__item--preview': tab.isPreview,
            'qn-tabbar__item--missing': tab.isMissing,
          },
        ]"
        :title="tab.absPath || tab.relPath || undefined"
        @click="tabsStore.activateTab(tab.key)"
        @dblclick="tabsStore.promoteTabToPinned(tab.key)"
        @mousedown.middle.prevent="tabsStore.closeTab(tab.key)"
      >
        <span class="qn-tabbar__title">{{ getTabTitle(tab) }}</span>
        <QnBadge v-if="tab.isMissing" tone="danger">失联</QnBadge>
        <span v-else-if="tab.isDirty" class="qn-tabbar__dirty" aria-label="未保存" />
        <button
          type="button"
          class="qn-tabbar__close"
          :aria-label="`关闭 ${getTabTitle(tab)}`"
          :data-testid="`editor-tab-close-${tab.relPath}`"
          @click.stop="tabsStore.closeTab(tab.key)"
        >
          ×
        </button>
      </div>
    </div>

    <div v-if="tabsStore.tabs.length > 1 && tabsStore.activeKey" class="qn-tabbar__actions">
      <button
        type="button"
        class="qn-tabbar__action-btn"
        data-testid="editor-tab-close-others"
        @click="tabsStore.closeOtherTabs(tabsStore.activeKey)"
      >
        关闭其他
      </button>
    </div>
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
  overflow-x: auto;
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

.qn-tabbar__actions {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.qn-tabbar__action-btn {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-tabbar__action-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}
</style>
