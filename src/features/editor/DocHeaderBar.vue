<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import type {
  ExternalBookmark,
  ExternalDirectoryListing,
  NoteMetaPatch,
  NoteType,
} from '@/bindings/types';
import {
  type ContextMenuEntry,
  QnBadge,
  QnButton,
  QnContextMenu,
  QnModalShell,
} from '@/components';
import { actionRegistry } from '@/lib/actions';
import {
  extractEditorErrorMessage,
  fetchExternalBookmarks,
  listCurrentDocSiblings,
  removeExternalBookmark,
  upsertExternalBookmark,
} from './api';
import DocPropertiesPopover from './DocPropertiesPopover.vue';
import { copyMarkdownAsRichText } from './export/markdownExport';
import SiblingFilesPopover from './SiblingFilesPopover.vue';
import { needsExternalFrontmatterConfirm, useEditorTabsStore } from './store';
import { isExternalActionId, useExternalActions } from './useExternalActions';

const editorTabsStore = useEditorTabsStore();
const externalActions = useExternalActions();

const titleDraft = ref('');
const tagInputDraft = ref('');
const propertiesOpen = ref(false);
const siblingsOpen = ref(false);
const siblingDirPath = ref<string | null>(null);
const externalBookmarks = ref<ExternalBookmark[]>([]);
const bookmarkBusy = ref(false);
const externalMenuOpen = ref(false);
const externalMenuPos = ref({ x: 0, y: 0 });
const moreMenuOpen = ref(false);
const moreMenuPos = ref({ x: 0, y: 0 });
const copyFeedbackText = ref<string | null>(null);
let copyFeedbackTimer: ReturnType<typeof setTimeout> | null = null;
const externalConfirmOpen = ref(false);
const pendingAction = ref<
  { kind: 'open-properties' } | { kind: 'patch'; patch: NoteMetaPatch } | null
>(null);

const doc = computed(() => editorTabsStore.activeDoc);
const tab = computed(() => editorTabsStore.activeTab);
const isExternalDoc = computed(() => Boolean(doc.value && !doc.value.isLibrary));

const isMarkdown = computed(() => {
  const currentDoc = doc.value;
  if (!currentDoc) {
    return false;
  }
  const path = currentDoc.absPath || currentDoc.relPath || '';
  return /\.(md|markdown)$/i.test(path);
});

const isInboxWithSelection = computed(() => {
  const currentDoc = doc.value;
  const currentTab = tab.value;
  if (!currentDoc || !currentTab) {
    return false;
  }
  return (currentDoc.isInbox || currentDoc.relPath === 'inbox.md') && currentTab.hasSelection;
});

const isCurrentFileBookmarked = computed(() => {
  const currentDoc = doc.value;
  if (!currentDoc || currentDoc.isLibrary) {
    return false;
  }
  if (currentDoc.meta.pinned) {
    return true;
  }
  return externalBookmarks.value.some(
    (item) => item.kind === 'file' && item.path === currentDoc.absPath,
  );
});

const resolvedExternalDirPath = computed(() => {
  if (!isExternalDoc.value) {
    return null;
  }
  return (
    siblingDirPath.value ??
    externalActions.getResolvedPath('external.vscode') ??
    externalActions.getResolvedPath('external.terminal') ??
    null
  );
});

const isCurrentDirBookmarked = computed(() => {
  const dirPath = resolvedExternalDirPath.value;
  if (!dirPath) {
    return false;
  }
  return externalBookmarks.value.some((item) => item.kind === 'directory' && item.path === dirPath);
});

const vsCodeActionTitle = computed(
  () => externalActions.getDisabledReason('external.vscode') ?? '在 VS Code 中打开',
);

async function loadExternalBookmarks(): Promise<void> {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    externalBookmarks.value = await fetchExternalBookmarks();
  } catch {
    // 独立单元测试或非桌面环境容错
  }
}

watch(
  () => doc.value?.meta.title ?? doc.value?.relPath ?? '',
  (nextTitle) => {
    titleDraft.value = nextTitle;
  },
  { immediate: true },
);

