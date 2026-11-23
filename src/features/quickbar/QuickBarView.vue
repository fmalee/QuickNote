<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue';
import { QnBadge, QnBanner, QnButton } from '@/components';
import { useQuickBarStore } from './store';

const store = useQuickBarStore();
const editorRef = ref<HTMLTextAreaElement | null>(null);

function focusEditor(): void {
  if (store.isUnconfigured) {
    return;
  }
  void nextTick(() => {
    editorRef.value?.focus();
  });
}

onMounted(() => {
  focusEditor();
});

watch(
  () => [store.focusTick, store.isUnconfigured] as const,
  ([, unconfigured]) => {
    if (!unconfigured) {
      focusEditor();
    }
  },
);

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    void store.hide('esc');
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
        <QnBadge :tone="store.isUnconfigured ? 'warning' : 'neutral'">
          {{ store.isUnconfigured ? '未选库 · S1' : '收件箱 · inbox.md' }}
        </QnBadge>
        <QnButton variant="ghost" size="sm" @click="store.hide('esc')">Esc</QnButton>
      </div>
    </header>

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

    <div v-else class="qn-quickbar__body">
      <textarea
        ref="editorRef"
        v-model="store.draft"
        class="qn-quickbar__editor"
        autofocus
        placeholder="输入要快速记录的想法、待办或片段…（M1 阶段接入真实追加写入）"
      />
      <footer class="qn-quickbar__footer">
        <span class="qn-quickbar__hint">默认目标：收件箱（M0 窗口骨架）</span>
        <div class="qn-quickbar__footer-actions">
          <QnButton variant="primary" size="sm" :disabled="true"> 追加 ⌘↵（M1 启用） </QnButton>
        </div>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.qn-quickbar {
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

.qn-quickbar__banner {
  padding: var(--qn-space-2) var(--qn-space-4) 0;
}

.qn-quickbar__s1 {
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

.qn-quickbar__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--qn-space-2) var(--qn-space-4);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-quickbar__hint {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-quickbar__footer-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}
</style>
