<script lang="ts">
export type ContextMenuEntry =
  | {
      type: 'item';
      id: string;
      title: string;
      shortcutLabel?: string;
      disabled?: boolean;
      disabledReason?: string | null;
      danger?: boolean;
    }
  | {
      type: 'separator';
      id: string;
    }
  | {
      type: 'submenu';
      id: string;
      title: string;
      disabled?: boolean;
      items: ContextMenuEntry[];
    };
</script>

<script setup lang="ts">
import { nextTick, onUnmounted, ref, watch } from 'vue';

const props = defineProps<{
  open: boolean;
  x: number;
  y: number;
  items: ContextMenuEntry[];
}>();

const emit = defineEmits<{
  select: [id: string];
  close: [];
}>();

const VIEWPORT_PADDING = 8;

const menuRef = ref<HTMLElement | null>(null);
const clampedX = ref(props.x);
const clampedY = ref(props.y);
const openSubmenuId = ref<string | null>(null);

function clampToViewport(): void {
  const vw = typeof window !== 'undefined' ? window.innerWidth : 1280;
  const vh = typeof window !== 'undefined' ? window.innerHeight : 800;
  const rect = menuRef.value?.getBoundingClientRect();
  const width = rect?.width ?? 220;
  const height = rect?.height ?? 200;

  const maxX = Math.max(VIEWPORT_PADDING, vw - width - VIEWPORT_PADDING);
  const maxY = Math.max(VIEWPORT_PADDING, vh - height - VIEWPORT_PADDING);

  clampedX.value = Math.min(Math.max(VIEWPORT_PADDING, props.x), maxX);
  clampedY.value = Math.min(Math.max(VIEWPORT_PADDING, props.y), maxY);
}

function handleDocumentPointerDown(event: PointerEvent): void {
  if (!props.open) {
    return;
  }
  const target = event.target;
  if (target instanceof Node && menuRef.value?.contains(target)) {
    return;
  }
  emit('close');
}

function handleDocumentKeyDown(event: KeyboardEvent): void {
  if (!props.open) {
    return;
  }
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    emit('close');
  }
}

function attachGlobalListeners(): void {
  if (typeof document === 'undefined') {
    return;
  }
  document.addEventListener('pointerdown', handleDocumentPointerDown, true);
  document.addEventListener('keydown', handleDocumentKeyDown, true);
}

function detachGlobalListeners(): void {
  if (typeof document === 'undefined') {
    return;
  }
  document.removeEventListener('pointerdown', handleDocumentPointerDown, true);
  document.removeEventListener('keydown', handleDocumentKeyDown, true);
}

watch(
  () => [props.open, props.x, props.y, props.items.length] as const,
  ([isOpen]) => {
    if (isOpen) {
      openSubmenuId.value = null;
      clampedX.value = props.x;
      clampedY.value = props.y;
      attachGlobalListeners();
      void nextTick(() => {
        clampToViewport();
      });
    } else {
      openSubmenuId.value = null;
      detachGlobalListeners();
    }
  },
  { immediate: true },
);

onUnmounted(() => {
  detachGlobalListeners();
});

