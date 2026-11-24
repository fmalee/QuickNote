<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { MergeView } from '@codemirror/merge';
import { Compartment, EditorState, type Extension } from '@codemirror/state';
import { EditorView, keymap, lineNumbers } from '@codemirror/view';
import { tags } from '@lezer/highlight';
import { QnBadge, QnBanner, QnButton } from '@/components';
import { useSettingsStore } from '@/features/settings';
import { detectDocumentLanguage, getStreamLanguageExtension } from './extensions/codeLanguages';
import { useEditorTabsStore } from './store';

const props = defineProps<{
  tabKey?: string;
  absPath?: string;
  diskBody?: string;
  draftBody?: string;
}>();

const editorTabsStore = useEditorTabsStore();
const settingsStore = useSettingsStore();

const containerRef = ref<HTMLDivElement | null>(null);
const currentMergedBody = ref<string>('');

const typographyCompartment = new Compartment();
const languageCompartment = new Compartment();

let mergeView: MergeView | null = null;
let suppressRightUpdate = false;

const activeTab = computed(() =>
  props.tabKey
    ? (editorTabsStore.tabs.find((tab) => tab.key === props.tabKey) ?? null)
    : editorTabsStore.activeTab,
);

const resolvedTabKey = computed(() => props.tabKey ?? activeTab.value?.key ?? '');

const resolvedAbsPath = computed(
  () => props.absPath ?? activeTab.value?.absPath ?? activeTab.value?.relPath ?? '',
);

const resolvedDiskBody = computed(() => {
  if (props.diskBody !== undefined) {
    return props.diskBody;
  }
  const conflict = activeTab.value?.conflictState;
  return conflict?.kind === 'modified' ? conflict.diskBody : '';
});

const resolvedDraftBody = computed(() => props.draftBody ?? activeTab.value?.draftBody ?? '');

const mergeSaving = computed(() => {
  const conflict = activeTab.value?.conflictState;
  return conflict?.kind === 'modified' ? Boolean(conflict.mergeSaving) : false;
});

const mergeError = computed(() => {
  const conflict = activeTab.value?.conflictState;
  return conflict?.kind === 'modified' ? (conflict.mergeError ?? null) : null;
});

const editorSettings = computed(() => settingsStore.snapshot?.settings?.editor);

const qnHighlightStyle = HighlightStyle.define([
  { tag: tags.heading1, fontWeight: '700', fontSize: '1.35em', color: 'var(--qn-text-primary)' },
  { tag: tags.heading2, fontWeight: '700', fontSize: '1.2em', color: 'var(--qn-text-primary)' },
  { tag: tags.heading3, fontWeight: '600', fontSize: '1.1em', color: 'var(--qn-text-primary)' },
  { tag: tags.strong, fontWeight: '700' },
  { tag: tags.emphasis, fontStyle: 'italic' },
  { tag: tags.link, color: 'var(--qn-accent)', textDecoration: 'underline' },
  { tag: tags.url, color: 'var(--qn-text-secondary)' },
  { tag: tags.monospace, fontFamily: 'var(--qn-font-mono)', color: 'var(--qn-text-primary)' },
  { tag: tags.quote, color: 'var(--qn-text-secondary)', fontStyle: 'italic' },
  { tag: tags.meta, color: 'var(--qn-text-tertiary)' },
  { tag: tags.keyword, color: 'var(--qn-accent)' },
  { tag: tags.string, color: 'var(--qn-success)' },
  { tag: tags.comment, color: 'var(--qn-text-tertiary)', fontStyle: 'italic' },
]);

const qnMergeEditorTheme = EditorView.theme({
  '&': {
    backgroundColor: 'var(--qn-bg-base)',
    color: 'var(--qn-text-primary)',
    height: '100%',
  },
  '.cm-scroller': {
    fontFamily: 'var(--qn-font-sans)',
    overflow: 'auto',
  },
  '.cm-content': {
    padding: 'var(--qn-space-4) var(--qn-space-4)',
    caretColor: 'var(--qn-accent)',
  },
  '.cm-gutters': {
    backgroundColor: 'var(--qn-bg-surface)',
    color: 'var(--qn-text-tertiary)',
    borderRight: '1px solid var(--qn-border-subtle)',
  },
  '.cm-cursor': {
    borderLeftColor: 'var(--qn-accent)',
  },
  '.cm-selectionBackground, ::selection': {
    backgroundColor: 'var(--qn-accent-soft)',
  },
});

