<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type { ExternalActionState, NoteSummary, TagNode } from '@/bindings/types';
import { type ContextMenuEntry, QnBadge, QnButton, QnContextMenu } from '@/components';
import {
  appendExternalContextMenuItems,
  isExternalActionId,
  useEditorTabsStore,
  useExternalActions,
} from '@/features/editor';
import { OutlinePanel, RelatedNotesPanel, usePanelsStore } from '@/features/panels';
import { actionRegistry } from '@/lib/actions';
import { pushToastNotice } from '@/lib/notifications';
import { deriveFallbackTitleFromPath } from '@/lib/utils/docTitle';
import DirTreeSection from './DirTreeSection.vue';
import DirWorkspaceBindModal from './DirWorkspaceBindModal.vue';
import PinnedExternalFilesSection from './PinnedExternalFilesSection.vue';
import { type NavTab, useNotesStore } from './store';
import TagCloudSection from './TagCloudSection.vue';
import TagRenameModal from './TagRenameModal.vue';
import { useDirectoryWorkspaces } from './useDirectoryWorkspaces';

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
const panelsStore = usePanelsStore();
const externalActions = useExternalActions();

const filterInputRef = ref<HTMLInputElement | null>(null);

const renamingRelPath = ref<string | null>(null);
const renameDraft = ref<string>('');

const tagRenameModalOpen = ref<boolean>(false);
const tagRenameOld = ref<string>('');

const { dirWorkspaceMap, loadDirectoryWorkspaceConfigs, handleOpenDirWorkspaceInVscode } =
  useDirectoryWorkspaces((message) => {
    notesStore.errorMessage = message;
    pushToastNotice({
      tone: 'danger',
      title: '打开工作区失败',
      message,
    });
  });

const dirWsModalOpen = ref<boolean>(false);
const dirWsRelDir = ref<string>('');
const dirWsDirName = ref<string>('');

const dirWsExistingEntry = computed(() =>
  dirWsRelDir.value ? (dirWorkspaceMap.value[dirWsRelDir.value] ?? null) : null,
);

function openDirWorkspaceModal(vaultRelDir: string, dirName: string): void {
  dirWsRelDir.value = vaultRelDir;
  dirWsDirName.value = dirName;
  dirWsModalOpen.value = true;
}

function closeDirWorkspaceModal(): void {
  dirWsModalOpen.value = false;
}

let unbindFolderActions: Array<() => void> = [];

onMounted(() => {
  unbindFolderActions = [
    actionRegistry.bindHandler('folder.bindWorkspaceConfig', (ctx) => {
      const vaultRelDir =
        typeof ctx.payload?.vaultRelDir === 'string' ? ctx.payload.vaultRelDir : '';
      const dirName = typeof ctx.payload?.dirName === 'string' ? ctx.payload.dirName : vaultRelDir;
      if (vaultRelDir) {
        openDirWorkspaceModal(vaultRelDir, dirName);
      }
    }),
    actionRegistry.bindHandler('folder.openWorkspaceInVscode', async (ctx) => {
      const vaultRelDir =
        typeof ctx.payload?.vaultRelDir === 'string' ? ctx.payload.vaultRelDir : '';
      if (vaultRelDir) {
        await handleOpenDirWorkspaceInVscode(vaultRelDir);
      }
    }),
  ];
  void loadDirectoryWorkspaceConfigs();
});

onUnmounted(() => {
  for (const unbind of unbindFolderActions) {
    unbind();
  }
  unbindFolderActions = [];
  if (sidebarResizing) {
    sidebarResizing = false;
    if (typeof window !== 'undefined') {
      window.removeEventListener('pointermove', onSidebarResizeMove);
      window.removeEventListener('pointerup', stopSidebarResize);
      window.removeEventListener('pointercancel', stopSidebarResize);
    }
  }
});

watch(
  () => notesStore.navTree.dirs,
  () => {
    void loadDirectoryWorkspaceConfigs();
  },
);

