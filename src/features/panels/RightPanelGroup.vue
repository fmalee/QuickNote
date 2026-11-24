<script setup lang="ts">
import { computed, onUnmounted } from 'vue';
import { formatKeyComboDisplay } from '@/lib/actions';
import RecentNotesPanel from './RecentNotesPanel.vue';
import RelatedNotesPanel from './RelatedNotesPanel.vue';
import TerminalPlaceholderPanel from './TerminalPlaceholderPanel.vue';
import { COLLAPSED_PANEL_STRIP_WIDTH, RIGHT_PANEL_REGISTRY, getRightPanelDef } from './registry';
import { usePanelsStore } from './store';

const panelsStore = usePanelsStore();

const activeDef = computed(() => getRightPanelDef(panelsStore.panelGroup.activeId));

let dragging = false;
let dragStartX = 0;
let dragStartWidth = 260;

function onPointerMove(event: PointerEvent): void {
  if (!dragging) {
    return;
  }
  const deltaX = dragStartX - event.clientX;
  const vpWidth = typeof window !== 'undefined' ? window.innerWidth : panelsStore.viewportWidth;
  panelsStore.setPanelWidth(dragStartWidth + deltaX, vpWidth);
}

function stopDragging(): void {
  if (!dragging) {
    return;
  }
  dragging = false;
  if (typeof window !== 'undefined') {
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', stopDragging);
    window.removeEventListener('pointercancel', stopDragging);
  }
  panelsStore.commitPanelWidth();
}

function startDragging(event: PointerEvent): void {
  if (event.button !== 0) {
    return;
  }
  event.preventDefault();
  dragging = true;
  dragStartX = event.clientX;
  dragStartWidth = panelsStore.panelGroup.width;
  if (typeof window !== 'undefined') {
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', stopDragging);
    window.addEventListener('pointercancel', stopDragging);
  }
}

onUnmounted(() => {
  stopDragging();
});
</script>

<template>
  <!-- 收起态：36px 窄图标条 -->
  <aside
    v-if="!panelsStore.isPanelGroupExpanded"
    class="qn-right-panel-strip"
    data-testid="right-panel-collapsed-strip"
    :style="{ width: `${COLLAPSED_PANEL_STRIP_WIDTH}px` }"
  >
    <button
      type="button"
      class="qn-right-panel-strip__btn"
      title="展开右侧面板 (⌃⌘0)"
      data-testid="right-panel-expand-btn"
      @click="panelsStore.toggleCollapseAll()"
    >
      ‹
    </button>
    <div class="qn-right-panel-strip__divider" />
    <button
      v-for="panel in RIGHT_PANEL_REGISTRY"
      :key="panel.id"
      type="button"
      :class="[
        'qn-right-panel-strip__btn',
        { 'qn-right-panel-strip__btn--last-active': panelsStore.panelGroup.activeId === panel.id },
      ]"
      :title="`${panel.title} (${formatKeyComboDisplay(panel.shortcut)})`"
      :data-testid="`right-panel-strip-${panel.id}`"
      @click="panelsStore.selectPanel(panel.id)"
    >
      {{ panel.shortLabel }}
    </button>
  </aside>

  <!-- 展开态：常规右侧栏或 <900px 覆盖式抽屉 -->
  <template v-else>
    <div
      v-if="panelsStore.isOverlayDrawer"
      class="qn-right-panel-backdrop"
      data-testid="right-panel-drawer-backdrop"
      @click="panelsStore.toggleCollapseAll()"
    />
    <aside
      :class="[
        'qn-right-panel-group',
        { 'qn-right-panel-group--drawer': panelsStore.isOverlayDrawer },
      ]"
      data-testid="right-panel-group"
      :style="{ width: `${panelsStore.panelGroup.width}px` }"
    >
      <div
        class="qn-right-panel-group__resizer"
        role="separator"
        aria-orientation="vertical"
        aria-label="调整右侧面板宽度"
        data-testid="right-panel-resizer"
        @pointerdown="startDragging"
      />

      <header class="qn-right-panel-group__header">
        <div class="qn-right-panel-group__tabs" role="tablist" aria-label="右侧面板">
          <button
            v-for="panel in RIGHT_PANEL_REGISTRY"
            :key="panel.id"
            type="button"
            role="tab"
            :aria-selected="panelsStore.panelGroup.activeId === panel.id"
            :title="`${panel.title} (${formatKeyComboDisplay(panel.shortcut)})`"
            :data-testid="`right-panel-tab-${panel.id}`"
            :class="[
              'qn-right-panel-group__tab',
              {
                'qn-right-panel-group__tab--active': panelsStore.panelGroup.activeId === panel.id,
              },
            ]"
            @click="panelsStore.selectPanel(panel.id)"
          >
            {{ panel.title }}
          </button>
        </div>

        <button
          type="button"
          class="qn-right-panel-group__collapse-btn"
          :title="`折叠右侧面板 (⌃⌘0) · ${activeDef.title}`"
          data-testid="right-panel-collapse-btn"
          @click="panelsStore.toggleCollapseAll()"
        >
          ›
        </button>
      </header>

      <div class="qn-right-panel-group__body">
        <!-- 大纲 -->
        <div
          v-if="panelsStore.panelGroup.activeId === 'outline'"
          class="qn-right-panel-group__outline"
          data-testid="outline-panel"
        >
          <div v-if="panelsStore.outlineItems.length === 0" class="qn-right-panel-group__empty">
            当前文档暂无标题
          </div>
          <ul v-else class="qn-right-panel-group__outline-list" data-testid="outline-list">
            <li
              v-for="(item, index) in panelsStore.outlineItems"
              :key="`${item.slug}-${item.line}`"
              class="qn-right-panel-group__outline-item-wrap"
            >
              <button
                type="button"
                :class="[
                  'qn-right-panel-group__outline-item',
                  {
                    'qn-right-panel-group__outline-item--active':
                      index === panelsStore.activeHeadingIndex,
                  },
                ]"
                :style="{ paddingLeft: `calc(var(--qn-space-2) + ${(item.level - 1) * 12}px)` }"
                :data-testid="`outline-item-${item.slug}`"
                @click="panelsStore.jumpToHeading(item)"
              >
                <span class="qn-right-panel-group__outline-text">{{ item.text }}</span>
              </button>
            </li>
          </ul>
        </div>

        <!-- 终端占位 -->
        <TerminalPlaceholderPanel v-else-if="panelsStore.panelGroup.activeId === 'terminal'" />

        <!-- 相关文档 -->
        <RelatedNotesPanel v-else-if="panelsStore.panelGroup.activeId === 'related'" />

        <!-- 最近编辑 -->
        <RecentNotesPanel v-else-if="panelsStore.panelGroup.activeId === 'recent'" />
      </div>
    </aside>
  </template>