function buildTypographyTheme(): Extension {
  const fontSize = editorSettings.value?.fontSize ?? 15;
  const lineHeight = editorSettings.value?.lineHeight ?? 1.7;
  return EditorView.theme({
    '&': {
      fontSize: `${fontSize}px`,
    },
    '.cm-scroller': {
      lineHeight: String(lineHeight),
    },
  });
}

function resolveLanguageExt(absPath: string, sampleBody: string): Extension {
  const detected = detectDocumentLanguage(absPath, sampleBody);
  if (detected.isMarkdown) {
    return markdown();
  }
  const streamLang = getStreamLanguageExtension(detected.id);
  return streamLang ? [streamLang] : [];
}

function buildSharedExtensions(absPath: string, sampleBody: string): Extension[] {
  return [
    lineNumbers(),
    EditorView.lineWrapping,
    qnMergeEditorTheme,
    syntaxHighlighting(qnHighlightStyle),
    typographyCompartment.of(buildTypographyTheme()),
    languageCompartment.of(resolveLanguageExt(absPath, sampleBody)),
  ];
}

function destroyMergeView(): void {
  if (mergeView) {
    mergeView.destroy();
    mergeView = null;
  }
}

function mountMergeView(): void {
  if (!containerRef.value) {
    return;
  }
  destroyMergeView();

  const diskText = resolvedDiskBody.value;
  const draftText = resolvedDraftBody.value;
  const absPath = resolvedAbsPath.value;
  currentMergedBody.value = draftText;

  const shared = buildSharedExtensions(absPath, draftText || diskText);

  mergeView = new MergeView({
    parent: containerRef.value,
    revertControls: 'a-to-b',
    highlightChanges: true,
    gutter: true,
    collapseUnchanged: { margin: 3, minSize: 4 },
    a: {
      doc: diskText,
      extensions: [EditorState.readOnly.of(true), EditorView.editable.of(false), ...shared],
    },
    b: {
      doc: draftText,
      extensions: [
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        EditorView.updateListener.of((update) => {
          if (suppressRightUpdate || !update.docChanged) {
            return;
          }
          const nextText = update.state.doc.toString();
          currentMergedBody.value = nextText;
          if (resolvedTabKey.value) {
            editorTabsStore.updateMergeDraftBody(resolvedTabKey.value, nextText);
          }
        }),
        ...shared,
      ],
    },
  });
}

function handleAcceptAllDisk(): void {
  if (!mergeView) {
    return;
  }
  const diskText = mergeView.a.state.doc.toString();
  const currentB = mergeView.b.state.doc.toString();
  if (currentB === diskText) {
    return;
  }
  mergeView.b.dispatch({
    changes: { from: 0, to: currentB.length, insert: diskText },
  });
}

function handleCancel(): void {
  if (!resolvedTabKey.value) {
    return;
  }
  editorTabsStore.cancelConflictMerge(resolvedTabKey.value);
}

async function handleCommit(): Promise<void> {
  if (!resolvedTabKey.value || mergeSaving.value) {
    return;
  }
  const bodyToCommit = mergeView ? mergeView.b.state.doc.toString() : currentMergedBody.value;
  await editorTabsStore.commitConflictMerge(resolvedTabKey.value, bodyToCommit);
}

watch(
  () => resolvedTabKey.value,
  () => {
    mountMergeView();
  },
);

watch(
  () => resolvedDiskBody.value,
  (nextDiskBody) => {
    if (!mergeView) {
      return;
    }
    const currentA = mergeView.a.state.doc.toString();
    if (currentA !== nextDiskBody) {
      mergeView.a.dispatch({
        changes: { from: 0, to: currentA.length, insert: nextDiskBody },
      });
    }
  },
);

watch(
  () => [editorSettings.value?.fontSize, editorSettings.value?.lineHeight],
  () => {
    if (!mergeView) {
      return;
    }
    const nextTheme = buildTypographyTheme();
    mergeView.a.dispatch({ effects: typographyCompartment.reconfigure(nextTheme) });
    mergeView.b.dispatch({ effects: typographyCompartment.reconfigure(nextTheme) });
  },
);

