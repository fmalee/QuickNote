<script setup lang="ts">
import { computed, ref } from 'vue';
import type { TagNode } from '@/bindings/types';
import { type ContextMenuEntry, QnContextMenu } from '@/components';

interface TagCloudItem {
  tag: string;
  count: number;
}

const props = defineProps<{
  tags: TagNode[];
  selectedTags: string[];
}>();

const emit = defineEmits<{
  (e: 'toggle-tag', tag: string): void;
  (e: 'clear-tags'): void;
  (e: 'request-rename', tag: string): void;
}>();

const tagCtxOpen = ref<boolean>(false);
const tagCtxPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const tagCtxTarget = ref<string | null>(null);
const tagCtxMenuItems: ContextMenuEntry[] = [
  { type: 'item', id: 'tag.rename', title: '重命名 / 合并…' },
];

function flattenTagNodes(nodes: TagNode[]): TagCloudItem[] {
  const result: TagCloudItem[] = [];
  function walk(items: TagNode[]): void {
    for (const node of items) {
      result.push({ tag: node.tag, count: node.count });
      if (node.children.length > 0) {
        walk(node.children);
      }
    }
  }
  walk(nodes);
  return result;
}

const tagCloudItems = computed<TagCloudItem[]>(() =>
  flattenTagNodes(props.tags)
    .sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag))
    .slice(0, 80),
);

function tagCloudTier(count: number): 'sm' | 'md' | 'lg' {
  const items = tagCloudItems.value;
  if (items.length === 0) {
    return 'sm';
  }
  const maxCount = items[0]?.count ?? 0;
  const minCount = items[items.length - 1]?.count ?? 0;
  if (maxCount <= minCount) {
    return 'md';
  }
  const ratio = (count - minCount) / (maxCount - minCount);
  if (ratio >= 2 / 3) {
    return 'lg';
  }
  if (ratio >= 1 / 3) {
    return 'md';
  }
  return 'sm';
}

function openTagContextMenu(event: MouseEvent, tag: string): void {
  tagCtxTarget.value = tag;
  tagCtxPos.value = { x: event.clientX, y: event.clientY };
  tagCtxOpen.value = true;
}

function handleTagContextSelect(id: string): void {
  const target = tagCtxTarget.value;
  tagCtxOpen.value = false;
  if (id === 'tag.rename' && target) {
    emit('request-rename', target);
  }
}
</script>

<template>
  <div class="qn-notes-sidebar__subpanel" data-testid="nav-tags-tree">
    <div class="qn-notes-sidebar__subpanel-header">
      <span>标签（多选 AND）</span>
      <button
        v-if="props.selectedTags.length > 0"
        type="button"
        class="qn-notes-sidebar__link-btn"
        data-testid="clear-tags-btn"
        @click="emit('clear-tags')"
      >
        清除筛选
      </button>
    </div>
    <div v-if="props.selectedTags.length > 0" class="qn-notes-sidebar__chips">
      <button
        v-for="tag in props.selectedTags"
        :key="tag"
        type="button"
        class="qn-notes-sidebar__chip"
        :data-testid="`selected-tag-chip-${tag}`"
        @click="emit('toggle-tag', tag)"
      >
        #{{ tag }} ×
      </button>
    </div>
    <div v-if="tagCloudItems.length === 0" class="qn-notes-sidebar__empty">暂无标签</div>
    <div v-else class="qn-notes-sidebar__tag-cloud" data-testid="tag-cloud">
      <button
        v-for="item in tagCloudItems"
        :key="item.tag"
        type="button"
        :class="[
          'qn-notes-sidebar__tag-cloud-item',
          `qn-notes-sidebar__tag-cloud-item--${tagCloudTier(item.count)}`,
          {
            'qn-notes-sidebar__tag-cloud-item--active': props.selectedTags.includes(item.tag),
          },
        ]"
        :title="`${item.tag} · ${item.count}`"
        :data-testid="`tag-cloud-item-${item.tag}`"
        @click="emit('toggle-tag', item.tag)"
        @contextmenu.prevent="openTagContextMenu($event, item.tag)"
      >
        #{{ item.tag }}
      </button>
    </div>
    <QnContextMenu
      :open="tagCtxOpen"
      :x="tagCtxPos.x"
      :y="tagCtxPos.y"
      :items="tagCtxMenuItems"
      @select="handleTagContextSelect"
      @close="tagCtxOpen = false"
    />
  </div>
</template>

<style scoped>
.qn-notes-sidebar__subpanel {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  max-height: 180px;
  overflow-y: auto;
}

.qn-notes-sidebar__subpanel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-secondary);
}

.qn-notes-sidebar__link-btn {
  border: none;
  background: transparent;
  padding: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-accent);
  cursor: pointer;
}

.qn-notes-sidebar__chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--qn-space-1);
}

.qn-notes-sidebar__chip {
  padding: 2px var(--qn-space-2);
  border: 1px solid var(--qn-accent);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-accent-soft);
  color: var(--qn-accent);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-notes-sidebar__empty {
  padding: var(--qn-space-4) var(--qn-space-2);
  text-align: center;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-notes-sidebar__tag-cloud {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--qn-space-1) var(--qn-space-2);
}

.qn-notes-sidebar__tag-cloud-item {
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  padding: 2px var(--qn-space-1);
  line-height: 1.3;
  cursor: pointer;
}

.qn-notes-sidebar__tag-cloud-item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-notes-sidebar__tag-cloud-item--active {
  background-color: var(--qn-accent-soft);
  color: var(--qn-accent);
}

.qn-notes-sidebar__tag-cloud-item--sm {
  font-size: var(--qn-text-sm);
}

.qn-notes-sidebar__tag-cloud-item--md {
  font-size: var(--qn-text-base);
  font-weight: 500;
}

.qn-notes-sidebar__tag-cloud-item--lg {
  font-size: var(--qn-text-lg);
  font-weight: 600;
}
</style>
