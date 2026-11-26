import { computed, ref, watch } from 'vue';
import { defineStore } from 'pinia';
import type {
  LeftSidebarUiState,
  LeftViewId,
  RelatedNoteItem,
  TerminalPanelUiState,
} from '@/bindings/types';
import { useEditorTabsStore } from '@/features/editor';
import {
  fetchRelatedNotes,
  fetchUiState,
  patchLeftSidebarUiState,
  patchTerminalPanelUiState,
} from './api';
import {
  type OutlineItem,
  extractMarkdownOutline,
  findActiveHeadingIndex,
} from './outlineExtractor';
import {
  BP_COLLAPSE_LEFT_SIDEBAR,
  BP_OVERLAY_DRAWER,
  MIN_EDITOR_WIDTH,
  SIDEBAR_COLLAPSED_STRIP_WIDTH,
} from './registry';

export const OUTLINE_DEBOUNCE_MS = 150;
export const MIN_SIDEBAR_WIDTH = 260;
export const MIN_TERMINAL_DOCK_HEIGHT = 120;
export const MAX_TERMINAL_DOCK_HEIGHT = 600;

const LAYOUT_PERSIST_DEBOUNCE_MS = 180;

export type RelatedEmptyReason = 'no-doc' | 'outside-library' | 'empty' | null;

