<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type {
  DirNode,
  DirectoryWorkspaceConfig,
  DirectoryWorkspaceConfigEntry,
  NoteSummary,
  SearchResultItem,
  TagNode,
} from '@/bindings/types';
import {
  type ContextMenuEntry,
  QnBadge,
  QnBanner,
  QnButton,
  QnContextMenu,
  QnInput,
  QnModalShell,
} from '@/components';
import { useEditorTabsStore } from '@/features/editor';
import { OutlinePanel, RelatedNotesPanel, usePanelsStore } from '@/features/panels';
import { type ActionContext, actionRegistry } from '@/lib/actions';
import {
  extractNotesErrorMessage,
  fetchDirectoryWorkspaceConfigMap,
  openDirectoryWorkspaceInVscode,
  saveDirectoryWorkspaceConfig,
} from './api';
import { type NavTab, useNotesStore } from './store';

interface FlatDirItem {
  node: DirNode;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
}

interface TagCloudItem {
  tag: string;
  count: number;
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
const panelsStore = usePanelsStore();

const filterInputRef = ref<HTMLInputElement | null>(null);
const expandedDirs = ref<Record<string, boolean>>({});

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

const tagCtxOpen = ref<boolean>(false);
const tagCtxPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const tagCtxTarget = ref<string | null>(null);
const tagCtxMenuItems: ContextMenuEntry[] = [
  { type: 'item', id: 'tag.rename', title: '重命名 / 合并…' },
];

const tagRenameModalOpen = ref<boolean>(false);
const tagRenameOld = ref<string>('');
const tagRenameDraft = ref<string>('');
const tagRenameNotice = ref<string | null>(null);

const FOLDER_CONTEXT_ACTION_IDS = [
  'folder.bindWorkspaceConfig',
  'folder.openWorkspaceInVscode',
] as const;

const dirWorkspaceMap = ref<Record<string, DirectoryWorkspaceConfigEntry>>({});
const dirCtxOpen = ref<boolean>(false);
const dirCtxPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const dirCtxTarget = ref<DirNode | null>(null);

const dirWsModalOpen = ref<boolean>(false);
const dirWsRelDir = ref<string>('');
const dirWsDirName = ref<string>('');
const dirWsVscodePath = ref<string>('');
const dirWsProjectRoot = ref<string>('');
const dirWsTerminalCwd = ref<string>('');
const dirWsSaving = ref<boolean>(false);
const dirWsError = ref<string | null>(null);
const dirWsParseError = ref<string | null>(null);

interface NormalizedDirWorkspaceFields {
  vscodeWorkspacePath: string;
  projectRoot: string;
  defaultTerminalCwd: string;
}

function extractDirWorkspaceFields(
  config: DirectoryWorkspaceConfig | null | undefined,
): NormalizedDirWorkspaceFields {
  return {
    vscodeWorkspacePath: (
      config?.vscodeWorkspacePath ??
      config?.vscode_workspace_path ??
      ''
    ).trim(),
    projectRoot: (config?.projectRoot ?? config?.project_root ?? '').trim(),
    defaultTerminalCwd: (config?.defaultTerminalCwd ?? config?.default_terminal_cwd ?? '').trim(),
  };
}

function hasBoundDirWorkspaceConfig(
  entry: DirectoryWorkspaceConfigEntry | undefined,
): entry is DirectoryWorkspaceConfigEntry {
  if (!entry) {
    return false;
  }
  if (entry.parseError) {
    return true;
  }
  const fields = extractDirWorkspaceFields(entry.config);
  return Boolean(fields.vscodeWorkspacePath || fields.projectRoot || fields.defaultTerminalCwd);
}

function getBoundDirWorkspaceEntry(relPath: string): DirectoryWorkspaceConfigEntry | null {
  const entry = dirWorkspaceMap.value[relPath];
  return hasBoundDirWorkspaceConfig(entry) ? entry : null;
}

function hasDirWorkspaceTarget(relPath: string): boolean {
  const entry = dirWorkspaceMap.value[relPath];
  if (!entry || entry.parseError || !entry.config) {
    return false;
  }
  const fields = extractDirWorkspaceFields(entry.config);
  return Boolean(fields.vscodeWorkspacePath || fields.projectRoot || fields.defaultTerminalCwd);
}

function formatDirWorkspaceTooltip(entry: DirectoryWorkspaceConfigEntry): string {
  if (entry.parseError) {
    return `工作区配置解析异常: ${entry.parseError}`;
  }
  const fields = extractDirWorkspaceFields(entry.config);
  const lines: string[] = [];
  if (fields.vscodeWorkspacePath) {
    lines.push(`VS Code 工作区: ${fields.vscodeWorkspacePath}`);
  }
  if (fields.projectRoot) {
    lines.push(`项目根目录: ${fields.projectRoot}`);
  }
  if (fields.defaultTerminalCwd) {
    lines.push(`默认终端目录: ${fields.defaultTerminalCwd}`);
  }
  return lines.join('\n') || '已绑定目录工作区配置';
}

async function loadDirectoryWorkspaceConfigs(): Promise<void> {
  try {
    dirWorkspaceMap.value = await fetchDirectoryWorkspaceConfigMap();
  } catch {
    // 忽略目录工作区映射加载失败，避免阻断侧栏主渲染
  }
}

function buildDirActionContext(node: DirNode | null): ActionContext {
  return {
    activeScope: 'list',
    payload: node
      ? {
          vaultRelDir: node.relPath,
          dirName: node.name,
          hasWorkspaceTarget: hasDirWorkspaceTarget(node.relPath),
        }
      : {},
  };
}

const dirCtxMenuItems = computed<ContextMenuEntry[]>(() => {
  const ctx = buildDirActionContext(dirCtxTarget.value);
  const entries: ContextMenuEntry[] = [];
  for (const id of FOLDER_CONTEXT_ACTION_IDS) {
    const def = actionRegistry.getById(id);
    if (!def) {
      continue;
    }
    const disabled = def.when ? !def.when(ctx) : false;
    entries.push({
      type: 'item',
      id: def.id,
      title: def.title,
      disabled,
      disabledReason:
        disabled && def.id === 'folder.openWorkspaceInVscode'
          ? '请先为该目录绑定工作区路径或项目根目录'
          : null,
    });
  }
  return entries;
});

const dirWsHasExistingConfig = computed<boolean>(() =>
  Boolean(dirWsRelDir.value && getBoundDirWorkspaceEntry(dirWsRelDir.value)),
);

function openDirContextMenu(event: MouseEvent, node: DirNode): void {
  dirCtxTarget.value = node;
  dirCtxPos.value = { x: event.clientX, y: event.clientY };
  dirCtxOpen.value = true;
}

async function handleDirContextSelect(id: string): Promise<void> {
  const target = dirCtxTarget.value;
  dirCtxOpen.value = false;
  if (!target) {
    return;
  }
  await actionRegistry.executeById(id, buildDirActionContext(target));
}

function openDirWorkspaceModal(vaultRelDir: string, dirName: string): void {
  const entry = dirWorkspaceMap.value[vaultRelDir];
  const fields = extractDirWorkspaceFields(entry?.config);
  dirWsRelDir.value = vaultRelDir;
  dirWsDirName.value = dirName;
  dirWsVscodePath.value = fields.vscodeWorkspacePath;
  dirWsProjectRoot.value = fields.projectRoot;
  dirWsTerminalCwd.value = fields.defaultTerminalCwd;
  dirWsParseError.value = entry?.parseError ?? null;
  dirWsError.value = null;
  dirWsModalOpen.value = true;
}

function closeDirWorkspaceModal(): void {
  if (dirWsSaving.value) {
    return;
  }
  dirWsModalOpen.value = false;
}

async function submitDirWorkspaceConfig(): Promise<void> {
  if (dirWsSaving.value || !dirWsRelDir.value) {
    return;
  }
  const vscodeWorkspacePath = dirWsVscodePath.value.trim();
  const projectRoot = dirWsProjectRoot.value.trim();
  const defaultTerminalCwd = dirWsTerminalCwd.value.trim();
  const payload: DirectoryWorkspaceConfig | null =
    vscodeWorkspacePath || projectRoot || defaultTerminalCwd
      ? {
          vscodeWorkspacePath: vscodeWorkspacePath || null,
          projectRoot: projectRoot || null,
          defaultTerminalCwd: defaultTerminalCwd || null,
        }
      : null;

  dirWsSaving.value = true;
  dirWsError.value = null;
  try {
    await saveDirectoryWorkspaceConfig(dirWsRelDir.value, payload);
    await loadDirectoryWorkspaceConfigs();
    dirWsModalOpen.value = false;
  } catch (error) {
    dirWsError.value = extractNotesErrorMessage(error);
  } finally {
    dirWsSaving.value = false;
  }
}

async function clearDirWorkspaceConfig(): Promise<void> {
  if (dirWsSaving.value || !dirWsRelDir.value) {
    return;
  }
  dirWsSaving.value = true;
  dirWsError.value = null;
  try {
    await saveDirectoryWorkspaceConfig(dirWsRelDir.value, null);
    await loadDirectoryWorkspaceConfigs();
    dirWsModalOpen.value = false;
  } catch (error) {
    dirWsError.value = extractNotesErrorMessage(error);
  } finally {
    dirWsSaving.value = false;
  }
}

async function handleOpenDirWorkspaceInVscode(vaultRelDir: string): Promise<void> {
  const entry = dirWorkspaceMap.value[vaultRelDir];
  if (!entry?.config) {
    return;
  }
  try {
    await openDirectoryWorkspaceInVscode(entry.config);
  } catch (error) {
    notesStore.errorMessage = extractNotesErrorMessage(error);
  }
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

const normalizedTagDraft = computed<string>(() => tagRenameDraft.value.trim().replace(/^#+/, ''));

const isTagMergeTarget = computed<boolean>(() => {
  const target = normalizedTagDraft.value;
  return Boolean(target && target !== tagRenameOld.value && allExistingTags.value.has(target));
});

const canSubmitTagRename = computed<boolean>(() => {
  const target = normalizedTagDraft.value;
  return Boolean(target && target !== tagRenameOld.value && !notesStore.mutating);
});

function openTagContextMenu(event: MouseEvent, tag: string): void {
  tagCtxTarget.value = tag;
  tagCtxPos.value = { x: event.clientX, y: event.clientY };
  tagCtxOpen.value = true;
}

function handleTagContextSelect(id: string): void {
  const target = tagCtxTarget.value;
  tagCtxOpen.value = false;
  if (id === 'tag.rename' && target) {
    openTagRenameModal(target);
  }
}

function openTagRenameModal(tag: string): void {
  tagRenameOld.value = tag;
  tagRenameDraft.value = tag;
  tagRenameModalOpen.value = true;
}

function closeTagRenameModal(): void {
  tagRenameModalOpen.value = false;
}

async function submitTagRename(): Promise<void> {
  if (!canSubmitTagRename.value) {
    return;
  }
  const updatedFiles = await notesStore.renameOrMergeTag(
    tagRenameOld.value,
    normalizedTagDraft.value,
  );
  if (!notesStore.errorMessage) {
    tagRenameModalOpen.value = false;
    tagRenameNotice.value = `已更新 ${updatedFiles} 篇笔记的标签`;
  }
}

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

const noteCtxOpen = ref<boolean>(false);
const noteCtxPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const noteCtxTarget = ref<NoteSummary | null>(null);

const noteCtxMenuItems = computed<ContextMenuEntry[]>(() => {
  const note = noteCtxTarget.value;
  const entries: ContextMenuEntry[] = [
    { type: 'item', id: 'note.open', title: '打开' },
    { type: 'item', id: 'note.openPinned', title: '在新标签打开' },
    { type: 'item', id: 'note.reveal', title: '在访达中显示' },
  ];
  if (note && canMutateNote(note)) {
    entries.push(
      { type: 'separator', id: 'note-sep' },
      { type: 'item', id: 'note.rename', title: '重命名…' },
      { type: 'item', id: 'note.delete', title: '删除（移到废纸篓）', danger: true },
    );
  }
  return entries;
});

function openNoteContextMenu(event: MouseEvent, note: NoteSummary): void {
  noteCtxTarget.value = note;
  noteCtxPos.value = { x: event.clientX, y: event.clientY };
  noteCtxOpen.value = true;
}

async function handleNoteContextSelect(id: string): Promise<void> {
  const note = noteCtxTarget.value;
  noteCtxOpen.value = false;
  if (!note) {
    return;
  }
  if (id === 'note.open') {
    openNoteFromList(note, true);
  } else if (id === 'note.openPinned') {
    openNoteFromList(note, false);
  } else if (id === 'note.reveal') {
    await notesStore.revealExternalFile(note.absPath);
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
  { id: 'related', label: '相关文档' },
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
  flattenTagNodes(notesStore.navTree.tags)
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

function toggleDirExpand(relPath: string): void {
  expandedDirs.value = {
    ...expandedDirs.value,
    [relPath]: !(expandedDirs.value[relPath] ?? true),
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
      <div
        v-if="notesStore.activeTab === 'projects'"
        class="qn-notes-sidebar__subpanel"
        data-testid="nav-projects-tree"
      >
        <div class="qn-notes-sidebar__subpanel-header">
          <span>目录</span>
          <button
            v-if="notesStore.selectedDir"
            type="button"
            class="qn-notes-sidebar__link-btn"
            @click="notesStore.selectDir(null)"
          >
            ← 根目录
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
                'qn-notes-sidebar__tree-item--active':
                  notesStore.selectedDir === entry.node.relPath,
              },
            ]"
            :data-testid="`dir-node-${entry.node.relPath}`"
            @click="notesStore.selectDir(entry.node.relPath)"
            @contextmenu.prevent="openDirContextMenu($event, entry.node)"
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
            <QnBadge
              v-if="getBoundDirWorkspaceEntry(entry.node.relPath)"
              :tone="
                getBoundDirWorkspaceEntry(entry.node.relPath)?.parseError ? 'warning' : 'accent'
              "
              size="sm"
              class="qn-notes-sidebar__ws-badge"
              :title="formatDirWorkspaceTooltip(getBoundDirWorkspaceEntry(entry.node.relPath)!)"
              :data-testid="`dir-ws-badge-${entry.node.relPath}`"
            >
              {{ getBoundDirWorkspaceEntry(entry.node.relPath)?.parseError ? '异常' : 'WS' }}
            </QnBadge>
            <span class="qn-notes-sidebar__tree-count">{{ entry.node.count }}</span>
          </div>
        </div>
        <QnContextMenu
          :open="dirCtxOpen"
          :x="dirCtxPos.x"
          :y="dirCtxPos.y"
          :items="dirCtxMenuItems"
          @select="handleDirContextSelect"
          @close="dirCtxOpen = false"
        />
      </div>

      <div
        v-else-if="notesStore.activeTab === 'tags'"
        class="qn-notes-sidebar__subpanel"
        data-testid="nav-tags-tree"
      >
        <div class="qn-notes-sidebar__subpanel-header">
          <span>标签（多选 AND）</span>
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
        <div v-if="tagRenameNotice" class="qn-notes-sidebar__tag-notice">
          <QnBanner tone="success" data-testid="tag-rename-success-banner">
            <span>{{ tagRenameNotice }}</span>
            <template #actions>
              <QnButton size="sm" variant="ghost" @click="tagRenameNotice = null">关闭</QnButton>
            </template>
          </QnBanner>
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
                'qn-notes-sidebar__tag-cloud-item--active': notesStore.selectedTags.includes(
                  item.tag,
                ),
              },
            ]"
            :title="`${item.tag} · ${item.count}`"
            :data-testid="`tag-cloud-item-${item.tag}`"
            @click="notesStore.toggleTag(item.tag)"
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
              {{ note.title || note.relPath || note.absPath }}
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

  <QnModalShell
    :open="tagRenameModalOpen"
    title="重命名 / 合并标签"
    max-width="sm"
    data-testid="tag-rename-modal"
    @close="closeTagRenameModal"
  >
    <div class="qn-tag-rename-modal__body">
      <div class="qn-tag-rename-modal__row">
        <span class="qn-tag-rename-modal__label">原标签</span>
        <code class="qn-tag-rename-modal__old">#{{ tagRenameOld }}</code>
      </div>
      <div class="qn-tag-rename-modal__row">
        <label class="qn-tag-rename-modal__label" for="qn-tag-rename-input">新标签</label>
        <QnInput
          id="qn-tag-rename-input"
          v-model="tagRenameDraft"
          placeholder="输入新标签名（不含 #）"
          data-testid="tag-rename-input"
          @keydown.enter.prevent="submitTagRename"
        />
      </div>
      <QnBanner v-if="isTagMergeTarget" tone="warning" data-testid="tag-merge-warning">
        目标标签
        <code>#{{ normalizedTagDraft }}</code>
        已存在，确认后将把原标签下的全部笔记<strong>合并</strong>到该标签下（自动去重，不覆盖注释与未知字段）。
      </QnBanner>
    </div>
    <template #footer>
      <QnButton size="sm" variant="ghost" @click="closeTagRenameModal">取消</QnButton>
      <QnButton
        size="sm"
        variant="primary"
        :disabled="!canSubmitTagRename"
        data-testid="tag-rename-submit-btn"
        @click="submitTagRename"
      >
        {{ isTagMergeTarget ? '确认合并' : '确认重命名' }}
      </QnButton>
    </template>
  </QnModalShell>

  <QnModalShell
    :open="dirWsModalOpen"
    :title="`绑定目录工作区配置 · ${dirWsDirName || dirWsRelDir}`"
    max-width="md"
    data-testid="dir-workspace-modal"
    @close="closeDirWorkspaceModal"
  >
    <div class="qn-dir-ws-modal__body">
      <div class="qn-dir-ws-modal__row">
        <span class="qn-dir-ws-modal__label">目标目录</span>
        <code class="qn-dir-ws-modal__path">{{ dirWsRelDir }}</code>
      </div>
      <QnBanner v-if="dirWsParseError" tone="warning" data-testid="dir-workspace-parse-warning">
        当前目录 `.quicknote.yaml` 解析失败（{{ dirWsParseError }}），保存新配置或清除后可恢复正常。
      </QnBanner>
      <div class="qn-dir-ws-modal__row">
        <label class="qn-dir-ws-modal__label" for="qn-dir-ws-vscode-input">
          VS Code 工作区文件（可选）
        </label>
        <QnInput
          id="qn-dir-ws-vscode-input"
          v-model="dirWsVscodePath"
          placeholder="例如：/Users/you/Projects/app.code-workspace"
          data-testid="dir-workspace-vscode-input"
        />
      </div>
      <div class="qn-dir-ws-modal__row">
        <label class="qn-dir-ws-modal__label" for="qn-dir-ws-root-input">
          项目根目录（可选）
        </label>
        <QnInput
          id="qn-dir-ws-root-input"
          v-model="dirWsProjectRoot"
          placeholder="例如：/Users/you/Projects/app"
          data-testid="dir-workspace-root-input"
        />
      </div>
      <div class="qn-dir-ws-modal__row">
        <label class="qn-dir-ws-modal__label" for="qn-dir-ws-cwd-input">
          默认终端目录（可选）
        </label>
        <QnInput
          id="qn-dir-ws-cwd-input"
          v-model="dirWsTerminalCwd"
          placeholder="例如：/Users/you/Projects/app"
          data-testid="dir-workspace-cwd-input"
          @keydown.enter.prevent="submitDirWorkspaceConfig"
        />
      </div>
      <p class="qn-dir-ws-modal__hint">
        保存后将写入该目录下的 <code>.quicknote.yaml</code>；全部留空或点击清除将移除配置文件。
      </p>
      <QnBanner v-if="dirWsError" tone="danger" data-testid="dir-workspace-error-banner">
        {{ dirWsError }}
      </QnBanner>
    </div>
    <template #footer>
      <QnButton
        v-if="dirWsHasExistingConfig"
        size="sm"
        variant="danger"
        :loading="dirWsSaving"
        data-testid="dir-workspace-clear-btn"
        @click="clearDirWorkspaceConfig"
      >
        清除配置
      </QnButton>
      <span class="qn-dir-ws-modal__footer-spacer" />
      <QnButton size="sm" variant="ghost" :disabled="dirWsSaving" @click="closeDirWorkspaceModal">
        取消
      </QnButton>
      <QnButton
        size="sm"
        variant="primary"
        :loading="dirWsSaving"
        data-testid="dir-workspace-save-btn"
        @click="submitDirWorkspaceConfig"
      >
        保存配置
      </QnButton>
    </template>
  </QnModalShell>
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

.qn-notes-sidebar__tag-notice {
  margin-bottom: var(--qn-space-1);
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

.qn-tag-rename-modal__body {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-tag-rename-modal__row {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-tag-rename-modal__label {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-tag-rename-modal__old {
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-sm);
  color: var(--qn-text-primary);
}

.qn-notes-sidebar__ws-badge {
  flex-shrink: 0;
}

.qn-dir-ws-modal__body {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-dir-ws-modal__row {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-dir-ws-modal__label {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-dir-ws-modal__path {
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-primary);
  background-color: var(--qn-bg-sidebar);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
}

.qn-dir-ws-modal__hint {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-dir-ws-modal__footer-spacer {
  flex: 1;
}
</style>
