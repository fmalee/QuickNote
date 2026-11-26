<script setup lang="ts">
import { ref } from 'vue';
import type { SearchResultItem } from '@/bindings/types';
import { type ContextMenuEntry, QnBadge, QnContextMenu } from '@/components';

const props = defineProps<{
  items: SearchResultItem[];
  activeAbsPath: string | null | undefined;
}>();

const emit = defineEmits<{
  (e: 'open', absPath: string, preview: boolean): void;
  (e: 'reveal', absPath: string): void;
  (e: 'unpin', absPath: string): void;
}>();

const expanded = ref<boolean>(true);
const ctxOpen = ref<boolean>(false);
const ctxPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const ctxItem = ref<SearchResultItem | null>(null);

const ctxMenuItems: ContextMenuEntry[] = [
  { type: 'item', id: 'external.open', title: '在标签页打开' },
  { type: 'item', id: 'external.reveal', title: '在访达中显示' },
  { type: 'separator', id: 'external-sep' },
  { type: 'item', id: 'external.unpin', title: '取消固定常用文件' },
];

function openContextMenu(event: MouseEvent, item: SearchResultItem): void {
  ctxItem.value = item;
  ctxPos.value = { x: event.clientX, y: event.clientY };
  ctxOpen.value = true;
}

function handleContextSelect(id: string): void {
  const item = ctxItem.value;
  if (!item) {
    return;
  }
  if (id === 'external.open') {
    emit('open', item.absPath, false);
  } else if (id === 'external.reveal') {
    emit('reveal', item.absPath);
  } else if (id === 'external.unpin') {
    emit('unpin', item.absPath);
  }
}
</script>

<template>
  <div
    v-if="props.items.length > 0"
    class="qn-notes-sidebar__pinned-external"
    data-testid="notes-pinned-external"
  >
    <button
      type="button"
      class="qn-notes-sidebar__pinned-header"
      data-testid="notes-pinned-external-toggle"
      @click="expanded = !expanded"
    >
      <span>{{ expanded ? '▾' : '▸' }} 常用外部文件</span>
      <QnBadge tone="accent" size="sm">{{ props.items.length }}</QnBadge>
    </button>

    <div v-if="expanded" class="qn-notes-sidebar__pinned-list">
      <div
        v-for="item in props.items"
        :key="item.absPath"
        :class="[
          'qn-notes-sidebar__pinned-item',
          {
            'qn-notes-sidebar__pinned-item--active': props.activeAbsPath === item.absPath,
          },
        ]"
        :title="item.absPath"
        :data-testid="`pinned-external-item-${item.absPath}`"
        @click="emit('open', item.absPath, true)"
        @dblclick="emit('open', item.absPath, false)"
        @contextmenu.prevent="openContextMenu($event, item)"
      >
        <span class="qn-notes-sidebar__pinned-title">{{ item.title }}</span>
        <button
          type="button"
          class="qn-notes-sidebar__pinned-unpin"
          title="取消固定"
          :aria-label="`取消固定 ${item.title}`"
          :data-testid="`pinned-external-unpin-${item.absPath}`"
          @click.stop="emit('unpin', item.absPath)"
        >
          ×
        </button>
      </div>
    </div>

    <QnContextMenu
      :open="ctxOpen"
      :x="ctxPos.x"
      :y="ctxPos.y"
      :items="ctxMenuItems"
      @select="handleContextSelect"
      @close="ctxOpen = false"
    />
  </div>
</template>

<style scoped>
.qn-notes-sidebar__pinned-external {
  border-bottom: 1px solid var(--qn-border-subtle);
  padding: var(--qn-space-1) var(--qn-space-2);
  background-color: var(--qn-bg-sidebar);
}

.qn-notes-sidebar__pinned-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: var(--qn-space-1);
  border: none;
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-weight: 600;
  cursor: pointer;
}

.qn-notes-sidebar__pinned-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 140px;
  overflow-y: auto;
  margin-top: var(--qn-space-1);
}

.qn-notes-sidebar__pinned-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-1);
  padding: 3px var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-notes-sidebar__pinned-item:hover,
.qn-notes-sidebar__pinned-item--active {
  background-color: var(--qn-bg-hover);
}

.qn-notes-sidebar__pinned-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-notes-sidebar__pinned-unpin {
  border: none;
  background: transparent;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  cursor: pointer;
  padding: 0 2px;
  border-radius: var(--qn-radius-sm);
}

.qn-notes-sidebar__pinned-unpin:hover {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}
</style>
