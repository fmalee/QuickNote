<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import {
  Compartment,
  EditorState,
  type Extension,
  Prec,
  type StateCommand,
} from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { tags } from '@lezer/highlight';
import {
  type ContextMenuEntry,
  QnBanner,
  QnButton,
  QnContextMenu,
  QnModalShell,
} from '@/components';
import { useSettingsStore } from '@/features/settings';
import { type ActionContext, actionRegistry } from '@/lib/actions';
import { onWindowFileDrop, type UnlistenFn, type WindowFileDropPayload } from '@/lib/ipc/events';
import { extractEditorErrorMessage, openExternalUrl, saveNoteImage } from './api';
import {
  DIAGNOSTICS_MAX_BYTES,
  diagnosticsField,
  dispatchDiagnostics,
  type EditorDiagnostic,
  PLAIN_TEXT_FALLBACK_BYTES,
  runLightweightDiagnostics,
} from './extensions/codeDiagnostics';
import {
  createCodeIndentCommand,
  createCodeOutdentCommand,
  detectDocumentLanguage,
  detectFileIndentation,
  type DetectedLanguage,
  type FileIndentation,
  getStreamLanguageExtension,
} from './extensions/codeLanguages';
import { EDITOR_FORMAT_ACTION_IDS, executeEditorFormatAction } from './extensions/markdownFormat';
import {
  extractImageFileFromDataTransfer,
  handleImageInsert,
  handleLocalImagePathsDrop,
} from './extensions/markdownImages';
import {
  createMarkdownIndentCommand,
  createMarkdownOutdentCommand,
  type IndentConfig,
} from './extensions/markdownIndent';
import {
  type CurrentDocContext,
  findLinkAtPos,
  isLinkTriggerEvent,
  markdownLinkPlugin,
  parseLinkTarget,
  resolveHeadingAnchor,
} from './extensions/markdownLinks';
import {
  todoBackspaceCommand,
  todoCheckboxPlugin,
  todoEnterCommand,
} from './extensions/markdownTodo';
import { type EditorTab, useEditorTabsStore } from './store';

interface EditorNotice {
  tone: 'info' | 'warning' | 'danger' | 'success';
  message: string;
}

const CONTEXT_LINK_OPEN_ID = '__qn_link_open__';
const CONTEXT_LINK_COPY_ID = '__qn_link_copy__';
const TEXT_FILE_EXT_RE = /\.(md|markdown|txt|ya?ml|json|ts|tsx|js|jsx|rs|css|html|toml)$/i;

const editorTabsStore = useEditorTabsStore();
const settingsStore = useSettingsStore();

const containerRef = ref<HTMLDivElement | null>(null);
const stateMap = new Map<string, EditorState>();
const syncedSeqMap = new Map<string, number>();

const contextMenuOpen = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const contextMenuItems = ref<ContextMenuEntry[]>([]);
const contextLinkUrl = ref<string | null>(null);

const isMetaPressed = ref(false);
const editorNotice = ref<EditorNotice | null>(null);

const localPathModalOpen = ref(false);
const pendingLocalPath = ref('');

const externalImageModalOpen = ref(false);
let externalImageConfirmResolver: ((confirmed: boolean) => void) | null = null;

const typographyCompartment = new Compartment();
const tabSizeCompartment = new Compartment();
const languageCompartment = new Compartment();
const diagnosticsCompartment = new Compartment();
const keymapCompartment = new Compartment();

const activeLanguageLabel = ref('Markdown');
const activeIndentLabel = ref('2 空格');
const activeDiagnostics = ref<EditorDiagnostic[]>([]);
const diagnosticsDisabledReason = ref<string | null>(null);
let diagnosticsTimer: ReturnType<typeof setTimeout> | null = null;
let lastFallbackMode: 'plain' | 'diag-off' | 'full' = 'full';

