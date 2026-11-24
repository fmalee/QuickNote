<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { QnBadge } from '@/components';
import type { QuickOpenResultItem, useQuickOpen } from './useQuickOpen';

const props = withDefaults(
  defineProps<{
    engine: ReturnType<typeof useQuickOpen>;
    variant?: 'compact' | 'large';
    focusTrigger?: number;
  }>(),
  {
    variant: 'compact',
    focusTrigger: 0,
  },
);

const inputRef = ref<HTMLInputElement | null>(null);

const modeLabel = computed<string>(() => {
  switch (props.engine.parsedQuery.value.mode) {
    case 'path':
      return '路径 /';
    case 'fulltext':
      return '全文 #';
    case 'tag':
      return '标签 @';
    case 'action':
      return '动作 >';
    default:
      return '混合';
  }
});

function sourceLabel(source: QuickOpenResultItem['source']): string {
  switch (source) {
    case 'library':
      return '库';
    case 'recent':
      return '最近';
    case 'spotlight':
      return 'Spotlight';
    case 'path':
      return '路径';
    case 'tag':
      return '标签';
    case 'action':
      return '动作';
  }
}

function sourceTone(
  source: QuickOpenResultItem['source'],
): 'neutral' | 'accent' | 'warning' | 'success' {
  switch (source) {
    case 'library':
      return 'neutral';
    case 'recent':
      return 'accent';
    case 'spotlight':
      return 'warning';
    case 'path':
      return 'success';
    case 'tag':
      return 'accent';
    case 'action':
      return 'neutral';
  }
}

function focusInput(): void {
  void nextTick(() => {
    inputRef.value?.focus();
    inputRef.value?.select();
  });
}

function handleInput(event: Event): void {
  const target = event.target as HTMLInputElement;
  props.engine.setQuery(target.value);
}

function handleKeydown(event: KeyboardEvent): void {
  if (props.engine.handleKeydown(event)) {
    event.stopPropagation?.();
  }
}

function handleItemClick(item: QuickOpenResultItem, index: number, event: MouseEvent): void {
  props.engine.setSelectedIndex(index);
  if (event.altKey && item.absPath) {
    void props.engine.revealItem(item);
    return;
  }
  void props.engine.confirmItem(item, event.metaKey ? 'newTab' : 'open');
}

onMounted(() => {
  focusInput();
});

watch(
  () => props.focusTrigger,
  () => {
    focusInput();
  },
);
</script>

<template>
  <div
    :class="['qn-quickopen', `qn-quickopen--${variant}`]"
    :data-testid="`quickopen-palette-${variant}`"
  >
    <div class="qn-quickopen__search-row">
      <QnBadge tone="accent" size="sm" data-testid="quickopen-mode-badge">
        {{ modeLabel }}
      </QnBadge>
      <input
        ref="inputRef"
        type="text"
        class="qn-quickopen__input"
        data-testid="quickopen-input"
        :value="engine.query.value"
        placeholder="搜索笔记、外部文件…（/ 路径 · # 全文 · @ 标签 · > 动作）"
        @input="handleInput"
        @keydown="handleKeydown"
      />
      <span
        v-if="engine.spotlightLoading.value"
        class="qn-quickopen__status"
        data-testid="quickopen-spotlight-loading"
      >
        Spotlight…
      </span>
      <button
        v-if="engine.query.value"
        type="button"
        class="qn-quickopen__clear-btn"
        aria-label="清空搜索"
        @click="engine.setQuery('', true)"
      >
        ×
      </button>
    </div>

    <div v-if="engine.errorMessage.value" class="qn-quickopen__error" data-testid="quickopen-error">
      {{ engine.errorMessage.value }}
    </div>

    <div class="qn-quickopen__list" role="listbox" data-testid="quickopen-results">
      <div
        v-if="engine.loading.value && engine.items.value.length === 0"
        class="qn-quickopen__empty"
      >
        正在检索…
      </div>
      <div v-else-if="engine.items.value.length === 0" class="qn-quickopen__empty">
        未找到匹配结果
      </div>
      <button
        v-for="(item, idx) in engine.items.value"
        :key="item.id"
        type="button"
        role="option"
        :aria-selected="engine.selectedIndex.value === idx"
        :disabled="item.disabled"
        :class="[
          'qn-quickopen__item',
          {
            'qn-quickopen__item--selected': engine.selectedIndex.value === idx,
            'qn-quickopen__item--disabled': item.disabled,
          },
        ]"
        :data-testid="`quickopen-item-${idx}`"
        @mouseenter="engine.setSelectedIndex(idx)"
        @click="handleItemClick(item, idx, $event)"
      >
        <div class="qn-quickopen__item-main">
          <QnBadge :tone="sourceTone(item.source)" size="sm">
            {{ sourceLabel(item.source) }}
          </QnBadge>
          <span class="qn-quickopen__item-title">{{ item.title }}</span>
          <span v-if="item.kind === 'dir'" class="qn-quickopen__dir-hint">目录</span>
        </div>
        <div class="qn-quickopen__item-meta">
          <span v-if="item.shortcutLabel" class="qn-quickopen__shortcut">
            {{ item.shortcutLabel }}
          </span>
          <span v-if="item.subtitle" class="qn-quickopen__item-subtitle">
            {{ item.subtitle }}
          </span>
        </div>
      </button>
    </div>

    <div
      v-if="variant === 'large' && engine.selectedItem.value"
      class="qn-quickopen__preview"
      data-testid="quickopen-preview-bar"
    >
      <span class="qn-quickopen__preview-label">路径/详情：</span>
      <span class="qn-quickopen__preview-value">
        {{ engine.selectedItem.value.absPath ?? engine.selectedItem.value.subtitle }}
      </span>
    </div>

    <footer class="qn-quickopen__footer" data-testid="quickopen-footer">
      <template v-if="variant === 'large'">
        <span>↵ 主窗口打开 · ⌘↵ 新标签 · ⌥↵ Finder 显示 · / 路径 · # 全文 · @ 标签 · > 动作</span>
      </template>
      <template v-else>
        <span>↵ 打开 · ⌘↵ 新标签 · ⌥↵ Finder · Tab 补全路径 · Esc 返回</span>
      </template>
    </footer>
  </div>
