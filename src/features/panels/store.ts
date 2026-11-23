import { computed, ref, watch } from 'vue';
import { defineStore } from 'pinia';
import type { PanelGroupUiState } from '@/bindings/types';
import { useEditorTabsStore } from '@/features/editor';
import { uiStateGet, uiStateSet } from '@/lib/ipc/commands';
import {
  type OutlineItem,
  extractMarkdownOutline,
  findActiveHeadingIndex,
} from './outlineExtractor';

export const OUTLINE_DEBOUNCE_MS = 150;
export const DEFAULT_PANEL_WIDTH = 260;
export const MIN_PANEL_WIDTH = 220;

export const usePanelsStore = defineStore('panels', () => {
  const editorTabsStore = useEditorTabsStore();

  const panelGroup = ref<PanelGroupUiState>({
    visible: true,
    activeId: 'outline',
    width: DEFAULT_PANEL_WIDTH,
    collapsed: false,
  });

  const outlineItems = ref<OutlineItem[]>([]);
  const activeHeadingIndex = ref<number>(-1);

  let outlineDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let lastComputedKey: string | null = null;

  const isOutlineVisible = computed<boolean>(
    () =>
      panelGroup.value.visible &&
      !panelGroup.value.collapsed &&
      panelGroup.value.activeId === 'outline',
  );

  function clearOutlineTimer(): void {
    if (outlineDebounceTimer !== null) {
      clearTimeout(outlineDebounceTimer);
      outlineDebounceTimer = null;
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

  watch(
    () => [editorTabsStore.activeKey, editorTabsStore.activeTab?.draftBody ?? ''] as const,
    ([nextKey]) => {
      if (nextKey !== lastComputedKey) {
        lastComputedKey = nextKey;
        recomputeOutlineNow();
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

  function persistPanelGroup(): void {
    void uiStateSet({
      key: 'panelGroup',
      value: { ...panelGroup.value },
    }).catch(() => undefined);
  }

  async function initialize(): Promise<void> {
    try {
      const uiState = await uiStateGet();
      if (uiState.panelGroup) {
        panelGroup.value = {
          visible: uiState.panelGroup.visible,
          activeId: uiState.panelGroup.activeId || 'outline',
          width: Math.max(MIN_PANEL_WIDTH, uiState.panelGroup.width || DEFAULT_PANEL_WIDTH),
          collapsed: uiState.panelGroup.collapsed,
        };
      }
    } catch {
      // 容错：回退默认面板状态
    }
  }

  function toggleOutlinePanel(): void {
    if (
      !panelGroup.value.visible ||
      panelGroup.value.collapsed ||
      panelGroup.value.activeId !== 'outline'
    ) {
      panelGroup.value.visible = true;
      panelGroup.value.collapsed = false;
      panelGroup.value.activeId = 'outline';
    } else {
      panelGroup.value.collapsed = true;
    }
    persistPanelGroup();
  }

  function toggleCollapseAll(): void {
    if (!panelGroup.value.visible || panelGroup.value.collapsed) {
      panelGroup.value.visible = true;
      panelGroup.value.collapsed = false;
    } else {
      panelGroup.value.collapsed = true;
    }
    persistPanelGroup();
  }

  function jumpToHeading(item: OutlineItem): void {
    editorTabsStore.requestScrollToLine(item.line, item.from);
  }

  function dispose(): void {
    clearOutlineTimer();
  }

  return {
    panelGroup,
    outlineItems,
    activeHeadingIndex,
    isOutlineVisible,
    initialize,
    toggleOutlinePanel,
    toggleCollapseAll,
    jumpToHeading,
    recomputeOutlineNow,
    dispose,
  };
});
