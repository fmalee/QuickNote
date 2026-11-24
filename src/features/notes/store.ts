import { ref } from 'vue';
import { defineStore } from 'pinia';
import type {
  FiltersUiState,
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  NotesFilterArgs,
  SearchResultItem,
} from '@/bindings/types';
import { useEditorTabsStore } from '@/features/editor';
import type { UnlistenFn } from '@/lib/ipc/events';
import {
  batchRenameTag,
  createNote,
  deleteNote,
  extractNotesErrorMessage,
  fetchNavTreeAndTags,
  fetchNotesList,
  fetchPinnedExternalFiles,
  fetchUiState,
  listNoteTemplates,
  patchUiState,
  renameNote,
  revealExternalFileByPath,
  subscribeIndexStatus,
  togglePinExternalFileByPath,
} from './api';

export type NavTab = 'inbox' | 'projects' | 'tags' | 'recent' | 'workspaces';

export const FILTER_DEBOUNCE_MS = 80;
const FILTERS_PERSIST_DEBOUNCE_MS = 250;

function normalizeNavTab(tab: string | undefined): NavTab {
  if (
    tab === 'inbox' ||
    tab === 'projects' ||
    tab === 'tags' ||
    tab === 'recent' ||
    tab === 'workspaces'
  ) {
    return tab;
  }
  return 'inbox';
}

function buildRenamedRelPath(oldRelPath: string, newNameInput: string): string {
  const trimmed = newNameInput.trim();
  const baseName = trimmed.endsWith('.md') ? trimmed : `${trimmed}.md`;
  const lastSlash = oldRelPath.lastIndexOf('/');
  if (lastSlash < 0) {
    return baseName;
  }
  const prefix = oldRelPath.slice(0, lastSlash + 1);
  return `${prefix}${baseName}`;
}

