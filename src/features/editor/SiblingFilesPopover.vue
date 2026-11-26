<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type {
  ExternalActionState,
  ExternalDirectoryEntry,
  ExternalDirectoryListing,
} from '@/bindings/types';
import { type ContextMenuEntry, QnContextMenu } from '@/components';
import { extractEditorErrorMessage, listCurrentDocSiblings } from './api';
import { useEditorTabsStore } from './store';
import {
  appendExternalContextMenuItems,
  isExternalActionId,
  useExternalActions,
} from './useExternalActions';

const props = defineProps<{
  open: boolean;
  docPath: string | null;
}>();

const emit = defineEmits<{
  close: [];
  select: [path: string];
  loaded: [listing: ExternalDirectoryListing];
}>();

const editorTabsStore = useEditorTabsStore();
const externalActions = useExternalActions();

const popoverRef = ref<HTMLDivElement | null>(null);
const filterInputRef = ref<HTMLInputElement | null>(null);
const filterText = ref('');
const loading = ref(false);
const errorMessage = ref<string | null>(null);
const listing = ref<ExternalDirectoryListing | null>(null);
const contextMenuOpen = ref(false);
const contextMenuPos = ref({ x: 0, y: 0 });
const contextEntry = ref<ExternalDirectoryEntry | null>(null);
const contextEntryStates = ref<ExternalActionState[]>([]);

let loadSeq = 0;
let entryMenuSeq = 0;

const contextMenuItems = computed<ContextMenuEntry[]>(() => {
  const baseEntries: ContextMenuEntry[] = [
    { type: 'item', id: 'sibling.open', title: '打开' },
    { type: 'item', id: 'sibling.openNewTab', title: '在新标签打开' },
  ];
  return appendExternalContextMenuItems(baseEntries, contextEntryStates.value, 'sibling-ext-sep');
});

const editableEntries = computed<ExternalDirectoryEntry[]>(() => {
  const entries = listing.value?.entries ?? [];
  return entries.filter((entry) => entry.kind === 'file');
});

const filteredEntries = computed<ExternalDirectoryEntry[]>(() => {
  const keyword = filterText.value.trim().toLowerCase();
  if (!keyword) {
    return editableEntries.value;
  }
  return editableEntries.value.filter((entry) => {
    const nameMatch = entry.name.toLowerCase().includes(keyword);
    const relMatch = entry.relPath.toLowerCase().includes(keyword);
    return nameMatch || relMatch;
  });
});

async function loadSiblings(docPath: string): Promise<void> {
  const seq = ++loadSeq;
  loading.value = true;
  errorMessage.value = null;

  try {
    const result = await listCurrentDocSiblings(docPath);
    if (seq !== loadSeq) {
      return;
    }
    listing.value = result;
    emit('loaded', result);
  } catch (error) {
    if (seq !== loadSeq) {
      return;
    }
    listing.value = null;
    errorMessage.value = extractEditorErrorMessage(error, '读取同目录文件列表失败');
  } finally {
    if (seq === loadSeq) {
      loading.value = false;
    }
  }
}

watch(
  () => [props.open, props.docPath] as const,
  async ([isOpen, nextPath]) => {
    if (!isOpen) {
      return;
    }
    filterText.value = '';
    if (!nextPath) {
      listing.value = null;
      errorMessage.value = '当前文档路径不可用';
      return;
    }
    void loadSiblings(nextPath);
    await nextTick();
    filterInputRef.value?.focus();
  },
  { immediate: true },
);

async function handleSelectEntry(entry: ExternalDirectoryEntry): Promise<void> {
  if (entry.path !== props.docPath) {
    await editorTabsStore.openNoteByAbsPath(entry.path, { preview: false });
  }
  emit('select', entry.path);
  emit('close');
}

