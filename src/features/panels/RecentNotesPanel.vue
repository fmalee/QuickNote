<script setup lang="ts">
import type { NoteSummary } from '@/bindings/types';
import { useEditorTabsStore } from '@/features/editor';
import { usePanelsStore } from './store';

const panelsStore = usePanelsStore();
const editorTabsStore = useEditorTabsStore();

function formatRelativeTime(mtimeMs: number): string {
  const diffSec = Math.max(0, Math.floor((Date.now() - mtimeMs) / 1000));
  if (diffSec < 60) {
    return '刚刚';
  }
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) {
    return `${diffMin} 分钟前`;
  }
  const diffHour = Math.floor(diffMin / 60);
  if (diffHour < 24) {
    return `${diffHour} 小时前`;
  }
  const diffDay = Math.floor(diffHour / 24);
  return `${diffDay} 天前`;
}

function getParentDir(relPath: string | null): string {
  if (!relPath) {
    return '库外文件';
  }
  const idx = relPath.lastIndexOf('/');
  return idx > 0 ? relPath.slice(0, idx) : '根目录';
}

function isCurrentNote(item: NoteSummary): boolean {
  const activeKey = editorTabsStore.activeKey;
  if (activeKey && activeKey === item.key) {
    return true;
  }
  return editorTabsStore.activeTab?.absPath === item.absPath;
}

async function openRecentNote(item: NoteSummary): Promise<void> {
  await editorTabsStore.openNoteByAbsPath(item.absPath, { preview: false });
}
</script>

<template>
  <div class="qn-recent-panel" data-testid="recent-notes-panel">
    <div
      v-if="panelsStore.recentLoading && panelsStore.recentItems.length === 0"
      class="qn-recent-panel__state"
    >
      正在加载最近编辑…
    </div>

    <div
      v-else-if="panelsStore.recentItems.length === 0"
      class="qn-recent-panel__state"
      data-testid="recent-empty"
    >
      暂无最近编辑记录
    </div>

    <ul v-else class="qn-recent-panel__list" data-testid="recent-notes-list">
      <li
        v-for="item in panelsStore.recentItems"
        :key="item.key"
        class="qn-recent-panel__item-wrap"
      >
        <button
          type="button"
          :class="[
            'qn-recent-panel__item',
            { 'qn-recent-panel__item--active': isCurrentNote(item) },
          ]"
          :data-testid="`recent-item-${item.key}`"
          @click="openRecentNote(item)"
        >
          <span class="qn-recent-panel__item-title">{{ item.title || item.relPath }}</span>
          <div class="qn-recent-panel__item-meta">
            <span class="qn-recent-panel__item-dir">{{ getParentDir(item.relPath) }}</span>
            <span class="qn-recent-panel__item-time">{{ formatRelativeTime(item.mtimeMs) }}</span>
          </div>
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.qn-recent-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.qn-recent-panel__state {
  padding: var(--qn-space-4) var(--qn-space-3);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-recent-panel__list {
  list-style: none;
  margin: 0;
  padding: var(--qn-space-2);
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-recent-panel__item-wrap {
  min-width: 0;
}

.qn-recent-panel__item {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.qn-recent-panel__item:hover {
  background-color: var(--qn-bg-hover);
}

.qn-recent-panel__item--active {
  background-color: var(--qn-accent-soft);
}

.qn-recent-panel__item-title {
  width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-primary);
}

.qn-recent-panel__item--active .qn-recent-panel__item-title {
  color: var(--qn-accent);
}

.qn-recent-panel__item-meta {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-recent-panel__item-dir {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.qn-recent-panel__item-time {
  flex-shrink: 0;
}
</style>
