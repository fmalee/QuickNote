import { computed, ref, watch } from 'vue';
import { defineStore } from 'pinia';
import type {
  AppendResult,
  CaptureMode,
  HideReason,
  LibraryInfo,
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  QuickBarMode,
  ResolvedTarget,
  SearchResultItem,
  SettingsLoadStatus,
  SettingsSnapshot,
  TagNode,
  UiState,
  WindowMode,
  WindowState,
  WorkspaceSummary,
} from '@/bindings/types';
import {
  deriveTitleFromContent,
  extractTemplateNameFromSummary,
  flattenDirRelPaths,
  inferDefaultTemplateName,
} from '@/features/notes';
import {
  useQuickOpen,
  writePendingOpenFile,
  type QuickOpenActionItem,
  type QuickOpenDirectoryScope,
  type QuickOpenOpenMode,
  type QuickOpenResultItem,
} from '@/features/quickopen';
import { useSettingsStore } from '@/features/settings';
import { actionRegistry, formatKeyComboDisplay, type ActionContext } from '@/lib/actions';
import { externalActionRun, windowSetQuickBarPinned, windowShowMain } from '@/lib/ipc/commands';
import type { UnlistenFn } from '@/lib/ipc/events';
import {
  appendQuickBarNote,
  createQuickBarNote,
  fetchQuickBarLibraryInfo,
  fetchQuickBarNavTree,
  fetchQuickBarNotesList,
  fetchQuickBarSettings,
  fetchQuickBarUiState,
  hideQuickBarWindow,
  listQuickBarTemplates,
  patchQuickBarUiState,
  resolveQuickBarTarget,
  searchQuickBarNotes,
  showMainWindowForOnboarding,
  subscribeQuickBarWindowState,
} from './api';

export const QUICKBAR_DRAFT_STORAGE_KEY = 'quicknote:quickbar-draft-v1';
export const QUICKBAR_SEARCH_DEBOUNCE_MS = 80;

export interface ExplicitQuickBarTarget {
  relPath: string;
  title: string;
  workspaceKey: string | null;
}

export interface QuickBarDraftStorage {
  mode: CaptureMode;
  targetRelPath: string | null;
  selectedWorkspaceKey: string | null;
  text: string;
  updatedAt: number;
}

export type QuickBarOpenGroupId =
  'action' | 'note' | 'tag' | 'file_bookmark' | 'dir_bookmark' | 'path_direct';

export type QuickBarOpenBadgeLabel =
  '动作' | '笔记' | '标签' | '文件书签' | '目录书签' | '路径直开';

export type QuickBarOpenActionKind = 'open_file' | 'enter_directory';

export interface QuickBarOpenItem {
  id: string;
  group: QuickBarOpenGroupId;
  badgeLabel: QuickBarOpenBadgeLabel;
  actionKind: QuickBarOpenActionKind;
  title: string;
  subtitle: string;
  absPath: string;
  isHidden?: boolean;
  pinned?: boolean;
  tagName?: string;
  actionId?: string;
  shortcutLabel?: string;
  disabled?: boolean;
  rawIndex?: number;
}

export interface QuickBarOpenGroup {
  id: QuickBarOpenGroupId;
  label: string;
  items: QuickBarOpenItem[];
}

export type QuickBarOpenDirectoryScope = QuickOpenDirectoryScope;

type ExplicitTargetSource = 'picker' | 'related' | null;

const QUICKBAR_OPEN_GROUP_ORDER: readonly QuickBarOpenGroupId[] = [
  'action',
  'note',
  'tag',
  'file_bookmark',
  'dir_bookmark',
  'path_direct',
];

function mapQuickOpenResultToQuickBarItem(
  item: QuickOpenResultItem,
  rawIndex: number,
): QuickBarOpenItem {
  if (item.kind === 'action') {
    return {
      id: item.id,
      group: 'action',
      badgeLabel: '动作',
      actionKind: 'open_file',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      actionId: item.actionId,
      shortcutLabel: item.shortcutLabel,
      disabled: item.disabled,
      rawIndex,
    };
  }
  if (item.kind === 'tag') {
    return {
      id: item.id,
      group: 'tag',
      badgeLabel: '标签',
      actionKind: 'open_file',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      tagName: item.tagName,
      rawIndex,
    };
  }
  if (item.kind === 'note') {
    return {
      id: item.id,
      group: 'note',
      badgeLabel: '笔记',
      actionKind: 'open_file',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      pinned: item.pinned,
      rawIndex,
    };
  }
  if (item.kind === 'file' && item.source === 'file_bookmark') {
    return {
      id: item.id,
      group: 'file_bookmark',
      badgeLabel: '文件书签',
      actionKind: 'open_file',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      pinned: item.pinned,
      rawIndex,
    };
  }
  if (item.kind === 'dir' && item.source === 'dir_bookmark') {
    return {
      id: item.id,
      group: 'dir_bookmark',
      badgeLabel: '目录书签',
      actionKind: 'enter_directory',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      pinned: item.pinned,
      rawIndex,
    };
  }
  const isDir = item.kind === 'dir';
  return {
    id: item.id,
    group: 'path_direct',
    badgeLabel: isDir ? '目录书签' : '路径直开',
    actionKind: isDir ? 'enter_directory' : 'open_file',
    title: item.title,
    subtitle: item.subtitle,
    absPath: item.absPath ?? '',
    isHidden: item.isHidden,
    pinned: item.pinned,
    rawIndex,
  };
}

