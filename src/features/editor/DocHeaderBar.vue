<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { NoteMetaPatch, NoteType } from '@/bindings/types';
import { QnBadge, QnButton, QnModalShell } from '@/components';
import DocPropertiesPopover from './DocPropertiesPopover.vue';
import { needsExternalFrontmatterConfirm, useEditorTabsStore } from './store';

const editorTabsStore = useEditorTabsStore();

const titleDraft = ref('');
const tagInputDraft = ref('');
const propertiesOpen = ref(false);
const externalConfirmOpen = ref(false);
const pendingAction = ref<
  { kind: 'open-properties' } | { kind: 'patch'; patch: NoteMetaPatch } | null
>(null);

const doc = computed(() => editorTabsStore.activeDoc);
const tab = computed(() => editorTabsStore.activeTab);

const isInboxWithSelection = computed(() => {
  const currentDoc = doc.value;
  const currentTab = tab.value;
  if (!currentDoc || !currentTab) {
    return false;
  }
  return (currentDoc.isInbox || currentDoc.relPath === 'inbox.md') && currentTab.hasSelection;
});

watch(
  () => doc.value?.meta.title ?? doc.value?.relPath ?? '',
  (nextTitle) => {
    titleDraft.value = nextTitle;
  },
  { immediate: true },
);

watch(
  () => doc.value?.absPath,
  () => {
    propertiesOpen.value = false;
    externalConfirmOpen.value = false;
    pendingAction.value = null;
  },
);

function requiresExternalConfirm(): boolean {
  const currentDoc = doc.value;
  if (!currentDoc) {
    return false;
  }
  return (
    needsExternalFrontmatterConfirm(currentDoc) &&
    !editorTabsStore.hasConfirmedExternalMeta(currentDoc.absPath)
  );
}

async function commitMetaPatch(patch: NoteMetaPatch): Promise<void> {
  if (requiresExternalConfirm()) {
    pendingAction.value = { kind: 'patch', patch };
    externalConfirmOpen.value = true;
    return;
  }
  await editorTabsStore.updateActiveMeta(patch);
}

async function handleTitleCommit(): Promise<void> {
  const currentDoc = doc.value;
  if (!currentDoc) {
    return;
  }
  const original = currentDoc.meta.title || currentDoc.relPath || '';
  const trimmed = titleDraft.value.trim();
  if (!trimmed) {
    titleDraft.value = original;
    return;
  }
  if (trimmed === currentDoc.meta.title) {
    return;
  }
  await commitMetaPatch({ title: trimmed });
}

function handleTitleKeydown(event: KeyboardEvent): void {
  if (event.isComposing || event.keyCode === 229) {
    return;
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    (event.target as HTMLInputElement | null)?.blur();
  } else if (event.key === 'Escape') {
    titleDraft.value = doc.value?.meta.title || doc.value?.relPath || '';
    (event.target as HTMLInputElement | null)?.blur();
  }
}

async function handleDocTypeChange(event: Event): Promise<void> {
  const target = event.target as HTMLSelectElement | null;
  if (!target || !doc.value) {
    return;
  }
  const nextType = target.value as NoteType;
  if (nextType === doc.value.meta.docType) {
    return;
  }
  await commitMetaPatch({ docType: nextType });
}

