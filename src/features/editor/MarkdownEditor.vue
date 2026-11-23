<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorState, Prec, type StateCommand } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { tags } from '@lezer/highlight';
import { type ContextMenuEntry, QnContextMenu } from '@/components';
import { useSettingsStore } from '@/features/settings';
import { type ActionContext, actionRegistry } from '@/lib/actions';
import { EDITOR_FORMAT_ACTION_IDS, executeEditorFormatAction } from './extensions/markdownFormat';
import {
  createMarkdownIndentCommand,
  createMarkdownOutdentCommand,
  type IndentConfig,
} from './extensions/markdownIndent';
import {
  todoBackspaceCommand,
  todoCheckboxPlugin,
  todoEnterCommand,
} from './extensions/markdownTodo';
import { type EditorTab, useEditorTabsStore } from './store';

const editorTabsStore = useEditorTabsStore();
const settingsStore = useSettingsStore();

const containerRef = ref<HTMLDivElement | null>(null);
const stateMap = new Map<string, EditorState>();
const syncedSeqMap = new Map<string, number>();

const contextMenuOpen = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const contextMenuItems = ref<ContextMenuEntry[]>([]);

let view: EditorView | null = null;
let currentTabKey: string | null = null;
let suppressUpdate = false;
const actionDisposers: Array<() => void> = [];

function getIndentConfig(): IndentConfig {
  const relPath = editorTabsStore.activeTab?.relPath ?? '';
  return {
    isMarkdown: relPath ? relPath.toLowerCase().endsWith('.md') : true,
    tabSizeProse: settingsStore.snapshot?.settings?.editor.tabSizeProse ?? 2,
    tabSizeCode: settingsStore.snapshot?.settings?.editor.tabSizeCode ?? 4,
  };
}

function buildEditorActionContext(): ActionContext {
  const activeTab = editorTabsStore.activeTab;
  const relPath = activeTab?.relPath ?? '';
  return {
    activeScope: 'editor',
    activeScopes: ['editor', 'main-window', 'global'],
    hasActiveDoc: Boolean(activeTab),
    hasSelection: view ? !view.state.selection.main.empty : false,
    isMarkdown: relPath ? relPath.toLowerCase().endsWith('.md') : true,
    isInbox: relPath === 'inbox.md',
  };
}

function guardComposing(cmd: StateCommand) {
  return (targetView: EditorView): boolean => {
    if (targetView.composing) {
      return false;
    }
    return cmd(targetView);
  };
}

const indentCommand = createMarkdownIndentCommand(getIndentConfig);
const outdentCommand = createMarkdownOutdentCommand(getIndentConfig);

const editorMaxWidth = computed<string>(() => {
  const maxWidth = settingsStore.snapshot?.settings?.editor.maxWidth ?? 900;
  return maxWidth === 0 ? '100%' : `${maxWidth}px`;
});

const qnHighlightStyle = HighlightStyle.define([
  { tag: tags.heading1, fontWeight: '700', fontSize: '1.35em', color: 'var(--qn-text-primary)' },
  { tag: tags.heading2, fontWeight: '700', fontSize: '1.2em', color: 'var(--qn-text-primary)' },
  { tag: tags.heading3, fontWeight: '600', fontSize: '1.1em', color: 'var(--qn-text-primary)' },
  {
    tag: [tags.heading4, tags.heading5, tags.heading6],
    fontWeight: '600',
    color: 'var(--qn-text-primary)',
  },
  { tag: tags.strong, fontWeight: '700', color: 'var(--qn-text-primary)' },
  { tag: tags.emphasis, fontStyle: 'italic', color: 'var(--qn-text-primary)' },
  { tag: tags.strikethrough, textDecoration: 'line-through', color: 'var(--qn-text-muted)' },
  { tag: tags.link, color: 'var(--qn-accent)', textDecoration: 'underline' },
  { tag: tags.url, color: 'var(--qn-text-secondary)' },
  {
    tag: [tags.monospace, tags.processingInstruction],
    fontFamily: 'var(--qn-font-mono)',
    color: 'var(--qn-accent)',
  },
  { tag: tags.quote, color: 'var(--qn-text-secondary)', fontStyle: 'italic' },
  { tag: tags.meta, color: 'var(--qn-text-muted)' },
]);

const qnEditorTheme = EditorView.theme({
  '&': {
    color: 'var(--qn-text-primary)',
    backgroundColor: 'var(--qn-bg-base)',
    fontFamily: 'var(--qn-font-sans)',
    fontSize: 'var(--qn-text-base)',
    height: '100%',
  },
  '&.cm-focused': {
    outline: 'none',
  },
  '.cm-scroller': {
    fontFamily: 'var(--qn-font-sans)',
    lineHeight: '1.7',
    overflow: 'auto',
  },
  '.cm-content': {
    padding: 'var(--qn-space-2) 0 var(--qn-space-6)',
    caretColor: 'var(--qn-accent)',
    minHeight: '240px',
  },
  '.cm-line': {
    padding: '0 2px',
  },
  '.cm-cursor, .cm-dropCursor': {
    borderLeftColor: 'var(--qn-accent)',
  },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: 'var(--qn-accent-soft)',
  },
  '.cm-activeLine': {
    backgroundColor: 'var(--qn-bg-hover)',
  },
  '.cm-gutters': {
    backgroundColor: 'var(--qn-bg-base)',
    color: 'var(--qn-text-muted)',
    border: 'none',
  },
  '.qn-cm-todo-checkbox': {
    marginRight: 'var(--qn-space-2)',
    verticalAlign: 'middle',
    accentColor: 'var(--qn-accent)',
    borderColor: 'var(--qn-border-strong)',
    cursor: 'pointer',
  },
});