onMounted(() => {
  mountMergeView();
});

onUnmounted(() => {
  suppressRightUpdate = true;
  destroyMergeView();
});
</script>

<template>
  <div class="qn-merge-view" data-testid="editor-merge-view">
    <div class="qn-merge-view__toolbar">
      <div class="qn-merge-view__panes-meta">
        <QnBadge tone="neutral" data-testid="merge-left-badge">磁盘最新版本（只读）</QnBadge>
        <span class="qn-merge-view__sep" aria-hidden="true">⇄</span>
        <QnBadge tone="accent" data-testid="merge-right-badge">本地未保存版本（可编辑）</QnBadge>
      </div>
      <div class="qn-merge-view__actions">
        <QnButton
          size="sm"
          variant="secondary"
          data-testid="merge-accept-all-disk-btn"
          :disabled="mergeSaving"
          @click="handleAcceptAllDisk"
        >
          全部采用磁盘
        </QnButton>
        <QnButton
          size="sm"
          variant="ghost"
          data-testid="merge-cancel-btn"
          :disabled="mergeSaving"
          @click="handleCancel"
        >
          取消返回
        </QnButton>
        <QnButton
          size="sm"
          variant="primary"
          data-testid="merge-commit-btn"
          :disabled="mergeSaving"
          @click="void handleCommit()"
        >
          {{ mergeSaving ? '正在保存…' : '完成合并并保存' }}
        </QnButton>
      </div>
    </div>

    <div v-if="mergeError" class="qn-merge-view__error">
      <QnBanner tone="warning" title="合并保存提示" data-testid="merge-error-banner">
        {{ mergeError }}
      </QnBanner>
    </div>

    <div ref="containerRef" class="qn-merge-view__container" />
  </div>
</template>

<style scoped>
.qn-merge-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background-color: var(--qn-bg-base);
}

.qn-merge-view__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-surface);
  flex-shrink: 0;
}

.qn-merge-view__panes-meta {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  min-width: 0;
}

.qn-merge-view__sep {
  color: var(--qn-text-tertiary);
  font-size: var(--qn-text-sm);
}

.qn-merge-view__actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  flex-shrink: 0;
}

.qn-merge-view__error {
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-base);
  flex-shrink: 0;
}

.qn-merge-view__container {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.qn-merge-view__container :deep(.cm-mergeView) {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: auto;
  background-color: var(--qn-bg-base);
}

.qn-merge-view__container :deep(.cm-mergeViewEditors) {
  display: flex;
  align-items: stretch;
  flex: 1;
  min-height: 0;
}

.qn-merge-view__container :deep(.cm-mergeViewEditor) {
  flex: 1;
  min-width: 0;
}

.qn-merge-view__container :deep(.cm-mergeViewEditor + .cm-mergeViewEditor) {
  border-left: 1px solid var(--qn-border-subtle);
}

.qn-merge-view__container :deep(.cm-changedLine) {
  background-color: var(--qn-warning-soft);
}

.qn-merge-view__container :deep(.cm-insertedLine),
.qn-merge-view__container :deep(.cm-changedText) {
  background-color: var(--qn-success-soft);
}

.qn-merge-view__container :deep(.cm-deletedLine),
.qn-merge-view__container :deep(.cm-deletedChunk) {
  background-color: var(--qn-danger-soft);
}

.qn-merge-view__container :deep(.cm-merge-revert) {
  background-color: var(--qn-bg-surface);
  border-left: 1px solid var(--qn-border-subtle);
  border-right: 1px solid var(--qn-border-subtle);
}

.qn-merge-view__container :deep(.cm-merge-revert button) {
  color: var(--qn-text-primary);
  background-color: var(--qn-bg-base);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  cursor: pointer;
}

.qn-merge-view__container :deep(.cm-merge-revert button:hover) {
  background-color: var(--qn-accent-soft);
  border-color: var(--qn-accent);
}

.qn-merge-view__container :deep(.cm-collapsedLines) {
  background-color: var(--qn-bg-surface);
  color: var(--qn-text-tertiary);
  border-top: 1px solid var(--qn-border-subtle);
  border-bottom: 1px solid var(--qn-border-subtle);
  padding: var(--qn-space-1) var(--qn-space-3);
}
</style>
