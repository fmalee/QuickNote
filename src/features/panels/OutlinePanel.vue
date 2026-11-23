<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { usePanelsStore } from './store';

const panelsStore = usePanelsStore();

function handleKeydown(event: KeyboardEvent): void {
  if (!event.ctrlKey || !event.metaKey || event.altKey || event.shiftKey) {
    return;
  }
  if (event.code === 'KeyO') {
    event.preventDefault();
    panelsStore.toggleOutlinePanel();
    return;
  }
  if (event.code === 'Digit0') {
    event.preventDefault();
    panelsStore.toggleCollapseAll();
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
  <aside
    v-if="panelsStore.isOutlineVisible"
    class="qn-outline-panel"
    data-testid="outline-panel"
    :style="{ width: `${panelsStore.panelGroup.width}px` }"
  >
    <header class="qn-outline-panel__header">
      <span class="qn-outline-panel__title">大纲</span>
      <button
        type="button"
        class="qn-outline-panel__collapse-btn"
        title="折叠右侧面板 (⌃⌘0)"
        data-testid="outline-collapse-btn"
        @click="panelsStore.toggleCollapseAll()"
      >
        折叠
      </button>
    </header>

    <div v-if="panelsStore.outlineItems.length === 0" class="qn-outline-panel__empty">
      当前文档暂无标题
    </div>

    <ul v-else class="qn-outline-panel__list" data-testid="outline-list">
      <li
        v-for="(item, index) in panelsStore.outlineItems"
        :key="`${item.slug}-${item.line}`"
        class="qn-outline-panel__item-wrap"
      >
        <button
          type="button"
          :class="[
            'qn-outline-panel__item',
            { 'qn-outline-panel__item--active': index === panelsStore.activeHeadingIndex },
          ]"
          :style="{ paddingLeft: `calc(var(--qn-space-2) + ${(item.level - 1) * 12}px)` }"
          :data-testid="`outline-item-${item.slug}`"
          @click="panelsStore.jumpToHeading(item)"
        >
          <span class="qn-outline-panel__item-text">{{ item.text }}</span>
        </button>
      </li>
    </ul>
  </aside>
</template>

<style scoped>
.qn-outline-panel {
  flex-shrink: 0;
  min-width: 220px;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  min-height: 0;
}

.qn-outline-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--qn-space-2) var(--qn-space-3);
  border-bottom: 1px solid var(--qn-border-subtle);
}

.qn-outline-panel__title {
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-secondary);
}

.qn-outline-panel__collapse-btn {
  border: none;
  background: transparent;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  cursor: pointer;
  padding: 2px var(--qn-space-1);
  border-radius: var(--qn-radius-sm);
}

.qn-outline-panel__collapse-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-outline-panel__empty {
  padding: var(--qn-space-4) var(--qn-space-3);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-outline-panel__list {
  list-style: none;
  margin: 0;
  padding: var(--qn-space-2);
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.qn-outline-panel__item-wrap {
  min-width: 0;
}

.qn-outline-panel__item {
  width: 100%;
  display: flex;
  align-items: center;
  border: none;
  background: transparent;
  text-align: left;
  padding-top: var(--qn-space-1);
  padding-bottom: var(--qn-space-1);
  padding-right: var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-sm);
  cursor: pointer;
}

.qn-outline-panel__item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-outline-panel__item--active {
  background-color: var(--qn-accent-soft);
  color: var(--qn-accent);
  font-weight: 500;
}

.qn-outline-panel__item-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