function createEditorState(tab: EditorTab): EditorState {
  const clampedPos = Math.max(0, Math.min(tab.cursorPos, tab.draftBody.length));
  return EditorState.create({
    doc: tab.draftBody,
    selection: { anchor: clampedPos },
    extensions: [
      history(),
      markdown(),
      syntaxHighlighting(qnHighlightStyle),
      qnEditorTheme,
      EditorView.lineWrapping,
      todoCheckboxPlugin,
      Prec.high(
        keymap.of([
          { key: 'Tab', run: guardComposing(indentCommand) },
          { key: 'Shift-Tab', run: guardComposing(outdentCommand) },
          { key: 'Enter', run: guardComposing(todoEnterCommand) },
          { key: 'Backspace', run: guardComposing(todoBackspaceCommand) },
        ]),
      ),
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        {
          key: 'Mod-s',
          run: () => {
            void editorTabsStore.flushSave();
            return true;
          },
        },
      ]),
      EditorView.updateListener.of((update) => {
        if (suppressUpdate || !currentTabKey) {
          return;
        }
        const key = currentTabKey;
        const main = update.state.selection.main;
        const head = main.head;
        const lineObj = update.state.doc.lineAt(head);
        const line = lineObj.number;
        const col = head - lineObj.from + 1;
        const fromLine = update.state.doc.lineAt(main.from).number;
        const effectiveTo = main.empty ? main.to : Math.max(main.from, main.to - 1);
        const toLine = update.state.doc.lineAt(effectiveTo).number;
        const hasSelection = !main.empty;

        if (update.docChanged) {
          editorTabsStore.updateTabBody(key, update.state.doc.toString(), head, line, col);
          editorTabsStore.updateTabSelection(key, fromLine, toLine, hasSelection);
          editorTabsStore.setActiveTopLine(line);
        } else if (update.selectionSet) {
          editorTabsStore.updateTabCursorAndScroll(key, {
            cursorPos: head,
            cursorLine: line,
            cursorCol: col,
          });
          editorTabsStore.updateTabSelection(key, fromLine, toLine, hasSelection);
          editorTabsStore.setActiveTopLine(line);
        }
      }),
      EditorView.domEventHandlers({
        scroll: (_event, targetView) => {
          if (!currentTabKey) {
            return false;
          }
          const topBlock = targetView.lineBlockAtHeight(targetView.scrollDOM.scrollTop);
          const topLine = targetView.state.doc.lineAt(topBlock.from).number;
          editorTabsStore.setActiveTopLine(topLine);
          editorTabsStore.updateTabCursorAndScroll(currentTabKey, {
            scrollTop: targetView.scrollDOM.scrollTop,
          });
          return false;
        },
      }),
    ],
  });
}

function saveCurrentViewSnapshot(): void {
  if (!view || !currentTabKey) {
    return;
  }
  stateMap.set(currentTabKey, view.state);
  const head = view.state.selection.main.head;
  const lineObj = view.state.doc.lineAt(head);
  editorTabsStore.updateTabCursorAndScroll(currentTabKey, {
    scrollTop: view.scrollDOM.scrollTop,
    cursorPos: head,
    cursorLine: lineObj.number,
    cursorCol: head - lineObj.from + 1,
    persist: true,
  });
}

function switchToTab(tab: EditorTab | null): void {
  if (!view) {
    return;
  }
  if (currentTabKey && currentTabKey !== tab?.key) {
    saveCurrentViewSnapshot();
  }
  if (!tab) {
    currentTabKey = null;
    return;
  }

  let nextState = stateMap.get(tab.key);
  const lastSyncSeq = syncedSeqMap.get(tab.key);
  if (!nextState || lastSyncSeq !== tab.syncSeq) {
    nextState = createEditorState(tab);
    stateMap.set(tab.key, nextState);
    syncedSeqMap.set(tab.key, tab.syncSeq);
  }

  currentTabKey = tab.key;
  suppressUpdate = true;
  try {
    view.setState(nextState);
    view.scrollDOM.scrollTop = tab.scrollTop;
    const head = nextState.selection.main.head;
    const lineObj = nextState.doc.lineAt(head);
    editorTabsStore.updateTabCursorAndScroll(tab.key, {
      cursorLine: lineObj.number,
      cursorCol: head - lineObj.from + 1,
    });
    editorTabsStore.setActiveTopLine(lineObj.number);
  } finally {
    suppressUpdate = false;
  }
}