</template>

<style scoped>
.qn-right-panel-strip {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2) 0;
  border-left: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-right-panel-strip__divider {
  width: 20px;
  height: 1px;
  background-color: var(--qn-border-subtle);
  margin: 2px 0;
}

.qn-right-panel-strip__btn {
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

.qn-right-panel-strip__btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-right-panel-strip__btn--last-active {
  color: var(--qn-accent);
}

.qn-right-panel-backdrop {
  position: absolute;
  inset: 0;
  background-color: var(--qn-bg-backdrop);
  z-index: var(--qn-z-drawer);
}

.qn-right-panel-group {
  position: relative;
  flex-shrink: 0;
  min-width: 220px;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  min-height: 0;
}

.qn-right-panel-group--drawer {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: var(--qn-z-drawer);
  box-shadow: var(--qn-shadow-lg);
}

.qn-right-panel-group__resizer {
  position: absolute;
  left: -3px;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: col-resize;
  z-index: var(--qn-z-base);
}

.qn-right-panel-group__resizer:hover {
  background-color: var(--qn-accent-soft);
}

.qn-right-panel-group__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2);
  border-bottom: 1px solid var(--qn-border-subtle);
}

.qn-right-panel-group__tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
  overflow-x: auto;
}

.qn-right-panel-group__tab {
  border: none;
  background: transparent;
  padding: 3px 6px;
  border-radius: var(--qn-radius-sm);
  font-size: var(--qn-text-xs);
  font-weight: 500;
  color: var(--qn-text-secondary);
  white-space: nowrap;
  cursor: pointer;
}

.qn-right-panel-group__tab:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-right-panel-group__tab--active {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
  font-weight: 600;
}

.qn-right-panel-group__collapse-btn {
  flex-shrink: 0;
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

.qn-right-panel-group__collapse-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-right-panel-group__body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.qn-right-panel-group__outline {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.qn-right-panel-group__empty {
  padding: var(--qn-space-4) var(--qn-space-3);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-right-panel-group__outline-list {
  list-style: none;
  margin: 0;
  padding: var(--qn-space-2);
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.qn-right-panel-group__outline-item-wrap {
  min-width: 0;
}

.qn-right-panel-group__outline-item {
  width: 100%;
  display: flex;
  align-items: center;
  border: none;
  background: transparent;
  text-align: left;
  padding-top: var(--qn-space-1);
  padding-bottom: var(--qn-space-1);
  padding-right: var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-sm);
  cursor: pointer;
}

.qn-right-panel-group__outline-item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-right-panel-group__outline-item--active {
  background-color: var(--qn-accent-soft);
  color: var(--qn-accent);
  font-weight: 500;
}

.qn-right-panel-group__outline-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