const allExistingTags = computed<Set<string>>(() => {
  const set = new Set<string>();
  function walk(nodes: TagNode[]): void {
    for (const node of nodes) {
      set.add(node.tag);
      walk(node.children);
    }
  }
  walk(notesStore.navTree.tags);
  return set;
});

function openTagRenameModal(tag: string): void {
  tagRenameOld.value = tag;
  tagRenameModalOpen.value = true;
}

function closeTagRenameModal(): void {
  tagRenameModalOpen.value = false;
}

async function submitTagRename(oldTag: string, newTag: string): Promise<void> {
  const updatedFiles = await notesStore.renameOrMergeTag(oldTag, newTag);
  if (!notesStore.errorMessage) {
    tagRenameModalOpen.value = false;
    pushToastNotice({
      tone: 'success',
      title: '标签已批量更新',
      message: `已更新 ${updatedFiles} 篇笔记的标签`,
    });
  }
}

const renameInputRef = ref<HTMLInputElement | null>(null);

const noteCtxOpen = ref<boolean>(false);
const noteCtxPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const noteCtxTarget = ref<NoteSummary | null>(null);
const noteCtxExternalStates = ref<ExternalActionState[]>([]);
let noteMenuSeq = 0;

const noteCtxMenuItems = computed<ContextMenuEntry[]>(() => {
  const note = noteCtxTarget.value;
  const entries: ContextMenuEntry[] = [
    { type: 'item', id: 'note.open', title: '打开' },
    { type: 'item', id: 'note.preview', title: '预览' },
  ];
  if (note && canMutateNote(note)) {
    entries.push(
      { type: 'separator', id: 'note-sep' },
      { type: 'item', id: 'note.rename', title: '重命名…' },
      { type: 'item', id: 'note.delete', title: '删除', danger: true },
    );
  }
  return appendExternalContextMenuItems(entries, noteCtxExternalStates.value, 'note-ext-sep');
});

async function openNoteContextMenu(event: MouseEvent, note: NoteSummary): Promise<void> {
  const reqSeq = ++noteMenuSeq;
  noteCtxTarget.value = note;
  noteCtxPos.value = { x: event.clientX, y: event.clientY };

  if (
    note.absPath &&
    note.absPath === externalActions.resolvedAbsPath &&
    externalActions.states.length > 0
  ) {
    noteCtxExternalStates.value = externalActions.states;
    noteCtxOpen.value = true;
    return;
  }

  noteCtxExternalStates.value = [];
  noteCtxOpen.value = true;
  try {
    const resolved = await externalActions.resolveForTarget({
      absPath: note.absPath || null,
    });
    if (reqSeq === noteMenuSeq) {
      noteCtxExternalStates.value = resolved;
    }
  } catch {
    if (reqSeq === noteMenuSeq) {
      noteCtxExternalStates.value = [];
    }
  }
}

async function handleNoteContextSelect(id: string): Promise<void> {
  const note = noteCtxTarget.value;
  noteCtxOpen.value = false;
  if (!note) {
    return;
  }
  if (isExternalActionId(id)) {
    await externalActions.executeAction(id, {
      overrideStates: noteCtxExternalStates.value,
      pinned: note.pinned,
    });
    return;
  }
  if (id === 'note.open') {
    openNoteFromList(note, false);
  } else if (id === 'note.preview') {
    openNoteFromList(note, true);
  } else if (id === 'note.rename') {
    await startRename(note);
  } else if (id === 'note.delete') {
    handleDeleteNote(note.relPath);
  }
}

let sidebarResizing = false;
let sidebarResizeStartX = 0;
let sidebarResizeStartWidth = 300;

function onSidebarResizeMove(event: PointerEvent): void {
  if (!sidebarResizing) {
    return;
  }
  const nextWidth = sidebarResizeStartWidth + (event.clientX - sidebarResizeStartX);
  panelsStore.setLeftSidebarWidth(nextWidth);
}