</template>

<style scoped>
.qn-quickopen {
  display: flex;
  flex-direction: column;
  min-height: 0;
  color: var(--qn-text-primary);
}

.qn-quickopen--compact {
  flex: 1;
  gap: var(--qn-space-2);
}

.qn-quickopen--large {
  gap: var(--qn-space-3);
}

.qn-quickopen__search-row {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
}

.qn-quickopen__search-row:focus-within {
  border-color: var(--qn-accent);
}

.qn-quickopen__input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--qn-text-primary);
  font-size: var(--qn-text-base);
}

.qn-quickopen__status {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  white-space: nowrap;
}

.qn-quickopen__clear-btn {
  border: none;
  background: transparent;
  color: var(--qn-text-secondary);
  cursor: pointer;
  font-size: var(--qn-text-base);
  line-height: 1;
  padding: 0 var(--qn-space-1);
}

.qn-quickopen__error {
  padding: var(--qn-space-2) var(--qn-space-3);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-elevated);
  color: var(--qn-danger);
  font-size: var(--qn-text-xs);
}

.qn-quickopen__list {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  overflow-y: auto;
  min-height: 0;
}

.qn-quickopen--compact .qn-quickopen__list {
  flex: 1;
  max-height: 220px;
}

.qn-quickopen--large .qn-quickopen__list {
  height: 360px;
}

.qn-quickopen__empty {
  padding: var(--qn-space-6) var(--qn-space-4);
  text-align: center;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-sm);
}

.qn-quickopen__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  width: 100%;
  text-align: left;
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-primary);
  cursor: pointer;
}

.qn-quickopen--compact .qn-quickopen__item {
  padding: var(--qn-space-1) var(--qn-space-2);
}

.qn-quickopen--large .qn-quickopen__item {
  padding: var(--qn-space-2) var(--qn-space-3);
}

.qn-quickopen__item:hover,
.qn-quickopen__item--selected {
  background-color: var(--qn-bg-hover);
  border-color: var(--qn-border-subtle);
}

.qn-quickopen__item--disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.qn-quickopen__item-main {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  min-width: 0;
  flex-shrink: 1;
}

.qn-quickopen__item-title {
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-quickopen__dir-hint {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-quickopen__item-meta {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  min-width: 0;
  flex-shrink: 1;
}

.qn-quickopen__shortcut {
  padding: 1px var(--qn-space-1);
  border-radius: var(--qn-radius-sm);
  border: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  white-space: nowrap;
}

.qn-quickopen__item-subtitle {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 320px;
}

.qn-quickopen__preview {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-3);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-sidebar);
  border: 1px solid var(--qn-border-subtle);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  min-width: 0;
}

.qn-quickopen__preview-label {
  font-weight: 500;
  color: var(--qn-text-primary);
  white-space: nowrap;
}

.qn-quickopen__preview-value {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-quickopen__footer {
  padding-top: var(--qn-space-1);
  border-top: 1px solid var(--qn-border-subtle);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}
</style>
