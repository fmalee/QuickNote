<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref } from 'vue';
import type { SearchMatchSummary, SearchOptionsState } from './extensions/search';

const DEFAULT_WIDTH = 400;
const MIN_WIDTH = 340;
const MAX_WIDTH = 720;

const props = defineProps<{
  open: boolean;
  replaceExpanded: boolean;
  modelValue: SearchOptionsState;
  summary: SearchMatchSummary;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: SearchOptionsState];
  'update:replaceExpanded': [value: boolean];
  findNext: [];
  findPrev: [];
  replaceOne: [];
  replaceAll: [];
  close: [];
}>();

const searchInputRef = ref<HTMLInputElement | null>(null);
const replaceInputRef = ref<HTMLInputElement | null>(null);
const isComposing = ref(false);
const barWidth = ref(DEFAULT_WIDTH);
const isResizing = ref(false);

let resizeStartX = 0;
let resizeStartWidth = DEFAULT_WIDTH;

const countText = computed(() => {
  if (!props.summary.valid) {
    return '正则错误';
  }
  if (!props.modelValue.search) {
    return '';
  }
  if (props.summary.total === 0) {
    return '无结果';
  }
  const suffix = props.summary.capped ? '+' : '';
  if (props.summary.current > 0) {
    return `${props.summary.current} / ${props.summary.total}${suffix}`;
  }
  return `共 ${props.summary.total}${suffix} 项`;
});

const isCountError = computed(
  () => !props.summary.valid || (Boolean(props.modelValue.search) && props.summary.total === 0),
);

function updateField<K extends keyof SearchOptionsState>(
  key: K,
  value: SearchOptionsState[K],
): void {
  emit('update:modelValue', {
    ...props.modelValue,
    [key]: value,
  });
}

function toggleOption(key: 'caseSensitive' | 'wholeWord' | 'regexp'): void {
  updateField(key, !props.modelValue[key]);
}

function handleSearchInput(event: Event): void {
  const target = event.target as HTMLInputElement;
  updateField('search', target.value);
}

function handleReplaceInput(event: Event): void {
  const target = event.target as HTMLInputElement;
  updateField('replace', target.value);
}

function isImeActive(event: KeyboardEvent): boolean {
  return event.isComposing || isComposing.value || event.keyCode === 229;
}

function handleSearchKeyDown(event: KeyboardEvent): void {
  if (isImeActive(event)) {
    return;
  }
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    emit('close');
    return;
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    event.stopPropagation();
    if (event.shiftKey) {
      emit('findPrev');
    } else {
      emit('findNext');
    }
  }
}

function handleReplaceKeyDown(event: KeyboardEvent): void {
  if (isImeActive(event)) {
    return;
  }
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    emit('close');
    return;
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    event.stopPropagation();
    if (event.metaKey || event.ctrlKey) {
      emit('replaceAll');
    } else {
      emit('replaceOne');
    }
  }
}

function handleResizePointerMove(event: PointerEvent): void {
  if (!isResizing.value) {
    return;
  }
  // 浮层右对齐，向左拖动增加宽度
  const deltaX = resizeStartX - event.clientX;
  const nextWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, resizeStartWidth + deltaX));
  barWidth.value = Math.round(nextWidth);
}

function stopResize(): void {
  if (!isResizing.value) {
    return;
  }
  isResizing.value = false;
  if (typeof window !== 'undefined') {
    window.removeEventListener('pointermove', handleResizePointerMove);
    window.removeEventListener('pointerup', stopResize);
    window.removeEventListener('pointercancel', stopResize);
  }
}

function startResize(event: PointerEvent): void {
  if (event.button !== 0) {
    return;
  }
  event.preventDefault();
  isResizing.value = true;
  resizeStartX = event.clientX;
  resizeStartWidth = barWidth.value;
  if (typeof window !== 'undefined') {
    window.addEventListener('pointermove', handleResizePointerMove);
    window.addEventListener('pointerup', stopResize);
    window.addEventListener('pointercancel', stopResize);
  }
}

function focusSearchInput(selectAll = true): void {
  void nextTick(() => {
    const input = searchInputRef.value;
    if (!input) {
      return;
    }
    input.focus();
    if (selectAll) {
      input.select();
    }
  });
}

function focusReplaceInput(selectAll = true): void {
  void nextTick(() => {
    const input = replaceInputRef.value;
    if (!input) {
      return;
    }
    input.focus();
    if (selectAll) {
      input.select();
    }
  });
}