let view: EditorView | null = null;
let currentTabKey: string | null = null;
let suppressUpdate = false;
let isUnmounted = false;
let unlistenFileDrop: UnlistenFn | null = null;
const actionDisposers: Array<() => void> = [];

const editorSettings = computed(() => settingsStore.snapshot?.settings?.editor);
const singleClickLink = computed<boolean>(() => editorSettings.value?.singleClickLink ?? false);
const editorMaxWidth = computed<string>(() => {
  const maxWidth = editorSettings.value?.maxWidth ?? 900;
  return maxWidth === 0 ? '100%' : `${maxWidth}px`;
});

function buildTypographyTheme() {
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

function computeTabSizeBytes(tab: EditorTab, contentOverride?: string): number {
  const text = contentOverride ?? tab.draftBody;
  const utf8Bytes = new TextEncoder().encode(text).length;
  return Math.max(tab.doc?.sizeBytes ?? 0, utf8Bytes);
}

function buildLanguageConfig(
  tab: EditorTab,
  contentOverride?: string,
): {
  lang: DetectedLanguage;
  indent: FileIndentation;
  sizeBytes: number;
  mode: 'plain' | 'diag-off' | 'full';
  languageExt: Extension;
  keymapExt: Extension;
  diagnosticsExt: Extension;
} {
  const content = contentOverride ?? tab.draftBody;
  const sizeBytes = computeTabSizeBytes(tab, content);
  const path = tab.absPath || tab.relPath || '';
  const isPlainTextFallback = sizeBytes > PLAIN_TEXT_FALLBACK_BYTES;
  const lang = isPlainTextFallback
    ? { id: 'plaintext' as const, label: 'Plain Text', isMarkdown: false }
    : detectDocumentLanguage(path, content);

  const proseSize = editorSettings.value?.tabSizeProse ?? 2;
  const indent: FileIndentation = lang.isMarkdown
    ? { useTabs: false, unit: ' '.repeat(Math.max(1, proseSize)), tabSize: proseSize }
    : detectFileIndentation(isPlainTextFallback ? content.slice(0, 8192) : content);

  const codeKeymap = Prec.high(
    keymap.of([
      { key: 'Tab', run: guardComposing(createCodeIndentCommand(indent.unit)) },
      { key: 'Shift-Tab', run: guardComposing(createCodeOutdentCommand(indent.unit)) },
    ]),
  );

  if (isPlainTextFallback) {
    return {
      lang,
      indent,
      sizeBytes,
      mode: 'plain',
      languageExt: [],
      keymapExt: codeKeymap,
      diagnosticsExt: [],
    };
  }

  const diagEnabled = sizeBytes <= DIAGNOSTICS_MAX_BYTES;
  const mode: 'diag-off' | 'full' = diagEnabled ? 'full' : 'diag-off';

  if (lang.isMarkdown) {
    return {
      lang,
      indent,
      sizeBytes,
      mode,
      languageExt: [markdown(), todoCheckboxPlugin, markdownLinkPlugin],
      keymapExt: Prec.high(
        keymap.of([
          { key: 'Tab', run: guardComposing(indentCommand) },
          { key: 'Shift-Tab', run: guardComposing(outdentCommand) },
          { key: 'Enter', run: guardComposing(todoEnterCommand) },
          { key: 'Backspace', run: guardComposing(todoBackspaceCommand) },
        ]),
      ),
      diagnosticsExt: diagEnabled ? diagnosticsField : [],
    };
  }

  const streamLang = getStreamLanguageExtension(lang.id);
  return {
    lang,
    indent,
    sizeBytes,
    mode,
    languageExt: streamLang ? [streamLang] : [],
    keymapExt: codeKeymap,
    diagnosticsExt: diagEnabled ? diagnosticsField : [],
  };
}

function updateStatusAndDiagnostics(tab: EditorTab, content: string): void {
  const cfg = buildLanguageConfig(tab, content);
  activeLanguageLabel.value = cfg.mode === 'plain' ? 'Plain Text (>5MB)' : cfg.lang.label;
  activeIndentLabel.value = cfg.indent.useTabs ? 'Tab' : `${cfg.indent.tabSize} 空格`;

  if (view && cfg.mode !== lastFallbackMode) {
    lastFallbackMode = cfg.mode;
    view.dispatch({
      effects: [
        languageCompartment.reconfigure(cfg.languageExt),
        keymapCompartment.reconfigure(cfg.keymapExt),
        diagnosticsCompartment.reconfigure(cfg.diagnosticsExt),
        tabSizeCompartment.reconfigure(EditorState.tabSize.of(cfg.indent.tabSize)),
      ],
    });
  }

  const result = runLightweightDiagnostics(
    cfg.lang.id,
    content,
    tab.doc?.meta.metaError ?? null,
    cfg.sizeBytes,
  );
  activeDiagnostics.value = result.diagnostics;
  diagnosticsDisabledReason.value = result.disabledReason;

  if (view && cfg.mode !== 'plain' && cfg.sizeBytes <= DIAGNOSTICS_MAX_BYTES) {
    dispatchDiagnostics(view, result.diagnostics);
  }
}

function scheduleDiagnosticsUpdate(tab: EditorTab, content: string): void {
  if (diagnosticsTimer !== null) {
    clearTimeout(diagnosticsTimer);
  }
  diagnosticsTimer = setTimeout(() => {
    diagnosticsTimer = null;
    if (!isUnmounted && currentTabKey === tab.key) {
      updateStatusAndDiagnostics(tab, content);
    }
  }, 300);
}

function jumpToDiagnostic(diag: EditorDiagnostic): void {
  editorTabsStore.requestScrollToLine(diag.line, diag.from);
}

function getEffectiveTabSize(): number {
  const activeTab = editorTabsStore.activeTab;
  if (!activeTab) {
    return editorSettings.value?.tabSizeProse ?? 2;
  }
  return buildLanguageConfig(activeTab).indent.tabSize;
}

function reconfigureEditorCompartments(): void {
  if (!view) {
    return;
  }
  const activeTab = editorTabsStore.activeTab;
  if (!activeTab) {
    view.dispatch({
      effects: [
        typographyCompartment.reconfigure(buildTypographyTheme()),
        tabSizeCompartment.reconfigure(EditorState.tabSize.of(getEffectiveTabSize())),
      ],
    });
    return;
  }
  const cfg = buildLanguageConfig(activeTab);
  lastFallbackMode = cfg.mode;
  view.dispatch({
    effects: [
      typographyCompartment.reconfigure(buildTypographyTheme()),
      tabSizeCompartment.reconfigure(EditorState.tabSize.of(cfg.indent.tabSize)),
      languageCompartment.reconfigure(cfg.languageExt),
      keymapCompartment.reconfigure(cfg.keymapExt),
      diagnosticsCompartment.reconfigure(cfg.diagnosticsExt),
    ],
  });
}

function showNotice(tone: EditorNotice['tone'], message: string): void {
  editorNotice.value = { tone, message };
}

function dismissNotice(): void {
  editorNotice.value = null;
}

function getIndentConfig(): IndentConfig {
  const activeTab = editorTabsStore.activeTab;
  const path = activeTab?.absPath || activeTab?.relPath || '';
  const isMarkdown = path
    ? detectDocumentLanguage(path, activeTab?.draftBody ?? '').isMarkdown
    : true;
  return {
    isMarkdown,
    tabSizeProse: editorSettings.value?.tabSizeProse ?? 2,
    tabSizeCode: editorSettings.value?.tabSizeCode ?? 4,
  };
}

function buildEditorActionContext(): ActionContext {
  const activeTab = editorTabsStore.activeTab;
  const relPath = activeTab?.relPath ?? '';
  const path = activeTab?.absPath || relPath;
  const isMarkdown = path
    ? detectDocumentLanguage(path, activeTab?.draftBody ?? '').isMarkdown
    : true;
  return {
    activeScope: 'editor',
    activeScopes: ['editor', 'main-window', 'global'],
    hasActiveDoc: Boolean(activeTab),
    hasSelection: view ? !view.state.selection.main.empty : false,
    isMarkdown,
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

async function openLinkUrl(rawUrl: string): Promise<void> {
  const activeTab = editorTabsStore.activeTab;
  if (!activeTab) {
    return;
  }
  const currentDoc: CurrentDocContext = {
    absPath: activeTab.absPath,
    relPath: activeTab.relPath,
    isLibrary: activeTab.doc?.isLibrary ?? Boolean(activeTab.relPath),
  };
  const libraryRoot = settingsStore.snapshot?.settings?.libraryRoot ?? null;
  const target = parseLinkTarget(rawUrl, currentDoc, libraryRoot);

  if (target.kind === 'invalid') {
    showNotice('warning', target.reason);
    return;
  }

  if (target.kind === 'external') {
    try {
      await openExternalUrl({ url: target.url });
    } catch (error) {
      showNotice('danger', extractEditorErrorMessage(error, '打开外部链接失败'));
    }
    return;
  }

  if (target.kind === 'anchor') {
    const item = resolveHeadingAnchor(activeTab.draftBody, target.anchor);
    if (item) {
      editorTabsStore.requestScrollToLine(item.line, item.from);
    } else {
      showNotice('info', `未找到标题锚点：#${target.anchor}`);
    }
    return;
  }

  if (target.kind === 'relativeNote') {
    const opened = await editorTabsStore.openNote(target.targetRelPath ?? target.targetAbsPath, {
      preview: false,
      absPath: target.targetAbsPath,
    });
    if (!opened || opened.isMissing || !opened.doc) {
      showNotice('warning', '目标文件不存在，可能已被移动或删除');
      return;
    }
    if (target.anchor) {
      const item = resolveHeadingAnchor(opened.draftBody, target.anchor);
      if (item) {
        editorTabsStore.requestScrollToLine(item.line, item.from);
      } else {
        showNotice('info', `未找到标题锚点：#${target.anchor}`);
      }
    }
    return;
  }

  if (target.kind === 'localAbsolute') {
    pendingLocalPath.value = target.path;
    localPathModalOpen.value = true;
  }
}

async function confirmOpenLocalPath(): Promise<void> {
  const targetPath = pendingLocalPath.value;
  localPathModalOpen.value = false;
  pendingLocalPath.value = '';
  if (!targetPath) {
    return;
  }
  if (!TEXT_FILE_EXT_RE.test(targetPath)) {
    showNotice('info', '仅支持在编辑器中打开 Markdown 或文本文件');
    return;
  }
  const opened = await editorTabsStore.openNote(targetPath, {
    absPath: targetPath,
    preview: false,
  });
  if (!opened || opened.isMissing) {
    showNotice('warning', '目标文件不存在或无法读取');
  }
}

function cancelOpenLocalPath(): void {
  localPathModalOpen.value = false;
  pendingLocalPath.value = '';
}

function requestExternalImageConfirm(): Promise<boolean> {
  if (externalImageModalOpen.value || externalImageConfirmResolver !== null) {
    return Promise.resolve(false);
  }
  externalImageModalOpen.value = true;
  return new Promise<boolean>((resolve) => {
    externalImageConfirmResolver = resolve;
  });
}

function resolveExternalImageConfirm(confirmed: boolean): void {
  externalImageModalOpen.value = false;
  const resolver = externalImageConfirmResolver;
  externalImageConfirmResolver = null;
  resolver?.(confirmed);
}

function insertSnippetAtViewPos(
  targetView: EditorView,
  insertPos: number | null,
  snippet: string,
): void {
  const docLen = targetView.state.doc.length;
  const sel = targetView.state.selection.main;
  const clampedPos = insertPos !== null ? Math.max(0, Math.min(insertPos, docLen)) : null;
  const from = clampedPos !== null ? clampedPos : sel.from;
  const to = clampedPos !== null ? clampedPos : sel.to;
  targetView.dispatch({
    changes: { from, to, insert: snippet },
    selection: { anchor: from + snippet.length },
  });
  targetView.focus();
}

async function processImageFile(
  file: File,
  targetView: EditorView,
  insertPos: number | null,
): Promise<void> {
  const activeTab = editorTabsStore.activeTab;
  if (!activeTab) {
    return;
  }
  const isLibrary = activeTab.doc?.isLibrary ?? Boolean(activeTab.relPath);
  await handleImageInsert({
    file,
    isLibrary,
    contextRelPath: activeTab.relPath,
    saveImage: saveNoteImage,
    confirmExternalSave: requestExternalImageConfirm,
    insertSnippet: (snippet) => {
      insertSnippetAtViewPos(targetView, insertPos, snippet);
    },
    onError: (message) => {
      showNotice('danger', message);
    },
  });
}

async function processDroppedImagePaths(payload: WindowFileDropPayload): Promise<void> {
  const activeTab = editorTabsStore.activeTab;
  if (!activeTab || !view || payload.paths.length === 0) {
    return;
  }
  const targetView = view;
  const dpr =
    typeof window !== 'undefined' && window.devicePixelRatio > 0 ? window.devicePixelRatio : 1;
  const clientX = payload.x / dpr;
  const clientY = payload.y / dpr;
  const rect = targetView.dom.getBoundingClientRect();
  const insideEditor =
    clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom;
  const insertPos = insideEditor ? targetView.posAtCoords({ x: clientX, y: clientY }) : null;
  const isLibrary = activeTab.doc?.isLibrary ?? Boolean(activeTab.relPath);

  await handleLocalImagePathsDrop({
    paths: payload.paths,
    isLibrary,
    contextRelPath: activeTab.relPath,
    saveImage: saveNoteImage,
    confirmExternalSave: requestExternalImageConfirm,
    insertSnippet: (snippet) => {
      insertSnippetAtViewPos(targetView, insertPos, snippet);
    },
    onError: (message) => {
      showNotice('danger', message);
    },
  });
}

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
  { tag: tags.comment, color: 'var(--qn-text-muted)', fontStyle: 'italic' },
  { tag: tags.string, color: 'var(--qn-success)' },
  { tag: tags.keyword, color: 'var(--qn-accent)', fontWeight: '600' },
  { tag: [tags.number, tags.bool, tags.atom], color: 'var(--qn-warning)' },
  { tag: [tags.propertyName, tags.attributeName], color: 'var(--qn-accent-hover)' },
  { tag: tags.variableName, color: 'var(--qn-text-primary)' },
  { tag: [tags.typeName, tags.tagName], color: 'var(--qn-danger)' },
  { tag: tags.operator, color: 'var(--qn-text-secondary)' },
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
  '.qn-cm-link': {
    color: 'var(--qn-accent)',
    textDecoration: 'underline',
    textUnderlineOffset: '2px',
  },
  '.qn-cm-diagnostic-error': {
    textDecoration: 'underline wavy var(--qn-danger)',
    textUnderlineOffset: '3px',
  },
  '.qn-cm-diagnostic-warning': {
    textDecoration: 'underline wavy var(--qn-warning)',
    textUnderlineOffset: '3px',
  },
});

function createEditorState(tab: EditorTab): EditorState {
  const clampedPos = Math.max(0, Math.min(tab.cursorPos, tab.draftBody.length));
  const cfg = buildLanguageConfig(tab);
  lastFallbackMode = cfg.mode;
  return EditorState.create({
    doc: tab.draftBody,
    selection: { anchor: clampedPos },
    extensions: [
      history(),
      languageCompartment.of(cfg.languageExt),
      syntaxHighlighting(qnHighlightStyle),
      qnEditorTheme,
      typographyCompartment.of(buildTypographyTheme()),
      tabSizeCompartment.of(EditorState.tabSize.of(cfg.indent.tabSize)),
      diagnosticsCompartment.of(cfg.diagnosticsExt),
      EditorView.lineWrapping,
      keymapCompartment.of(cfg.keymapExt),
      keymap.of([...defaultKeymap, ...historyKeymap]),
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
          const nextBody = update.state.doc.toString();
          editorTabsStore.updateTabBody(key, nextBody, head, line, col);
          editorTabsStore.updateTabSelection(key, fromLine, toLine, hasSelection);
          editorTabsStore.setActiveTopLine(line);
          const activeTab = editorTabsStore.activeTab;
          if (activeTab && activeTab.key === key) {
            scheduleDiagnosticsUpdate(activeTab, nextBody);
          }
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
        keydown: (event, targetView) => {
          if (
            event.key === 'Tab' &&
            !event.metaKey &&
            !event.ctrlKey &&
            !event.altKey &&
            ((!event.isComposing && event.keyCode !== 229) || targetView.composing)
          ) {
            event.stopPropagation();
          }
          return false;
        },
        click: (event, targetView) => {
          if (!isLinkTriggerEvent(event, singleClickLink.value)) {
            return false;
          }
          const pos = targetView.posAtCoords({ x: event.clientX, y: event.clientY });
          if (pos === null) {
            return false;
          }
          const link = findLinkAtPos(targetView.state, pos);
          if (!link) {
            return false;
          }
          event.preventDefault();
          event.stopPropagation();
          void openLinkUrl(link.url);
          return true;
        },
        paste: (event, targetView) => {
          const file = extractImageFileFromDataTransfer(event.clipboardData);
          if (!file) {
            return false;
          }
          event.preventDefault();
          void processImageFile(file, targetView, null);
          return true;
        },
        drop: (event, targetView) => {
          const file = extractImageFileFromDataTransfer(event.dataTransfer);
          if (!file) {
            return false;
          }
          event.preventDefault();
          const dropPos = targetView.posAtCoords({ x: event.clientX, y: event.clientY });
          void processImageFile(file, targetView, dropPos);
          return true;
        },
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
    reconfigureEditorCompartments();
    view.scrollDOM.scrollTop = tab.scrollTop;
    const head = nextState.selection.main.head;
    const lineObj = nextState.doc.lineAt(head);
    editorTabsStore.updateTabCursorAndScroll(tab.key, {
      cursorLine: lineObj.number,
      cursorCol: head - lineObj.from + 1,
    });
    editorTabsStore.setActiveTopLine(lineObj.number);
    updateStatusAndDiagnostics(tab, nextState.doc.toString());
  } finally {
    suppressUpdate = false;
  }
}

function handleWindowBlur(): void {
  isMetaPressed.value = false;
  if (currentTabKey) {
    saveCurrentViewSnapshot();
  }
  void editorTabsStore.flushSave();
}

function handleKeyDown(event: KeyboardEvent): void {
  if (event.metaKey) {
    isMetaPressed.value = true;
  }
}

function handleKeyUp(event: KeyboardEvent): void {
  if (!event.metaKey) {
    isMetaPressed.value = false;
  }
}

function handleContextMenu(event: MouseEvent): void {
  const ctx = buildEditorActionContext();
  const entries: ContextMenuEntry[] = [];
  contextLinkUrl.value = null;

  if (view) {
    const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (pos !== null) {
      const link = findLinkAtPos(view.state, pos);
      if (link) {
        contextLinkUrl.value = link.url;
        entries.push(
          { type: 'item', id: CONTEXT_LINK_OPEN_ID, title: '打开链接' },
          { type: 'item', id: CONTEXT_LINK_COPY_ID, title: '复制链接' },
        );
      }
    }
  }

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

  if (entries.length > 0 && (formatEntries.length > 0 || contextEntries.length > 0)) {
    entries.push({ type: 'separator', id: 'sep-link-context' });
  }
  entries.push(...formatEntries);
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
  const linkUrl = contextLinkUrl.value;
  contextLinkUrl.value = null;

  if (id === CONTEXT_LINK_OPEN_ID && linkUrl) {
    void openLinkUrl(linkUrl);
    view?.focus();
    return;
  }
  if (id === CONTEXT_LINK_COPY_ID && linkUrl) {
    void navigator.clipboard?.writeText(linkUrl);
    view?.focus();
    return;
  }

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
  () => [
    editorSettings.value?.fontSize,
    editorSettings.value?.lineHeight,
    editorSettings.value?.tabSizeProse,
    editorSettings.value?.tabSizeCode,
    editorSettings.value?.singleClickLink,
  ],
  () => {
    reconfigureEditorCompartments();
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
      reconfigureEditorCompartments();
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
  isUnmounted = false;
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
      updateStatusAndDiagnostics(initialTab, initialState.doc.toString());
    }
  }
  for (const actionId of EDITOR_FORMAT_ACTION_IDS) {
    actionDisposers.push(
      actionRegistry.bindHandler(actionId, () => {
        if (!view || !buildEditorActionContext().isMarkdown) {
          return;
        }
        executeEditorFormatAction(view, actionId, getIndentConfig());
        view.focus();
      }),
    );
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('blur', handleWindowBlur);
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
  }
  void onWindowFileDrop((payload) => {
    void processDroppedImagePaths(payload);
  })
    .then((unlisten) => {
      if (isUnmounted) {
        unlisten();
      } else {
        unlistenFileDrop = unlisten;
      }
    })
    .catch(() => {
      // 非 Tauri 测试环境容错
    });
});

onUnmounted(() => {
  isUnmounted = true;
  if (diagnosticsTimer !== null) {
    clearTimeout(diagnosticsTimer);
    diagnosticsTimer = null;
  }
  if (unlistenFileDrop) {
    unlistenFileDrop();
    unlistenFileDrop = null;
  }
  while (actionDisposers.length > 0) {
    const dispose = actionDisposers.pop();
    dispose?.();
  }
  if (typeof window !== 'undefined') {
    window.removeEventListener('blur', handleWindowBlur);
    window.removeEventListener('keydown', handleKeyDown);
    window.removeEventListener('keyup', handleKeyUp);
  }
  if (externalImageConfirmResolver) {
    externalImageConfirmResolver(false);
    externalImageConfirmResolver = null;
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
  <div class="qn-markdown-editor-shell">
    <QnBanner
      v-if="editorNotice"
      :tone="editorNotice.tone"
      class="qn-markdown-editor-notice"
      data-testid="editor-notice-banner"
    >
      <span>{{ editorNotice.message }}</span>
      <template #actions>
        <QnButton size="sm" variant="ghost" @click="dismissNotice">关闭</QnButton>
      </template>
    </QnBanner>
    <div
      ref="containerRef"
      :class="[
        'qn-markdown-editor',
        {
          'qn-links-single-click': singleClickLink,
          'qn-meta-pressed': isMetaPressed,
        },
      ]"
      data-testid="markdown-editor"
      :style="{ '--qn-editor-max-width': editorMaxWidth }"
      @contextmenu.prevent="handleContextMenu"
    />
    <div
      v-if="editorTabsStore.activeTab"
      class="qn-editor-status-bar"
      data-testid="editor-diagnostics-statusbar"
    >
      <span class="qn-editor-status-bar__badge" data-testid="editor-language-badge">
        {{ activeLanguageLabel }}
      </span>
      <span class="qn-editor-status-bar__badge" data-testid="editor-indent-badge">
        {{ activeIndentLabel }}
      </span>
      <span
        v-if="diagnosticsDisabledReason"
        class="qn-editor-status-bar__notice"
        data-testid="editor-diagnostics-disabled"
      >
        {{ diagnosticsDisabledReason }}
      </span>
      <button
        v-else-if="activeDiagnostics.length > 0"
        type="button"
        class="qn-editor-status-bar__diag-btn"
        data-testid="editor-diagnostics-bar"
        :title="activeDiagnostics[0]?.message"
        @click="activeDiagnostics[0] && jumpToDiagnostic(activeDiagnostics[0])"
      >
        {{ activeDiagnostics.length }} 个问题 · L{{ activeDiagnostics[0]?.line }}:
        {{ activeDiagnostics[0]?.message }}
      </button>
      <span v-else class="qn-editor-status-bar__ok">语法正常</span>
    </div>
  </div>
  <QnContextMenu
    :open="contextMenuOpen"
    :x="contextMenuX"
    :y="contextMenuY"
    :items="contextMenuItems"
    @select="handleContextMenuSelect"
    @close="contextMenuOpen = false"
  />
  <QnModalShell
    :open="localPathModalOpen"
    title="确认打开本地路径"
    max-width="sm"
    data-testid="local-path-confirm-modal"
    @close="cancelOpenLocalPath"
  >
    <p class="qn-editor-confirm-desc">即将打开以下本地绝对路径，请确认来源可信：</p>
    <code class="qn-editor-confirm-path">{{ pendingLocalPath }}</code>
    <template #footer>
      <QnButton size="sm" variant="ghost" @click="cancelOpenLocalPath">取消</QnButton>
      <QnButton size="sm" variant="primary" @click="confirmOpenLocalPath">确认打开</QnButton>
    </template>
  </QnModalShell>
  <QnModalShell
    :open="externalImageModalOpen"
    title="保存图片至笔记库"
    max-width="sm"
    data-testid="external-image-confirm-modal"
    @close="resolveExternalImageConfirm(false)"
  >
    <p class="qn-editor-confirm-desc">
      当前为库外文件，是否将图片保存至笔记库 <code>assets/YYYY-MM/</code> 并插入引用？
    </p>
    <template #footer>
      <QnButton size="sm" variant="ghost" @click="resolveExternalImageConfirm(false)"
        >取消</QnButton
      >
      <QnButton size="sm" variant="primary" @click="resolveExternalImageConfirm(true)">
        确认保存
      </QnButton>
    </template>
  </QnModalShell>
</template>

<style scoped>
.qn-markdown-editor-shell {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-markdown-editor-notice {
  width: 100%;
  max-width: var(--qn-editor-max-width, 900px);
}

.qn-markdown-editor {
  width: 100%;
  max-width: var(--qn-editor-max-width);
  min-height: 240px;
}

.qn-markdown-editor.qn-links-single-click :deep(.qn-cm-link),
.qn-markdown-editor.qn-meta-pressed :deep(.qn-cm-link) {
  cursor: pointer;
}

.qn-editor-confirm-desc {
  margin: 0 0 var(--qn-space-3);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-sm);
  line-height: 1.6;
}

.qn-editor-confirm-path {
  display: block;
  padding: var(--qn-space-2) var(--qn-space-3);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  color: var(--qn-text-primary);
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-xs);
  word-break: break-all;
}

.qn-editor-status-bar {
  width: 100%;
  max-width: var(--qn-editor-max-width, 900px);
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-top: 1px solid var(--qn-border-subtle);
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
}

.qn-editor-status-bar__badge {
  padding: 1px var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  color: var(--qn-text-secondary);
}

.qn-editor-status-bar__notice {
  color: var(--qn-warning);
}

.qn-editor-status-bar__diag-btn {
  border: none;
  background: transparent;
  padding: 0;
  color: var(--qn-danger);
  font: inherit;
  cursor: pointer;
  text-align: left;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-editor-status-bar__diag-btn:hover {
  text-decoration: underline;
}

.qn-editor-status-bar__ok {
  color: var(--qn-text-muted);
}
</style>
