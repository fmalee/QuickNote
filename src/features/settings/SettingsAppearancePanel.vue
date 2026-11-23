<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import type { EditorTheme } from '@/bindings/types';
import { QnBanner, QnButton, QnPanelCard } from '@/components';
import { useSettingsStore } from './store';

const store = useSettingsStore();

const themeOptions: { value: EditorTheme; label: string; desc: string }[] = [
  { value: 'system', label: '跟随系统', desc: '自动匹配 macOS 深浅色外观' },
  { value: 'light', label: '浅色主题', desc: '高对比日间纸面阅读体验' },
  { value: 'dark', label: '深色主题', desc: '低眩光夜间专注工作模式' },
];

const form = reactive({
  fontSize: 14,
  lineHeight: 1.6,
  maxWidth: 900,
  tabSizeProse: 2,
  tabSizeCode: 4,
  autoSaveDebounceMs: 600,
  autoSaveMaxWaitMs: 2000,
  singleClickLink: false,
});

const validationError = ref<string | null>(null);

watch(
  () => store.snapshot?.settings.editor,
  (editor) => {
    if (!editor) {
      return;
    }
    form.fontSize = editor.fontSize;
    form.lineHeight = editor.lineHeight;
    form.maxWidth = editor.maxWidth;
    form.tabSizeProse = editor.tabSizeProse;
    form.tabSizeCode = editor.tabSizeCode;
    form.autoSaveDebounceMs = editor.autoSaveDebounceMs;
    form.autoSaveMaxWaitMs = editor.autoSaveMaxWaitMs;
    form.singleClickLink = editor.singleClickLink;
  },
  { immediate: true, deep: true },
);

function clampInt(val: number, min: number, max: number): number {
  if (!Number.isFinite(val)) {
    return min;
  }
  return Math.min(max, Math.max(min, Math.round(val)));
}

function clampFloat(val: number, min: number, max: number): number {
  if (!Number.isFinite(val)) {
    return min;
  }
  return Math.min(max, Math.max(min, Math.round(val * 10) / 10));
}

async function commitEditorSettings(): Promise<void> {
  validationError.value = null;

  const fontSize = clampInt(form.fontSize, 10, 32);
  const lineHeight = clampFloat(form.lineHeight, 1.0, 3.0);
  const maxWidth = Math.max(0, Math.round(Number.isFinite(form.maxWidth) ? form.maxWidth : 900));
  const tabSizeProse = clampInt(form.tabSizeProse, 1, 8);
  const tabSizeCode = clampInt(form.tabSizeCode, 1, 8);
  const autoSaveDebounceMs = Math.max(
    100,
    Math.round(Number.isFinite(form.autoSaveDebounceMs) ? form.autoSaveDebounceMs : 600),
  );
  const autoSaveMaxWaitMs = Math.max(
    100,
    Math.round(Number.isFinite(form.autoSaveMaxWaitMs) ? form.autoSaveMaxWaitMs : 2000),
  );

  if (autoSaveMaxWaitMs < autoSaveDebounceMs) {
    validationError.value = '自动保存最大等待时间必须大于或等于防抖时间。';
    return;
  }

  form.fontSize = fontSize;
  form.lineHeight = lineHeight;
  form.maxWidth = maxWidth;
  form.tabSizeProse = tabSizeProse;
  form.tabSizeCode = tabSizeCode;
  form.autoSaveDebounceMs = autoSaveDebounceMs;
  form.autoSaveMaxWaitMs = autoSaveMaxWaitMs;

  await store.patchSettings({
    editor: {
      fontSize,
      lineHeight,
      maxWidth,
      tabSizeProse,
      tabSizeCode,
      autoSaveDebounceMs,
      autoSaveMaxWaitMs,
      singleClickLink: form.singleClickLink,
    },
  });
}

async function onToggleSingleClickLink(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement | null;
  if (!target) {
    return;
  }
  form.singleClickLink = target.checked;
  await store.patchSettings({
    editor: {
      singleClickLink: form.singleClickLink,
    },
  });
}
</script>