async function openEntryContextMenu(
  event: MouseEvent,
  entry: ExternalDirectoryEntry,
): Promise<void> {
  const reqSeq = ++entryMenuSeq;
  contextEntry.value = entry;
  contextMenuPos.value = { x: event.clientX, y: event.clientY };

  const activeDoc = editorTabsStore.activeDoc;
  const workspaceKey = entry.path === activeDoc?.absPath ? (activeDoc.meta.project ?? null) : null;

  if (entry.path === externalActions.resolvedAbsPath && externalActions.states.length > 0) {
    contextEntryStates.value = externalActions.states;
    contextMenuOpen.value = true;
    return;
  }

  contextEntryStates.value = [];
  contextMenuOpen.value = true;
  try {
    const resolved = await externalActions.resolveForTarget({
      absPath: entry.path,
      workspaceKey,
    });
    if (reqSeq === entryMenuSeq) {
      contextEntryStates.value = resolved;
    }
  } catch {
    if (reqSeq === entryMenuSeq) {
      contextEntryStates.value = [];
    }
  }
}

async function handleContextMenuSelect(id: string): Promise<void> {
  const entry = contextEntry.value;
  contextMenuOpen.value = false;
  if (!entry) {
    return;
  }
  if (id === 'sibling.open') {
    await handleSelectEntry(entry);
    return;
  }
  if (id === 'sibling.openNewTab') {
    await editorTabsStore.openNoteByAbsPath(entry.path, { preview: false });
    emit('select', entry.path);
    emit('close');
    return;
  }
  if (isExternalActionId(id)) {
    const matchedTab = editorTabsStore.tabs.find((tab) => tab.absPath === entry.path);
    await externalActions.executeAction(id, {
      overrideStates: contextEntryStates.value,
      pinned: matchedTab?.doc?.meta.pinned,
    });
  }
}

function handlePointerDownOutside(event: PointerEvent): void {
  if (!props.open || contextMenuOpen.value) {
    return;
  }
  const target = event.target as Node | null;
  if (target && popoverRef.value && !popoverRef.value.contains(target)) {
    emit('close');
  }
}

function handleKeydown(event: KeyboardEvent): void {
  if (!props.open || contextMenuOpen.value) {
    return;
  }
  if (event.key === 'Escape') {
    event.preventDefault();
    emit('close');
  }
}

onMounted(() => {
  if (typeof window === 'undefined') {
    return;
  }
  window.addEventListener('pointerdown', handlePointerDownOutside);
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  if (typeof window === 'undefined') {
    return;
  }
  window.removeEventListener('pointerdown', handlePointerDownOutside);
  window.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <div
    v-if="open"
    ref="popoverRef"
    class="qn-sibling-files-popover"
    data-testid="sibling-files-popover"
    role="dialog"
    aria-label="切换同目录文件"
  >
    <div class="qn-sibling-files-popover__header">
      <div class="qn-sibling-files-popover__title-group">
        <span class="qn-sibling-files-popover__title">同目录文件</span>
        <span
          v-if="listing?.rootPath"
          class="qn-sibling-files-popover__root"
          data-testid="sibling-files-root-path"
          :title="listing.rootPath"
        >
          {{ listing.rootPath }}
        </span>
      </div>
      <button
        type="button"
        class="qn-sibling-files-popover__close"
        aria-label="关闭同目录文件弹窗"
        @click="emit('close')"
      >
        ×
      </button>
    </div>

    <div class="qn-sibling-files-popover__search">
      <input
        ref="filterInputRef"
        v-model="filterText"
        type="text"
        class="qn-sibling-files-popover__input"
        data-testid="sibling-files-filter-input"
        placeholder="输入文件名或相对路径过滤…"
        aria-label="过滤同目录文件"
      />
    </div>

    <div class="qn-sibling-files-popover__body">
      <div
        v-if="loading"
        class="qn-sibling-files-popover__status"
        data-testid="sibling-files-loading"
      >
        正在读取目录文件…
      </div>

      <div
        v-else-if="errorMessage"
        class="qn-sibling-files-popover__error"
        data-testid="sibling-files-error"
      >
        {{ errorMessage }}
      </div>

      <div
        v-else-if="filteredEntries.length === 0"
        class="qn-sibling-files-popover__status"
        data-testid="sibling-files-empty"
      >
        {{ filterText.trim() ? '没有匹配的可编辑文件' : '当前目录暂无其他可编辑文件' }}
      </div>

      <ul v-else class="qn-sibling-files-popover__list" data-testid="sibling-files-list">
        <li v-for="entry in filteredEntries" :key="entry.path">
          <button
            type="button"
            class="qn-sibling-files-popover__item"
            :class="{ 'is-current': entry.path === docPath }"
            data-testid="sibling-files-item"
            :title="entry.path"
            @click="handleSelectEntry(entry)"
            @contextmenu.prevent="openEntryContextMenu($event, entry)"
          >
            <span class="qn-sibling-files-popover__item-main">
              <span class="qn-sibling-files-popover__item-name">{{ entry.name }}</span>
              <span v-if="entry.path === docPath" class="qn-sibling-files-popover__current-tag">
                当前
              </span>
            </span>
            <span
              v-if="entry.relPath && entry.relPath !== entry.name"
              class="qn-sibling-files-popover__item-rel"
              :title="entry.relPath"
            >
              {{ entry.relPath }}
            </span>
          </button>
        </li>
      </ul>
    </div>

    <div
      v-if="listing?.truncated"
      class="qn-sibling-files-popover__footer"
      data-testid="sibling-files-truncated-hint"
    >
      目录条目较多，已展示前 {{ editableEntries.length }} 个文件，可输入关键字缩小范围
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
</template>

<style scoped>
.qn-sibling-files-popover {
  position: absolute;
  top: calc(100% + var(--qn-space-1));
  right: 0;
  z-index: var(--qn-z-popover);
  width: 340px;
  max-height: 380px;
  display: flex;
  flex-direction: column;
  padding: var(--qn-space-3);
  gap: var(--qn-space-2);
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  box-shadow: var(--qn-shadow-lg);
}

.qn-sibling-files-popover__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--qn-space-2);
}