defineExpose({
  focusSearchInput,
  focusReplaceInput,
});

onUnmounted(() => {
  stopResize();
});
</script>

<template>
  <div
    v-if="open"
    class="qn-find-bar"
    :class="{ 'is-resizing': isResizing }"
    :style="{ width: `${barWidth}px` }"
    data-testid="editor-find-replace-bar"
    role="region"
    aria-label="查找与替换"
  >
    <div
      class="qn-find-bar__resizer"
      data-testid="editor-find-bar-resizer"
      title="拖动调整查找框宽度"
      @pointerdown="startResize"
    />

    <button
      type="button"
      class="qn-find-bar__toggle"
      :aria-expanded="replaceExpanded"
      :title="replaceExpanded ? '收起替换栏' : '展开替换栏'"
      data-testid="editor-find-toggle-replace"
      @click="emit('update:replaceExpanded', !replaceExpanded)"
    >
      <svg
        width="12"
        height="12"
        viewBox="0 0 12 12"
        fill="none"
        :class="{ 'is-expanded': replaceExpanded }"
        aria-hidden="true"
      >
        <path
          d="M4.25 2.5L7.75 6L4.25 9.5"
          stroke="currentColor"
          stroke-width="1.4"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </button>

    <div class="qn-find-bar__rows">
      <div class="qn-find-bar__row">
        <div class="qn-find-bar__input-wrap" :class="{ 'has-error': !summary.valid }">
          <input
            ref="searchInputRef"
            :value="modelValue.search"
            type="text"
            class="qn-find-bar__input"
            placeholder="查找"
            aria-label="查找"
            data-testid="editor-find-input"
            @input="handleSearchInput"
            @compositionstart="isComposing = true"
            @compositionend="isComposing = false"
            @keydown="handleSearchKeyDown"
          />
          <div class="qn-find-bar__toggles">
            <button
              type="button"
              class="qn-find-bar__opt-btn"
              :class="{ 'is-active': modelValue.caseSensitive }"
              :aria-pressed="modelValue.caseSensitive"
              title="区分大小写"
              data-testid="editor-find-opt-case"
              @click="toggleOption('caseSensitive')"
            >
              Aa
            </button>
            <button
              type="button"
              class="qn-find-bar__opt-btn"
              :class="{ 'is-active': modelValue.wholeWord }"
              :aria-pressed="modelValue.wholeWord"
              title="全字匹配"
              data-testid="editor-find-opt-word"
              @click="toggleOption('wholeWord')"
            >
              ab
            </button>
            <button
              type="button"
              class="qn-find-bar__opt-btn"
              :class="{ 'is-active': modelValue.regexp }"
              :aria-pressed="modelValue.regexp"
              title="使用正则表达式"
              data-testid="editor-find-opt-regex"
              @click="toggleOption('regexp')"
            >
              .*
            </button>
          </div>
        </div>

        <span
          class="qn-find-bar__count"
          :class="{ 'is-error': isCountError }"
          data-testid="editor-find-count"
        >
          {{ countText }}
        </span>

        <div class="qn-find-bar__actions">
          <button
            type="button"
            class="qn-find-bar__icon-btn"
            :disabled="!modelValue.search || !summary.valid || summary.total === 0"
            title="上一个匹配项 (⇧⌘G / Shift+Enter)"
            aria-label="上一个匹配项"
            data-testid="editor-find-prev"
            @click="emit('findPrev')"
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
              <path
                d="M7 11V3M7 3L3.75 6.25M7 3L10.25 6.25"
                stroke="currentColor"
                stroke-width="1.35"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
          <button
            type="button"
            class="qn-find-bar__icon-btn"
            :disabled="!modelValue.search || !summary.valid || summary.total === 0"
            title="下一个匹配项 (⌘G / Enter)"
            aria-label="下一个匹配项"
            data-testid="editor-find-next"
            @click="emit('findNext')"
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
              <path
                d="M7 3V11M7 11L3.75 7.75M7 11L10.25 7.75"
                stroke="currentColor"
                stroke-width="1.35"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
          <button
            type="button"
            class="qn-find-bar__icon-btn"
            title="关闭 (Esc)"
            aria-label="关闭查找面板"
            data-testid="editor-find-close"
            @click="emit('close')"
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
              <path
                d="M3.5 3.5L10.5 10.5M10.5 3.5L3.5 10.5"
                stroke="currentColor"
                stroke-width="1.35"
                stroke-linecap="round"
              />
            </svg>
          </button>
        </div>
      </div>

      <div v-show="replaceExpanded" class="qn-find-bar__row">
        <div class="qn-find-bar__input-wrap">
          <input
            ref="replaceInputRef"
            :value="modelValue.replace"
            type="text"
            class="qn-find-bar__input"
            placeholder="替换为"
            aria-label="替换为"
            data-testid="editor-replace-input"
            @input="handleReplaceInput"
            @compositionstart="isComposing = true"
            @compositionend="isComposing = false"
            @keydown="handleReplaceKeyDown"
          />
        </div>

        <div class="qn-find-bar__replace-actions">
          <button
            type="button"
            class="qn-find-bar__text-btn"
            :disabled="!modelValue.search || !summary.valid || summary.total === 0"
            title="替换当前匹配项 (Enter)"
            data-testid="editor-replace-one"
            @click="emit('replaceOne')"
          >
            替换
          </button>
          <button
            type="button"
            class="qn-find-bar__text-btn"
            :disabled="!modelValue.search || !summary.valid || summary.total === 0"
            title="全部替换 (⌘Enter)"
            data-testid="editor-replace-all"
            @click="emit('replaceAll')"
          >
            全部
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.qn-find-bar {
  position: relative;
  pointer-events: auto;
  display: flex;
  align-items: stretch;
  gap: var(--qn-space-1);
  max-width: calc(100% - var(--qn-space-4));
  padding: var(--qn-space-2) var(--qn-space-2) var(--qn-space-2) var(--qn-space-1);
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  box-shadow: var(--qn-shadow-md);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-find-bar.is-resizing {
  user-select: none;
}

.qn-find-bar__resizer {
  position: absolute;
  top: 0;
  bottom: 0;
  left: -3px;
  width: 6px;
  cursor: ew-resize;
  border-radius: var(--qn-radius-sm);
}

.qn-find-bar__resizer::after {
  content: '';
  position: absolute;
  top: 6px;
  bottom: 6px;
  left: 2px;
  width: 2px;
  border-radius: var(--qn-radius-full);
  background-color: transparent;
  transition: background-color 120ms ease;
}

.qn-find-bar__resizer:hover::after,
.qn-find-bar.is-resizing .qn-find-bar__resizer::after {
  background-color: var(--qn-accent);
}

.qn-find-bar__toggle {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  padding: 0;
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-muted);
  cursor: pointer;
}

