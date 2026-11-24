<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import type { NoteDoc } from '@/bindings/types';
import { QnBanner, QnButton, QnInput, QnModalShell } from '@/components';
import { exportNotePdf, extractEditorErrorMessage, runExternalAction } from './api';
import { buildDefaultPdfExportPath, buildPrintHtmlDocument } from './export/markdownExport';

const props = defineProps<{
  open: boolean;
  doc: NoteDoc | null;
  body: string;
}>();

const emit = defineEmits<{
  close: [];
}>();

const destPathDraft = ref('');
const submitting = ref(false);
const errorMessage = ref<string | null>(null);
const exportedPath = ref<string | null>(null);

watch(
  () => props.open,
  (isOpen) => {
    if (!isOpen) {
      return;
    }
    destPathDraft.value = props.doc
      ? buildDefaultPdfExportPath(props.doc)
      : '~/Desktop/untitled.pdf';
    errorMessage.value = null;
    exportedPath.value = null;
    void nextTick(() => {
      if (typeof document !== 'undefined') {
        const input = document.querySelector<HTMLInputElement>(
          '[data-testid="export-pdf-path-input"]',
        );
        input?.focus();
      }
    });
  },
  { immediate: true },
);

async function handleSubmit(): Promise<void> {
  if (!props.doc || submitting.value) {
    return;
  }
  const destPath = destPathDraft.value.trim();
  if (!destPath) {
    errorMessage.value = '请输入导出目标路径。';
    return;
  }

  submitting.value = true;
  errorMessage.value = null;
  exportedPath.value = null;
  try {
    const htmlDocument = buildPrintHtmlDocument(props.doc, props.body);
    await exportNotePdf({ htmlDocument, destPath });
    exportedPath.value = destPath;
  } catch (error) {
    errorMessage.value = extractEditorErrorMessage(
      error,
      '导出 PDF 失败，请检查目标路径或磁盘权限。',
    );
  } finally {
    submitting.value = false;
  }
}

async function handleRevealInFinder(): Promise<void> {
  if (!exportedPath.value) {
    return;
  }
  try {
    await runExternalAction({
      id: 'external.reveal',
      absPath: exportedPath.value,
      dirPath: null,
    });
  } catch (error) {
    errorMessage.value = extractEditorErrorMessage(error, '无法在访达中显示该文件。');
  }
}

function handleClose(): void {
  if (submitting.value) {
    return;
  }
  emit('close');
}

defineExpose({
  destPathDraft,
  exportedPath,
  errorMessage,
  handleSubmit,
  handleRevealInFinder,
});
</script>

<template>
  <QnModalShell :open="open" title="导出为 PDF" max-width="md" @close="handleClose">
    <form class="qn-export-pdf" data-testid="export-pdf-modal" @submit.prevent="handleSubmit">
      <div class="qn-export-pdf__field">
        <label class="qn-export-pdf__label" for="export-pdf-path-input">目标文件路径</label>
        <QnInput
          id="export-pdf-path-input"
          v-model="destPathDraft"
          data-testid="export-pdf-path-input"
          placeholder="~/Desktop/note.pdf"
          :disabled="submitting"
        />
        <p class="qn-export-pdf__hint">
          支持 `~/...` 或绝对路径，将使用打印级浅色高对比度 A4 排版导出。
        </p>
      </div>

      <QnBanner v-if="errorMessage" tone="danger" title="导出失败" data-testid="export-pdf-error">
        {{ errorMessage }}
      </QnBanner>

      <QnBanner
        v-if="exportedPath"
        tone="success"
        title="PDF 导出成功"
        data-testid="export-pdf-success"
      >
        已保存至 <code>{{ exportedPath }}</code>
      </QnBanner>

      <div class="qn-export-pdf__actions">
        <QnButton
          v-if="exportedPath"
          type="button"
          variant="secondary"
          data-testid="export-pdf-reveal-btn"
          @click="handleRevealInFinder"
        >
          在访达中显示
        </QnButton>
        <QnButton
          type="button"
          variant="ghost"
          data-testid="export-pdf-close-btn"
          :disabled="submitting"
          @click="handleClose"
        >
          关闭
        </QnButton>
        <QnButton
          type="submit"
          variant="primary"
          data-testid="export-pdf-submit-btn"
          :disabled="submitting || !doc"
        >
          {{ submitting ? '正在导出…' : '导出 PDF' }}
        </QnButton>
      </div>
    </form>
  </QnModalShell>
</template>

<style scoped>
.qn-export-pdf {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-export-pdf__field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-export-pdf__label {
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-primary);
}

.qn-export-pdf__hint {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-export-pdf__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--qn-space-2);
  margin-top: var(--qn-space-1);
}
</style>