.qn-sibling-files-popover__title-group {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 2px;
}

.qn-sibling-files-popover__title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-sibling-files-popover__root {
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
  color: var(--qn-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-sibling-files-popover__close {
  border: none;
  background: transparent;
  color: var(--qn-text-muted);
  cursor: pointer;
  font-size: var(--qn-text-base);
  line-height: 1;
  padding: var(--qn-space-1);
  flex-shrink: 0;
}

.qn-sibling-files-popover__close:hover {
  color: var(--qn-text-primary);
}

.qn-sibling-files-popover__search {
  display: flex;
}

.qn-sibling-files-popover__input {
  width: 100%;
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-sans);
}

.qn-sibling-files-popover__input:focus {
  outline: none;
  border-color: var(--qn-accent);
}

.qn-sibling-files-popover__body {
  overflow-y: auto;
  min-height: 80px;
  max-height: 240px;
}

.qn-sibling-files-popover__status {
  padding: var(--qn-space-3) var(--qn-space-2);
  text-align: center;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-sibling-files-popover__error {
  padding: var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-danger-soft);
  border: 1px solid var(--qn-danger-border);
  color: var(--qn-danger);
  font-size: var(--qn-text-xs);
  line-height: 1.4;
}

.qn-sibling-files-popover__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.qn-sibling-files-popover__item {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-primary);
  text-align: left;
  cursor: pointer;
}

.qn-sibling-files-popover__item:hover {
  background-color: var(--qn-bg-hover);
}

.qn-sibling-files-popover__item.is-current {
  background-color: var(--qn-accent-soft);
}

.qn-sibling-files-popover__item-main {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
}

.qn-sibling-files-popover__item-name {
  font-size: var(--qn-text-xs);
  font-weight: 500;
  color: var(--qn-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-sibling-files-popover__current-tag {
  flex-shrink: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-accent);
  font-weight: 600;
}

.qn-sibling-files-popover__item-rel {
  width: 100%;
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
  color: var(--qn-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-sibling-files-popover__footer {
  padding-top: var(--qn-space-1);
  border-top: 1px solid var(--qn-border-subtle);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}
</style>
