<script setup lang="ts">
import type { RelatedNoteItem, RelatedReason } from '@/bindings/types';
import { QnBadge } from '@/components';
import { useEditorTabsStore } from '@/features/editor';
import { usePanelsStore } from './store';

const panelsStore = usePanelsStore();
const editorTabsStore = useEditorTabsStore();

function reasonLabel(reason: RelatedReason): string {
  switch (reason) {
    case 'backlink':
      return '反向链接';
    case 'shared_tags':
      return '共有标签';
    case 'same_project':
      return '同项目';
  }
}

function reasonTone(reason: RelatedReason): 'accent' | 'success' | 'neutral' {
  switch (reason) {
    case 'backlink':
      return 'accent';
    case 'shared_tags':
      return 'success';
    case 'same_project':
      return 'neutral';
  }
}

async function openRelatedNote(item: RelatedNoteItem): Promise<void> {
  await editorTabsStore.openNote(item.relPath, { preview: false });
}
</script>

<template>
  <div class="qn-related-panel" data-testid="related-notes-panel">
    <div v-if="panelsStore.relatedLoading" class="qn-related-panel__state">正在计算相关文档…</div>

    <div
      v-else-if="panelsStore.relatedEmptyReason === 'outside-library'"
      class="qn-related-panel__state"
      data-testid="related-outside-library"
    >
      库外文件暂不计算库内关联
    </div>

    <div v-else-if="panelsStore.relatedEmptyReason === 'no-doc'" class="qn-related-panel__state">
      打开库内笔记后显示相关文档
    </div>

    <div v-else-if="panelsStore.relatedError" class="qn-related-panel__state">
      {{ panelsStore.relatedError }}
    </div>

    <div
      v-else-if="panelsStore.relatedItems.length === 0"
      class="qn-related-panel__state"
      data-testid="related-empty"
    >
      暂无相关文档
    </div>

    <ul v-else class="qn-related-panel__list" data-testid="related-notes-list">
      <li
        v-for="item in panelsStore.relatedItems"
        :key="item.key"
        class="qn-related-panel__item-wrap"
      >
        <button
          type="button"
          class="qn-related-panel__item"
          :data-testid="`related-item-${item.key}`"
          @click="openRelatedNote(item)"
        >
          <div class="qn-related-panel__item-top">
            <span class="qn-related-panel__item-title">{{ item.title || item.relPath }}</span>
            <QnBadge :tone="reasonTone(item.reason)">
              {{ reasonLabel(item.reason) }}
            </QnBadge>
          </div>
          <span class="qn-related-panel__item-path">{{ item.relPath }}</span>
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.qn-related-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.qn-related-panel__state {
  padding: var(--qn-space-4) var(--qn-space-3);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-related-panel__list {
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

.qn-related-panel__item-wrap {
  min-width: 0;
}

.qn-related-panel__item {
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

.qn-related-panel__item:hover {
  background-color: var(--qn-bg-hover);
}

.qn-related-panel__item-top {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  min-width: 0;
}

.qn-related-panel__item-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-primary);
}

.qn-related-panel__item-path {
  width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}
</style>
