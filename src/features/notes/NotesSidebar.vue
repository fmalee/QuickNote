<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import type { DirNode, NoteSummary, SearchResultItem, TagNode } from '@/bindings/types';
import { QnBadge, QnBanner, QnButton, QnContextMenu, type ContextMenuEntry } from '@/components';
import { useEditorTabsStore } from '@/features/editor';
import { type NavTab, useNotesStore } from './store';

interface FlatDirItem {
  node: DirNode;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
}

interface FlatTagItem {
  node: TagNode;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
}

const props = withDefaults(
  defineProps<{
    collapsed?: boolean;
    overlayDrawer?: boolean;
  }>(),
  {
    collapsed: false,
    overlayDrawer: false,
  },
);

const emit = defineEmits<{
  (e: 'toggle-collapse'): void;
}>();

const notesStore = useNotesStore();
const editorTabsStore = useEditorTabsStore();

const filterInputRef = ref<HTMLInputElement | null>(null);
const expandedDirs = ref<Record<string, boolean>>({});
const expandedTags = ref<Record<string, boolean>>({});

const renamingRelPath = ref<string | null>(null);
const renameDraft = ref<string>('');
const pinnedExternalExpanded = ref<boolean>(true);
const externalCtxOpen = ref<boolean>(false);
const externalCtxPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const externalCtxItem = ref<SearchResultItem | null>(null);

const externalCtxMenuItems: ContextMenuEntry[] = [
  { type: 'item', id: 'external.open', title: '在标签页打开' },
  { type: 'item', id: 'external.reveal', title: '在访达中显示' },
  { type: 'separator', id: 'external-sep' },
  { type: 'item', id: 'external.unpin', title: '取消固定常用文件' },
];

function openExternalContextMenu(event: MouseEvent, item: SearchResultItem): void {
  externalCtxItem.value = item;
  externalCtxPos.value = { x: event.clientX, y: event.clientY };
  externalCtxOpen.value = true;
}

async function handleExternalContextSelect(id: string): Promise<void> {
  const item = externalCtxItem.value;
  if (!item) {
    return;
  }
  if (id === 'external.open') {
    await editorTabsStore.openNoteByAbsPath(item.absPath, { preview: false });
  } else if (id === 'external.reveal') {
    await notesStore.revealExternalFile(item.absPath);
  } else if (id === 'external.unpin') {
    await notesStore.togglePinExternalFile(item.absPath);
  }
}
const renameInputRef = ref<HTMLInputElement | null>(null);

const navItems: { id: NavTab; label: string }[] = [
  { id: 'inbox', label: '收件箱' },
  { id: 'projects', label: '项目' },
  { id: 'tags', label: '标签' },
  { id: 'recent', label: '最近' },
  { id: 'workspaces', label: '工作区' },
];

const flatDirs = computed<FlatDirItem[]>(() => {
  const result: FlatDirItem[] = [];
  function walk(nodes: DirNode[], depth: number): void {
    for (const node of nodes) {
      const hasChildren = node.children.length > 0;
      const expanded = expandedDirs.value[node.relPath] ?? depth === 0;
      result.push({ node, depth, hasChildren, expanded });
      if (hasChildren && expanded) {
        walk(node.children, depth + 1);
      }
    }
  }
  walk(notesStore.navTree.dirs, 0);
  return result;
});

const flatTags = computed<FlatTagItem[]>(() => {
  const result: FlatTagItem[] = [];
  function walk(nodes: TagNode[], depth: number): void {
    for (const node of nodes) {
      const hasChildren = node.children.length > 0;
      const expanded = expandedTags.value[node.tag] ?? depth === 0;
      result.push({ node, depth, hasChildren, expanded });
      if (hasChildren && expanded) {
        walk(node.children, depth + 1);
      }
    }
  }
  walk(notesStore.navTree.tags, 0);
  return result;
});

function toggleDirExpand(relPath: string): void {
  expandedDirs.value = {
    ...expandedDirs.value,
    [relPath]: !(expandedDirs.value[relPath] ?? true),
  };
}