function createFallbackUiState(): UiState {
  return {
    selectedWorkspaceKey: null,
    workspaceModes: {},
    filters: {
      activeTab: 'recent',
      selectedDir: null,
      selectedTags: [],
      listScrollTop: 0,
    },
    tabs: {
      items: [],
      activeKey: null,
      closedStack: [],
    },
    lastActiveDoc: null,
    leftSidebar: {
      width: 300,
      activeView: 'workspace',
      collapsed: false,
    },
    terminalPanel: {
      visible: false,
      height: 240,
      collapsed: false,
    },
    windows: {
      main: {
        isOpen: false,
        mode: 'standard',
        x: 0,
        y: 0,
        width: 1200,
        height: 800,
      },
    },
    hotkey: {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
  };
}

function createFallbackResolvedTarget(): ResolvedTarget {
  return {
    scenario: 'S8',
    kind: 'inbox',
    targetRelPath: 'inbox.md',
    targetTitle: '收件箱',
    sectionHint: null,
    workspaceKey: null,
    workspaceName: null,
    isCrossWorkspaceFallback: false,
    recommendedMode: 'list',
    chipLabel: '收件箱 · inbox.md',
  };
}

function isCaptureMode(value: unknown): value is CaptureMode {
  return value === 'list' || value === 'todo' || value === 'prose';
}

function getCaptureModeSectionLabel(mode: CaptureMode): string {
  switch (mode) {
    case 'list':
      return '列表';
    case 'todo':
      return '待办';
    case 'prose':
      return '正文';
  }
}

function readDraftStorage(): QuickBarDraftStorage | null {
  try {
    if (typeof globalThis.localStorage === 'undefined') {
      return null;
    }
    const raw = globalThis.localStorage.getItem(QUICKBAR_DRAFT_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) {
      return null;
    }
    const candidate = parsed as Record<string, unknown>;
    if (typeof candidate.text !== 'string') {
      return null;
    }
    return {
      mode: isCaptureMode(candidate.mode) ? candidate.mode : 'list',
      targetRelPath: typeof candidate.targetRelPath === 'string' ? candidate.targetRelPath : null,
      selectedWorkspaceKey:
        typeof candidate.selectedWorkspaceKey === 'string' ? candidate.selectedWorkspaceKey : null,
      text: candidate.text,
      updatedAt: typeof candidate.updatedAt === 'number' ? candidate.updatedAt : Date.now(),
    };
  } catch {
    return null;
  }
}

function writeDraftStorage(payload: QuickBarDraftStorage): void {
  try {
    if (typeof globalThis.localStorage === 'undefined') {
      return;
    }
    globalThis.localStorage.setItem(QUICKBAR_DRAFT_STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // 忽略存储配额或环境异常
  }
}

function removeDraftStorage(): void {
  try {
    if (typeof globalThis.localStorage === 'undefined') {
      return;
    }
    globalThis.localStorage.removeItem(QUICKBAR_DRAFT_STORAGE_KEY);
  } catch {
    // 忽略环境异常
  }
}

export const useQuickBarStore = defineStore('quickbar', () => {
  const settingsStore = useSettingsStore();
  const libraryInfo = ref<LibraryInfo | null>(null);
  const settingsSnapshot = ref<SettingsSnapshot | null>(null);
  const uiState = ref<UiState>(createFallbackUiState());
  const mode = ref<QuickBarMode>('capture');
  const captureMode = ref<CaptureMode>('list');
  const draft = ref<string>('');
  const loading = ref<boolean>(false);
  const submitting = ref<boolean>(false);
  const errorMessage = ref<string | null>(null);
  const isPinned = computed<boolean>(
    () => settingsStore.snapshot?.settings.quickBar.pinned ?? false,
  );

  async function togglePin(): Promise<void> {
    errorMessage.value = null;
    try {
      await windowSetQuickBarPinned(!isPinned.value);
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '切换 Quick Bar 固定状态失败。';
    }
  }
  const focusTick = ref<number>(0);
  const mainWindowMode = ref<WindowMode>('standard');
  const mainWindowVisible = ref<boolean>(false);

  const resolvedTarget = ref<ResolvedTarget | null>(null);
  const explicitTarget = ref<ExplicitQuickBarTarget | null>(null);
  const explicitTargetSource = ref<ExplicitTargetSource>(null);
  const selectedWorkspaceKey = ref<string | null>(null);
  const modeTouchedInSession = ref<boolean>(false);

  const relatedNotes = ref<SearchResultItem[]>([]);
  const selectedRelatedIndex = ref<number>(-1);

  const targetPickerOpen = ref<boolean>(false);
  const saveAsModalOpen = ref<boolean>(false);
  const saveAsTitle = ref<string>('');
  const saveAsDir = ref<string>('');
  const saveAsTemplate = ref<string>('note');
  const availableTemplates = ref<string[]>(['note', 'doc', 'command-book']);
  const saveAsDirs = ref<string[]>([]);
  const pickerLoading = ref<boolean>(false);
  const pickerSearchQuery = ref<string>('');
  const pickerSelectedTag = ref<string | null>(null);
  const pickerNotes = ref<NoteSummary[]>([]);
  const pickerSearchItems = ref<SearchResultItem[] | null>(null);
  const pickerTags = ref<TagNode[]>([]);
  const pickerWorkspaces = ref<WorkspaceSummary[]>([]);

  function buildQuickBarActionContext(): ActionContext {
    return {
      activeScope: 'quickbar',
      activeScopes: ['quickbar', 'global'],
    };
  }

  function quickBarActionItems(): QuickOpenActionItem[] {
    const ctx = buildQuickBarActionContext();
    return actionRegistry
      .getAll()
      .filter((action) => action.scope === 'quickbar' || action.scope === 'global')
      .map((action) => ({
        id: action.id,
        title: action.title,
        subtitle: action.id,
        shortcutLabel: formatKeyComboDisplay(actionRegistry.getEffectiveKeys(action.id)),
        disabled: action.when ? !action.when(ctx) : false,
      }));
  }

  async function openItemFromQuickBar(
    item: QuickOpenResultItem,
    openMode: QuickOpenOpenMode,
  ): Promise<void> {
    if (!item.absPath) {
      return;
    }
    await openFileInMain(item.absPath, openMode === 'newTab');
  }

  async function runActionFromQuickBar(actionId: string): Promise<void> {
    await actionRegistry.executeById(actionId, buildQuickBarActionContext());
  }

  async function selectTagFromQuickBar(tagName: string): Promise<void> {
    writePendingOpenFile({
      kind: 'tag',
      tag: tagName,
      ts: Date.now(),
    });
    await showMainWindowForOnboarding();
    await hide('esc');
  }

  async function revealItemFromQuickBar(item: QuickOpenResultItem): Promise<void> {
    if (!item.absPath) {
      return;
    }
    await externalActionRun({
      id: 'external.reveal',
      absPath: item.absPath,
      dirPath: null,
    });
  }

  const quickOpen = useQuickOpen({
    getActionItems: () => quickBarActionItems(),
    onOpenItem: (item, openMode) => openItemFromQuickBar(item, openMode),
    onExecuteAction: (actionId) => runActionFromQuickBar(actionId),
    onSelectTag: (tagName) => selectTagFromQuickBar(tagName),
    onRevealItem: (item) => revealItemFromQuickBar(item),
  });

  const externalBookmarks = quickOpen.externalBookmarks;
  const openQuery = quickOpen.query;
  const openLoading = quickOpen.loading;
  const openError = quickOpen.errorMessage;
  const openSelectedIndex = quickOpen.selectedIndex;
  const openDirectoryStack = quickOpen.directoryStack;
  const openProbeResult = quickOpen.pathProbeResult;

  let unlistenWindowState: UnlistenFn | null = null;
  let relatedDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let pickerDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let relatedRequestId = 0;
  let pickerRequestId = 0;
  let suppressDraftWatch = false;
  let hasLiveMainWindowState = false;

  const isUnconfigured = computed<boolean>(() => !libraryInfo.value?.root);
  const isAuxiliarySidebarActive = computed<boolean>(
    () => mainWindowVisible.value && mainWindowMode.value === 'auxiliary',
  );
  const canSubmit = computed<boolean>(
    () => !isUnconfigured.value && !submitting.value && draft.value.trim().length > 0,
  );
  const settingsStatus = computed<SettingsLoadStatus | null>(
    () => settingsSnapshot.value?.status ?? null,
  );

  const effectiveTargetRelPath = computed<string>(
    () => explicitTarget.value?.relPath ?? resolvedTarget.value?.targetRelPath ?? 'inbox.md',
  );

  const isInboxTarget = computed<boolean>(() => effectiveTargetRelPath.value === 'inbox.md');

  const activeWorkspaceKey = computed<string | null>(
    () =>
      explicitTarget.value?.workspaceKey ??
      resolvedTarget.value?.workspaceKey ??
      selectedWorkspaceKey.value,
  );

  const targetChipLabel = computed<string>(() => {
    if (isUnconfigured.value) {
      return '未选库 · S1';
    }
    if (explicitTarget.value) {
      return `${explicitTarget.value.title} › ${getCaptureModeSectionLabel(captureMode.value)}`;
    }
    if (resolvedTarget.value) {
      if (resolvedTarget.value.isCrossWorkspaceFallback) {
        return '收件箱 · 跨工作区';
      }
      return resolvedTarget.value.chipLabel;
    }
    return '收件箱 · inbox.md';
  });

  const targetChipTone = computed<'neutral' | 'accent' | 'warning'>(() => {
    if (isUnconfigured.value) {
      return 'warning';
    }
    if (explicitTarget.value) {
      return 'accent';
    }
    if (resolvedTarget.value?.isCrossWorkspaceFallback) {
      return 'warning';
    }
    return 'neutral';
  });

  const sectionHintText = computed<string | null>(() => {
    if (isUnconfigured.value || explicitTarget.value) {
      return null;
    }
    return resolvedTarget.value?.sectionHint ?? null;
  });

  const availableWorkspaces = computed<WorkspaceSummary[]>(() => {
    if (pickerWorkspaces.value.length > 0) {
      return pickerWorkspaces.value;
    }
    const configured = settingsSnapshot.value?.settings.workspaces ?? {};
    return Object.entries(configured).map(([key, item]) => ({
      key,
      name: item.name,
      root: item.root,
      openDir: item.openDir,
    }));
  });

  const activeOpenDirectoryScope = quickOpen.activeDirectoryScope;

  const openFlatItems = computed<QuickBarOpenItem[]>(() =>
    quickOpen.items.value.map((item, idx) => mapQuickOpenResultToQuickBarItem(item, idx)),
  );

  const openGroups = computed<QuickBarOpenGroup[]>(() => {
    const buckets = new Map<QuickBarOpenGroupId, QuickBarOpenItem[]>();
    for (const item of openFlatItems.value) {
      let list = buckets.get(item.group);
      if (!list) {
        list = [];
        buckets.set(item.group, list);
      }
      list.push(item);
    }

    const scope = activeOpenDirectoryScope.value;
    const probe = openProbeResult.value;
    const pathDirectLabel = scope
      ? `目录文件 · ${scope.dirPath}`
      : probe?.exists && probe.kind === 'directory'
        ? `目录内容 · ${probe.resolvedPath}`
        : '路径直开';

    const labels: Record<QuickBarOpenGroupId, string> = {
      action: '动作',
      note: '笔记库文档',
      tag: '标签',
      file_bookmark: '外部文件书签',
      dir_bookmark: '外部目录书签',
      path_direct: pathDirectLabel,
    };

    const groups: QuickBarOpenGroup[] = [];
    for (const groupId of QUICKBAR_OPEN_GROUP_ORDER) {
      const items = buckets.get(groupId);
      if (items && items.length > 0) {
        groups.push({
          id: groupId,
          label: labels[groupId],
          items,
        });
      }
    }
    return groups;
  });

  function resetOpenModeState(): void {
    quickOpen.reset();
  }

  function setOpenQuery(query: string, immediate = false): void {
    quickOpen.setQuery(query, immediate);
  }

  async function refreshOpenMode(resetQuery = false): Promise<void> {
    if (resetQuery) {
      quickOpen.reset();
    }
    quickOpen.refresh(true);
    await quickOpen.waitForPendingSearch();
  }

  function moveOpenSelection(delta: number): void {
    quickOpen.moveSelection(delta >= 0 ? 1 : -1);
  }

  async function enterOpenDirectory(dirPath: string, title?: string): Promise<void> {
    quickOpen.enterDirectory(dirPath, title);
    await quickOpen.waitForPendingSearch();
  }

  async function exitOpenDirectory(): Promise<boolean> {
    const exited = quickOpen.exitDirectory();
    if (exited) {
      await quickOpen.waitForPendingSearch();
    }
    return exited;
  }

  async function selectOpenItem(
    item: QuickBarOpenItem,
    openMode: QuickOpenOpenMode = 'open',
  ): Promise<void> {
    const raw =
      (typeof item.rawIndex === 'number' ? quickOpen.items.value[item.rawIndex] : undefined) ??
      quickOpen.items.value.find((candidate) => candidate.id === item.id) ??
      null;
    if (raw) {
      await quickOpen.confirmItem(raw, openMode);
      await quickOpen.waitForPendingSearch();
      return;
    }
    if (item.actionKind === 'enter_directory' && item.absPath) {
      await enterOpenDirectory(item.absPath, item.title);
      return;
    }
    if (item.absPath) {
      await openFileInMain(item.absPath, openMode === 'newTab');
    }
  }

  async function confirmOpenSelection(openMode: QuickOpenOpenMode = 'open'): Promise<void> {
    await quickOpen.confirmItem(quickOpen.selectedItem.value, openMode);
    await quickOpen.waitForPendingSearch();
  }

  function handleOpenKeydown(event: KeyboardEvent): boolean {
    return quickOpen.handleKeydown(event);
  }

  function handleOpenTabComplete(): Promise<boolean> {
    return quickOpen.handleTabComplete();
  }

  function clearRelatedTimer(): void {
    if (relatedDebounceTimer !== null) {
      clearTimeout(relatedDebounceTimer);
      relatedDebounceTimer = null;
    }
  }

  function clearPickerTimer(): void {
    if (pickerDebounceTimer !== null) {
      clearTimeout(pickerDebounceTimer);
      pickerDebounceTimer = null;
    }
  }

  function persistDraftNow(): void {
    if (draft.value.length === 0 && explicitTarget.value === null && !modeTouchedInSession.value) {
      removeDraftStorage();
      return;
    }
    writeDraftStorage({
      mode: captureMode.value,
      targetRelPath: explicitTarget.value?.relPath ?? null,
      selectedWorkspaceKey: selectedWorkspaceKey.value,
      text: draft.value,
      updatedAt: Date.now(),
    });
  }

  function restoreDraftFromStorage(): void {
    const saved = readDraftStorage();
    if (!saved) {
      return;
    }
    suppressDraftWatch = true;
    draft.value = saved.text;
    captureMode.value = saved.mode;
    modeTouchedInSession.value = true;
    suppressDraftWatch = false;
  }

  async function resolveCurrentTarget(currentUiState: UiState): Promise<void> {
    if (isUnconfigured.value) {
      resolvedTarget.value = null;
      return;
    }

    const input = {
      mainVisible: currentUiState.windows?.main?.isOpen ?? false,
      activeDocKey: currentUiState.tabs?.activeKey ?? null,
      selectedWorkspaceKey: currentUiState.selectedWorkspaceKey ?? null,
      isInboxView: currentUiState.filters?.activeTab === 'inbox',
    };

    try {
      const target = await Promise.resolve(resolveQuickBarTarget(input));
      resolvedTarget.value = target ?? createFallbackResolvedTarget();
    } catch {
      resolvedTarget.value = createFallbackResolvedTarget();
    }

    if (!modeTouchedInSession.value && resolvedTarget.value) {
      captureMode.value =
        resolvedTarget.value.targetRelPath === 'inbox.md'
          ? 'list'
          : resolvedTarget.value.recommendedMode;
    }
  }

  async function refreshState(): Promise<void> {
    loading.value = true;
    try {
      const [info, snapshot, fetchedUiState] = await Promise.all([
        fetchQuickBarLibraryInfo(),
        fetchQuickBarSettings(),
        Promise.resolve(fetchQuickBarUiState()).catch(() => null),
      ]);
      libraryInfo.value = info;
      settingsSnapshot.value = snapshot;
      externalBookmarks.value = snapshot.settings.externalBookmarks ?? [];
      const normalizedUi = fetchedUiState ?? createFallbackUiState();
      uiState.value = normalizedUi;
      selectedWorkspaceKey.value = normalizedUi.selectedWorkspaceKey ?? null;
      if (!hasLiveMainWindowState) {
        mainWindowMode.value = normalizedUi.windows?.main?.mode ?? 'standard';
        mainWindowVisible.value = normalizedUi.windows?.main?.isOpen ?? false;
      }

      if (info.root !== null) {
        await resolveCurrentTarget(normalizedUi);
      } else {
        resolvedTarget.value = null;
      }
    } finally {
      loading.value = false;
    }
  }

  function scheduleRelatedSearch(query: string): void {
    clearRelatedTimer();
    const trimmed = query.trim();
    if (isUnconfigured.value || trimmed.length < 2) {
      relatedRequestId += 1;
      relatedNotes.value = [];
      if (selectedRelatedIndex.value !== -1) {
        selectedRelatedIndex.value = -1;
        if (explicitTargetSource.value === 'related') {
          explicitTarget.value = null;
          explicitTargetSource.value = null;
        }
      }
      return;
    }

    relatedDebounceTimer = setTimeout(() => {
      relatedDebounceTimer = null;
      relatedRequestId += 1;
      const reqId = relatedRequestId;
      void searchQuickBarNotes({
        query: trimmed,
        limit: 3,
        requestId: reqId,
      })
        .then((res) => {
          if (res.requestId !== relatedRequestId) {
            return;
          }
          const filtered = res.items
            .filter((item) => item.kind === 'note' || item.relPath !== null)
            .slice(0, 3);
          relatedNotes.value = filtered;
          if (selectedRelatedIndex.value >= filtered.length) {
            selectedRelatedIndex.value = -1;
            if (explicitTargetSource.value === 'related') {
              explicitTarget.value = null;
              explicitTargetSource.value = null;
            }
          }
        })
        .catch(() => {
          // 检索失败时静默保持当前状态
        });
    }, QUICKBAR_SEARCH_DEBOUNCE_MS);
  }

  watch(draft, (nextText) => {
    if (suppressDraftWatch) {
      return;
    }
    persistDraftNow();
    scheduleRelatedSearch(nextText);
  });

  function setCaptureMode(nextMode: CaptureMode): void {
    captureMode.value = nextMode;
    modeTouchedInSession.value = true;
    persistDraftNow();

    const wsKey = activeWorkspaceKey.value;
    if (wsKey && !isInboxTarget.value) {
      const nextWorkspaceModes: Record<string, CaptureMode> = {
        ...uiState.value.workspaceModes,
        [wsKey]: nextMode,
      };
      uiState.value = {
        ...uiState.value,
        workspaceModes: nextWorkspaceModes,
      };
      void Promise.resolve(patchQuickBarUiState('workspaceModes', nextWorkspaceModes)).catch(
        () => undefined,
      );
    }
  }

  function selectRelatedNote(index: number): void {
    if (index < 0 || index >= relatedNotes.value.length) {
      selectedRelatedIndex.value = -1;
      if (explicitTargetSource.value === 'related') {
        explicitTarget.value = null;
        explicitTargetSource.value = null;
        persistDraftNow();
      }
      return;
    }
    const item = relatedNotes.value[index];
    if (!item) {
      return;
    }
    selectedRelatedIndex.value = index;
    explicitTarget.value = {
      relPath: item.relPath ?? item.key,
      title: item.title,
      workspaceKey: null,
    };
    explicitTargetSource.value = 'related';
    persistDraftNow();
  }

  function cycleRelatedTarget(direction: 'next' | 'prev'): void {
    const len = relatedNotes.value.length;
    if (len === 0) {
      return;
    }
    const current = selectedRelatedIndex.value;
    let nextIndex: number;
    if (direction === 'next') {
      nextIndex = current >= len - 1 ? -1 : current + 1;
    } else {
      nextIndex = current <= -1 ? len - 1 : current - 1;
    }
    selectRelatedNote(nextIndex);
  }

  async function loadTargetPickerData(): Promise<void> {
    if (isUnconfigured.value) {
      return;
    }
    pickerLoading.value = true;
    try {
      const [tree, notes] = await Promise.all([
        Promise.resolve(fetchQuickBarNavTree()).catch(() => null),
        Promise.resolve(
          fetchQuickBarNotesList({
            tab: 'recent',
            dirRelPath: null,
            tags: pickerSelectedTag.value ? [pickerSelectedTag.value] : [],
            workspaceKey: null,
            filterText: null,
          }),
        ).catch(() => [] as NoteSummary[]),
      ]);
      if (tree) {
        const snapshot: NavTreeSnapshot = tree;
        pickerTags.value = snapshot.tags;
        pickerWorkspaces.value = snapshot.workspaces;
      }
      pickerNotes.value = notes;
    } finally {
      pickerLoading.value = false;
    }
  }

  function openTargetPicker(): void {
    if (isUnconfigured.value) {
      return;
    }
    targetPickerOpen.value = true;
    void loadTargetPickerData();
  }

  function closeTargetPicker(): void {
    targetPickerOpen.value = false;
    focusTick.value += 1;
  }

  function toggleTargetPicker(): void {
    if (targetPickerOpen.value) {
      closeTargetPicker();
    } else {
      openTargetPicker();
    }
  }

  function setPickerSearchQuery(query: string): void {
    pickerSearchQuery.value = query;
    clearPickerTimer();
    const trimmed = query.trim();
    if (trimmed.length === 0) {
      pickerRequestId += 1;
      pickerSearchItems.value = null;
      return;
    }
    pickerDebounceTimer = setTimeout(() => {
      pickerDebounceTimer = null;
      pickerRequestId += 1;
      const reqId = pickerRequestId;
      void searchQuickBarNotes({
        query: trimmed,
        limit: 12,
        requestId: reqId,
      })
        .then((res) => {
          if (res.requestId !== pickerRequestId) {
            return;
          }
          pickerSearchItems.value = res.items.filter(
            (item) => item.kind === 'note' || item.relPath !== null,
          );
        })
        .catch(() => {
          // 容错
        });
    }, QUICKBAR_SEARCH_DEBOUNCE_MS);
  }

  async function selectPickerTag(tag: string | null): Promise<void> {
    pickerSelectedTag.value = pickerSelectedTag.value === tag ? null : tag;
    if (isUnconfigured.value) {
      return;
    }
    try {
      pickerNotes.value = await fetchQuickBarNotesList({
        tab: 'recent',
        dirRelPath: null,
        tags: pickerSelectedTag.value ? [pickerSelectedTag.value] : [],
        workspaceKey: null,
        filterText: null,
      });
    } catch {
      // 容错
    }
  }

  async function selectWorkspaceLock(workspaceKey: string | null): Promise<void> {
    selectedWorkspaceKey.value = workspaceKey;
    explicitTarget.value = null;
    explicitTargetSource.value = null;
    selectedRelatedIndex.value = -1;

    const updatedUi = await Promise.resolve(
      patchQuickBarUiState('selectedWorkspace', workspaceKey),
    ).catch(() => null);
    const nextUiState: UiState = updatedUi ?? {
      ...uiState.value,
      selectedWorkspaceKey: workspaceKey,
    };
    uiState.value = nextUiState;
    persistDraftNow();
    await resolveCurrentTarget(nextUiState);
  }

  function selectExplicitDoc(target: ExplicitQuickBarTarget): void {
    explicitTarget.value = target;
    explicitTargetSource.value = 'picker';
    selectedRelatedIndex.value = -1;
    targetPickerOpen.value = false;
    focusTick.value += 1;
    persistDraftNow();
  }

  function clearExplicitTarget(): void {
    explicitTarget.value = null;
    explicitTargetSource.value = null;
    selectedRelatedIndex.value = -1;
    persistDraftNow();
  }

  function clearDraft(): void {
    clearRelatedTimer();
    relatedRequestId += 1;
    suppressDraftWatch = true;
    draft.value = '';
    suppressDraftWatch = false;
    relatedNotes.value = [];
    selectedRelatedIndex.value = -1;
    if (explicitTargetSource.value === 'related') {
      explicitTarget.value = null;
      explicitTargetSource.value = null;
    }
    modeTouchedInSession.value = false;
    errorMessage.value = null;
    removeDraftStorage();
    focusTick.value += 1;
  }

  async function submitAppend(openInMain: boolean): Promise<AppendResult | null> {
    if (!canSubmit.value) {
      return null;
    }
    submitting.value = true;
    errorMessage.value = null;
    try {
      const result = await appendQuickBarNote({
        targetRelPath: effectiveTargetRelPath.value,
        content: draft.value,
        mode: captureMode.value,
        workspaceKey: activeWorkspaceKey.value,
      });

      clearRelatedTimer();
      relatedRequestId += 1;
      suppressDraftWatch = true;
      draft.value = '';
      suppressDraftWatch = false;
      explicitTarget.value = null;
      explicitTargetSource.value = null;
      selectedRelatedIndex.value = -1;
      relatedNotes.value = [];
      modeTouchedInSession.value = false;
      targetPickerOpen.value = false;
      removeDraftStorage();

      if (openInMain) {
        await hideQuickBarWindow('esc');
        await showMainWindowForOnboarding();
      } else {
        await hideQuickBarWindow('esc');
      }

      return result;
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '追加笔记失败，请重试。';
      return null;
    } finally {
      submitting.value = false;
    }
  }

  function formatDraftBodyForNewNote(text: string, currentMode: CaptureMode): string {
    const trimmed = text.trim();
    if (!trimmed) {
      return '';
    }
    if (currentMode === 'prose') {
      return trimmed;
    }
    const prefix = currentMode === 'todo' ? '- [ ] ' : '- ';
    return trimmed
      .split(/\r?\n/)
      .map((line) => {
        const lineTrimmed = line.trim();
        if (!lineTrimmed) {
          return '';
        }
        if (currentMode === 'todo' && /^- \[[ xX]\]\s/.test(lineTrimmed)) {
          return lineTrimmed;
        }
        if (currentMode === 'list' && /^[-*]\s/.test(lineTrimmed)) {
          return lineTrimmed;
        }
        return `${prefix}${lineTrimmed}`;
      })
      .join('\n');
  }

  function resolveSaveAsDefaultDir(): string {
    const relPath = effectiveTargetRelPath.value;
    if (!relPath || relPath.startsWith('_templates/')) {
      return '';
    }
    const lastSlash = relPath.lastIndexOf('/');
    return lastSlash > 0 ? relPath.slice(0, lastSlash) : '';
  }

  function openSaveAsNoteDialog(): void {
    if (!canSubmit.value) {
      return;
    }
    const defaultDir = resolveSaveAsDefaultDir();
    saveAsTitle.value = deriveTitleFromContent(draft.value);
    saveAsDir.value = defaultDir;
    saveAsTemplate.value = inferDefaultTemplateName(defaultDir);
    saveAsModalOpen.value = true;

    void Promise.all([
      Promise.resolve(listQuickBarTemplates()).catch(() => [] as NoteSummary[]),
      Promise.resolve(fetchQuickBarNavTree()).catch(() => null),
    ]).then(([templateItems, tree]) => {
      const merged = new Set<string>(['note', 'doc', 'command-book']);
      for (const item of templateItems) {
        merged.add(extractTemplateNameFromSummary(item));
      }
      availableTemplates.value = Array.from(merged);
      if (tree) {
        saveAsDirs.value = flattenDirRelPaths(tree.dirs);
      }
    });
  }

  function closeSaveAsNoteDialog(): void {
    saveAsModalOpen.value = false;
    focusTick.value += 1;
  }

  async function confirmSaveAsNote(): Promise<NoteDoc | null> {
    if (!canSubmit.value) {
      return null;
    }
    submitting.value = true;
    errorMessage.value = null;
    try {
      const body = formatDraftBodyForNewNote(draft.value, captureMode.value);
      const doc = await createQuickBarNote({
        title: saveAsTitle.value.trim() || '未命名笔记',
        dirRelPath: saveAsDir.value.trim(),
        templateName: saveAsTemplate.value || null,
        body,
      });

      clearRelatedTimer();
      relatedRequestId += 1;
      suppressDraftWatch = true;
      draft.value = '';
      suppressDraftWatch = false;
      explicitTarget.value = null;
      explicitTargetSource.value = null;
      selectedRelatedIndex.value = -1;
      relatedNotes.value = [];
      modeTouchedInSession.value = false;
      targetPickerOpen.value = false;
      saveAsModalOpen.value = false;
      removeDraftStorage();

      await Promise.resolve(
        patchQuickBarUiState('lastActiveDoc', {
          relPath: doc.relPath ?? doc.absPath,
          workspaceKey: doc.meta.project ?? null,
          touchedAtMs: Date.now(),
        }),
      ).catch(() => undefined);
      await hideQuickBarWindow('esc');
      await showMainWindowForOnboarding();

      return doc;
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : '另存新笔记失败，请重试。';
      return null;
    } finally {
      submitting.value = false;
    }
  }

  async function handleEscape(): Promise<void> {
    if (saveAsModalOpen.value) {
      closeSaveAsNoteDialog();
      return;
    }
    if (targetPickerOpen.value) {
      closeTargetPicker();
      return;
    }
    await hide('esc');
  }

  async function initialize(): Promise<void> {
    restoreDraftFromStorage();
    await refreshState();
    if (draft.value.trim().length >= 2 && !isUnconfigured.value) {
      scheduleRelatedSearch(draft.value);
    }
    if (!unlistenWindowState) {
      unlistenWindowState = await subscribeQuickBarWindowState((state) => {
        if (state.windowId === 'main') {
          hasLiveMainWindowState = true;
          mainWindowVisible.value = state.visible;
          if (state.mode !== null) {
            mainWindowMode.value = state.mode;
          }
          return;
        }
        if (state.windowId === 'quickbar' && state.visible) {
          if (state.mode === null) {
            mode.value = 'capture';
          }
          void refreshState().then(() => {
            focusTick.value += 1;
          });
        } else if (state.windowId === 'quickbar' && !state.visible) {
          // 切 Space、热键或 Esc 隐藏时关闭弹层、复位至默认 capture 模式并完整保留草稿
          mode.value = 'capture';
          targetPickerOpen.value = false;
          saveAsModalOpen.value = false;
          resetOpenModeState();
        }
      });
    }
    focusTick.value += 1;
  }

  function dispose(): void {
    clearRelatedTimer();
    clearPickerTimer();
    quickOpen.dispose();
    if (unlistenWindowState) {
      unlistenWindowState();
      unlistenWindowState = null;
    }
  }

  async function openMainOnboarding(): Promise<void> {
    await showMainWindowForOnboarding();
  }

  async function toggleAuxiliarySidebarMode(): Promise<WindowState | null> {
    const targetMode: WindowMode = isAuxiliarySidebarActive.value ? 'standard' : 'auxiliary';
    errorMessage.value = null;
    try {
      const nextState = await windowShowMain({ mode: targetMode });
      hasLiveMainWindowState = true;
      mainWindowVisible.value = nextState?.visible ?? true;
      mainWindowMode.value = nextState?.mode ?? targetMode;
      await hide('esc');
      return nextState ?? null;
    } catch (error) {
      errorMessage.value =
        error instanceof Error ? error.message : '切换 45% 侧栏辅助窗口失败，请重试。';
      return null;
    }
  }

  async function hide(reason: HideReason = 'esc'): Promise<void> {
    mode.value = 'capture';
    targetPickerOpen.value = false;
    saveAsModalOpen.value = false;
    resetOpenModeState();
    await hideQuickBarWindow(reason);
  }

  function setMode(nextMode: QuickBarMode): void {
    mode.value = nextMode;
    focusTick.value += 1;
  }

  async function openFileInMain(
    absPath: string,
    newTab = false,
    line: number | null = null,
  ): Promise<void> {
    writePendingOpenFile({
      absPath,
      newTab,
      line,
      ts: Date.now(),
    });
    await showMainWindowForOnboarding();
    await hide('esc');
  }

  return {
    libraryInfo,
    settingsSnapshot,
    uiState,
    mode,
    captureMode,
    draft,
    loading,
    submitting,
    errorMessage,
    isPinned,
    togglePin,
    focusTick,
    mainWindowMode,
    mainWindowVisible,
    isAuxiliarySidebarActive,
    resolvedTarget,
    explicitTarget,
    selectedWorkspaceKey,
    relatedNotes,
    selectedRelatedIndex,
    targetPickerOpen,
    saveAsModalOpen,
    saveAsTitle,
    saveAsDir,
    saveAsTemplate,
    availableTemplates,
    saveAsDirs,
    pickerLoading,
    pickerSearchQuery,
    pickerSelectedTag,
    pickerNotes,
    pickerSearchItems,
    pickerTags,
    availableWorkspaces,
    externalBookmarks,
    openQuery,
    openLoading,
    openError,
    openGroups,
    openFlatItems,
    openSelectedIndex,
    openDirectoryStack,
    activeOpenDirectoryScope,
    openProbeResult,
    isUnconfigured,
    canSubmit,
    settingsStatus,
    effectiveTargetRelPath,
    isInboxTarget,
    activeWorkspaceKey,
    targetChipLabel,
    targetChipTone,
    sectionHintText,
    refreshState,
    initialize,
    dispose,
    openMainOnboarding,
    toggleAuxiliarySidebarMode,
    hide,
    setMode,
    openFileInMain,
    setOpenQuery,
    refreshOpenMode,
    resetOpenModeState,
    moveOpenSelection,
    enterOpenDirectory,
    exitOpenDirectory,
    selectOpenItem,
    confirmOpenSelection,
    handleOpenKeydown,
    handleOpenTabComplete,
    setCaptureMode,
    selectRelatedNote,
    cycleRelatedTarget,
    openTargetPicker,
    closeTargetPicker,
    toggleTargetPicker,
    setPickerSearchQuery,
    selectPickerTag,
    selectWorkspaceLock,
    selectExplicitDoc,
    clearExplicitTarget,
    clearDraft,
    submitAppend,
    openSaveAsNoteDialog,
    closeSaveAsNoteDialog,
    confirmSaveAsNote,
    handleEscape,
  };
});
