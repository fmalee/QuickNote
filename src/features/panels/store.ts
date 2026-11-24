import { computed, ref, watch } from 'vue';
import { defineStore } from 'pinia';
import type { NoteSummary, PanelGroupUiState, PanelId, RelatedNoteItem } from '@/bindings/types';
import { useEditorTabsStore } from '@/features/editor';
import {
  RECENT_NOTES_LIMIT,
  fetchRecentNotes,
  fetchRelatedNotes,
  fetchUiState,
  patchPanelGroupUiState,
} from './api';
import {
  type OutlineItem,
  extractMarkdownOutline,
  findActiveHeadingIndex,
} from './outlineExtractor';
import {
  BP_COLLAPSE_LEFT_SIDEBAR,
  BP_COLLAPSE_RIGHT_PANEL,
  BP_OVERLAY_DRAWER,
  MIN_EDITOR_WIDTH,
  MIN_PANEL_WIDTH,
  SIDEBAR_COLLAPSED_STRIP_WIDTH,
  SIDEBAR_EXPANDED_WIDTH,
  getRightPanelDef,
} from './registry';

export const OUTLINE_DEBOUNCE_MS = 150;
export const DEFAULT_PANEL_WIDTH = 260;
export const PANEL_WIDTH_PERSIST_DEBOUNCE_MS = 180;

export { MIN_PANEL_WIDTH };

export type RelatedEmptyReason = 'no-doc' | 'outside-library' | 'empty' | null;

