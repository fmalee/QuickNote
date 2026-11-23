<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import type { NoteSummary } from '@/bindings/types';
import { QnButton, QnModalShell } from '@/components';
import {
  deriveTitleFromContent,
  extractTemplateNameFromSummary,
  flattenDirRelPaths,
  inferDefaultTemplateName,
  useNotesStore,
} from '@/features/notes';
import { listTemplates } from './api';
import { useEditorTabsStore } from './store';

const editorTabsStore = useEditorTabsStore();
const notesStore = useNotesStore();

const titleInputRef = ref<HTMLInputElement | null>(null);
const titleDraft = ref('');
const targetDirDraft = ref('');
const templateDraft = ref('note');
const templateTouched = ref(false);
const templates = ref<NoteSummary[]>([]);
const submitting = ref(false);

const activeTab = computed(() => editorTabsStore.activeTab);
const fromLine = computed(() => activeTab.value?.selectionFromLine ?? 1);
const toLine = computed(() => activeTab.value?.selectionToLine ?? 1);
const dirSuggestions = computed(() => flattenDirRelPaths(notesStore.navTree.dirs));

const templateOptions = computed<Array<{ value: string; label: string }>>(() => {
  const seen = new Set<string>();
  const options: Array<{ value: string; label: string }> = [];

  for (const item of templates.value) {
    const name = extractTemplateNameFromSummary(item);
    if (!seen.has(name)) {
      seen.add(name);
      options.push({
        value: name,
        label: item.title && item.title !== name ? `${item.title} (${name})` : name,
      });
    }
  }

  const builtins: Array<{ value: string; label: string }> = [
    { value: 'note', label: '标准笔记 (note)' },
    { value: 'doc', label: '项目文档 (doc)' },
    { value: 'command-book', label: '命令手册 (command-book)' },
  ];
  for (const builtin of builtins) {
    if (!seen.has(builtin.value)) {
      seen.add(builtin.value);
      options.push(builtin);
    }
  }

  return options;
});

watch(
  () => editorTabsStore.inboxExtractModalOpen,
  (isOpen) => {
    if (!isOpen) {
      return;
    }
    const tab = activeTab.value;
    const lines = (tab?.draftBody ?? '').split(/\r?\n/);
    const startIdx = Math.max(0, fromLine.value - 1);
    const endIdx = Math.max(startIdx, toLine.value - 1);
    const selectedText = lines.slice(startIdx, endIdx + 1).join('\n');

    titleDraft.value = deriveTitleFromContent(selectedText);
    targetDirDraft.value =
      notesStore.activeTab === 'projects' && notesStore.selectedDir ? notesStore.selectedDir : '';
    templateDraft.value = inferDefaultTemplateName(targetDirDraft.value);
    templateTouched.value = false;

    void listTemplates()
      .then((items) => {
        templates.value = items;
      })
      .catch(() => undefined);

    void nextTick(() => {
      titleInputRef.value?.focus();
      titleInputRef.value?.select();
    });
  },
);

watch(targetDirDraft, (nextDir) => {
  if (!templateTouched.value) {
    templateDraft.value = inferDefaultTemplateName(nextDir);
  }
});

function handleTemplateChange(event: Event): void {
  const target = event.target as HTMLSelectElement | null;
  if (target) {
    templateDraft.value = target.value;
    templateTouched.value = true;
  }
}

async function handleSubmit(): Promise<void> {
  submitting.value = true;
  try {
    const created = await editorTabsStore.extractInboxSelectionToNote({
      newTitle: titleDraft.value.trim() || '未命名笔记',
      targetDirRelPath: targetDirDraft.value.trim(),
      templateName: templateDraft.value || 'note',
      fromLine: fromLine.value,
      toLine: toLine.value,
    });
    if (created) {
      editorTabsStore.closeInboxExtractModal();
    }
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <QnModalShell
    :open="editorTabsStore.inboxExtractModalOpen"
    title="将选中内容移到新笔记"
    max-width="sm"
    @close="editorTabsStore.closeInboxExtractModal()"
  >
    <form
      class="qn-inbox-extract-form"
      data-testid="inbox-extract-form"
      @submit.prevent="handleSubmit"
    >
      <p class="qn-inbox-extract-form__range" data-testid="inbox-extract-range">
        提取范围：inbox.md 第 L{{ fromLine }}–L{{ toLine }} 行
      </p>

      <label class="qn-inbox-extract-form__field">
        <span class="qn-inbox-extract-form__label">新笔记标题</span>
        <input
          ref="titleInputRef"
          v-model="titleDraft"
          type="text"
          class="qn-inbox-extract-form__input"
          data-testid="inbox-extract-title-input"
          placeholder="输入新笔记标题"
        />
      </label>

      <label class="qn-inbox-extract-form__field">
        <span class="qn-inbox-extract-form__label">目标目录（留空为库根目录）</span>
        <input
          v-model="targetDirDraft"
          type="text"
          list="qn-inbox-extract-dirs"
          class="qn-inbox-extract-form__input"
          data-testid="inbox-extract-dir-input"
          placeholder="例如 projects/quicknote"
        />
        <datalist id="qn-inbox-extract-dirs">
          <option v-for="dir in dirSuggestions" :key="dir" :value="dir" />
        </datalist>
      </label>

      <label class="qn-inbox-extract-form__field">
        <span class="qn-inbox-extract-form__label">使用模板</span>
        <select
          :value="templateDraft"
          class="qn-inbox-extract-form__select"
          data-testid="inbox-extract-template-select"
          @change="handleTemplateChange"
        >
          <option v-for="opt in templateOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </select>
      </label>

      <div class="qn-inbox-extract-form__actions">
        <QnButton
          type="button"
          variant="ghost"
          size="sm"
          @click="editorTabsStore.closeInboxExtractModal()"
        >
          取消
        </QnButton>
        <QnButton
          type="submit"
          variant="primary"
          size="sm"
          :loading="submitting"
          data-testid="inbox-extract-submit-btn"
        >
          提取并打开
        </QnButton>
      </div>
    </form>
  </QnModalShell>
</template>

<style scoped>
.qn-inbox-extract-form {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-inbox-extract-form__range {
  margin: 0;
  padding: var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
}

.qn-inbox-extract-form__field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-inbox-extract-form__label {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-inbox-extract-form__input,
.qn-inbox-extract-form__select {
  width: 100%;
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
  font-family: var(--qn-font-sans);
}

.qn-inbox-extract-form__input:focus,
.qn-inbox-extract-form__select:focus {
  outline: none;
  border-color: var(--qn-accent);
}

.qn-inbox-extract-form__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--qn-space-2);
  margin-top: var(--qn-space-2);
}
</style>