async function handleAddTag(): Promise<void> {
  const currentDoc = doc.value;
  if (!currentDoc) {
    return;
  }
  const cleaned = tagInputDraft.value.trim().replace(/^#+/, '').trim();
  if (!cleaned) {
    tagInputDraft.value = '';
    return;
  }
  tagInputDraft.value = '';
  if (currentDoc.meta.tags.includes(cleaned)) {
    return;
  }
  await commitMetaPatch({ tags: [...currentDoc.meta.tags, cleaned] });
}

async function handleTagKeydown(event: KeyboardEvent): Promise<void> {
  if (event.isComposing || event.keyCode === 229) {
    return;
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    await handleAddTag();
  }
}

async function handleRemoveTag(tag: string): Promise<void> {
  const currentDoc = doc.value;
  if (!currentDoc) {
    return;
  }
  const nextTags = currentDoc.meta.tags.filter((item) => item !== tag);
  await commitMetaPatch({ tags: nextTags });
}

function handlePropertiesClick(): void {
  if (propertiesOpen.value) {
    propertiesOpen.value = false;
    return;
  }
  if (requiresExternalConfirm()) {
    pendingAction.value = { kind: 'open-properties' };
    externalConfirmOpen.value = true;
    return;
  }
  propertiesOpen.value = true;
}

async function confirmExternalFrontmatter(): Promise<void> {
  const currentDoc = doc.value;
  const action = pendingAction.value;
  externalConfirmOpen.value = false;
  pendingAction.value = null;
  if (!currentDoc || !action) {
    return;
  }
  editorTabsStore.markExternalMetaConfirmed(currentDoc.absPath);
  if (action.kind === 'open-properties') {
    propertiesOpen.value = true;
  } else {
    await editorTabsStore.updateActiveMeta(action.patch);
  }
}

function cancelExternalFrontmatter(): void {
  externalConfirmOpen.value = false;
  pendingAction.value = null;
  if (doc.value) {
    titleDraft.value = doc.value.meta.title || doc.value.relPath || '';
  }
}
</script>

<template>
  <header v-if="doc" class="qn-doc-header" data-testid="doc-header-bar">
    <div class="qn-doc-header__top">
      <input
        v-model="titleDraft"
        type="text"
        class="qn-doc-header__title-input"
        data-testid="doc-header-title-input"
        aria-label="文档标题"
        @blur="handleTitleCommit"
        @keydown="handleTitleKeydown"
      />

      <div class="qn-doc-header__controls">
        <select
          :value="doc.meta.docType"
          class="qn-doc-header__type-select"
          data-testid="doc-header-type-select"
          aria-label="文档类型"
          @change="handleDocTypeChange"
        >
          <option value="note">note</option>
          <option value="doc">doc</option>
          <option value="command-book">command-book</option>
          <option value="template">template</option>
        </select>

        <QnButton
          v-if="isInboxWithSelection"
          variant="secondary"
          size="sm"
          data-testid="inbox-extract-btn"
          @click="editorTabsStore.openInboxExtractModal()"
        >
          移到新笔记…
        </QnButton>

        <div class="qn-doc-header__prop-anchor">
          <QnButton
            variant="ghost"
            size="sm"
            data-testid="doc-header-properties-btn"
            @click="handlePropertiesClick"
          >
            属性
          </QnButton>
          <DocPropertiesPopover
            :open="propertiesOpen"
            :meta="doc.meta"
            @close="propertiesOpen = false"
          />
        </div>
      </div>
    </div>

    <div class="qn-doc-header__meta-row">
      <div class="qn-doc-header__tags" data-testid="doc-header-tags">
        <span v-for="tag in doc.meta.tags" :key="tag" class="qn-doc-header__tag-chip">
          <span>#{{ tag }}</span>
          <button
            type="button"
            class="qn-doc-header__tag-remove"
            :aria-label="`移除标签 ${tag}`"
            @click="handleRemoveTag(tag)"
          >
            ×
          </button>
        </span>

        <input
          v-model="tagInputDraft"
          type="text"
          class="qn-doc-header__tag-input"
          data-testid="doc-header-tag-input"
          placeholder="+ 标签"
          @keydown.enter="handleTagKeydown"
        />
      </div>

      <QnBadge v-if="doc.meta.metaError" tone="warning" data-testid="editor-doc-meta-error">
        Frontmatter 异常：{{ doc.meta.metaError }}
      </QnBadge>

      <span v-if="editorTabsStore.errorMessage" class="qn-doc-header__error">
        {{ editorTabsStore.errorMessage }}
      </span>
    </div>

    <QnModalShell
      :open="externalConfirmOpen"
      title="写入 YAML Frontmatter 确认"
      max-width="sm"
      @close="cancelExternalFrontmatter"
    >
      <p class="qn-doc-header__confirm-text" data-testid="external-frontmatter-confirm-text">
        该外部文件当前不包含 YAML frontmatter，保存属性将在文件顶部写入最小 frontmatter，是否继续？
      </p>
      <template #footer>
        <QnButton variant="ghost" size="sm" @click="cancelExternalFrontmatter">取消</QnButton>
        <QnButton
          variant="primary"
          size="sm"
          data-testid="external-frontmatter-confirm-btn"
          @click="confirmExternalFrontmatter"
        >
          继续写入
        </QnButton>
      </template>
    </QnModalShell>
  </header>
</template>

<style scoped>
.qn-doc-header {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
  padding-bottom: var(--qn-space-3);
  border-bottom: 1px solid var(--qn-border-subtle);
}

.qn-doc-header__top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
}

.qn-doc-header__title-input {
  flex: 1;
  min-width: 0;
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xl);
  font-weight: 700;
  font-family: var(--qn-font-sans);
  padding: var(--qn-space-1) var(--qn-space-2);
  margin-left: calc(var(--qn-space-2) * -1);
}

.qn-doc-header__title-input:hover,
.qn-doc-header__title-input:focus {
  outline: none;
  border-color: var(--qn-border-subtle);
  background-color: var(--qn-bg-elevated);
}

.qn-doc-header__controls {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  flex-shrink: 0;
}

.qn-doc-header__type-select {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-elevated);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
}

.qn-doc-header__prop-anchor {
  position: relative;
}

.qn-doc-header__meta-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-doc-header__tags {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-doc-header__tag-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--qn-space-1);
  padding: 2px var(--qn-space-2);
  border-radius: var(--qn-radius-full);
  background-color: var(--qn-accent-soft);
  color: var(--qn-accent);
  font-size: var(--qn-text-xs);
}

.qn-doc-header__tag-remove {
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  padding: 0;
  font-size: var(--qn-text-xs);
  line-height: 1;
}

.qn-doc-header__tag-input {
  width: 88px;
  padding: 2px var(--qn-space-2);
  border: 1px dashed var(--qn-border-subtle);
  border-radius: var(--qn-radius-full);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
}

.qn-doc-header__tag-input:focus {
  outline: none;
  border-color: var(--qn-accent);
  border-style: solid;
  color: var(--qn-text-primary);
}

.qn-doc-header__error {
  font-size: var(--qn-text-xs);
  color: var(--qn-danger);
}

.qn-doc-header__confirm-text {
  margin: 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-primary);
  line-height: 1.5;
}
</style>