function toggleTagExpand(tag: string): void {
  expandedTags.value = {
    ...expandedTags.value,
    [tag]: !(expandedTags.value[tag] ?? true),
  };
}

function onSidebarKeydown(event: KeyboardEvent): void {
  if (event.metaKey && !event.ctrlKey && !event.altKey && event.key.toLowerCase() === 'f') {
    event.preventDefault();
    filterInputRef.value?.focus();
    filterInputRef.value?.select();
  }
}

function onFilterInput(event: Event): void {
  const target = event.target as HTMLInputElement | null;
  if (target) {
    notesStore.setFilterText(target.value);
  }
}

function formatMtime(mtimeMs: number): string {
  if (!mtimeMs) {
    return '';
  }
  const date = new Date(mtimeMs);
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  return `${month}-${day} ${hours}:${minutes}`;
}

function isProtectedInbox(note: NoteSummary): boolean {
  return note.isInbox || note.relPath === 'inbox.md';
}

function canMutateNote(note: NoteSummary): boolean {
  return !isProtectedInbox(note) && note.relPath !== null;
}

function openNoteFromList(note: NoteSummary, preview: boolean): void {
  void editorTabsStore.openNote(note.relPath ?? note.absPath, {
    preview,
    absPath: note.absPath,
  });
}

function extractBaseNameWithoutExt(relPath: string): string {
  const parts = relPath.split('/');
  const filename = parts[parts.length - 1] ?? relPath;
  return filename.endsWith('.md') ? filename.slice(0, -3) : filename;
}

async function startRename(note: NoteSummary): Promise<void> {
  if (!canMutateNote(note) || note.relPath === null) {
    return;
  }
  renamingRelPath.value = note.relPath;
  renameDraft.value = extractBaseNameWithoutExt(note.relPath);
  await nextTick();
  renameInputRef.value?.focus();
  renameInputRef.value?.select();
}

function cancelRename(): void {
  renamingRelPath.value = null;
  renameDraft.value = '';
}

async function commitRename(oldRelPath: string | null): Promise<void> {
  if (!oldRelPath || renamingRelPath.value !== oldRelPath) {
    return;
  }
  const draft = renameDraft.value.trim();
  renamingRelPath.value = null;
  renameDraft.value = '';
  if (!draft) {
    return;
  }
  await notesStore.renameNoteItem(oldRelPath, draft);
}

function handleDeleteNote(relPath: string | null): void {
  if (!relPath) {
    return;
  }
  void notesStore.deleteNoteItem(relPath);
}

async function handleSelectTab(tab: NavTab): Promise<void> {
  await notesStore.setActiveTab(tab);
  if (tab === 'inbox') {
    await editorTabsStore.openNote('inbox.md', { preview: true });
  }
}
</script>