export const usePanelsStore = defineStore('panels', () => {
  const editorTabsStore = useEditorTabsStore();

  const panelGroup = ref<PanelGroupUiState>({
    visible: true,
    activeId: 'outline',
    width: DEFAULT_PANEL_WIDTH,
    collapsed: false,
  });

  const preferredVisible = ref<boolean>(true);
  const preferredCollapsed = ref<boolean>(false);
  const viewportWidth = ref<number>(1280);
  const forceOverlayDrawer = ref<boolean>(false);
  const leftSidebarCollapsed = ref<boolean>(false);
  const autoCollapsedByNarrowRight = ref<boolean>(false);
  let hasViewportMeasured = false;

  const outlineItems = ref<OutlineItem[]>([]);
  const activeHeadingIndex = ref<number>(-1);

  const relatedItems = ref<RelatedNoteItem[]>([]);
  const relatedLoading = ref<boolean>(false);
  const relatedError = ref<string | null>(null);
  const relatedEmptyReason = ref<RelatedEmptyReason>(null);

  const recentItems = ref<NoteSummary[]>([]);
  const recentLoading = ref<boolean>(false);

  let outlineDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let widthPersistTimer: ReturnType<typeof setTimeout> | null = null;
  let lastComputedKey: string | null = null;
  let relatedRequestSeq = 0;
  let recentRequestSeq = 0;

  const isPanelGroupExpanded = computed<boolean>(
    () => panelGroup.value.visible && !panelGroup.value.collapsed,
  );

  const isOutlineVisible = computed<boolean>(
    () => isPanelGroupExpanded.value && panelGroup.value.activeId === 'outline',
  );

  const isOverlayDrawer = computed<boolean>(
    () => viewportWidth.value < BP_OVERLAY_DRAWER || forceOverlayDrawer.value,
  );
  const isNarrowLeftBreakpoint = computed<boolean>(
    () => viewportWidth.value < BP_COLLAPSE_LEFT_SIDEBAR,
  );
  const isNarrowRightBreakpoint = computed<boolean>(
    () => viewportWidth.value < BP_COLLAPSE_RIGHT_PANEL,
  );

  function clearOutlineTimer(): void {
    if (outlineDebounceTimer !== null) {
      clearTimeout(outlineDebounceTimer);
      outlineDebounceTimer = null;
    }
  }

  function clearWidthPersistTimer(): void {
    if (widthPersistTimer !== null) {
      clearTimeout(widthPersistTimer);
      widthPersistTimer = null;
    }
  }

  function recomputeOutlineNow(): void {
    clearOutlineTimer();
    const tab = editorTabsStore.activeTab;
    const body = tab && !tab.isMissing ? tab.draftBody : '';
    outlineItems.value = extractMarkdownOutline(body);
    activeHeadingIndex.value = findActiveHeadingIndex(
      outlineItems.value,
      editorTabsStore.activeTopLine,
    );
  }

  function scheduleOutlineRecompute(): void {
    clearOutlineTimer();
    outlineDebounceTimer = setTimeout(() => {
      outlineDebounceTimer = null;
      recomputeOutlineNow();
    }, OUTLINE_DEBOUNCE_MS);
  }

  function persistPanelGroup(): void {
    clearWidthPersistTimer();
    void patchPanelGroupUiState({ ...panelGroup.value }).catch(() => undefined);
  }

  function schedulePersistPanelGroup(): void {
    clearWidthPersistTimer();
    widthPersistTimer = setTimeout(() => {
      widthPersistTimer = null;
      void patchPanelGroupUiState({ ...panelGroup.value }).catch(() => undefined);
    }, PANEL_WIDTH_PERSIST_DEBOUNCE_MS);
  }

  async function refreshRelatedNotes(): Promise<void> {
    const seq = ++relatedRequestSeq;
    const activeTab = editorTabsStore.activeTab;
    const activeDoc = editorTabsStore.activeDoc;

    if (!activeTab || activeTab.isMissing || !activeDoc) {
      relatedItems.value = [];
      relatedError.value = null;
      relatedLoading.value = false;
      relatedEmptyReason.value = 'no-doc';
      return;
    }

    if (!activeDoc.isLibrary || !activeDoc.relPath) {
      relatedItems.value = [];
      relatedError.value = null;
      relatedLoading.value = false;
      relatedEmptyReason.value = 'outside-library';
      return;
    }

    relatedLoading.value = true;
    relatedError.value = null;
    try {
      const items = await fetchRelatedNotes(activeDoc.relPath);
      if (seq !== relatedRequestSeq) {
        return;
      }
      relatedItems.value = items;
      relatedEmptyReason.value = items.length === 0 ? 'empty' : null;
    } catch (error) {
      if (seq !== relatedRequestSeq) {
        return;
      }
      relatedItems.value = [];
      relatedEmptyReason.value = 'empty';
      relatedError.value = error instanceof Error ? error.message : '关联文档加载失败';
    } finally {
      if (seq === relatedRequestSeq) {
        relatedLoading.value = false;
      }
    }
  }

  async function refreshRecentNotes(): Promise<void> {
    const seq = ++recentRequestSeq;
    recentLoading.value = true;
    try {
      const items = await fetchRecentNotes();
      if (seq !== recentRequestSeq) {
        return;
      }
      recentItems.value = items.slice(0, RECENT_NOTES_LIMIT);
    } catch {
      if (seq === recentRequestSeq) {
        recentItems.value = [];
      }
    } finally {
      if (seq === recentRequestSeq) {
        recentLoading.value = false;
      }
    }
  }

  function refreshActivePanelData(): void {
    if (!isPanelGroupExpanded.value) {
      return;
    }
    if (panelGroup.value.activeId === 'related') {
      void refreshRelatedNotes();
    } else if (panelGroup.value.activeId === 'recent') {
      void refreshRecentNotes();
    }
  }

  watch(
    () => [editorTabsStore.activeKey, editorTabsStore.activeTab?.draftBody ?? ''] as const,
    ([nextKey]) => {
      if (nextKey !== lastComputedKey) {
        lastComputedKey = nextKey;
        recomputeOutlineNow();
        refreshActivePanelData();
      } else {
        scheduleOutlineRecompute();
      }
    },
    { immediate: true },
  );

  watch(
    () => editorTabsStore.activeTopLine,
    (line) => {
      activeHeadingIndex.value = findActiveHeadingIndex(outlineItems.value, line);
    },
  );

  watch(
    () => editorTabsStore.activeTab?.saveStatus,
    (status) => {
      if (
        status === 'saved' &&
        isPanelGroupExpanded.value &&
        panelGroup.value.activeId === 'recent'
      ) {
        void refreshRecentNotes();
      }
    },
  );

  function applyPanelGroupState(nextGroup: PanelGroupUiState): void {
    const def = getRightPanelDef(nextGroup.activeId || 'outline');
    panelGroup.value = {
      visible: nextGroup.visible,
      activeId: def.id,
      width: Math.max(MIN_PANEL_WIDTH, nextGroup.width || def.defaultWidth),
      collapsed: nextGroup.collapsed,
    };
    preferredVisible.value = panelGroup.value.visible;
    preferredCollapsed.value = panelGroup.value.collapsed;
    refreshActivePanelData();
  }

  async function initialize(): Promise<void> {
    try {
      const uiState = await fetchUiState();
      if (uiState.panelGroup) {
        applyPanelGroupState(uiState.panelGroup);
      }
    } catch {
      // 容错：回退默认面板状态
    }
  }

  function selectPanel(id: PanelId): void {
    panelGroup.value.visible = true;
    panelGroup.value.collapsed = false;
    panelGroup.value.activeId = id;
    preferredVisible.value = true;
    preferredCollapsed.value = false;
    autoCollapsedByNarrowRight.value = false;
    persistPanelGroup();
    refreshActivePanelData();
  }

  function togglePanel(id: PanelId): void {
    if (
      !panelGroup.value.visible ||
      panelGroup.value.collapsed ||
      panelGroup.value.activeId !== id
    ) {
      selectPanel(id);
      return;
    }
    panelGroup.value.collapsed = true;
    preferredCollapsed.value = true;
    autoCollapsedByNarrowRight.value = false;
    persistPanelGroup();
  }

  function toggleOutlinePanel(): void {
    togglePanel('outline');
  }

  function toggleCollapseAll(): void {
    if (!panelGroup.value.visible || panelGroup.value.collapsed) {
      panelGroup.value.visible = true;
      panelGroup.value.collapsed = false;
      preferredVisible.value = true;
      preferredCollapsed.value = false;
      autoCollapsedByNarrowRight.value = false;
      refreshActivePanelData();
    } else {
      panelGroup.value.collapsed = true;
      preferredCollapsed.value = true;
      autoCollapsedByNarrowRight.value = false;
    }
    persistPanelGroup();
  }

  function computeMaxPanelWidth(vpWidth: number): number {
    if (vpWidth < BP_OVERLAY_DRAWER || forceOverlayDrawer.value) {
      return Math.max(MIN_PANEL_WIDTH, vpWidth - 48);
    }
    const leftWidth = leftSidebarCollapsed.value
      ? SIDEBAR_COLLAPSED_STRIP_WIDTH
      : SIDEBAR_EXPANDED_WIDTH;
    return Math.max(MIN_PANEL_WIDTH, vpWidth - leftWidth - MIN_EDITOR_WIDTH);
  }

  function setPanelWidth(nextWidth: number, vpWidth?: number): number {
    const effectiveVp = vpWidth && vpWidth > 0 ? vpWidth : viewportWidth.value;
    const maxAllowed = computeMaxPanelWidth(effectiveVp);
    const clamped = Math.min(maxAllowed, Math.max(MIN_PANEL_WIDTH, Math.round(nextWidth)));
    panelGroup.value.width = clamped;
    schedulePersistPanelGroup();
    return clamped;
  }

  function commitPanelWidth(): void {
    persistPanelGroup();
  }

  function applyViewportWidth(width: number): void {
    if (!Number.isFinite(width) || width <= 0) {
      return;
    }
    const prevWidth = viewportWidth.value;
    const firstMeasure = !hasViewportMeasured;
    hasViewportMeasured = true;
    viewportWidth.value = width;

    // 1. 左侧栏断点 < 1000px
    if (width < BP_COLLAPSE_LEFT_SIDEBAR) {
      if (firstMeasure || prevWidth >= BP_COLLAPSE_LEFT_SIDEBAR) {
        leftSidebarCollapsed.value = true;
      }
    } else if (prevWidth < BP_COLLAPSE_LEFT_SIDEBAR) {
      leftSidebarCollapsed.value = false;
    }

    // 2. 右侧栏断点 < 900px（抽屉模式）与 < 1200px（自动收起）
    if (width < BP_OVERLAY_DRAWER) {
      if (firstMeasure || prevWidth >= BP_OVERLAY_DRAWER) {
        if (isPanelGroupExpanded.value) {
          panelGroup.value.collapsed = true;
          autoCollapsedByNarrowRight.value = true;
        }
      }
    } else if (width < BP_COLLAPSE_RIGHT_PANEL) {
      if (firstMeasure || prevWidth >= BP_COLLAPSE_RIGHT_PANEL) {
        if (isPanelGroupExpanded.value) {
          panelGroup.value.collapsed = true;
          autoCollapsedByNarrowRight.value = true;
        }
      }
    } else {
      if (autoCollapsedByNarrowRight.value) {
        panelGroup.value.visible = preferredVisible.value;
        panelGroup.value.collapsed = preferredCollapsed.value;
        autoCollapsedByNarrowRight.value = false;
        refreshActivePanelData();
      }
    }

    // 3. 非抽屉展开状态下确保编辑区 >= 480px
    if (!isOverlayDrawer.value && isPanelGroupExpanded.value) {
      const maxAllowed = computeMaxPanelWidth(width);
      if (panelGroup.value.width > maxAllowed) {
        panelGroup.value.width = maxAllowed;
      }
    }
  }

  function setForceOverlayDrawer(enabled: boolean): void {
    forceOverlayDrawer.value = enabled;
    if (enabled) {
      leftSidebarCollapsed.value = true;
      if (isPanelGroupExpanded.value) {
        panelGroup.value.collapsed = true;
        autoCollapsedByNarrowRight.value = true;
      }
      return;
    }

    if (viewportWidth.value >= BP_COLLAPSE_LEFT_SIDEBAR) {
      leftSidebarCollapsed.value = false;
    }
    if (viewportWidth.value >= BP_COLLAPSE_RIGHT_PANEL && autoCollapsedByNarrowRight.value) {
      panelGroup.value.visible = preferredVisible.value;
      panelGroup.value.collapsed = preferredCollapsed.value;
      autoCollapsedByNarrowRight.value = false;
      refreshActivePanelData();
    }
  }

  function toggleLeftSidebar(): void {
    leftSidebarCollapsed.value = !leftSidebarCollapsed.value;
  }

  function jumpToHeading(item: OutlineItem): void {
    editorTabsStore.requestScrollToLine(item.line, item.from);
  }

  function dispose(): void {
    clearOutlineTimer();
    clearWidthPersistTimer();
  }

  return {
    panelGroup,
    viewportWidth,
    forceOverlayDrawer,
    leftSidebarCollapsed,
    outlineItems,
    activeHeadingIndex,
    relatedItems,
    relatedLoading,
    relatedError,
    relatedEmptyReason,
    recentItems,
    recentLoading,
    isPanelGroupExpanded,
    isOutlineVisible,
    isOverlayDrawer,
    isNarrowLeftBreakpoint,
    isNarrowRightBreakpoint,
    applyPanelGroupState,
    initialize,
    selectPanel,
    togglePanel,
    toggleOutlinePanel,
    toggleCollapseAll,
    setPanelWidth,
    commitPanelWidth,
    applyViewportWidth,
    setForceOverlayDrawer,
    toggleLeftSidebar,
    refreshRelatedNotes,
    refreshRecentNotes,
    jumpToHeading,
    recomputeOutlineNow,
    dispose,
  };
});
