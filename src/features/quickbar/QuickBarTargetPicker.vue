<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { QnBadge, QnButton, QnInput } from '@/components';
import { useQuickBarStore } from './store';

interface PickerDocCandidate {
  key: string;
  relPath: string;
  title: string;
  subtitle: string;
  workspaceKey: string | null;
}

const store = useQuickBarStore();
const searchInputQuery = ref<string>('');
const searchBoxRef = ref<HTMLDivElement | null>(null);

watch(
  () => store.targetPickerOpen,
  (isOpen) => {
    if (isOpen) {
      searchInputQuery.value = store.pickerSearchQuery;
      void nextTick(() => {
        const input = searchBoxRef.value?.querySelector('input');
        input?.focus();
      });
    }
  },
);

function handleSearchInput(value: string): void {
  searchInputQuery.value = value;
  store.setPickerSearchQuery(value);
}

const displayedDocs = computed<PickerDocCandidate[]>(() => {
  if (store.pickerSearchItems !== null) {
    return store.pickerSearchItems
      .filter((item) => item.relPath !== null)
      .map((item) => ({
        key: item.key,
        relPath: item.relPath ?? item.key,
        title: item.title,
        subtitle: item.subtitle || item.relPath || '',
        workspaceKey: null,
      }));
  }
  return store.pickerNotes
    .filter((note): note is typeof note & { relPath: string } => note.relPath !== null)
    .map((note) => ({
      key: note.key,
      relPath: note.relPath,
      title: note.title || note.relPath,
      subtitle: note.relPath,
      workspaceKey: null,
    }));
});

function selectInbox(): void {
  store.selectExplicitDoc({
    relPath: 'inbox.md',
    title: '收件箱',
    workspaceKey: null,
  });
}

function handleEnterSelect(): void {
  const first = displayedDocs.value[0];
  if (!first) {
    return;
  }
  store.selectExplicitDoc({
    relPath: first.relPath,
    title: first.title,
    workspaceKey: first.workspaceKey,
  });
}
</script>

<template>
  <div v-if="store.targetPickerOpen" class="qn-target-picker" data-testid="quickbar-target-picker">
    <div class="qn-target-picker__header">
      <span class="qn-target-picker__title">选择追加目标与锁定工作区 (⌘T)</span>
      <QnButton variant="ghost" size="sm" @click="store.closeTargetPicker()">关闭 Esc</QnButton>
    </div>

    <div class="qn-target-picker__section">
      <div class="qn-target-picker__label">锁定工作区</div>
      <div class="qn-target-picker__chips">
        <button
          type="button"
          :class="[
            'qn-target-picker__chip',
            { 'qn-target-picker__chip--active': store.selectedWorkspaceKey === null },
          ]"
          data-testid="picker-workspace-unlock"
          @click="store.selectWorkspaceLock(null)"
        >
          不锁定工作区
        </button>
        <button
          v-for="ws in store.availableWorkspaces"
          :key="ws.key"
          type="button"
          :class="[
            'qn-target-picker__chip',
            { 'qn-target-picker__chip--active': store.selectedWorkspaceKey === ws.key },
          ]"
          :data-testid="`picker-workspace-${ws.key}`"
          @click="store.selectWorkspaceLock(ws.key)"
        >
          {{ ws.name }}
        </button>
      </div>
    </div>

    <div class="qn-target-picker__section">
      <div
        ref="searchBoxRef"
        class="qn-target-picker__search"
        @keydown.enter.prevent="handleEnterSelect"
      >
        <QnInput
          :model-value="searchInputQuery"
          placeholder="搜索目标笔记标题或路径…"
          data-testid="picker-search-input"
          @update:model-value="handleSearchInput"
        />
      </div>

      <div v-if="store.pickerTags.length > 0" class="qn-target-picker__chips">
        <button
          v-for="tag in store.pickerTags.slice(0, 8)"
          :key="tag.tag"
          type="button"
          :class="[
            'qn-target-picker__chip',
            { 'qn-target-picker__chip--active': store.pickerSelectedTag === tag.tag },
          ]"
          @click="store.selectPickerTag(tag.tag)"
        >
          #{{ tag.tag }} ({{ tag.count }})
        </button>
      </div>
    </div>

    <div class="qn-target-picker__list">
      <button
        type="button"
        class="qn-target-picker__item"
        data-testid="picker-doc-inbox"
        @click="selectInbox"
      >
        <span class="qn-target-picker__item-title">inbox.md（收件箱）</span>
        <QnBadge tone="neutral">默认收件箱</QnBadge>
      </button>

      <button
        v-for="doc in displayedDocs"
        :key="doc.key"
        type="button"
        class="qn-target-picker__item"
        :data-testid="`picker-doc-${doc.relPath}`"
        @click="
          store.selectExplicitDoc({
            relPath: doc.relPath,
            title: doc.title,
            workspaceKey: doc.workspaceKey,
          })
        "
      >
        <span class="qn-target-picker__item-title">{{ doc.title }}</span>
        <span class="qn-target-picker__item-sub">{{ doc.subtitle }}</span>
      </button>

      <div
        v-if="!store.pickerLoading && displayedDocs.length === 0"
        class="qn-target-picker__empty"
      >
        暂无匹配的库内笔记
      </div>
    </div>
  </div>
</template>

<style scoped>
.qn-target-picker {
  position: absolute;
  top: calc(var(--qn-space-10) + var(--qn-space-2));
  right: var(--qn-space-4);
  left: var(--qn-space-4);
  z-index: var(--qn-z-popover);
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
  max-height: 360px;
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-default);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-elevated);
  box-shadow: var(--qn-shadow-md);
}

.qn-target-picker__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
}

.qn-target-picker__title {
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-secondary);
}

.qn-target-picker__section {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-target-picker__label {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-target-picker__chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--qn-space-1);
}

.qn-target-picker__chip {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-sidebar);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-target-picker__chip:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-target-picker__chip--active {
  border-color: var(--qn-accent);
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-target-picker__search {
  width: 100%;
}

.qn-target-picker__list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  border-top: 1px solid var(--qn-border-subtle);
  padding-top: var(--qn-space-2);
}

.qn-target-picker__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-primary);
  text-align: left;
  cursor: pointer;
}

.qn-target-picker__item:hover {
  background-color: var(--qn-bg-hover);
}

.qn-target-picker__item-title {
  font-size: var(--qn-text-sm);
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-target-picker__item-sub {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-target-picker__empty {
  padding: var(--qn-space-3);
  text-align: center;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}
</style>