function stopSidebarResize(): void {
  if (!sidebarResizing) {
    return;
  }
  sidebarResizing = false;
  if (typeof window !== 'undefined') {
    window.removeEventListener('pointermove', onSidebarResizeMove);
    window.removeEventListener('pointerup', stopSidebarResize);
    window.removeEventListener('pointercancel', stopSidebarResize);
  }
  panelsStore.commitLeftSidebarWidth();
}

function startSidebarResize(event: PointerEvent): void {
  if (event.button !== 0 || props.overlayDrawer) {
    return;
  }
  event.preventDefault();
  sidebarResizing = true;
  sidebarResizeStartX = event.clientX;
  sidebarResizeStartWidth = panelsStore.leftSidebar.width;
  if (typeof window !== 'undefined') {
    window.addEventListener('pointermove', onSidebarResizeMove);
    window.addEventListener('pointerup', stopSidebarResize);
    window.addEventListener('pointercancel', stopSidebarResize);
  }
}

const navItems: { id: string; label: string }[] = [
  { id: 'projects', label: '目录' },
  { id: 'tags', label: '标签' },
  { id: 'recent', label: '最近' },
  { id: 'related', label: '相关' },
  { id: 'outline', label: '大纲' },
];

const showSidebarToolbar = computed<boolean>(
  () =>
    panelsStore.leftSidebar.activeView === 'workspace' &&
    (notesStore.activeTab === 'projects' ||
      notesStore.activeTab === 'tags' ||
      notesStore.activeTab === 'recent'),
);

const filterPlaceholder = computed<string>(() => {
  if (notesStore.activeTab === 'tags') {
    return '过滤标签文档…';
  }
  if (notesStore.activeTab === 'recent') {
    return '过滤最近文档…';
  }
  return '过滤目录文档…';
});

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
  panelsStore.setLeftView('workspace');
  await notesStore.setActiveTab(tab);
}

function isNavItemActive(id: string): boolean {
  if (id === 'related' || id === 'outline') {
    return panelsStore.leftSidebar.activeView === id;
  }
  return panelsStore.leftSidebar.activeView === 'workspace' && notesStore.activeTab === id;
}

function handleNavItemClick(id: string): void {
  if (id === 'related' || id === 'outline') {
    panelsStore.toggleLeftView(id);
    return;
  }
  void handleSelectTab(id as NavTab);
}