const moreMenuItems: ContextMenuEntry[] = [
  { type: 'item', id: 'editor.copyRichText', title: '复制为富文本' },
  { type: 'separator', id: 'export-separator' },
  { type: 'item', id: 'editor.exportPdf', title: '导出为 PDF…' },
];

watch(
  () => doc.value?.absPath,
  () => {
    propertiesOpen.value = false;
    siblingsOpen.value = false;
    siblingDirPath.value = null;
    externalMenuOpen.value = false;
    moreMenuOpen.value = false;
    externalConfirmOpen.value = false;
    pendingAction.value = null;
    if (isExternalDoc.value) {
      void loadExternalBookmarks();
    }
  },
);

onMounted(() => {
  if (isExternalDoc.value) {
    void loadExternalBookmarks();
  }
});

onUnmounted(() => {
  if (copyFeedbackTimer !== null) {
    clearTimeout(copyFeedbackTimer);
    copyFeedbackTimer = null;
  }
});

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

function handleExternalMenuClick(event: MouseEvent): void {
  if (externalMenuOpen.value) {
    externalMenuOpen.value = false;
    return;
  }
  const target = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  const rect = target?.getBoundingClientRect();
  externalMenuPos.value = rect
    ? { x: rect.left, y: rect.bottom + 4 }
    : { x: event.clientX, y: event.clientY };
  externalMenuOpen.value = true;
}

async function handleExternalMenuSelect(id: string): Promise<void> {
  if (!isExternalActionId(id)) {
    return;
  }
  await externalActions.executeAction(id, { pinned: doc.value?.meta.pinned });
}

function handleMoreMenuClick(event: MouseEvent): void {
  if (moreMenuOpen.value) {
    moreMenuOpen.value = false;
    return;
  }
  const target = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  const rect = target?.getBoundingClientRect();
  moreMenuPos.value = rect
    ? { x: rect.left, y: rect.bottom + 4 }
    : { x: event.clientX, y: event.clientY };
  moreMenuOpen.value = true;
}

function showCopyFeedback(message: string): void {
  copyFeedbackText.value = message;
  if (copyFeedbackTimer !== null) {
    clearTimeout(copyFeedbackTimer);
  }
  copyFeedbackTimer = setTimeout(() => {
    copyFeedbackText.value = null;
    copyFeedbackTimer = null;
  }, 2400);
}

async function handleMoreMenuSelect(id: string): Promise<void> {
  moreMenuOpen.value = false;
  const currentDoc = doc.value;
  if (!currentDoc) {
    return;
  }
  if (id === 'editor.copyRichText') {
    const body = tab.value?.draftBody ?? currentDoc.body;
    const mode = await copyMarkdownAsRichText(body);
    showCopyFeedback(mode === 'rich' ? '已复制为富文本' : '已复制纯文本');
  } else if (id === 'editor.exportPdf') {
    await actionRegistry.executeById('editor.exportPdf', {
      activeScope: 'editor',
      hasActiveDoc: true,
    });
  }
}

async function handleToggleFileBookmark(): Promise<void> {
  const currentDoc = doc.value;
  if (!currentDoc || currentDoc.isLibrary || bookmarkBusy.value) {
    return;
  }
  bookmarkBusy.value = true;
  try {
    if (isCurrentFileBookmarked.value) {
      externalBookmarks.value = await removeExternalBookmark(currentDoc.absPath);
      if (currentDoc.meta.pinned) {
        await externalActions.executeAction('external.pin', { pinned: true });
      }
      showCopyFeedback('已取消收藏文件书签');
    } else {
      externalBookmarks.value = await upsertExternalBookmark({
        path: currentDoc.absPath,
        kind: 'file',
        alias: null,
        pinned: true,
        lastOpenedAt: null,
      });
      showCopyFeedback('已收藏此文件为书签');
    }
  } catch (error) {
    externalActions.feedback = {
      tone: 'danger',
      message: extractEditorErrorMessage(error, '更新外部文件书签失败'),
    };
  } finally {
    bookmarkBusy.value = false;
  }
}

