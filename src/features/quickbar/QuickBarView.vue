<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { QnBadge, QnBanner, QnButton } from '@/components';
import QuickBarTargetPicker from './QuickBarTargetPicker.vue';
import { useQuickBarStore } from './store';

const store = useQuickBarStore();
const editorRef = ref<HTMLTextAreaElement | null>(null);

function focusEditor(): void {
  if (store.isUnconfigured || store.mode !== 'capture' || store.targetPickerOpen) {
    return;
  }
  void nextTick(() => {
    editorRef.value?.focus();
  });
}

onMounted(() => {
  focusEditor();
});

onUnmounted(() => {
  store.dispose();
});

watch(
  () => [store.focusTick, store.isUnconfigured, store.mode, store.targetPickerOpen] as const,
  ([, unconfigured, currentMode, pickerOpen]) => {
    if (!unconfigured && currentMode === 'capture' && !pickerOpen) {
      focusEditor();
    }
  },
);

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    void store.handleEscape();
    return;
  }

  if (!event.metaKey || store.isUnconfigured) {
    return;
  }

  const lowerKey = event.key.toLowerCase();

  if (lowerKey === 'k') {
    event.preventDefault();
    store.mode = event.shiftKey ? 'command' : 'open';
    return;
  }

  if (lowerKey === 't') {
    event.preventDefault();
    store.toggleTargetPicker();
    return;
  }

  if (event.key === '1') {
    event.preventDefault();
    store.setCaptureMode('list');
    return;
  }

  if (event.key === '2') {
    event.preventDefault();
    store.setCaptureMode('todo');
    return;
  }

  if (event.key === '3') {
    event.preventDefault();
    store.setCaptureMode('prose');
    return;
  }

  if (event.key === 'ArrowDown') {
    event.preventDefault();
    store.cycleRelatedTarget('next');
    return;
  }

  if (event.key === 'ArrowUp') {
    event.preventDefault();
    store.cycleRelatedTarget('prev');
    return;
  }

  if (event.key === 'Enter') {
    event.preventDefault();
    void store.submitAppend(event.shiftKey);
  }
}
</script>