function handleWindowBlur(): void {
  if (currentTabKey) {
    saveCurrentViewSnapshot();
  }
  void editorTabsStore.flushSave();
}

function handleContextMenu(event: MouseEvent): void {
  const ctx = buildEditorActionContext();
  const formatEntries: ContextMenuEntry[] = actionRegistry
    .getMenuItems('format', ctx)
    .map((item) => ({
      type: 'item',
      id: item.id,
      title: item.title,
      shortcutLabel: item.shortcutLabel,
      disabled: item.disabled,
    }));
  const contextEntries: ContextMenuEntry[] = actionRegistry
    .getMenuItems('editor-context', ctx)
    .map((item) => ({
      type: 'item',
      id: item.id,
      title: item.title,
      shortcutLabel: item.shortcutLabel,
      disabled: item.disabled,
    }));

  const entries: ContextMenuEntry[] = [...formatEntries];
  if (formatEntries.length > 0 && contextEntries.length > 0) {
    entries.push({ type: 'separator', id: 'sep-editor-context' });
  }
  entries.push(...contextEntries);

  contextMenuItems.value = entries;
  contextMenuX.value = event.clientX;
  contextMenuY.value = event.clientY;
  contextMenuOpen.value = entries.length > 0;
}

function handleContextMenuSelect(id: string): void {
  contextMenuOpen.value = false;
  void actionRegistry.executeById(id, buildEditorActionContext());
  view?.focus();
}

watch(
  () => editorTabsStore.activeTab,
  (nextTab) => {
    switchToTab(nextTab);
  },
);

watch(
  () => editorTabsStore.activeTab?.syncSeq,
  () => {
    const tab = editorTabsStore.activeTab;
    if (!tab || !view) {
      return;
    }
    const lastSyncSeq = syncedSeqMap.get(tab.key);
    if (lastSyncSeq === tab.syncSeq) {
      return;
    }
    const savedScrollTop = view.scrollDOM.scrollTop;
    const nextState = createEditorState(tab);
    stateMap.set(tab.key, nextState);
    syncedSeqMap.set(tab.key, tab.syncSeq);
    suppressUpdate = true;
    try {
      view.setState(nextState);
      view.scrollDOM.scrollTop = savedScrollTop;
    } finally {
      suppressUpdate = false;
    }
  },
);

watch(
  () => editorTabsStore.tabs.map((tab) => tab.key),
  (activeKeys) => {
    const valid = new Set(activeKeys);
    for (const key of stateMap.keys()) {
      if (!valid.has(key)) {
        stateMap.delete(key);
        syncedSeqMap.delete(key);
      }
    }
  },
);

watch(
  () => editorTabsStore.scrollTarget,
  (target) => {
    if (!target || !view || target.key !== currentTabKey) {
      return;
    }
    const maxOffset = view.state.doc.length;
    const anchor = Math.max(0, Math.min(target.from, maxOffset));
    view.dispatch({
      selection: { anchor },
      effects: EditorView.scrollIntoView(anchor, { y: 'start', yMargin: 48 }),
    });
    editorTabsStore.setActiveTopLine(target.line);
    view.focus();
  },
);

onMounted(() => {
  if (containerRef.value) {
    const initialTab = editorTabsStore.activeTab;
    const initialState = initialTab
      ? createEditorState(initialTab)
      : EditorState.create({ doc: '' });
    if (initialTab) {
      currentTabKey = initialTab.key;
      stateMap.set(initialTab.key, initialState);
      syncedSeqMap.set(initialTab.key, initialTab.syncSeq);
    }
    view = new EditorView({
      state: initialState,
      parent: containerRef.value,
    });
    if (initialTab) {
      view.scrollDOM.scrollTop = initialTab.scrollTop;
    }
  }
  for (const actionId of EDITOR_FORMAT_ACTION_IDS) {
    actionDisposers.push(
      actionRegistry.bindHandler(actionId, () => {
        if (!view) {
          return;
        }
        executeEditorFormatAction(view, actionId, getIndentConfig());
        view.focus();
      }),
    );
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('blur', handleWindowBlur);
  }
});

onUnmounted(() => {
  while (actionDisposers.length > 0) {
    const dispose = actionDisposers.pop();
    dispose?.();
  }
  if (typeof window !== 'undefined') {
    window.removeEventListener('blur', handleWindowBlur);
  }
  if (view) {
    view.destroy();
    view = null;
  }
  stateMap.clear();
  syncedSeqMap.clear();
});
</script>

<template>
  <div
    ref="containerRef"
    class="qn-markdown-editor"
    data-testid="markdown-editor"
    :style="{ '--qn-editor-max-width': editorMaxWidth }"
    @contextmenu.prevent="handleContextMenu"
  />
  <QnContextMenu
    :open="contextMenuOpen"
    :x="contextMenuX"
    :y="contextMenuY"
    :items="contextMenuItems"
    @select="handleContextMenuSelect"
    @close="contextMenuOpen = false"
  />
</template>

<style scoped>
.qn-markdown-editor {
  width: 100%;
  max-width: var(--qn-editor-max-width);
  min-height: 240px;
}
</style>
