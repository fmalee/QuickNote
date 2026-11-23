<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { QnButton, QnModalShell } from '@/components';
import { useNotesStore } from './store';
import {
  extractTemplateNameFromSummary,
  flattenDirRelPaths,
  inferDefaultTemplateName,
} from './templateUtils';

const notesStore = useNotesStore();

const titleInputRef = ref<HTMLInputElement | null>(null);
const titleDraft = ref('未命名笔记');
const dirDraft = ref('');
const templateDraft = ref('note');
const templateTouched = ref(false);

const dirSuggestions = computed<string[]>(() => flattenDirRelPaths(notesStore.navTree.dirs));

const templateOptions = computed<Array<{ value: string; label: string }>>(() => {
  const seen = new Set<string>();
  const options: Array<{ value: string; label: string }> = [];

  for (const item of notesStore.templates) {
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
  () => notesStore.newNoteModalOpen,
  (isOpen) => {
    if (isOpen) {
      titleDraft.value = '未命名笔记';
      dirDraft.value = notesStore.newNoteDefaultDir;
      templateDraft.value = inferDefaultTemplateName(notesStore.newNoteDefaultDir);
      templateTouched.value = false;
      void nextTick(() => {
        titleInputRef.value?.focus();
        titleInputRef.value?.select();
      });
    }
  },
);

watch(dirDraft, (nextDir) => {
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
  const created = await notesStore.createNoteAndOpen({
    title: titleDraft.value.trim() || '未命名笔记',
    dirRelPath: dirDraft.value.trim(),
    templateName: templateDraft.value || null,
  });
  if (created) {
    notesStore.closeNewNoteModal();
  }
}
</script>

<template>
  <QnModalShell
    :open="notesStore.newNoteModalOpen"
    title="从模板新建笔记"
    max-width="sm"
    @close="notesStore.closeNewNoteModal()"
  >
    <form
      class="qn-new-note-form"
      data-testid="new-note-template-form"
      @submit.prevent="handleSubmit"
    >
      <label class="qn-new-note-form__field">
        <span class="qn-new-note-form__label">笔记标题</span>
        <input
          ref="titleInputRef"
          v-model="titleDraft"
          type="text"
          class="qn-new-note-form__input"
          data-testid="new-note-title-input"
          placeholder="输入笔记标题"
        />
      </label>

      <label class="qn-new-note-form__field">
        <span class="qn-new-note-form__label">目标目录（留空为库根目录）</span>
        <input
          v-model="dirDraft"
          type="text"
          list="qn-new-note-dir-suggestions"
          class="qn-new-note-form__input"
          data-testid="new-note-dir-input"
          placeholder="例如 projects/quicknote 或 commands"
        />
        <datalist id="qn-new-note-dir-suggestions">
          <option v-for="dir in dirSuggestions" :key="dir" :value="dir" />
        </datalist>
      </label>

      <label class="qn-new-note-form__field">
        <span class="qn-new-note-form__label">使用模板</span>
        <select
          :value="templateDraft"
          class="qn-new-note-form__select"
          data-testid="new-note-template-select"
          @change="handleTemplateChange"
        >
          <option v-for="opt in templateOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </select>
      </label>

      <div class="qn-new-note-form__actions">
        <QnButton type="button" variant="ghost" size="sm" @click="notesStore.closeNewNoteModal()">
          取消
        </QnButton>
        <QnButton
          type="submit"
          variant="primary"
          size="sm"
          :loading="notesStore.mutating"
          data-testid="new-note-submit-btn"
        >
          创建并打开
        </QnButton>
      </div>
    </form>
  </QnModalShell>
</template>

<style scoped>
.qn-new-note-form {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-new-note-form__field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-new-note-form__label {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-new-note-form__input,
.qn-new-note-form__select {
  width: 100%;
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
  font-family: var(--qn-font-sans);
}

.qn-new-note-form__input:focus,
.qn-new-note-form__select:focus {
  outline: none;
  border-color: var(--qn-accent);
}

.qn-new-note-form__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--qn-space-2);
  margin-top: var(--qn-space-2);
}
</style>