<template>
  <aside
    v-if="props.collapsed"
    class="qn-notes-sidebar-strip"
    data-testid="notes-sidebar-collapsed-strip"
    tabindex="-1"
    @keydown="onSidebarKeydown"
  >
    <button
      type="button"
      class="qn-notes-sidebar-strip__btn"
      title="展开左侧导航"
      data-testid="notes-sidebar-expand-btn"
      @click="emit('toggle-collapse')"
    >
      ›
    </button>
    <div class="qn-notes-sidebar-strip__divider" />
    <button
      v-for="item in navItems"
      :key="item.id"
      type="button"
      :class="[
        'qn-notes-sidebar-strip__btn',
        { 'qn-notes-sidebar-strip__btn--active': notesStore.activeTab === item.id },
      ]"
      :title="item.label"
      :data-testid="`notes-sidebar-strip-${item.id}`"
      @click="
        emit('toggle-collapse');
        handleSelectTab(item.id);
      "
    >
      {{ item.label.slice(0, 1) }}
    </button>
  </aside>

  <aside
    v-else
    :class="['qn-notes-sidebar', { 'qn-notes-sidebar--drawer': props.overlayDrawer }]"
    data-testid="notes-sidebar"
    tabindex="-1"
    @keydown="onSidebarKeydown"
  >
    <div class="qn-notes-sidebar__nav-tabs" role="tablist" aria-label="导航分区">
      <button
        v-for="item in navItems"
        :key="item.id"
        type="button"
        role="tab"
        :aria-selected="notesStore.activeTab === item.id"
        :data-testid="`nav-tab-${item.id}`"
        :class="[
          'qn-notes-sidebar__nav-btn',
          { 'qn-notes-sidebar__nav-btn--active': notesStore.activeTab === item.id },
        ]"
        @click="handleSelectTab(item.id)"
      >
        <span>{{ item.label }}</span>
        <QnBadge v-if="item.id === 'inbox'" tone="neutral" data-testid="inbox-count-badge">
          {{ notesStore.navTree.inboxCount }}
        </QnBadge>
      </button>
      <button
        type="button"
        class="qn-notes-sidebar__collapse-btn"
        title="收起左侧导航"
        data-testid="notes-sidebar-collapse-btn"
        @click="emit('toggle-collapse')"
      >
        ‹
      </button>
    </div>

    <div
      v-if="notesStore.activeTab === 'projects'"
      class="qn-notes-sidebar__subpanel"
      data-testid="nav-projects-tree"
    >
      <div class="qn-notes-sidebar__subpanel-header">
        <span>项目目录</span>
        <button
          v-if="notesStore.selectedDir"
          type="button"
          class="qn-notes-sidebar__link-btn"
          @click="notesStore.selectDir(null)"
        >
          全部项目
        </button>
      </div>
      <div v-if="flatDirs.length === 0" class="qn-notes-sidebar__empty">暂无子目录</div>
      <div v-else class="qn-notes-sidebar__tree">
        <div
          v-for="entry in flatDirs"
          :key="entry.node.relPath"
          :style="{ paddingLeft: `calc(var(--qn-space-2) + ${entry.depth} * var(--qn-space-3))` }"
          :class="[
            'qn-notes-sidebar__tree-item',
            {
              'qn-notes-sidebar__tree-item--active': notesStore.selectedDir === entry.node.relPath,
            },
          ]"
          :data-testid="`dir-node-${entry.node.relPath}`"
          @click="notesStore.selectDir(entry.node.relPath)"
        >
          <button
            v-if="entry.hasChildren"
            type="button"
            class="qn-notes-sidebar__expand-btn"
            :aria-label="entry.expanded ? '折叠目录' : '展开目录'"
            @click.stop="toggleDirExpand(entry.node.relPath)"
          >
            {{ entry.expanded ? '▾' : '▸' }}
          </button>
          <span v-else class="qn-notes-sidebar__expand-spacer" />
          <span class="qn-notes-sidebar__tree-label">{{ entry.node.name }}</span>
          <span class="qn-notes-sidebar__tree-count">{{ entry.node.count }}</span>
        </div>
      </div>
    </div>

    <div
      v-else-if="notesStore.activeTab === 'tags'"
      class="qn-notes-sidebar__subpanel"
      data-testid="nav-tags-tree"
    >
      <div class="qn-notes-sidebar__subpanel-header">
        <span>标签树（多选 AND）</span>
        <button
          v-if="notesStore.selectedTags.length > 0"
          type="button"
          class="qn-notes-sidebar__link-btn"
          data-testid="clear-tags-btn"
          @click="notesStore.clearTags()"
        >
          清除筛选
        </button>
      </div>
      <div v-if="notesStore.selectedTags.length > 0" class="qn-notes-sidebar__chips">
        <button
          v-for="tag in notesStore.selectedTags"
          :key="tag"
          type="button"
          class="qn-notes-sidebar__chip"
          :data-testid="`selected-tag-chip-${tag}`"
          @click="notesStore.toggleTag(tag)"
        >
          #{{ tag }} ×
        </button>
      </div>
      <div v-if="flatTags.length === 0" class="qn-notes-sidebar__empty">暂无标签</div>
      <div v-else class="qn-notes-sidebar__tree">
        <div
          v-for="entry in flatTags"
          :key="entry.node.tag"
          :style="{ paddingLeft: `calc(var(--qn-space-2) + ${entry.depth} * var(--qn-space-3))` }"
          :class="[
            'qn-notes-sidebar__tree-item',
            {
              'qn-notes-sidebar__tree-item--active': notesStore.selectedTags.includes(
                entry.node.tag,
              ),
            },
          ]"
          :data-testid="`tag-node-${entry.node.tag}`"
          @click="notesStore.toggleTag(entry.node.tag)"
        >
          <button
            v-if="entry.hasChildren"
            type="button"
            class="qn-notes-sidebar__expand-btn"
            :aria-label="entry.expanded ? '折叠标签' : '展开标签'"
            @click.stop="toggleTagExpand(entry.node.tag)"
          >
            {{ entry.expanded ? '▾' : '▸' }}
          </button>
          <span v-else class="qn-notes-sidebar__expand-spacer" />
          <span class="qn-notes-sidebar__tree-label">#{{ entry.node.tag }}</span>
          <span class="qn-notes-sidebar__tree-count">{{ entry.node.count }}</span>
        </div>
      </div>
    </div>

    <div
      v-else-if="notesStore.activeTab === 'workspaces'"
      class="qn-notes-sidebar__subpanel"
      data-testid="nav-workspaces-list"
    >
      <div class="qn-notes-sidebar__subpanel-header">
        <span>工作区</span>
        <button
          v-if="notesStore.selectedWorkspace"
          type="button"
          class="qn-notes-sidebar__link-btn"
          @click="notesStore.selectWorkspace(null)"
        >
          清除锁定
        </button>
      </div>
      <div v-if="notesStore.navTree.workspaces.length === 0" class="qn-notes-sidebar__empty">
        暂无工作区
      </div>
      <div v-else class="qn-notes-sidebar__tree">
        <div
          v-for="ws in notesStore.navTree.workspaces"
          :key="ws.key"
          :class="[
            'qn-notes-sidebar__tree-item',
            { 'qn-notes-sidebar__tree-item--active': notesStore.selectedWorkspace === ws.key },
          ]"
          :data-testid="`workspace-node-${ws.key}`"
          @click="notesStore.selectWorkspace(ws.key)"
        >
          <span class="qn-notes-sidebar__tree-label">{{ ws.name }}</span>
          <span class="qn-notes-sidebar__tree-count">{{ ws.key }}</span>
        </div>
      </div>
    </div>

    <div class="qn-notes-sidebar__toolbar">
      <input
        ref="filterInputRef"
        type="search"
        :value="notesStore.filterText"
        placeholder="过滤标题或标签… (⌘F)"
        class="qn-notes-sidebar__filter-input"
        data-testid="notes-filter-input"
        @input="onFilterInput"
      />
      <QnButton
        variant="primary"
        size="sm"
        :loading="notesStore.mutating"
        data-testid="notes-create-btn"
        @click="notesStore.openNewNoteModal()"
      >
        + 新建
      </QnButton>
    </div>

    <QnBanner v-if="notesStore.errorMessage" tone="danger" data-testid="notes-error-banner">
      {{ notesStore.errorMessage }}
    </QnBanner>

    <div
      v-if="notesStore.pinnedExternalFiles.length > 0"
      class="qn-notes-sidebar__pinned-external"
      data-testid="notes-pinned-external"
    >
      <button
        type="button"
        class="qn-notes-sidebar__pinned-header"
        data-testid="notes-pinned-external-toggle"
        @click="pinnedExternalExpanded = !pinnedExternalExpanded"
      >
        <span>{{ pinnedExternalExpanded ? '▾' : '▸' }} 常用外部文件</span>
        <QnBadge tone="accent" size="sm">{{ notesStore.pinnedExternalFiles.length }}</QnBadge>
      </button>

      <div v-if="pinnedExternalExpanded" class="qn-notes-sidebar__pinned-list">
        <div
          v-for="item in notesStore.pinnedExternalFiles"
          :key="item.absPath"
          :class="[
            'qn-notes-sidebar__pinned-item',
            {
              'qn-notes-sidebar__pinned-item--active':
                editorTabsStore.activeTab?.absPath === item.absPath,
            },
          ]"
          :title="item.absPath"
          :data-testid="`pinned-external-item-${item.absPath}`"
          @click="editorTabsStore.openNoteByAbsPath(item.absPath, { preview: true })"
          @dblclick="editorTabsStore.openNoteByAbsPath(item.absPath, { preview: false })"
          @contextmenu.prevent="openExternalContextMenu($event, item)"
        >
          <span class="qn-notes-sidebar__pinned-title">{{ item.title }}</span>
          <button
            type="button"
            class="qn-notes-sidebar__pinned-unpin"
            title="取消固定"
            :aria-label="`取消固定 ${item.title}`"
            :data-testid="`pinned-external-unpin-${item.absPath}`"
            @click.stop="notesStore.togglePinExternalFile(item.absPath)"
          >
            ×
          </button>
        </div>
      </div>

      <QnContextMenu
        :open="externalCtxOpen"
        :x="externalCtxPos.x"
        :y="externalCtxPos.y"
        :items="externalCtxMenuItems"
        @select="handleExternalContextSelect"
        @close="externalCtxOpen = false"
      />
    </div>

    <div class="qn-notes-sidebar__list" data-testid="notes-list">
      <div v-if="notesStore.notes.length === 0" class="qn-notes-sidebar__empty">
        {{ notesStore.loadingList ? '正在加载笔记…' : '当前筛选下暂无笔记' }}
      </div>
      <div
        v-for="note in notesStore.notes"
        :key="note.key"
        :class="[
          'qn-notes-sidebar__card',
          {
            'qn-notes-sidebar__card--active': editorTabsStore.activeTab?.relPath === note.relPath,
          },
        ]"
        :data-testid="`note-card-${note.relPath ?? note.absPath}`"
        @click="openNoteFromList(note, true)"
        @dblclick="openNoteFromList(note, false)"
      >
        <div class="qn-notes-sidebar__card-header">
          <input
            v-if="note.relPath && renamingRelPath === note.relPath"
            ref="renameInputRef"
            v-model="renameDraft"
            type="text"
            class="qn-notes-sidebar__rename-input"
            data-testid="note-rename-input"
            @click.stop
            @dblclick.stop
            @keydown.enter.prevent="commitRename(note.relPath)"
            @keydown.escape.prevent="cancelRename"
            @blur="commitRename(note.relPath)"
          />
          <span v-else class="qn-notes-sidebar__card-title">
            <span v-if="note.pinned" class="qn-notes-sidebar__pin" title="已置顶">★</span>
            {{ note.title || note.relPath || note.absPath }}
          </span>
          <QnBadge v-if="note.metaError" tone="warning" data-testid="note-meta-error-badge">
            Meta异常
          </QnBadge>
        </div>

        <p v-if="note.excerpt" class="qn-notes-sidebar__card-excerpt">
          {{ note.excerpt }}
        </p>

        <div class="qn-notes-sidebar__card-footer">
          <div class="qn-notes-sidebar__card-tags">
            <span v-for="tag in note.tags.slice(0, 3)" :key="tag" class="qn-notes-sidebar__tag">
              #{{ tag }}
            </span>
          </div>
          <div class="qn-notes-sidebar__card-meta">
            <span class="qn-notes-sidebar__card-time">{{ formatMtime(note.mtimeMs) }}</span>
            <template v-if="canMutateNote(note)">
              <button
                type="button"
                class="qn-notes-sidebar__card-action"
                :data-testid="`note-rename-btn-${note.relPath}`"
                @click.stop="startRename(note)"
              >
                重命名
              </button>
              <button
                type="button"
                class="qn-notes-sidebar__card-action qn-notes-sidebar__card-action--danger"
                :data-testid="`note-delete-btn-${note.relPath}`"
                @click.stop="handleDeleteNote(note.relPath)"
              >
                删除
              </button>
            </template>
          </div>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.qn-notes-sidebar-strip {
  width: 36px;
  flex-shrink: 0;
  border-right: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2) 0;
  min-height: 0;
  outline: none;
}