async function handleToggleDirBookmark(): Promise<void> {
  const currentDoc = doc.value;
  if (!currentDoc || currentDoc.isLibrary || bookmarkBusy.value) {
    return;
  }
  bookmarkBusy.value = true;
  try {
    let dirPath = resolvedExternalDirPath.value;
    if (!dirPath) {
      const listing = await listCurrentDocSiblings(currentDoc.absPath);
      siblingDirPath.value = listing.rootPath;
      dirPath = listing.rootPath;
    }
    if (!dirPath) {
      externalActions.feedback = {
        tone: 'warning',
        message: '未能解析当前外部文件所在目录',
      };
      return;
    }
    if (isCurrentDirBookmarked.value) {
      externalBookmarks.value = await removeExternalBookmark(dirPath);
      showCopyFeedback('已取消收藏常用目录书签');
    } else {
      externalBookmarks.value = await upsertExternalBookmark({
        path: dirPath,
        kind: 'directory',
        alias: null,
        pinned: true,
        lastOpenedAt: null,
      });
      showCopyFeedback('已收藏所在目录为常用目录书签');
    }
  } catch (error) {
    externalActions.feedback = {
      tone: 'danger',
      message: extractEditorErrorMessage(error, '更新常用目录书签失败'),
    };
  } finally {
    bookmarkBusy.value = false;
  }
}

function handleToggleSiblingsPopover(): void {
  if (siblingsOpen.value) {
    siblingsOpen.value = false;
    return;
  }
  propertiesOpen.value = false;
  externalMenuOpen.value = false;
  moreMenuOpen.value = false;
  siblingsOpen.value = true;
}

function handleSiblingsLoaded(listing: ExternalDirectoryListing): void {
  siblingDirPath.value = listing.rootPath;
}

