<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import type { NoteMeta, NoteMetaPatch } from '@/bindings/types';
import { QnButton } from '@/components';
import { useNotesStore } from '@/features/notes';
import { useEditorTabsStore } from './store';

const props = defineProps<{
  open: boolean;
  meta: NoteMeta;
}>();

const emit = defineEmits<{
  close: [];
}>();

const editorTabsStore = useEditorTabsStore();
const notesStore = useNotesStore();
const popoverRef = ref<HTMLDivElement | null>(null);

const projectDraft = ref('');
const openDirDraft = ref('');
const codeWorkspaceDraft = ref('');
const terminalDirDraft = ref('');
const pinnedDraft = ref(false);
const archivedDraft = ref(false);
const saving = ref(false);

const workspaceSuggestions = computed(() => notesStore.navTree.workspaces);

function syncFromMeta(meta: NoteMeta): void {
  projectDraft.value = meta.project ?? '';
  openDirDraft.value = meta.openDir ?? '';
  codeWorkspaceDraft.value = meta.codeWorkspace ?? '';
  terminalDirDraft.value = meta.terminalDir ?? '';
  pinnedDraft.value = meta.pinned;
  archivedDraft.value = meta.archived;
}

watch(
  () => [props.open, props.meta] as const,
  ([isOpen, nextMeta]) => {
    if (isOpen) {
      syncFromMeta(nextMeta);
    }
  },
  { immediate: true },
);

function normalizeNullableString(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

async function handleSave(): Promise<void> {
  saving.value = true;
  try {
    const patch: NoteMetaPatch = {
      project: normalizeNullableString(projectDraft.value),
      openDir: normalizeNullableString(openDirDraft.value),
      codeWorkspace: normalizeNullableString(codeWorkspaceDraft.value),
      terminalDir: normalizeNullableString(terminalDirDraft.value),
      pinned: pinnedDraft.value,
      archived: archivedDraft.value,
    };
    const updated = await editorTabsStore.updateActiveMeta(patch);
    if (updated) {
      emit('close');
    }
  } finally {
    saving.value = false;
  }
}

function handleDocumentMouseDown(event: MouseEvent): void {
  if (!props.open) {
    return;
  }
  const target = event.target as Node | null;
  if (
    target &&
    popoverRef.value &&
    !popoverRef.value.contains(target) &&
    !(target instanceof Element && target.closest('[data-testid="doc-header-properties-btn"]'))
  ) {
    emit('close');
  }
}

function handleDocumentKeydown(event: KeyboardEvent): void {
  if (props.open && event.key === 'Escape') {
    event.preventDefault();
    emit('close');
  }
}

onMounted(() => {
  if (typeof document !== 'undefined') {
    document.addEventListener('mousedown', handleDocumentMouseDown);
    document.addEventListener('keydown', handleDocumentKeydown);
  }
});

onUnmounted(() => {
  if (typeof document !== 'undefined') {
    document.removeEventListener('mousedown', handleDocumentMouseDown);
    document.removeEventListener('keydown', handleDocumentKeydown);
  }
});
</script>

<template>
  <div
    v-if="open"
    ref="popoverRef"
    class="qn-doc-properties-popover"
    data-testid="doc-properties-popover"
  >
    <div class="qn-doc-properties-popover__header">
      <span class="qn-doc-properties-popover__title">文档属性</span>
      <button
        type="button"
        class="qn-doc-properties-popover__close"
        aria-label="关闭属性面板"
        @click="emit('close')"
      >
        ×
      </button>
    </div>

    <form class="qn-doc-properties-popover__form" @submit.prevent="handleSave">
      <label class="qn-doc-properties-popover__field">
        <span class="qn-doc-properties-popover__label">所属项目 (project)</span>
        <input
          v-model="projectDraft"
          type="text"
          list="qn-doc-prop-workspaces"
          class="qn-doc-properties-popover__input"
          data-testid="doc-prop-project-input"
          placeholder="工作区或项目标识"
        />
        <datalist id="qn-doc-prop-workspaces">
          <option v-for="ws in workspaceSuggestions" :key="ws.key" :value="ws.key">
            {{ ws.name }}
          </option>
        </datalist>
      </label>

      <label class="qn-doc-properties-popover__field">
        <span class="qn-doc-properties-popover__label">项目目录 (open_dir)</span>
        <input
          v-model="openDirDraft"
          type="text"
          class="qn-doc-properties-popover__input"
          data-testid="doc-prop-opendir-input"
          placeholder="/path/to/project"
        />
      </label>

      <label class="qn-doc-properties-popover__field">
        <span class="qn-doc-properties-popover__label">VS Code 工作区 (code_workspace)</span>
        <input
          v-model="codeWorkspaceDraft"
          type="text"
          class="qn-doc-properties-popover__input"
          data-testid="doc-prop-codews-input"
          placeholder="/path/to/project.code-workspace"
        />
      </label>

      <label class="qn-doc-properties-popover__field">
        <span class="qn-doc-properties-popover__label">终端目录 (terminal_dir)</span>
        <input
          v-model="terminalDirDraft"
          type="text"
          class="qn-doc-properties-popover__input"
          data-testid="doc-prop-terminaldir-input"
          placeholder="/path/to/terminal/cwd"
        />
      </label>

      <div class="qn-doc-properties-popover__checks">
        <label class="qn-doc-properties-popover__check">
          <input v-model="pinnedDraft" type="checkbox" data-testid="doc-prop-pinned-checkbox" />
          <span>置顶文档 (pinned)</span>
        </label>
        <label class="qn-doc-properties-popover__check">
          <input v-model="archivedDraft" type="checkbox" data-testid="doc-prop-archived-checkbox" />
          <span>已归档 (archived)</span>
        </label>
      </div>

      <div class="qn-doc-properties-popover__actions">
        <QnButton type="button" variant="ghost" size="sm" @click="emit('close')">取消</QnButton>
        <QnButton
          type="submit"
          variant="primary"
          size="sm"
          :loading="saving"
          data-testid="doc-prop-save-btn"
        >
          保存属性
        </QnButton>
      </div>
    </form>
  </div>
</template>

<style scoped>
.qn-doc-properties-popover {
  position: absolute;
  top: calc(100% + var(--qn-space-1));
  right: 0;
  z-index: var(--qn-z-popover);
  width: 320px;
  padding: var(--qn-space-3);
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  box-shadow: var(--qn-shadow-lg);
}

.qn-doc-properties-popover__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--qn-space-2);
}

.qn-doc-properties-popover__title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-doc-properties-popover__close {
  border: none;
  background: transparent;
  color: var(--qn-text-muted);
  cursor: pointer;
  font-size: var(--qn-text-base);
  line-height: 1;
  padding: var(--qn-space-1);
}

.qn-doc-properties-popover__close:hover {
  color: var(--qn-text-primary);
}

.qn-doc-properties-popover__form {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-doc-properties-popover__field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-doc-properties-popover__label {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-doc-properties-popover__input {
  width: 100%;
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
}

.qn-doc-properties-popover__input:focus {
  outline: none;
  border-color: var(--qn-accent);
}

.qn-doc-properties-popover__checks {
  display: flex;
  gap: var(--qn-space-4);
  padding-top: var(--qn-space-1);
}

.qn-doc-properties-popover__check {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-primary);
  cursor: pointer;
}

.qn-doc-properties-popover__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--qn-space-2);
  margin-top: var(--qn-space-2);
}
</style>