function handleItemClick(entry: Extract<ContextMenuEntry, { type: 'item' }>): void {
  if (entry.disabled) {
    return;
  }
  emit('select', entry.id);
  emit('close');
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      ref="menuRef"
      class="qn-context-menu"
      role="menu"
      :style="{ left: `${clampedX}px`, top: `${clampedY}px` }"
      @contextmenu.prevent
    >
      <template v-for="entry in items" :key="entry.id">
        <div
          v-if="entry.type === 'separator'"
          class="qn-context-menu__separator"
          role="separator"
        />
        <div
          v-else-if="entry.type === 'submenu'"
          class="qn-context-menu__submenu-wrapper"
          @mouseenter="!entry.disabled && (openSubmenuId = entry.id)"
          @mouseleave="openSubmenuId === entry.id && (openSubmenuId = null)"
        >
          <button
            type="button"
            class="qn-context-menu__item"
            :class="{ 'qn-context-menu__item--disabled': entry.disabled }"
            role="menuitem"
            aria-haspopup="menu"
            :aria-expanded="!entry.disabled && openSubmenuId === entry.id"
            :data-menu-id="entry.id"
            :disabled="entry.disabled"
            @focus="!entry.disabled && (openSubmenuId = entry.id)"
          >
            <span class="qn-context-menu__main">
              <span class="qn-context-menu__label">{{ entry.title }}</span>
            </span>
            <span class="qn-context-menu__submenu-arrow" aria-hidden="true">▸</span>
          </button>
          <div
            v-if="!entry.disabled && openSubmenuId === entry.id"
            class="qn-context-menu__submenu"
            role="menu"
          >
            <template v-for="subEntry in entry.items" :key="subEntry.id">
              <div
                v-if="subEntry.type === 'separator'"
                class="qn-context-menu__separator"
                role="separator"
              />
              <button
                v-else-if="subEntry.type === 'item'"
                type="button"
                class="qn-context-menu__item"
                :class="{
                  'qn-context-menu__item--disabled': subEntry.disabled,
                  'qn-context-menu__item--danger': subEntry.danger,
                }"
                role="menuitem"
                :data-menu-id="subEntry.id"
                :disabled="subEntry.disabled"
                :title="
                  subEntry.disabled && subEntry.disabledReason ? subEntry.disabledReason : undefined
                "
                @click="handleItemClick(subEntry)"
              >
                <span class="qn-context-menu__main">
                  <span class="qn-context-menu__label">{{ subEntry.title }}</span>
                  <span
                    v-if="subEntry.disabled && subEntry.disabledReason"
                    class="qn-context-menu__reason"
                  >
                    {{ subEntry.disabledReason }}
                  </span>
                </span>
                <span v-if="subEntry.shortcutLabel" class="qn-context-menu__shortcut">
                  {{ subEntry.shortcutLabel }}
                </span>
              </button>
            </template>
          </div>
        </div>
        <button
          v-else
          type="button"
          class="qn-context-menu__item"
          :class="{
            'qn-context-menu__item--disabled': entry.disabled,
            'qn-context-menu__item--danger': entry.danger,
          }"
          role="menuitem"
          :data-menu-id="entry.id"
          :disabled="entry.disabled"
          :title="entry.disabled && entry.disabledReason ? entry.disabledReason : undefined"
          @click="handleItemClick(entry)"
        >
          <span class="qn-context-menu__main">
            <span class="qn-context-menu__label">{{ entry.title }}</span>
            <span v-if="entry.disabled && entry.disabledReason" class="qn-context-menu__reason">
              {{ entry.disabledReason }}
            </span>
          </span>
          <span v-if="entry.shortcutLabel" class="qn-context-menu__shortcut">
            {{ entry.shortcutLabel }}
          </span>
        </button>
      </template>
    </div>
  </Teleport>
</template>

<style scoped>
.qn-context-menu {
  position: fixed;
  z-index: var(--qn-z-popover);
  min-width: 196px;
  max-width: 300px;
  padding: var(--qn-space-1);
  border-radius: var(--qn-radius-md);
  border: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-elevated);
  box-shadow: var(--qn-shadow-md);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.qn-context-menu__separator {
  height: 1px;
  margin: var(--qn-space-1) var(--qn-space-2);
  background-color: var(--qn-border-subtle);
}

.qn-context-menu__item {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-4);
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-primary);
  font-family: var(--qn-font-sans);
  font-size: var(--qn-text-sm);
  line-height: 1.4;
  text-align: left;
  cursor: pointer;
}

.qn-context-menu__item:hover:not(:disabled) {
  background-color: var(--qn-bg-hover);
}

.qn-context-menu__item--danger {
  color: var(--qn-danger);
}

.qn-context-menu__item--danger:hover:not(:disabled) {
  background-color: var(--qn-danger-soft);
}

.qn-context-menu__item--disabled {
  color: var(--qn-text-muted);
  opacity: 0.6;
  cursor: not-allowed;
}

.qn-context-menu__main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.qn-context-menu__label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-context-menu__reason {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-context-menu__shortcut {
  flex-shrink: 0;
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-context-menu__submenu-wrapper {
  position: relative;
}

.qn-context-menu__submenu-arrow {
  flex-shrink: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-context-menu__submenu {
  position: absolute;
  top: 0;
  left: 100%;
  z-index: var(--qn-z-popover);
  min-width: 196px;
  max-width: 300px;
  padding: var(--qn-space-1);
  border-radius: var(--qn-radius-md);
  border: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-elevated);
  box-shadow: var(--qn-shadow-md);
  display: flex;
  flex-direction: column;
  gap: 2px;
}
</style>
