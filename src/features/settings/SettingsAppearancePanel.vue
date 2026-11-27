<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import { QnBanner, QnPanelCard } from '@/components';
import {
  CODE_FONT_PRESETS,
  CUSTOM_FONT_SENTINEL,
  isPresetFontValue,
  PROSE_FONT_PRESETS,
} from '@/lib/utils';
import { useSettingsStore } from './store';

const store = useSettingsStore();

const form = reactive({
  fontFamily: '',
  codeFontFamily: '',
  fontSize: 14,
  lineHeight: 1.6,
  maxWidth: 900,
  tabSizeProse: 2,
  tabSizeCode: 4,
  autoSaveDebounceMs: 600,
  autoSaveMaxWaitMs: 2000,
  singleClickLink: false,
});

const proseFontMode = ref<string>('');
const codeFontMode = ref<string>('');
const validationError = ref<string | null>(null);

watch(
  () => store.snapshot?.settings.editor,
  (editor) => {
    if (!editor) {
      return;
    }
    const nextFont = editor.fontFamily ?? '';
    const nextCodeFont = editor.codeFontFamily ?? '';
    form.fontFamily = nextFont;
    form.codeFontFamily = nextCodeFont;
    proseFontMode.value = isPresetFontValue(nextFont, PROSE_FONT_PRESETS)
      ? nextFont.trim()
      : CUSTOM_FONT_SENTINEL;
    codeFontMode.value = isPresetFontValue(nextCodeFont, CODE_FONT_PRESETS)
      ? nextCodeFont.trim()
      : CUSTOM_FONT_SENTINEL;
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

async function onProseFontSelectChange(event: Event): Promise<void> {
  const target = event.target as HTMLSelectElement | null;
  if (!target) {
    return;
  }
  proseFontMode.value = target.value;
  if (target.value === CUSTOM_FONT_SENTINEL) {
    return;
  }
  form.fontFamily = target.value;
  await store.patchSettings({
    editor: {
      fontFamily: form.fontFamily,
    },
  });
}

async function onCodeFontSelectChange(event: Event): Promise<void> {
  const target = event.target as HTMLSelectElement | null;
  if (!target) {
    return;
  }
  codeFontMode.value = target.value;
  if (target.value === CUSTOM_FONT_SENTINEL) {
    return;
  }
  form.codeFontFamily = target.value;
  await store.patchSettings({
    editor: {
      codeFontFamily: form.codeFontFamily,
    },
  });
}

async function commitCustomFonts(): Promise<void> {
  const fontFamily = form.fontFamily.trim();
  const codeFontFamily = form.codeFontFamily.trim();
  form.fontFamily = fontFamily;
  form.codeFontFamily = codeFontFamily;
  await store.patchSettings({
    editor: {
      fontFamily,
      codeFontFamily,
    },
  });
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
    <div class="qn-appearance-panel__header">
      <h2 class="qn-baseline__heading">编辑器排版与行为</h2>
      <p class="qn-baseline__subheading">字号、行高、阅读宽度、缩进与自动保存策略</p>
    </div>

    <QnBanner v-if="validationError" tone="warning" title="参数校验提示">
      {{ validationError }}
    </QnBanner>

    <QnPanelCard title="字体与排版" subtitle="配置正文/代码字体、字号、行高与最大阅读宽度">
      <div class="qn-appearance-panel__form">
        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">正文字体</span>
          <select
            :value="proseFontMode"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-font-family-select"
            class="qn-appearance-panel__control"
            @change="onProseFontSelectChange"
          >
            <option v-for="opt in PROSE_FONT_PRESETS" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </option>
            <option :value="CUSTOM_FONT_SENTINEL">自定义字体…</option>
          </select>
          <input
            v-if="proseFontMode === CUSTOM_FONT_SENTINEL"
            v-model="form.fontFamily"
            type="text"
            placeholder="输入字体名称，如 LXGW WenKai"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-font-family-custom-input"
            class="qn-appearance-panel__control"
            @change="commitCustomFonts"
          />
        </label>

        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">代码字体</span>
          <select
            :value="codeFontMode"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-code-font-family-select"
            class="qn-appearance-panel__control"
            @change="onCodeFontSelectChange"
          >
            <option v-for="opt in CODE_FONT_PRESETS" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </option>
            <option :value="CUSTOM_FONT_SENTINEL">自定义字体…</option>
          </select>
          <input
            v-if="codeFontMode === CUSTOM_FONT_SENTINEL"
            v-model="form.codeFontFamily"
            type="text"
            placeholder="输入等宽字体名称，如 JetBrains Mono"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-code-font-family-custom-input"
            class="qn-appearance-panel__control"
            @change="commitCustomFonts"
          />
        </label>

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
            class="qn-appearance-panel__control"
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
            class="qn-appearance-panel__control"
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
            class="qn-appearance-panel__control"
            @change="commitEditorSettings"
          />
        </label>
      </div>
    </QnPanelCard>

    <QnPanelCard title="缩进规则" subtitle="控制 Markdown 正文与围栏代码块的 Tab 缩进空格数">
      <div class="qn-appearance-panel__form">
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
            class="qn-appearance-panel__control"
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
            class="qn-appearance-panel__control"
            @change="commitEditorSettings"
          />
        </label>
      </div>
    </QnPanelCard>

    <QnPanelCard title="自动保存与交互" subtitle="调整自动保存节流阈值与链接点击方式">
      <div class="qn-appearance-panel__form">
        <label class="qn-appearance-panel__field">
          <span class="qn-appearance-panel__label">自动保存防抖 (ms)</span>
          <input
            v-model.number="form.autoSaveDebounceMs"
            type="number"
            min="100"
            step="100"
            :disabled="store.isSettingsInvalid"
            data-testid="editor-autosave-debounce-input"
            class="qn-appearance-panel__control"
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
            class="qn-appearance-panel__control"
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

.qn-appearance-panel__header {
  display: flex;
  flex-direction: column;
}

.qn-baseline__heading {
  margin: 0;
  font-size: var(--qn-text-lg);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-baseline__subheading {
  margin: var(--qn-space-1) 0 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
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

.qn-appearance-panel__control {
  height: 34px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-appearance-panel__control:disabled {
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
  padding-top: var(--qn-space-1);
}
</style>