<template>
  <div class="qn-quickbar" @keydown="handleKeydown">
    <header class="qn-quickbar__header">
      <div class="qn-quickbar__modes">
        <button
          type="button"
          :class="['qn-quickbar__mode', { 'qn-quickbar__mode--active': store.mode === 'capture' }]"
          @click="store.mode = 'capture'"
        >
          记录
        </button>
        <button
          type="button"
          :class="['qn-quickbar__mode', { 'qn-quickbar__mode--active': store.mode === 'open' }]"
          @click="store.mode = 'open'"
        >
          打开 ⌘K
        </button>
        <button
          type="button"
          :class="['qn-quickbar__mode', { 'qn-quickbar__mode--active': store.mode === 'command' }]"
          @click="store.mode = 'command'"
        >
          命令 ⌘⇧K
        </button>
      </div>

      <div class="qn-quickbar__header-right">
        <button
          v-if="!store.isUnconfigured"
          type="button"
          class="qn-quickbar__chip-btn"
          data-testid="quickbar-target-chip"
          @click="store.toggleTargetPicker()"
        >
          <QnBadge :tone="store.targetChipTone">
            {{ store.targetChipLabel }}
          </QnBadge>
        </button>
        <QnBadge v-else tone="warning" data-testid="quickbar-target-chip">
          {{ store.targetChipLabel }}
        </QnBadge>

        <button
          v-if="store.explicitTarget && !store.isUnconfigured"
          type="button"
          class="qn-quickbar__clear-target"
          data-testid="quickbar-clear-explicit"
          @click="store.clearExplicitTarget()"
        >
          × 恢复默认
        </button>

        <span
          v-if="store.sectionHintText"
          class="qn-quickbar__section-hint"
          data-testid="quickbar-section-hint"
        >
          {{ store.sectionHintText }}
        </span>

        <QnButton variant="ghost" size="sm" @click="store.handleEscape()">Esc</QnButton>
      </div>
    </header>

    <QuickBarTargetPicker />

    <div v-if="store.settingsStatus && !store.settingsStatus.valid" class="qn-quickbar__banner">
      <QnBanner tone="danger" title="settings.yaml 解析异常">
        <span v-if="store.settingsStatus.errorLine !== null">
          第 {{ store.settingsStatus.errorLine }} 行
        </span>
        <span v-if="store.settingsStatus.errorColumn !== null">
          第 {{ store.settingsStatus.errorColumn }} 列：
        </span>
        {{ store.settingsStatus.errorMessage ?? '配置存在语法错误，已回退安全默认配置。' }}
      </QnBanner>
    </div>

    <div v-if="store.errorMessage" class="qn-quickbar__banner">
      <QnBanner tone="danger" title="追加失败">
        {{ store.errorMessage }}
      </QnBanner>
    </div>

    <div v-if="store.isUnconfigured" class="qn-quickbar__s1" data-testid="quickbar-s1-onboarding">
      <div class="qn-quickbar__s1-card">
        <QnBadge tone="warning" size="md">S1 未选库引导态</QnBadge>
        <h2 class="qn-quickbar__s1-title">请先选择或创建笔记库</h2>
        <p class="qn-quickbar__s1-desc">
          尚未配置本地笔记库根目录。当前已禁用输入与追加提交，请先在主窗口完成初始化。
        </p>
        <QnButton variant="primary" @click="store.openMainOnboarding()">
          打开主窗口完成初始化
        </QnButton>
      </div>
    </div>

    <div v-else-if="store.mode !== 'capture'" class="qn-quickbar__placeholder">
      <div class="qn-quickbar__s1-card">
        <QnBadge tone="neutral" size="md">
          {{ store.mode === 'open' ? '快速打开模式 (⌘K)' : '命令面板模式 (⌘⇧K)' }}
        </QnBadge>
        <p class="qn-quickbar__s1-desc">
          该模式将在 M3 阶段接入完整检索与命令执行。按 Esc 可退回记录模式。
        </p>
        <QnButton variant="secondary" size="sm" @click="store.mode = 'capture'">
          返回记录模式 (Esc)
        </QnButton>
      </div>
    </div>

    <div v-else class="qn-quickbar__body">
      <textarea
        ref="editorRef"
        v-model="store.draft"
        class="qn-quickbar__editor"
        autofocus
        placeholder="输入要快速记录的想法、待办或片段…（⌘1 列表 / ⌘2 待办 / ⌘3 正文，⌘T 选目标）"
      />

      <div
        v-if="store.relatedNotes.length > 0"
        class="qn-quickbar__related"
        data-testid="quickbar-related-list"
      >
        <span class="qn-quickbar__related-label">相关笔记 (⌘↑/⌘↓ 改投):</span>
        <div class="qn-quickbar__related-items">
          <button
            v-for="(item, idx) in store.relatedNotes"
            :key="item.key"
            type="button"
            :class="[
              'qn-quickbar__related-item',
              { 'qn-quickbar__related-item--active': store.selectedRelatedIndex === idx },
            ]"
            :data-testid="`quickbar-related-item-${idx}`"
            @click="store.selectRelatedNote(store.selectedRelatedIndex === idx ? -1 : idx)"
          >
            {{ item.title }}
          </button>
        </div>
      </div>

      <footer class="qn-quickbar__footer">
        <div class="qn-quickbar__capture-modes">
          <button
            type="button"
            :class="[
              'qn-quickbar__capture-btn',
              { 'qn-quickbar__capture-btn--active': store.captureMode === 'list' },
            ]"
            data-testid="capture-mode-list"
            @click="store.setCaptureMode('list')"
          >
            列表 ⌘1
          </button>
          <button
            type="button"
            :class="[
              'qn-quickbar__capture-btn',
              { 'qn-quickbar__capture-btn--active': store.captureMode === 'todo' },
            ]"
            data-testid="capture-mode-todo"
            @click="store.setCaptureMode('todo')"
          >
            待办 ⌘2
          </button>
          <button
            type="button"
            :class="[
              'qn-quickbar__capture-btn',
              { 'qn-quickbar__capture-btn--active': store.captureMode === 'prose' },
            ]"
            data-testid="capture-mode-prose"
            @click="store.setCaptureMode('prose')"
          >
            正文 ⌘3
          </button>
        </div>

        <div class="qn-quickbar__footer-actions">
          <QnButton
            v-if="store.draft.length > 0"
            variant="ghost"
            size="sm"
            data-testid="quickbar-clear-draft"
            @click="store.clearDraft()"
          >
            清空
          </QnButton>
          <QnButton
            variant="secondary"
            size="sm"
            :disabled="!store.canSubmit"
            data-testid="quickbar-submit-open"
            @click="store.submitAppend(true)"
          >
            追加并打开 ⇧⌘↵
          </QnButton>
          <QnButton
            variant="primary"
            size="sm"
            :disabled="!store.canSubmit"
            data-testid="quickbar-submit-append"
            @click="store.submitAppend(false)"
          >
            追加 ⌘↵
          </QnButton>
        </div>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.qn-quickbar {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-lg);
  overflow: hidden;
}

.qn-quickbar__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  padding: var(--qn-space-3) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-quickbar__modes {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-quickbar__mode {
  padding: var(--qn-space-1) var(--qn-space-3);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-weight: 500;
  cursor: pointer;
}

.qn-quickbar__mode:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-quickbar__mode--active {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-quickbar__header-right {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-quickbar__chip-btn {
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.qn-quickbar__clear-target {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-quickbar__clear-target:hover {
  color: var(--qn-text-primary);
}

.qn-quickbar__section-hint {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-quickbar__banner {
  padding: var(--qn-space-2) var(--qn-space-4) 0;
}

.qn-quickbar__s1,
.qn-quickbar__placeholder {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--qn-space-6);
}

.qn-quickbar__s1-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: var(--qn-space-3);
  max-width: 420px;
}

.qn-quickbar__s1-title {
  margin: 0;
  font-size: var(--qn-text-md);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-quickbar__s1-desc {
  margin: 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
}

.qn-quickbar__body {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.qn-quickbar__editor {
  flex: 1;
  width: 100%;
  padding: var(--qn-space-4);
  border: none;
  resize: none;
  background-color: var(--qn-bg-elevated);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-base);
  line-height: 1.6;
}

.qn-quickbar__editor:focus {
  outline: none;
}

.qn-quickbar__related {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-4);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-base);
  overflow-x: auto;
}

.qn-quickbar__related-label {
  flex-shrink: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-quickbar__related-items {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-quickbar__related-item {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-sidebar);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  white-space: nowrap;
  cursor: pointer;
}

.qn-quickbar__related-item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-quickbar__related-item--active {
  border-color: var(--qn-accent);
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-quickbar__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-4);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-quickbar__capture-modes {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-quickbar__capture-btn {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-quickbar__capture-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-quickbar__capture-btn--active {
  border-color: var(--qn-border-default);
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-quickbar__footer-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}
</style>