function handleCollapsedStripClick(id: string): void {
  emit('toggle-collapse');
  if (id === 'related' || id === 'outline') {
    panelsStore.setLeftView(id);
    return;
  }
  void handleSelectTab(id as NavTab);
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
        { 'qn-notes-sidebar-strip__btn--active': isNavItemActive(item.id) },
      ]"
      :title="item.label"
      :data-testid="`notes-sidebar-strip-${item.id}`"
      @click="handleCollapsedStripClick(item.id)"
    >
      {{ item.label.slice(0, 1) }}
    </button>
  </aside>

  <aside
    v-else
    :class="['qn-notes-sidebar', { 'qn-notes-sidebar--drawer': props.overlayDrawer }]"
    :style="{ width: `${panelsStore.leftSidebarWidth}px` }"
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
        :aria-selected="isNavItemActive(item.id)"
        :data-testid="`nav-tab-${item.id}`"
        :class="[
          'qn-notes-sidebar__nav-btn',
          { 'qn-notes-sidebar__nav-btn--active': isNavItemActive(item.id) },
        ]"
        @click="handleNavItemClick(item.id)"
      >
        <span>{{ item.label }}</span>
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

    <div v-if="showSidebarToolbar" class="qn-notes-sidebar__toolbar">
      <input
        ref="filterInputRef"
        type="search"
        :value="notesStore.filterText"
        :placeholder="filterPlaceholder"
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

    <div
      v-show="panelsStore.leftSidebar.activeView === 'workspace'"
      class="qn-notes-sidebar__workspace"
    >
      <DirTreeSection
        v-if="notesStore.activeTab === 'projects'"
        :dirs="notesStore.navTree.dirs"
        :selected-dir="notesStore.selectedDir"
        :dir-workspace-map="dirWorkspaceMap"
        @select-dir="notesStore.selectDir($event)"
      />

      <TagCloudSection
        v-else-if="notesStore.activeTab === 'tags'"
        :tags="notesStore.navTree.tags"
        :selected-tags="notesStore.selectedTags"
        @toggle-tag="notesStore.toggleTag($event)"
        @clear-tags="notesStore.clearTags()"
        @request-rename="openTagRenameModal($event)"
      />

      <PinnedExternalFilesSection
        :items="notesStore.pinnedExternalFiles"
        :active-abs-path="editorTabsStore.activeTab?.absPath"
        @open="(absPath, preview) => editorTabsStore.openNoteByAbsPath(absPath, { preview })"
        @reveal="(absPath) => notesStore.revealExternalFile(absPath)"
        @unpin="(absPath) => notesStore.togglePinExternalFile(absPath)"
      />

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
          @contextmenu.prevent="openNoteContextMenu($event, note)"
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
              {{ note.title || deriveFallbackTitleFromPath(note.relPath || note.absPath) }}
            </span>
            <QnBadge v-if="note.metaError" tone="warning" data-testid="note-meta-error-badge">
              Meta异常
            </QnBadge>
          </div>

          <p v-if="note.excerpt" class="qn-notes-sidebar__card-excerpt" :title="note.excerpt">
            {{ note.excerpt }}
          </p>

          <div class="qn-notes-sidebar__card-footer">
            <div class="qn-notes-sidebar__card-tags">
              <span v-for="tag in note.tags.slice(0, 4)" :key="tag" class="qn-notes-sidebar__tag">
                #{{ tag }}
              </span>
            </div>
            <div class="qn-notes-sidebar__card-meta">
              <span class="qn-notes-sidebar__card-time">{{ formatMtime(note.mtimeMs) }}</span>
            </div>
          </div>
        </div>
      </div>

      <QnContextMenu
        :open="noteCtxOpen"
        :x="noteCtxPos.x"
        :y="noteCtxPos.y"
        :items="noteCtxMenuItems"
        @select="handleNoteContextSelect"
        @close="noteCtxOpen = false"
      />
    </div>

    <RelatedNotesPanel
      v-if="panelsStore.leftSidebar.activeView === 'related'"
      data-testid="left-view-related"
    />
    <OutlinePanel
      v-else-if="panelsStore.leftSidebar.activeView === 'outline'"
      data-testid="left-view-outline"
    />

    <div
      v-if="!props.overlayDrawer"
      class="qn-notes-sidebar__resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label="调整左侧栏宽度"
      data-testid="left-sidebar-resizer"
      @pointerdown="startSidebarResize"
    />
  </aside>

  <TagRenameModal
    :open="tagRenameModalOpen"
    :old-tag="tagRenameOld"
    :existing-tags="allExistingTags"
    :mutating="notesStore.mutating"
    @close="closeTagRenameModal"
    @submit="submitTagRename"
  />

  <DirWorkspaceBindModal
    :open="dirWsModalOpen"
    :vault-rel-dir="dirWsRelDir"
    :dir-name="dirWsDirName"
    :existing-entry="dirWsExistingEntry"
    @close="closeDirWorkspaceModal"
    @saved="loadDirectoryWorkspaceConfigs"
  />
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

.qn-notes-sidebar__workspace {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
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
  position: relative;
  flex: 0 0 auto;
  min-width: 0;
  border-right: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  display: flex;
  flex-direction: column;
  padding: var(--qn-space-3);
  gap: var(--qn-space-2);
  min-height: 0;
  outline: none;
}

.qn-notes-sidebar__resizer {
  position: absolute;
  top: 0;
  right: -3px;
  bottom: 0;
  width: 6px;
  cursor: col-resize;
}

.qn-notes-sidebar__resizer:hover {
  background-color: var(--qn-accent-soft);
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
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.qn-notes-sidebar__card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-1);
}

.qn-notes-sidebar__card-tags {
  flex: 1;
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
  white-space: nowrap;
}
</style>