export const usePanelsStore = defineStore('panels', () => {
  const editorTabsStore = useEditorTabsStore();

  const leftSidebar = ref<LeftSidebarUiState>({
    width: 300,
    activeView: 'workspace',
    collapsed: false,
  });
  const terminalPanel = ref<TerminalPanelUiState>({
    visible: false,
    height: 240,
    collapsed: false,
  });

  const viewportWidth = ref<number>(1280);
  const forceOverlayDrawer = ref<boolean>(false);
  const leftSidebarCollapsed = ref<boolean>(false);
  let hasViewportMeasured = false;

  const outlineItems = ref<OutlineItem[]>([]);
  const activeHeadingIndex = ref<number>(-1);

  const relatedItems = ref<RelatedNoteItem[]>([]);
  const relatedLoading = ref<boolean>(false);
  const relatedError = ref<string | null>(null);
  const relatedEmptyReason = ref<RelatedEmptyReason>(null);

  let outlineDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let leftSidebarWidthPersistTimer: ReturnType<typeof setTimeout> | null = null;
  let terminalHeightPersistTimer: ReturnType<typeof setTimeout> | null = null;
  let lastComputedKey: string | null = null;
  let relatedRequestSeq = 0;

  const isTerminalPanelShown = computed<boolean>(
    () => terminalPanel.value.visible && !terminalPanel.value.collapsed,
  );

  const leftSidebarWidth = computed<number>(() => {
    const maxAllowed = Math.max(
      MIN_SIDEBAR_WIDTH,
      viewportWidth.value - MIN_EDITOR_WIDTH - SIDEBAR_COLLAPSED_STRIP_WIDTH,
    );
    const raw = isOverlayDrawer.value
      ? Math.max(MIN_SIDEBAR_WIDTH, viewportWidth.value - 48)
      : leftSidebar.value.width;
    return Math.min(maxAllowed, Math.max(MIN_SIDEBAR_WIDTH, raw));
  });

  const isOverlayDrawer = computed<boolean>(
    () => viewportWidth.value < BP_OVERLAY_DRAWER || forceOverlayDrawer.value,
  );
  const isNarrowLeftBreakpoint = computed<boolean>(
    () => viewportWidth.value < BP_COLLAPSE_LEFT_SIDEBAR,
  );

  function clearOutlineTimer(): void {
    if (outlineDebounceTimer !== null) {
      clearTimeout(outlineDebounceTimer);
      outlineDebounceTimer = null;
    }
  }

  function clearLeftSidebarWidthPersistTimer(): void {
    if (leftSidebarWidthPersistTimer !== null) {
      clearTimeout(leftSidebarWidthPersistTimer);
      leftSidebarWidthPersistTimer = null;
    }
  }

  function clearTerminalHeightPersistTimer(): void {
    if (terminalHeightPersistTimer !== null) {
      clearTimeout(terminalHeightPersistTimer);
      terminalHeightPersistTimer = null;
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

  function persistLeftSidebar(): void {
    clearLeftSidebarWidthPersistTimer();
    void patchLeftSidebarUiState({ ...leftSidebar.value }).catch(() => undefined);
  }

  function schedulePersistLeftSidebar(): void {
    clearLeftSidebarWidthPersistTimer();
    leftSidebarWidthPersistTimer = setTimeout(() => {
      leftSidebarWidthPersistTimer = null;
      void patchLeftSidebarUiState({ ...leftSidebar.value }).catch(() => undefined);
    }, LAYOUT_PERSIST_DEBOUNCE_MS);
  }

  function persistTerminalPanel(): void {
    clearTerminalHeightPersistTimer();
    void patchTerminalPanelUiState({ ...terminalPanel.value }).catch(() => undefined);
  }

  function schedulePersistTerminalPanel(): void {
    clearTerminalHeightPersistTimer();
    terminalHeightPersistTimer = setTimeout(() => {
      terminalHeightPersistTimer = null;
      void patchTerminalPanelUiState({ ...terminalPanel.value }).catch(() => undefined);
    }, LAYOUT_PERSIST_DEBOUNCE_MS);
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

  function refreshActivePanelData(): void {
    if (leftSidebar.value.activeView === 'related') {
      void refreshRelatedNotes();
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
    () => leftSidebar.value.activeView,
    (view) => {
      if (view === 'related') {
        void refreshRelatedNotes();
      }
    },
  );

  function clampLeftSidebarWidth(width: number): number {
    const maxAllowed = Math.max(
      MIN_SIDEBAR_WIDTH,
      viewportWidth.value - MIN_EDITOR_WIDTH - SIDEBAR_COLLAPSED_STRIP_WIDTH,
    );
    return Math.min(maxAllowed, Math.max(MIN_SIDEBAR_WIDTH, Math.round(width)));
  }

  function clampTerminalPanelHeight(height: number): number {
    return Math.min(
      MAX_TERMINAL_DOCK_HEIGHT,
      Math.max(MIN_TERMINAL_DOCK_HEIGHT, Math.round(height)),
    );
  }

  async function initialize(): Promise<void> {
    try {
      const uiState = await fetchUiState();
      if (uiState.leftSidebar) {
        leftSidebar.value = {
          width: clampLeftSidebarWidth(uiState.leftSidebar.width),
          activeView: uiState.leftSidebar.activeView ?? 'workspace',
          collapsed: uiState.leftSidebar.collapsed,
        };
      }
      if (uiState.terminalPanel) {
        terminalPanel.value = {
          visible: uiState.terminalPanel.visible,
          height: clampTerminalPanelHeight(uiState.terminalPanel.height),
          collapsed: uiState.terminalPanel.collapsed,
        };
      }
    } catch {
      // 容错：回退默认面板状态
    }
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

    // 2. 窄窗抽屉断点 < 900px：若底部终端展开则自动折叠（不持久化）
    if (width < BP_OVERLAY_DRAWER) {
      if ((firstMeasure || prevWidth >= BP_OVERLAY_DRAWER) && isTerminalPanelShown.value) {
        terminalPanel.value.collapsed = true;
      }
    }
  }

  function setForceOverlayDrawer(enabled: boolean): void {
    forceOverlayDrawer.value = enabled;
    if (enabled) {
      leftSidebarCollapsed.value = true;
      return;
    }

    if (viewportWidth.value >= BP_COLLAPSE_LEFT_SIDEBAR) {
      leftSidebarCollapsed.value = false;
    }
  }

  function toggleLeftSidebar(): void {
    leftSidebarCollapsed.value = !leftSidebarCollapsed.value;
  }

  function setLeftView(view: LeftViewId): void {
    leftSidebar.value.activeView = view;
    persistLeftSidebar();
  }

  function toggleLeftView(view: 'related' | 'outline'): void {
    leftSidebar.value.activeView = leftSidebar.value.activeView === view ? 'workspace' : view;
    persistLeftSidebar();
  }

  function setLeftSidebarWidth(width: number): void {
    leftSidebar.value.width = clampLeftSidebarWidth(width);
    schedulePersistLeftSidebar();
  }

  function commitLeftSidebarWidth(): void {
    persistLeftSidebar();
  }

  function toggleTerminalPanel(): void {
    if (isTerminalPanelShown.value) {
      terminalPanel.value.visible = false;
    } else {
      terminalPanel.value.visible = true;
      terminalPanel.value.collapsed = false;
    }
    persistTerminalPanel();
  }

  function toggleTerminalCollapse(): void {
    terminalPanel.value.visible = true;
    terminalPanel.value.collapsed = !terminalPanel.value.collapsed;
    persistTerminalPanel();
  }

  function setTerminalPanelHeight(height: number): void {
    terminalPanel.value.height = clampTerminalPanelHeight(height);
    schedulePersistTerminalPanel();
  }

  function commitTerminalPanelHeight(): void {
    persistTerminalPanel();
  }

  function jumpToHeading(item: OutlineItem): void {
    editorTabsStore.requestScrollToLine(item.line, item.from);
  }

  function dispose(): void {
    clearOutlineTimer();
    clearLeftSidebarWidthPersistTimer();
    clearTerminalHeightPersistTimer();
  }

  return {
    leftSidebar,
    terminalPanel,
    viewportWidth,
    forceOverlayDrawer,
    leftSidebarCollapsed,
    outlineItems,
    activeHeadingIndex,
    relatedItems,
    relatedLoading,
    relatedError,
    relatedEmptyReason,
    isTerminalPanelShown,
    leftSidebarWidth,
    isOverlayDrawer,
    isNarrowLeftBreakpoint,
    initialize,
    applyViewportWidth,
    setForceOverlayDrawer,
    toggleLeftSidebar,
    setLeftView,
    toggleLeftView,
    setLeftSidebarWidth,
    commitLeftSidebarWidth,
    toggleTerminalPanel,
    toggleTerminalCollapse,
    setTerminalPanelHeight,
    commitTerminalPanelHeight,
    refreshRelatedNotes,
    jumpToHeading,
    recomputeOutlineNow,
    dispose,
  };
});