async function handleOpenInVsCode(): Promise<void> {
  await externalActions.executeAction('external.vscode');
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
        v-if="isMarkdown"
        v-model="titleDraft"
        type="text"
        class="qn-doc-header__title-input"
        data-testid="doc-header-title-input"
        aria-label="文档标题"
        @blur="handleTitleCommit"
        @keydown="handleTitleKeydown"
      />
      <span v-else class="qn-doc-header__title-readonly" data-testid="doc-header-title-readonly">
        {{ doc.meta.title || doc.relPath || doc.absPath }}
      </span>

      <div class="qn-doc-header__controls">
        <QnBadge
          v-if="isExternalDoc"
          tone="accent"
          size="sm"
          data-testid="doc-header-external-badge"
        >
          外部
        </QnBadge>

        <select
          v-if="isMarkdown"
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
          v-if="isMarkdown && isInboxWithSelection"
          variant="secondary"
          size="sm"
          data-testid="inbox-extract-btn"
          @click="editorTabsStore.openInboxExtractModal()"
        >
          移到新笔记…
        </QnButton>

        <QnButton
          variant="ghost"
          size="sm"
          data-testid="doc-header-open-vscode-btn"
          :title="vsCodeActionTitle"
          @click="handleOpenInVsCode"
        >
          在 VS Code 中打开
        </QnButton>

        <div class="qn-doc-header__prop-anchor">
          <QnButton
            variant="ghost"
            size="sm"
            data-testid="doc-header-external-btn"
            @click="handleExternalMenuClick"
          >
            外部动作 ▾
          </QnButton>
          <QnContextMenu
            :open="externalMenuOpen"
            :x="externalMenuPos.x"
            :y="externalMenuPos.y"
            :items="externalActions.menuItems"
            @select="handleExternalMenuSelect"
            @close="externalMenuOpen = false"
          />
        </div>

        <div v-if="isMarkdown" class="qn-doc-header__prop-anchor">
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

        <span
          v-if="copyFeedbackText"
          class="qn-doc-header__copy-feedback"
          data-testid="doc-header-copy-feedback"
        >
          {{ copyFeedbackText }}
        </span>

        <div class="qn-doc-header__prop-anchor">
          <QnButton
            variant="ghost"
            size="sm"
            data-testid="doc-header-more-btn"
            aria-label="更多操作"
            @click="handleMoreMenuClick"
          >
            ⋯
          </QnButton>
          <QnContextMenu
            :open="moreMenuOpen"
            :x="moreMenuPos.x"
            :y="moreMenuPos.y"
            :items="moreMenuItems"
            @select="handleMoreMenuSelect"
            @close="moreMenuOpen = false"
          />
        </div>
      </div>
    </div>

    <div
      v-if="isExternalDoc"
      class="qn-doc-header__external-bar"
      data-testid="doc-header-external-bar"
    >
      <span
        class="qn-doc-header__external-path"
        data-testid="doc-header-external-path"
        :title="doc.absPath"
      >
        {{ doc.absPath }}
      </span>

      <div class="qn-doc-header__external-actions">
        <QnButton
          variant="ghost"
          size="sm"
          :disabled="bookmarkBusy"
          data-testid="doc-header-pin-external-btn"
          @click="handleToggleFileBookmark"
        >
          {{ isCurrentFileBookmarked ? '取消固定书签' : '收藏文件书签' }}
        </QnButton>

        <QnButton
          variant="ghost"
          size="sm"
          :disabled="bookmarkBusy"
          data-testid="doc-header-bookmark-dir-btn"
          @click="handleToggleDirBookmark"
        >
          {{ isCurrentDirBookmarked ? '取消目录书签' : '收藏目录书签' }}
        </QnButton>

        <div class="qn-doc-header__prop-anchor">
          <QnButton
            variant="ghost"
            size="sm"
            data-testid="doc-header-siblings-btn"
            @click="handleToggleSiblingsPopover"
          >
            同目录文件 ▾
          </QnButton>
          <SiblingFilesPopover
            :open="siblingsOpen"
            :doc-path="doc.absPath"
            @close="siblingsOpen = false"
            @loaded="handleSiblingsLoaded"
          />
        </div>
      </div>
    </div>

    <div
      v-if="externalActions.feedback"
      class="qn-doc-header__external-feedback"
      :class="`is-${externalActions.feedback.tone}`"
      data-testid="doc-header-external-feedback"
      role="status"
    >
      <span class="qn-doc-header__external-feedback-text">
        {{ externalActions.feedback.message }}
      </span>
      <button
        type="button"
        class="qn-doc-header__external-feedback-close"
        aria-label="关闭提示"
        @click="externalActions.clearFeedback()"
      >
        ×
      </button>
    </div>

    <div
      v-if="isMarkdown || doc.meta.metaError || editorTabsStore.errorMessage"
      class="qn-doc-header__meta-row"
    >
      <div v-if="isMarkdown" class="qn-doc-header__tags" data-testid="doc-header-tags">
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

.qn-doc-header__title-readonly {
  flex: 1;
  min-width: 0;
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xl);
  font-weight: 700;
  font-family: var(--qn-font-sans);
  padding: var(--qn-space-1) 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.qn-doc-header__copy-feedback {
  font-size: var(--qn-text-xs);
  color: var(--qn-success);
  white-space: nowrap;
}

.qn-doc-header__confirm-text {
  margin: 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-primary);
  line-height: 1.5;
}

.qn-doc-header__external-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-sidebar);
  border: 1px solid var(--qn-border-subtle);
}

.qn-doc-header__external-path {
  flex: 1;
  min-width: 0;
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
  color: var(--qn-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-doc-header__external-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  flex-shrink: 0;
}

.qn-doc-header__external-feedback {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  font-size: var(--qn-text-xs);
  border: 1px solid var(--qn-border-subtle);
}

.qn-doc-header__external-feedback.is-success {
  background-color: var(--qn-success-soft);
  border-color: var(--qn-success-border);
  color: var(--qn-success);
}

.qn-doc-header__external-feedback.is-warning {
  background-color: var(--qn-warning-soft);
  border-color: var(--qn-warning-border);
  color: var(--qn-warning);
}

.qn-doc-header__external-feedback.is-danger {
  background-color: var(--qn-danger-soft);
  border-color: var(--qn-danger-border);
  color: var(--qn-danger);
}

.qn-doc-header__external-feedback-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-doc-header__external-feedback-close {
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  padding: 0 var(--qn-space-1);
  font-size: var(--qn-text-sm);
  line-height: 1;
  flex-shrink: 0;
}
</style>