<template>
  <div class="qn-appearance-panel">
    <QnPanelCard title="主题模式" subtitle="实时切换 --qn-* 浅色与深色视觉变量并持久化">
      <div class="qn-appearance-panel__theme-grid">
        <button
          v-for="opt in themeOptions"
          :key="opt.value"
          type="button"
          :disabled="store.isSettingsInvalid"
          :data-testid="`theme-option-${opt.value}`"
          :class="[
            'qn-appearance-panel__theme-card',
            { 'qn-appearance-panel__theme-card--active': store.themePreference === opt.value },
          ]"
          @click="store.setTheme(opt.value, { persist: true })"
        >
          <span class="qn-appearance-panel__theme-name">{{ opt.label }}</span>
          <span class="qn-appearance-panel__theme-desc">{{ opt.desc }}</span>
        </button>
      </div>
      <div class="qn-appearance-panel__footer">
        <QnButton variant="secondary" size="sm" @click="store.activeSection = 'baseline'">
          打开视觉基线预览页
        </QnButton>
      </div>
    </QnPanelCard>

    <QnPanelCard title="编辑器排版与行为" subtitle="字号、行高、阅读宽度、缩进与自动保存策略">
      <QnBanner v-if="validationError" tone="warning" title="参数校验提示">
        {{ validationError }}
      </QnBanner>

      <div class="qn-appearance-panel__form">
        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">字号 (10–32 px)</span>
          <input
            v-model.number="form.fontSize"
            type="number"
            min="10"
            max="32"
            step="1"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-font-size-input"
            class="qn-appearance-panel__number"
            @change="commitEditorSettings"
          />
        </label>

        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">行高 (1.0–3.0)</span>
          <input
            v-model.number="form.lineHeight"
            type="number"
            min="1.0"
            max="3.0"
            step="0.1"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-line-height-input"
            class="qn-appearance-panel__number"
            @change="commitEditorSettings"
          />
        </label>

        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">最大阅读宽度 (px，0 为不限制)</span>
          <input
            v-model.number="form.maxWidth"
            type="number"
            min="0"
            step="20"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-max-width-input"
            class="qn-appearance-panel__number"
            @change="commitEditorSettings"
          />
        </label>

        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">正文缩进空格数 (1–8)</span>
          <input
            v-model.number="form.tabSizeProse"
            type="number"
            min="1"
            max="8"
            step="1"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-tab-prose-input"
            class="qn-appearance-panel__number"
            @change="commitEditorSettings"
          />
        </label>

        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">代码块缩进空格数 (1–8)</span>
          <input
            v-model.number="form.tabSizeCode"
            type="number"
            min="1"
            max="8"
            step="1"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-tab-code-input"
            class="qn-appearance-panel__number"
            @change="commitEditorSettings"
          />
        </label>

        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">自动保存防抖 (ms)</span>
          <input
            v-model.number="form.autoSaveDebounceMs"
            type="number"
            min="100"
            step="100"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-autosave-debounce-input"
            class="qn-appearance-panel__number"
            @change="commitEditorSettings"
          />
        </label>

        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">自动保存最大等待 (ms，≥ 防抖)</span>
          <input
            v-model.number="form.autoSaveMaxWaitMs"
            type="number"
            min="100"
            step="100"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-autosave-maxwait-input"
            class="qn-appearance-panel__number"
            @change="commitEditorSettings"
          />
        </label>

        <label class="qn-appearance-panel__checkbox-row">
          <input
            :checked="form.singleClickLink"
            type="checkbox"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-single-click-link-checkbox"
            @change="onToggleSingleClickLink"
          />
          <span>允许普通单击打开链接（关闭时需按住 ⌘ + 单击）</span>
        </label>
      </div>
    </QnPanelCard>
  </div>
</template>

<style scoped>
.qn-appearance-panel {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-appearance-panel__theme-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--qn-space-3);
}

.qn-appearance-panel__theme-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--qn-space-1);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
  cursor: pointer;
  text-align: left;
}

.qn-appearance-panel__theme-card:hover:not(:disabled) {
  background-color: var(--qn-bg-hover);
}

.qn-appearance-panel__theme-card:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.qn-appearance-panel__theme-card--active {
  border-color: var(--qn-accent);
  background-color: var(--qn-accent-soft);
}

.qn-appearance-panel__theme-name {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-appearance-panel__theme-desc {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-appearance-panel__footer {
  margin-top: var(--qn-space-4);
  display: flex;
  justify-content: flex-end;
}

.qn-appearance-panel__form {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--qn-space-3);
}

.qn-appearance-panel__field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-appearance-panel__label {
  font-size: var(--qn-text-xs);
  font-weight: 500;
  color: var(--qn-text-secondary);
}

.qn-appearance-panel__number {
  height: 34px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-appearance-panel__number:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.qn-appearance-panel__checkbox-row {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  font-size: var(--qn-text-sm);
  color: var(--qn-text-primary);
  padding-top: var(--qn-space-2);
}
</style>