export const useNotesStore = defineStore('notes', () => {
  const editorTabsStore = useEditorTabsStore();

  const activeTab = ref<NavTab>('inbox');
  const selectedDir = ref<string | null>(null);
  const selectedTags = ref<string[]>([]);
  const selectedWorkspace = ref<string | null>(null);
  const filterText = ref<string>('');
  const listScrollTop = ref<number>(0);

  const navTree = ref<NavTreeSnapshot>({
    inboxCount: 0,
    dirs: [],
    tags: [],
    workspaces: [],
  });
  const notes = ref<NoteSummary[]>([]);
  const loadingTree = ref<boolean>(false);
  const loadingList = ref<boolean>(false);
  const mutating = ref<boolean>(false);
  const errorMessage = ref<string | null>(null);
  const newNoteModalOpen = ref<boolean>(false);
  const newNoteDefaultDir = ref<string>('');
  const templates = ref<NoteSummary[]>([]);
  const pinnedExternalFiles = ref<SearchResultItem[]>([]);

  let requestSeq = 0;
  let pinnedExternalSeq = 0;
  let filterDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let filtersPersistTimer: ReturnType<typeof setTimeout> | null = null;
  let unlistenIndexStatus: UnlistenFn | null = null;

  function buildFilterArgs(): NotesFilterArgs {
    const trimmedFilter = filterText.value.trim();
    return {
      tab: activeTab.value,
      dirRelPath: activeTab.value === 'projects' ? selectedDir.value : null,
      tags: [...selectedTags.value],
      workspaceKey: activeTab.value === 'workspaces' ? selectedWorkspace.value : null,
      filterText: trimmedFilter ? trimmedFilter : null,
    };
  }

  function persistFiltersDebounced(): void {
    if (filtersPersistTimer !== null) {
      clearTimeout(filtersPersistTimer);
    }
    filtersPersistTimer = setTimeout(() => {
      filtersPersistTimer = null;
      const payload: FiltersUiState = {
        activeTab: activeTab.value,
        selectedDir: selectedDir.value,
        selectedTags: [...selectedTags.value],
        listScrollTop: listScrollTop.value,
      };
      void patchUiState('filters', payload).catch(() => undefined);
    }, FILTERS_PERSIST_DEBOUNCE_MS);
  }

  async function loadTreeAndTags(): Promise<void> {
    loadingTree.value = true;
    try {
      navTree.value = await fetchNavTreeAndTags();
    } catch (error) {
      errorMessage.value = extractNotesErrorMessage(error);
    } finally {
      loadingTree.value = false;
    }
  }

  async function loadPinnedExternalFiles(): Promise<void> {
    const seq = ++pinnedExternalSeq;
    try {
      const items = await fetchPinnedExternalFiles();
      if (seq === pinnedExternalSeq) {
        pinnedExternalFiles.value = items;
      }
    } catch {
      // 容错：未配置或测试环境未 mock 时保持现有列表
    }
  }

  async function togglePinExternalFile(absPath: string): Promise<void> {
    await togglePinExternalFileByPath(absPath);
    await loadPinnedExternalFiles();
  }

  async function revealExternalFile(absPath: string): Promise<void> {
    await revealExternalFileByPath(absPath);
  }

  async function loadNotesList(): Promise<void> {
    const seq = ++requestSeq;
    loadingList.value = true;
    errorMessage.value = null;
    try {
      const items = await fetchNotesList(buildFilterArgs());
      if (seq !== requestSeq) {
        return;
      }
      notes.value = items;
    } catch (error) {
      if (seq !== requestSeq) {
        return;
      }
      errorMessage.value = extractNotesErrorMessage(error);
    } finally {
      if (seq === requestSeq) {
        loadingList.value = false;
      }
    }
  }

  function setFilterText(nextText: string): void {
    filterText.value = nextText;
    if (filterDebounceTimer !== null) {
      clearTimeout(filterDebounceTimer);
    }
    filterDebounceTimer = setTimeout(() => {
      filterDebounceTimer = null;
      void loadNotesList();
    }, FILTER_DEBOUNCE_MS);
  }

  async function setActiveTab(tab: NavTab): Promise<void> {
    activeTab.value = tab;
    persistFiltersDebounced();
    await loadNotesList();
  }

  async function selectDir(dirRelPath: string | null): Promise<void> {
    selectedDir.value = dirRelPath === null || selectedDir.value === dirRelPath ? null : dirRelPath;
    persistFiltersDebounced();
    await loadNotesList();
  }

  async function toggleTag(tag: string): Promise<void> {
    if (selectedTags.value.includes(tag)) {
      selectedTags.value = selectedTags.value.filter((item) => item !== tag);
    } else {
      selectedTags.value = [...selectedTags.value, tag];
    }
    persistFiltersDebounced();
    await loadNotesList();
  }

  async function clearTags(): Promise<void> {
    if (selectedTags.value.length === 0) {
      return;
    }
    selectedTags.value = [];
    persistFiltersDebounced();
    await loadNotesList();
  }

  async function selectWorkspace(workspaceKey: string | null): Promise<void> {
    const next =
      workspaceKey === null || selectedWorkspace.value === workspaceKey ? null : workspaceKey;
    selectedWorkspace.value = next;
    void patchUiState('selectedWorkspace', next).catch(() => undefined);
    await loadNotesList();
  }

  async function loadTemplates(): Promise<NoteSummary[]> {
    try {
      const items = await listNoteTemplates();
      templates.value = items;
      return items;
    } catch {
      return templates.value;
    }
  }

  function resolveContextDirRelPath(): string {
    if (activeTab.value === 'projects' && selectedDir.value) {
      return selectedDir.value;
    }
    const activeRelPath = editorTabsStore.activeDoc?.relPath;
    if (activeRelPath && !activeRelPath.startsWith('_templates/')) {
      const lastSlash = activeRelPath.lastIndexOf('/');
      if (lastSlash > 0) {
        return activeRelPath.slice(0, lastSlash);
      }
    }
    return '';
  }

  async function openNewNoteModal(defaultDirRelPath?: string | null): Promise<void> {
    newNoteDefaultDir.value =
      defaultDirRelPath !== undefined ? (defaultDirRelPath ?? '') : resolveContextDirRelPath();
    newNoteModalOpen.value = true;
    await loadTemplates();
  }

  function closeNewNoteModal(): void {
    newNoteModalOpen.value = false;
  }

  async function createNoteAndOpen(args: {
    title?: string;
    dirRelPath?: string;
    templateName?: string | null;
    body?: string | null;
  }): Promise<NoteDoc | null> {
    mutating.value = true;
    errorMessage.value = null;
    try {
      const dirRelPath =
        args.dirRelPath !== undefined
          ? args.dirRelPath.trim()
          : activeTab.value === 'projects' && selectedDir.value
            ? selectedDir.value
            : '';
      const doc = await createNote({
        title: args.title?.trim() || '未命名笔记',
        dirRelPath,
        templateName: args.templateName ?? null,
        body: args.body ?? null,
      });
      await editorTabsStore.openNote(doc.relPath ?? doc.absPath, {
        preview: false,
        absPath: doc.absPath,
      });
      await Promise.all([loadTreeAndTags(), loadNotesList()]);
      return doc;
    } catch (error) {
      errorMessage.value = extractNotesErrorMessage(error);
      return null;
    } finally {
      mutating.value = false;
    }
  }

  async function createNewNote(title = '未命名笔记'): Promise<NoteDoc | null> {
    return createNoteAndOpen({ title });
  }

  async function renameNoteItem(oldRelPath: string, newName: string): Promise<NoteDoc | null> {
    if (oldRelPath === 'inbox.md') {
      errorMessage.value = '收件箱 inbox.md 不可重命名。';
      return null;
    }
    const trimmed = newName.trim();
    if (!trimmed) {
      errorMessage.value = '文件名不能为空。';
      return null;
    }

    const newRelPath = buildRenamedRelPath(oldRelPath, trimmed);
    if (newRelPath === oldRelPath) {
      return null;
    }

    mutating.value = true;
    errorMessage.value = null;
    try {
      const updated = await renameNote({
        relPath: oldRelPath,
        newRelPath,
      });
      editorTabsStore.handleNoteRenamed(oldRelPath, updated);
      await Promise.all([loadTreeAndTags(), loadNotesList()]);
      return updated;
    } catch (error) {
      errorMessage.value = extractNotesErrorMessage(error);
      return null;
    } finally {
      mutating.value = false;
    }
  }

  async function deleteNoteItem(relPath: string): Promise<boolean> {
    if (relPath === 'inbox.md') {
      errorMessage.value = '收件箱 inbox.md 不可删除。';
      return false;
    }

    mutating.value = true;
    errorMessage.value = null;
    try {
      await deleteNote({ relPath });
      editorTabsStore.handleNoteDeleted(relPath);
      await Promise.all([loadTreeAndTags(), loadNotesList()]);
      return true;
    } catch (error) {
      errorMessage.value = extractNotesErrorMessage(error);
      return false;
    } finally {
      mutating.value = false;
    }
  }

  async function renameOrMergeTag(oldTagInput: string, newTagInput: string): Promise<number> {
    const oldTag = oldTagInput.trim().replace(/^#+/, '');
    const newTag = newTagInput.trim().replace(/^#+/, '');
    if (!oldTag || !newTag) {
      errorMessage.value = '标签名称不能为空。';
      return 0;
    }
    if (oldTag === newTag) {
      errorMessage.value = '新标签名称不能与原标签相同。';
      return 0;
    }

    mutating.value = true;
    errorMessage.value = null;
    try {
      const result = await batchRenameTag(oldTag, newTag);
      if (selectedTags.value.length > 0) {
        const replaced = selectedTags.value.map((tag) => {
          if (tag === oldTag) {
            return newTag;
          }
          if (tag.startsWith(`${oldTag}/`)) {
            return `${newTag}${tag.slice(oldTag.length)}`;
          }
          return tag;
        });
        selectedTags.value = Array.from(new Set(replaced));
        persistFiltersDebounced();
      }
      await Promise.all([loadTreeAndTags(), loadNotesList()]);
      await editorTabsStore.refreshOpenTabsMetaFromDisk();
      return result.updatedFiles;
    } catch (error) {
      errorMessage.value = extractNotesErrorMessage(error);
      return 0;
    } finally {
      mutating.value = false;
    }
  }

  async function refreshAll(): Promise<void> {
    await Promise.all([loadTreeAndTags(), loadNotesList(), loadPinnedExternalFiles()]);
  }

  async function initialize(): Promise<void> {
    try {
      const uiState = await fetchUiState();
      activeTab.value = normalizeNavTab(uiState.filters.activeTab);
      selectedDir.value = uiState.filters.selectedDir ?? null;
      selectedTags.value = [...(uiState.filters.selectedTags ?? [])];
      listScrollTop.value = uiState.filters.listScrollTop ?? 0;
      selectedWorkspace.value = uiState.selectedWorkspaceKey ?? null;
    } catch {
      // 容错：读取 ui-state 失败时保留默认筛选状态
    }

    if (!unlistenIndexStatus) {
      try {
        unlistenIndexStatus = await subscribeIndexStatus(() => {
          void Promise.all([loadTreeAndTags(), loadNotesList()]);
        });
      } catch {
        // 非 Tauri 测试环境忽略
      }
    }

    await Promise.all([loadTreeAndTags(), loadNotesList(), loadPinnedExternalFiles()]);
  }

  function dispose(): void {
    if (filterDebounceTimer !== null) {
      clearTimeout(filterDebounceTimer);
      filterDebounceTimer = null;
    }
    if (filtersPersistTimer !== null) {
      clearTimeout(filtersPersistTimer);
      filtersPersistTimer = null;
    }
    if (unlistenIndexStatus) {
      unlistenIndexStatus();
      unlistenIndexStatus = null;
    }
  }

  return {
    activeTab,
    selectedDir,
    selectedTags,
    selectedWorkspace,
    filterText,
    listScrollTop,
    navTree,
    notes,
    loadingTree,
    loadingList,
    mutating,
    errorMessage,
    newNoteModalOpen,
    newNoteDefaultDir,
    templates,
    pinnedExternalFiles,
    loadTreeAndTags,
    loadNotesList,
    loadPinnedExternalFiles,
    togglePinExternalFile,
    revealExternalFile,
    loadTemplates,
    refreshAll,
    setFilterText,
    setActiveTab,
    selectDir,
    toggleTag,
    clearTags,
    selectWorkspace,
    openNewNoteModal,
    closeNewNoteModal,
    createNoteAndOpen,
    createNewNote,
    renameNoteItem,
    deleteNoteItem,
    renameOrMergeTag,
    initialize,
    dispose,
  };
});