.qn-notes-sidebar-strip__divider {
  width: 20px;
  height: 1px;
  background-color: var(--qn-border-subtle);
  margin: 2px 0;
}

.qn-notes-sidebar-strip__btn {
  width: 26px;
  height: 26px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-weight: 600;
  cursor: pointer;
}

.qn-notes-sidebar-strip__btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-notes-sidebar-strip__btn--active {
  background-color: var(--qn-bg-active);
  color: var(--qn-accent);
}

.qn-notes-sidebar {
  width: 300px;
  flex-shrink: 0;
  border-right: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  display: flex;
  flex-direction: column;
  padding: var(--qn-space-3);
  gap: var(--qn-space-2);
  min-height: 0;
  outline: none;
}

.qn-notes-sidebar--drawer {
  position: absolute;
  top: 0;
  left: 0;
  bottom: 0;
  z-index: var(--qn-z-drawer);
  box-shadow: var(--qn-shadow-lg);
}

.qn-notes-sidebar__nav-tabs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-notes-sidebar__collapse-btn {
  margin-left: auto;
  width: 22px;
  height: 22px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-muted);
  cursor: pointer;
}

.qn-notes-sidebar__collapse-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-notes-sidebar__nav-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--qn-space-1);
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-weight: 500;
  cursor: pointer;
}