.qn-find-bar__toggle:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-find-bar__toggle svg {
  transition: transform 120ms ease;
}

.qn-find-bar__toggle svg.is-expanded {
  transform: rotate(90deg);
}

.qn-find-bar__rows {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-find-bar__row {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  min-width: 0;
}

.qn-find-bar__input-wrap {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px var(--qn-space-1) 2px var(--qn-space-2);
  background-color: var(--qn-bg-base);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
}

.qn-find-bar__input-wrap:focus-within {
  border-color: var(--qn-accent);
}

.qn-find-bar__input-wrap.has-error {
  border-color: var(--qn-danger);
}

.qn-find-bar__input {
  flex: 1 1 auto;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--qn-text-primary);
  font-family: inherit;
  font-size: var(--qn-text-sm);
  line-height: 20px;
  padding: 0;
}

.qn-find-bar__input::placeholder {
  color: var(--qn-text-muted);
}

.qn-find-bar__toggles {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 2px;
}

.qn-find-bar__opt-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  height: 20px;
  padding: 0 3px;
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-muted);
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-xs);
  line-height: 1;
  cursor: pointer;
}

.qn-find-bar__opt-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-find-bar__opt-btn.is-active {
  background-color: var(--qn-accent-soft);
  border-color: var(--qn-accent);
  color: var(--qn-accent);
}

.qn-find-bar__count {
  flex: 0 0 auto;
  min-width: 48px;
  text-align: center;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.qn-find-bar__count.is-error {
  color: var(--qn-danger);
}

.qn-find-bar__actions,
.qn-find-bar__replace-actions {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 2px;
}

.qn-find-bar__icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  cursor: pointer;
}

.qn-find-bar__icon-btn:hover:not(:disabled) {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-find-bar__icon-btn:disabled {
  opacity: 0.4;
  cursor: default;
}

.qn-find-bar__text-btn {
  height: 24px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-family: inherit;
  font-size: var(--qn-text-xs);
  cursor: pointer;
  white-space: nowrap;
}

.qn-find-bar__text-btn:hover:not(:disabled) {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-find-bar__text-btn:disabled {
  opacity: 0.4;
  cursor: default;
}
</style>