.qn-notes-sidebar__nav-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-notes-sidebar__nav-btn--active {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

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

.qn-notes-sidebar__tree {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.qn-notes-sidebar__tree-item {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  cursor: pointer;
}

.qn-notes-sidebar__tree-item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-notes-sidebar__tree-item--active {
  background-color: var(--qn-accent-soft);
  color: var(--qn-accent);
}

.qn-notes-sidebar__expand-btn {
  width: 14px;
  height: 14px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  cursor: pointer;
  flex-shrink: 0;
}

.qn-notes-sidebar__expand-spacer {
  width: 14px;
  flex-shrink: 0;
}

.qn-notes-sidebar__tree-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-notes-sidebar__tree-count {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-notes-sidebar__toolbar {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-notes-sidebar__filter-input {
  flex: 1;
  min-width: 0;
  height: 28px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
}

.qn-notes-sidebar__filter-input:focus {
  outline: none;
  border-color: var(--qn-accent);
}

.qn-notes-sidebar__list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-notes-sidebar__empty {
  padding: var(--qn-space-3);
  text-align: center;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-notes-sidebar__card {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  cursor: pointer;
  transition:
    border-color 0.12s ease,
    background-color 0.12s ease;
}

.qn-notes-sidebar__card:hover {
  background-color: var(--qn-bg-hover);
}

.qn-notes-sidebar__card--active {
  border-color: var(--qn-accent);
  background-color: var(--qn-accent-soft);
}

.qn-notes-sidebar__card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-1);
}

.qn-notes-sidebar__card-title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-notes-sidebar__pin {
  color: var(--qn-warning);
  margin-right: var(--qn-space-1);
}

.qn-notes-sidebar__rename-input {
  width: 100%;
  height: 24px;
  padding: 0 var(--qn-space-1);
  border: 1px solid var(--qn-accent);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
}

.qn-notes-sidebar__card-excerpt {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.qn-notes-sidebar__card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-1);
}

.qn-notes-sidebar__card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--qn-space-1);
  min-width: 0;
}

.qn-notes-sidebar__tag {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-notes-sidebar__card-meta {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  flex-shrink: 0;
}

.qn-notes-sidebar__card-time {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-notes-sidebar__card-action {
  padding: 0 var(--qn-space-1);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-notes-sidebar__card-action:hover {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-notes-sidebar__card-action--danger:hover {
  color: var(--qn-danger);
}

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
