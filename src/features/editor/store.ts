import { computed, ref, watch } from 'vue';
import { defineStore } from 'pinia';
import type { EditorState } from '@codemirror/state';
import type {
  AppendResult,
  ExternalChangeEvent,
  NoteDoc,
  NoteMeta,
  NoteMetaPatch,
  NoteSummary,
  TabItemState,
  TabsUiState,
} from '@/bindings/types';
import { useNotesStore } from '@/features/notes';
import { useSettingsStore } from '@/features/settings';
import type { UnlistenFn } from '@/lib/ipc/events';
import {
  extractEditorErrorMessage,
  extractInboxToNote,
  fetchUiState,
  isConflictError,
  isNoteNotFoundError,
  listAllNotes,
  parseConflictInfo,
  patchUiState,
  readNote,
  subscribeFsExternalChange,
  updateNoteMeta,
  writeNote,
} from './api';
import { createDocTabStub, type DocTabKind, type DocTabStub } from './core/docSession';
import { DEFAULT_EDITOR_STATE_POOL_CAPACITY, EditorStatePool } from './core/editorPool';

export const DEFAULT_INBOX_BLOAT_THRESHOLD_LINES = 500;
export const MAX_CLOSED_STACK_SIZE = 20;
export const DEFAULT_AUTO_SAVE_DEBOUNCE_MS = 600;
export const DEFAULT_AUTO_SAVE_MAX_WAIT_MS = 2000;
const TABS_PERSIST_DEBOUNCE_MS = 250;

export type EditorSaveStatus =
  'saved' | 'dirty' | 'saving' | 'conflict' | 'deleted' | 'external-synced' | 'error';

export type EditorConflictState =
  | {
      kind: 'modified';
      diskBody: string;
      diskBodyHash: string;
      diskMeta: NoteMeta | null;
      bannerDismissed?: boolean;
      mergeMode?: boolean;
      mergeSaving?: boolean;
      mergeError?: string | null;
    }
  | {
      kind: 'deleted';
    };

export interface EditorPoolEntry {
  state: EditorState | null;
  syncedSeq: number;
  doc: NoteDoc | null;
  draftBody: string;
  baseBodyHash: string;
}

export interface EditorTab extends TabItemState {
  doc: NoteDoc | null;
  isDirty: boolean;
  isMissing: boolean;
  evicted?: boolean;
  lastAccessedAt?: number;
  draftBody: string;
  baseBodyHash: string;
  conflictState: EditorConflictState | null;
  saveStatus: EditorSaveStatus;
  cursorLine: number;
  cursorCol: number;
  selectionFromLine: number;
  selectionToLine: number;
  hasSelection: boolean;
  syncSeq: number;
}

export function needsExternalFrontmatterConfirm(doc: NoteDoc): boolean {
  return (
    !doc.isLibrary &&
    doc.meta.id === null &&
    doc.meta.created === null &&
    doc.meta.tags.length === 0 &&
    doc.meta.project === null &&
    doc.meta.openDir === null &&
    doc.meta.codeWorkspace === null &&
    doc.meta.terminalDir === null &&
    !doc.meta.pinned &&
    !doc.meta.archived &&
    doc.meta.docType === 'note' &&
    doc.meta.metaError === null
  );
}

export interface OpenNoteOptions {
  preview?: boolean;
  absPath?: string;
}

export interface EditorScrollTarget {
  key: string;
  line: number;
  from: number;
  seq: number;
}

export interface EditorStatusInfo {
  docKindLabel: string;
  indentLabel: string;
  diagnosticsLabel: string;
}

function toTabItemState(tab: EditorTab): TabItemState {
  return {
    key: tab.key,
    relPath: tab.relPath,
    absPath: tab.absPath,
    isPinned: tab.isPinned,
    isPreview: tab.isPreview,
    scrollTop: tab.scrollTop,
    cursorPos: tab.cursorPos,
  };
}

function deriveTabTitle(tab: Pick<EditorTab, 'doc' | 'relPath' | 'absPath'>): string {
  const metaTitle = tab.doc?.meta.title?.trim();
  if (metaTitle) {
    return metaTitle;
  }
  const source = tab.relPath || tab.absPath;
  if (!source) {
    return '未命名笔记';
  }
  const segments = source.split('/');
  const base = segments[segments.length - 1] || source;
  return base.replace(/\.md$/i, '') || base;
}

function deriveTabKind(tab: Pick<EditorTab, 'doc' | 'relPath'>): DocTabKind {
  if (tab.doc) {
    return tab.doc.isLibrary ? 'vault' : 'external';
  }
  return tab.relPath ? 'vault' : 'external';
}

function toDocTabStub(tab: EditorTab): DocTabStub {
  return createDocTabStub({
    tabKey: tab.key,
    kind: deriveTabKind(tab),
    path: tab.absPath || tab.relPath || '',
    title: deriveTabTitle(tab),
    isDirty: tab.isDirty,
    cursorOffset: tab.cursorPos,
    scrollTop: tab.scrollTop,
    lastAccessedAt: tab.lastAccessedAt ?? 0,
    evicted: Boolean(tab.evicted),
  });
}

function buildTabFromDoc(doc: NoteDoc, isPreview: boolean): EditorTab {
  return {
    key: doc.key,
    relPath: doc.relPath,
    absPath: doc.absPath,
    isPinned: !isPreview,
    isPreview,
    scrollTop: 0,
    cursorPos: 0,
    doc,
    isDirty: false,
    isMissing: false,
    evicted: false,
    lastAccessedAt: Date.now(),
    draftBody: doc.body,
    baseBodyHash: doc.bodyHash,
    conflictState: null,
    saveStatus: 'saved',
    cursorLine: 1,
    cursorCol: 1,
    selectionFromLine: 1,
    selectionToLine: 1,
    hasSelection: false,
    syncSeq: 0,
  };
}

function isSameNoteMeta(a: NoteMeta, b: NoteMeta | undefined): boolean {
  if (!b) {
    return false;
  }
  return (
    a.title === b.title &&
    a.docType === b.docType &&
    a.tags.length === b.tags.length &&
    a.tags.every((tag, idx) => tag === b.tags[idx]) &&
    a.project === b.project &&
    a.openDir === b.openDir &&
    a.codeWorkspace === b.codeWorkspace &&
    a.terminalDir === b.terminalDir &&
    a.pinned === b.pinned &&
    a.archived === b.archived
  );
}

export const useEditorTabsStore = defineStore('editorTabs', () => {
  const settingsStore = useSettingsStore();

  const tabs = ref<EditorTab[]>([]);
  const activeKey = ref<string | null>(null);
  const closedStack = ref<string[]>([]);
  const editorStatus = ref<EditorStatusInfo>({
    docKindLabel: '',
    indentLabel: '',
    diagnosticsLabel: '',
  });

  function setEditorStatus(next: EditorStatusInfo): void {
    editorStatus.value = next;
  }
  const loading = ref<boolean>(false);
  const errorMessage = ref<string | null>(null);
  const initialized = ref<boolean>(false);
  const activeTopLine = ref<number>(1);
  const scrollTarget = ref<EditorScrollTarget | null>(null);
  const confirmedExternalMetaPaths = ref<Set<string>>(new Set());
  const inboxExtractModalOpen = ref<boolean>(false);

  let persistTimer: ReturnType<typeof setTimeout> | null = null;
  let scrollSeq = 0;
  let unlistenExternalChange: UnlistenFn | null = null;
  let lastQuickBarAppendedRelPath: string | null = null;
  const debounceTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const maxWaitTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const inFlightSaveMap = new Map<string, Promise<boolean>>();

  const activeTab = computed<EditorTab | null>(
    () => tabs.value.find((tab) => tab.key === activeKey.value) ?? null,
  );

  const activeDoc = computed<NoteDoc | null>(() => activeTab.value?.doc ?? null);

  const inboxBloatThreshold = computed<number>(
    () =>
      settingsStore.snapshot?.settings?.inbox.bloatThresholdLines ??
      DEFAULT_INBOX_BLOAT_THRESHOLD_LINES,
  );

  const autoSaveDebounceMs = computed<number>(
    () =>
      settingsStore.snapshot?.settings?.editor.autoSaveDebounceMs ?? DEFAULT_AUTO_SAVE_DEBOUNCE_MS,
  );

  const autoSaveMaxWaitMs = computed<number>(
    () =>
      settingsStore.snapshot?.settings?.editor.autoSaveMaxWaitMs ?? DEFAULT_AUTO_SAVE_MAX_WAIT_MS,
  );

  const maxActiveEditorRenders = computed<number>(() => {
    const settings = settingsStore.snapshot?.settings as
      | (Record<string, unknown> & {
          maxActiveEditorRenders?: number;
          max_active_editor_renders?: number;
          editor?: Record<string, unknown> & {
            maxActiveEditorRenders?: number;
            max_active_editor_renders?: number;
          };
        })
      | undefined;
    const raw =
      settings?.maxActiveEditorRenders ??
      settings?.max_active_editor_renders ??
      settings?.editor?.maxActiveEditorRenders ??
      settings?.editor?.max_active_editor_renders;
    return typeof raw === 'number' && Number.isFinite(raw)
      ? raw
      : DEFAULT_EDITOR_STATE_POOL_CAPACITY;
  });

  const editorStatePool = new EditorStatePool<EditorPoolEntry>(maxActiveEditorRenders.value);

  const tabStubs = computed<DocTabStub[]>(() => tabs.value.map(toDocTabStub));

  function isTabProtectedFromEviction(key: string): boolean {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab) {
      return false;
    }
    return tab.isDirty || tab.conflictState !== null || tab.saveStatus === 'saving';
  }

  function evictOverflowTabs(): string[] {
    return editorStatePool.evictOverflow(
      activeKey.value,
      isTabProtectedFromEviction,
      (evictedKey, entry) => {
        const tab = tabs.value.find((item) => item.key === evictedKey);
        if (!tab) {
          return;
        }
        if (entry.state) {
          tab.cursorPos = entry.state.selection.main.head;
        }
        tab.doc = null;
        tab.draftBody = '';
        tab.evicted = true;
      },
    );
  }

  function syncTabToPool(
    tab: EditorTab,
    options: { resetState?: boolean; touch?: boolean } = {},
  ): void {
    const existing = editorStatePool.has(tab.key) ? editorStatePool.get(tab.key) : undefined;
    const nextState = options.resetState ? null : (existing?.state ?? null);
    const nextEntry: EditorPoolEntry = {
      state: nextState,
      syncedSeq: options.resetState ? -1 : (existing?.syncedSeq ?? tab.syncSeq),
      doc: tab.doc,
      draftBody: tab.draftBody,
      baseBodyHash: tab.baseBodyHash,
    };
    editorStatePool.set(tab.key, nextEntry);
    tab.evicted = false;
    if (options.touch !== false) {
      tab.lastAccessedAt = Date.now();
      editorStatePool.touch(tab.key);
    }
  }

  function touchTabInPool(key: string): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (tab) {
      tab.lastAccessedAt = Date.now();
    }
    editorStatePool.touch(key);
    evictOverflowTabs();
  }

  function migratePoolKey(oldKey: string, newKey: string): void {
    if (oldKey === newKey || !editorStatePool.has(oldKey)) {
      return;
    }
    const existing = editorStatePool.get(oldKey);
    editorStatePool.delete(oldKey);
    if (existing) {
      const stateStale =
        existing.state !== null && existing.state.doc.toString() !== existing.draftBody;
      editorStatePool.set(newKey, {
        ...existing,
        state: stateStale ? null : existing.state,
        syncedSeq: stateStale ? -1 : existing.syncedSeq,
      });
    }
  }

  function getPooledEditorState(key: string): { state: EditorState; syncedSeq: number } | null {
    if (!editorStatePool.has(key)) {
      return null;
    }
    const entry = editorStatePool.get(key);
    if (!entry || !entry.state) {
      return null;
    }
    return {
      state: entry.state,
      syncedSeq: entry.syncedSeq,
    };
  }

  function setPooledEditorState(key: string, state: EditorState, syncedSeq: number): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || tab.evicted) {
      return;
    }
    editorStatePool.set(key, {
      state,
      syncedSeq,
      doc: tab.doc,
      draftBody: tab.draftBody,
      baseBodyHash: tab.baseBodyHash,
    });
    tab.lastAccessedAt = Date.now();
    editorStatePool.touch(key);
    evictOverflowTabs();
  }

  function setPoolCapacity(limit: number): number {
    const applied = editorStatePool.setCapacity(limit);
    evictOverflowTabs();
    return applied;
  }

  function getPoolCapacity(): number {
    return editorStatePool.getCapacity();
  }

  function getPoolSize(): number {
    return editorStatePool.size();
  }

  function isTabInPool(key: string): boolean {
    return editorStatePool.has(key);
  }

  watch(maxActiveEditorRenders, (nextLimit) => {
    editorStatePool.setCapacity(nextLimit);
    evictOverflowTabs();
  });

  const isInboxBloated = computed<boolean>(() => {
    const tab = activeTab.value;
    const doc = tab?.doc;
    if (!tab || !doc) {
      return false;
    }
    if (!doc.isInbox && doc.relPath !== 'inbox.md') {
      return false;
    }
    const lineCount = tab.draftBody.split('\n').length;
    return lineCount > inboxBloatThreshold.value;
  });

  function notifyNotesRefresh(): void {
    try {
      const notesStore = useNotesStore();
      void notesStore.refreshAll().catch(() => undefined);
    } catch {
      // 独立单元测试环境忽略
    }
  }

  function clearAutoSaveTimers(key: string): void {
    const debounce = debounceTimers.get(key);
    if (debounce !== undefined) {
      clearTimeout(debounce);
      debounceTimers.delete(key);
    }
    const maxWait = maxWaitTimers.get(key);
    if (maxWait !== undefined) {
      clearTimeout(maxWait);
      maxWaitTimers.delete(key);
    }
  }

  function migrateTimerKeys(oldKey: string, newKey: string): void {
    if (oldKey === newKey) {
      return;
    }
    const debounce = debounceTimers.get(oldKey);
    if (debounce !== undefined) {
      debounceTimers.delete(oldKey);
      debounceTimers.set(newKey, debounce);
    }
    const maxWait = maxWaitTimers.get(oldKey);
    if (maxWait !== undefined) {
      maxWaitTimers.delete(oldKey);
      maxWaitTimers.set(newKey, maxWait);
    }
  }

  function scheduleAutoSave(key: string): void {
    const existingDebounce = debounceTimers.get(key);
    if (existingDebounce !== undefined) {
      clearTimeout(existingDebounce);
    }
    const debounceTimer = setTimeout(() => {
      debounceTimers.delete(key);
      void saveTab(key);
    }, autoSaveDebounceMs.value);
    debounceTimers.set(key, debounceTimer);

    if (!maxWaitTimers.has(key)) {
      const maxWaitTimer = setTimeout(() => {
        maxWaitTimers.delete(key);
        void saveTab(key);
      }, autoSaveMaxWaitMs.value);
      maxWaitTimers.set(key, maxWaitTimer);
    }
  }

  function scheduleAutoSaveIfDirty(key: string): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (
      tab &&
      tab.isDirty &&
      tab.conflictState === null &&
      !tab.isMissing &&
      tab.saveStatus !== 'saving'
    ) {
      scheduleAutoSave(key);
    }
  }

  function pushClosedStack(absPath: string): void {
    if (!absPath) {
      return;
    }
    const filtered = closedStack.value.filter((item) => item !== absPath);
    filtered.push(absPath);
    if (filtered.length > MAX_CLOSED_STACK_SIZE) {
      filtered.splice(0, filtered.length - MAX_CLOSED_STACK_SIZE);
    }
    closedStack.value = filtered;
  }

  function buildTabsUiState(): TabsUiState {
    return {
      items: tabs.value.map(toTabItemState),
      activeKey: activeKey.value,
      closedStack: [...closedStack.value],
    };
  }

  function persistTabsDebounced(): void {
    if (persistTimer !== null) {
      clearTimeout(persistTimer);
    }
    persistTimer = setTimeout(() => {
      persistTimer = null;
      void patchUiState('tabs', buildTabsUiState()).catch(() => undefined);
    }, TABS_PERSIST_DEBOUNCE_MS);
  }

  function recordLastActiveDoc(doc: NoteDoc): void {
    if (!doc.isLibrary) {
      return;
    }
    void patchUiState('lastActiveDoc', {
      relPath: doc.relPath,
      touchedAtMs: Date.now(),
    }).catch(() => undefined);
  }

  function applyDocToTab(tab: EditorTab, doc: NoteDoc): void {
    tab.relPath = doc.relPath;
    tab.absPath = doc.absPath;
    tab.doc = doc;
    tab.isMissing = false;
    tab.evicted = false;
    tab.draftBody = doc.body;
    tab.baseBodyHash = doc.bodyHash;
    tab.isDirty = false;
    tab.conflictState = null;
    tab.saveStatus = 'saved';
    tab.syncSeq += 1;
    syncTabToPool(tab, { resetState: true, touch: true });
  }

  async function hydrateTabFromDisk(tab: EditorTab): Promise<boolean> {
    if (tab.isMissing) {
      return false;
    }
    const savedCursorOffset = tab.cursorPos;
    const savedScrollTop = tab.scrollTop;
    try {
      const doc = await readNote({ absPath: tab.absPath });
      applyDocToTab(tab, doc);
      tab.cursorPos = Math.max(0, Math.min(savedCursorOffset, doc.body.length));
      tab.scrollTop = savedScrollTop;
      evictOverflowTabs();
      return true;
    } catch (error) {
      if (isNoteNotFoundError(error)) {
        tab.isMissing = true;
      } else {
        errorMessage.value = extractEditorErrorMessage(error);
      }
      return false;
    }
  }

  function upsertOpenedDoc(doc: NoteDoc, preview: boolean): EditorTab {
    const existingByKey = tabs.value.find((tab) => tab.key === doc.key);
    if (existingByKey) {
      existingByKey.relPath = doc.relPath;
      existingByKey.absPath = doc.absPath;
      existingByKey.doc = doc;
      existingByKey.isMissing = false;
      existingByKey.evicted = false;
      const shouldResetState = !existingByKey.isDirty;
      if (shouldResetState) {
        existingByKey.draftBody = doc.body;
        existingByKey.baseBodyHash = doc.bodyHash;
        existingByKey.saveStatus = 'saved';
        existingByKey.syncSeq += 1;
      }
      if (!preview && existingByKey.isPreview) {
        existingByKey.isPreview = false;
        existingByKey.isPinned = true;
      }
      activeKey.value = existingByKey.key;
      syncTabToPool(existingByKey, { resetState: shouldResetState, touch: true });
      evictOverflowTabs();
      return existingByKey;
    }

    const nextTab = buildTabFromDoc(doc, preview);
    if (preview) {
      const previewIndex = tabs.value.findIndex((tab) => tab.isPreview);
      if (previewIndex >= 0) {
        const replaced = tabs.value[previewIndex];
        if (replaced) {
          clearAutoSaveTimers(replaced.key);
          editorStatePool.delete(replaced.key);
        }
        tabs.value.splice(previewIndex, 1, nextTab);
      } else {
        tabs.value.push(nextTab);
      }
    } else {
      tabs.value.push(nextTab);
    }

    activeKey.value = nextTab.key;
    syncTabToPool(nextTab, { resetState: true, touch: true });
    evictOverflowTabs();
    return nextTab;
  }

  async function resolveAbsPath(target: string, explicitAbsPath?: string): Promise<string> {
    if (explicitAbsPath) {
      return explicitAbsPath;
    }
    if (target.startsWith('/')) {
      return target;
    }
    try {
      const allNotes = await listAllNotes();
      const matched = allNotes.find((item) => item.relPath === target);
      if (matched) {
        return matched.absPath;
      }
    } catch {
      // 容错：回退使用传入路径
    }
    return target;
  }

  async function openNote(
    relPathOrAbs: string,
    options: OpenNoteOptions = {},
  ): Promise<EditorTab | null> {
    const preview = options.preview ?? true;
    errorMessage.value = null;

    if (activeKey.value) {
      await flushSave(activeKey.value);
    }

    const existing = tabs.value.find(
      (tab) =>
        tab.relPath === relPathOrAbs ||
        tab.absPath === relPathOrAbs ||
        (options.absPath !== undefined && tab.absPath === options.absPath),
    );
    if (existing) {
      activeKey.value = existing.key;
      if (!preview && existing.isPreview) {
        existing.isPreview = false;
        existing.isPinned = true;
      }
      if ((existing.evicted || !existing.doc) && !existing.isMissing) {
        await hydrateTabFromDisk(existing);
      } else {
        touchTabInPool(existing.key);
      }
      if (existing.doc) {
        recordLastActiveDoc(existing.doc);
      }
      persistTabsDebounced();
      return existing;
    }

    loading.value = true;
    try {
      const absPath = await resolveAbsPath(relPathOrAbs, options.absPath);
      const doc = await readNote({ absPath });
      const opened = upsertOpenedDoc(doc, preview);
      recordLastActiveDoc(doc);
      persistTabsDebounced();
      return opened;
    } catch (error) {
      errorMessage.value = extractEditorErrorMessage(error);
      return null;
    } finally {
      loading.value = false;
    }
  }

  async function openNoteByAbsPath(
    absPath: string,
    options: OpenNoteOptions & { line?: number | null } = {},
  ): Promise<EditorTab | null> {
    const opened = await openNote(absPath, {
      ...options,
      absPath,
    });
    if (opened && options.line != null && options.line > 0) {
      requestScrollToLine(options.line, 0);
    }
    return opened;
  }

  async function activateTab(key: string): Promise<void> {
    const target = tabs.value.find((tab) => tab.key === key);
    if (!target) {
      return;
    }

    if (activeKey.value && activeKey.value !== key) {
      await flushSave(activeKey.value);
    }

    activeKey.value = target.key;

    if ((target.evicted || !target.doc) && !target.isMissing) {
      await hydrateTabFromDisk(target);
    } else {
      touchTabInPool(target.key);
    }

    if (target.doc) {
      recordLastActiveDoc(target.doc);
    }
    persistTabsDebounced();
  }

  async function activateTabByIndex(index: number): Promise<void> {
    const target = tabs.value[index];
    if (!target) {
      return;
    }
    await activateTab(target.key);
  }

  function promoteTabToPinned(key: string): void {
    const target = tabs.value.find((tab) => tab.key === key);
    if (!target || !target.isPreview) {
      return;
    }
    target.isPreview = false;
    target.isPinned = true;
    persistTabsDebounced();
  }

  function updateTabBody(
    key: string,
    newBody: string,
    cursorOffset?: number,
    line?: number,
    col?: number,
    composing = false,
  ): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab) {
      return;
    }

    tab.draftBody = newBody;
    if (cursorOffset !== undefined) {
      tab.cursorPos = cursorOffset;
    }
    if (line !== undefined) {
      tab.cursorLine = line;
    }
    if (col !== undefined) {
      tab.cursorCol = col;
    }

    let promoted = false;
    if (tab.isPreview) {
      tab.isPreview = false;
      tab.isPinned = true;
      promoted = true;
    }

    tab.isDirty = true;
    syncTabToPool(tab, { touch: true });

    if (tab.conflictState === null && !tab.isMissing) {
      tab.saveStatus = 'dirty';
      if (!composing) {
        scheduleAutoSave(tab.key);
      }
    }

    if (promoted) {
      persistTabsDebounced();
    }
  }

  function updateTabCursorAndScroll(
    key: string,
    payload: {
      scrollTop?: number;
      cursorPos?: number;
      cursorLine?: number;
      cursorCol?: number;
      persist?: boolean;
    },
  ): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab) {
      return;
    }
    if (payload.scrollTop !== undefined) {
      tab.scrollTop = payload.scrollTop;
    }
    if (payload.cursorPos !== undefined) {
      tab.cursorPos = payload.cursorPos;
    }
    if (payload.cursorLine !== undefined) {
      tab.cursorLine = payload.cursorLine;
    }
    if (payload.cursorCol !== undefined) {
      tab.cursorCol = payload.cursorCol;
    }
    if (payload.persist) {
      persistTabsDebounced();
    }
  }

  function setActiveTopLine(line: number): void {
    activeTopLine.value = line;
  }

  function requestScrollToLine(line: number, from: number): void {
    if (!activeKey.value) {
      return;
    }
    scrollSeq += 1;
    scrollTarget.value = {
      key: activeKey.value,
      line,
      from,
      seq: scrollSeq,
    };
  }

  async function saveTab(key: string): Promise<boolean> {
    const existingInFlight = inFlightSaveMap.get(key);
    if (existingInFlight) {
      return existingInFlight;
    }

    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || !tab.doc || tab.isMissing || tab.conflictState !== null) {
      return false;
    }
    if (tab.saveStatus === 'saving') {
      return false;
    }

    clearAutoSaveTimers(key);

    if (!tab.isDirty && tab.draftBody === tab.doc.body) {
      tab.saveStatus = 'saved';
      return true;
    }

    const savingBody = tab.draftBody;
    tab.saveStatus = 'saving';

    const savePromise = (async (): Promise<boolean> => {
      try {
        const res = await writeNote({
          absPath: tab.absPath,
          body: savingBody,
          baseBodyHash: tab.baseBodyHash,
        });

        tab.baseBodyHash = res.bodyHash;
        tab.doc!.body = savingBody;
        tab.doc!.bodyHash = res.bodyHash;
        tab.doc!.mtimeMs = res.mtimeMs;

        if (tab.draftBody !== savingBody) {
          tab.isDirty = true;
          tab.saveStatus = 'dirty';
          scheduleAutoSave(tab.key);
        } else {
          tab.isDirty = false;
          tab.saveStatus = 'saved';
        }

        if (res.updatedMeta !== null) {
          tab.doc!.meta = res.updatedMeta;
          if (tab.key.startsWith('path:') && res.updatedMeta.id) {
            const oldKey = tab.key;
            const newKey = res.updatedMeta.id;
            tab.key = newKey;
            tab.doc!.key = newKey;
            if (activeKey.value === oldKey) {
              activeKey.value = newKey;
            }
            migrateTimerKeys(oldKey, newKey);
            migratePoolKey(oldKey, newKey);
            persistTabsDebounced();
          }
          notifyNotesRefresh();
        }

        syncTabToPool(tab, { touch: false });
        evictOverflowTabs();

        return true;
      } catch (error) {
        if (isConflictError(error)) {
          clearAutoSaveTimers(tab.key);
          const info = parseConflictInfo(error);
          tab.conflictState = {
            kind: 'modified',
            diskBody: info?.currentBody ?? '',
            diskBodyHash: info?.currentBodyHash ?? '',
            diskMeta: null,
          };
          tab.saveStatus = 'conflict';
          return false;
        }
        tab.saveStatus = 'error';
        errorMessage.value = extractEditorErrorMessage(error);
        return false;
      } finally {
        inFlightSaveMap.delete(key);
        inFlightSaveMap.delete(tab.key);
      }
    })();

    inFlightSaveMap.set(key, savePromise);
    return savePromise;
  }

  async function flushSave(key?: string, options: { manual?: boolean } = {}): Promise<void> {
    const targetKey = key ?? activeKey.value;
    if (!targetKey) {
      return;
    }
    const tab = tabs.value.find((item) => item.key === targetKey);
    if (!tab) {
      return;
    }
    if (
      options.manual &&
      tab.conflictState?.kind === 'modified' &&
      tab.conflictState.bannerDismissed
    ) {
      tab.conflictState = {
        ...tab.conflictState,
        bannerDismissed: false,
      };
      return;
    }
    const inFlight = inFlightSaveMap.get(tab.key) ?? inFlightSaveMap.get(targetKey);
    if (inFlight) {
      await inFlight;
    }
    if (
      tab.saveStatus === 'saving' ||
      !tab.isDirty ||
      tab.conflictState !== null ||
      tab.isMissing
    ) {
      return;
    }
    await saveTab(tab.key);
  }

  async function flushAllDirtyTabs(): Promise<void> {
    const dirtyKeys = tabs.value
      .filter((tab) => tab.isDirty && tab.conflictState === null && !tab.isMissing)
      .map((tab) => tab.key);
    await Promise.all(dirtyKeys.map((key) => flushSave(key)));
  }

  function handleExternalChange(event: ExternalChangeEvent): void {
    for (const tab of tabs.value) {
      if (tab.absPath !== event.absPath) {
        continue;
      }

      if (event.kind === 'deleted') {
        clearAutoSaveTimers(tab.key);
        tab.conflictState = { kind: 'deleted' };
        tab.saveStatus = 'deleted';
        continue;
      }

      if (event.kind === 'modified') {
        if (tab.evicted && !tab.isDirty) {
          notifyNotesRefresh();
          continue;
        }
        const isEcho =
          Boolean(event.newBodyHash) &&
          event.newBodyHash === tab.baseBodyHash &&
          (!event.newMeta || isSameNoteMeta(event.newMeta, tab.doc?.meta));
        if (isEcho) {
          continue;
        }
        if (!tab.isDirty) {
          const nextBody = event.newBody ?? '';
          const nextHash = event.newBodyHash ?? tab.baseBodyHash;
          tab.draftBody = nextBody;
          tab.baseBodyHash = nextHash;
          if (tab.doc) {
            tab.doc.body = nextBody;
            tab.doc.bodyHash = nextHash;
            if (event.newMeta) {
              tab.doc.meta = event.newMeta;
            }
          }
          tab.conflictState = null;
          tab.saveStatus = 'external-synced';
          tab.cursorPos = Math.min(tab.cursorPos, nextBody.length);
          tab.syncSeq += 1;
          syncTabToPool(tab, { resetState: true, touch: false });
          notifyNotesRefresh();
        } else {
          clearAutoSaveTimers(tab.key);
          const inMergeMode =
            tab.conflictState?.kind === 'modified' && Boolean(tab.conflictState.mergeMode);
          tab.conflictState = {
            kind: 'modified',
            diskBody: event.newBody ?? '',
            diskBodyHash: event.newBodyHash ?? '',
            diskMeta: event.newMeta,
            ...(inMergeMode
              ? {
                  bannerDismissed: false,
                  mergeMode: true,
                  mergeSaving:
                    tab.conflictState?.kind === 'modified' ? tab.conflictState.mergeSaving : false,
                  mergeError:
                    tab.conflictState?.kind === 'modified' ? tab.conflictState.mergeError : null,
                }
              : {}),
          };
          tab.saveStatus = 'conflict';
        }
      }
    }
  }

  function resolveConflictReload(key: string): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || tab.conflictState?.kind !== 'modified') {
      return;
    }
    const { diskBody, diskBodyHash, diskMeta } = tab.conflictState;
    clearAutoSaveTimers(tab.key);
    tab.draftBody = diskBody;
    tab.baseBodyHash = diskBodyHash;
    if (tab.doc) {
      tab.doc.body = diskBody;
      tab.doc.bodyHash = diskBodyHash;
      if (diskMeta) {
        tab.doc.meta = diskMeta;
      }
    }
    tab.isDirty = false;
    tab.conflictState = null;
    tab.saveStatus = 'saved';
    tab.cursorPos = Math.min(tab.cursorPos, diskBody.length);
    tab.syncSeq += 1;
    syncTabToPool(tab, { resetState: true, touch: false });
    evictOverflowTabs();
  }

  async function resolveConflictOverwrite(key: string): Promise<boolean> {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || tab.conflictState?.kind !== 'modified') {
      return false;
    }
    const diskHash = tab.conflictState.diskBodyHash;
    tab.baseBodyHash = diskHash;
    tab.conflictState = null;
    tab.isDirty = true;
    return await saveTab(tab.key);
  }

  function dismissConflictBanner(key: string): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || tab.conflictState?.kind !== 'modified') {
      return;
    }
    tab.conflictState = {
      ...tab.conflictState,
      bannerDismissed: true,
    };
  }

  function reopenConflictBanner(key: string): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || tab.conflictState?.kind !== 'modified') {
      return;
    }
    tab.conflictState = {
      ...tab.conflictState,
      bannerDismissed: false,
    };
  }

  function enterConflictMerge(key: string): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || tab.conflictState?.kind !== 'modified') {
      return;
    }
    clearAutoSaveTimers(tab.key);
    tab.conflictState = {
      ...tab.conflictState,
      bannerDismissed: false,
      mergeMode: true,
      mergeSaving: false,
      mergeError: null,
    };
  }

  function cancelConflictMerge(key: string): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || tab.conflictState?.kind !== 'modified') {
      return;
    }
    tab.conflictState = {
      ...tab.conflictState,
      bannerDismissed: false,
      mergeMode: false,
      mergeSaving: false,
      mergeError: null,
    };
    tab.syncSeq += 1;
  }

  function updateMergeDraftBody(key: string, nextBody: string): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab) {
      return;
    }
    tab.draftBody = nextBody;
    tab.isDirty = true;
  }

  async function commitConflictMerge(key: string, mergedBody: string): Promise<boolean> {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || !tab.doc || tab.conflictState?.kind !== 'modified') {
      return false;
    }
    if (tab.conflictState.mergeSaving) {
      return false;
    }

    const diskHash = tab.conflictState.diskBodyHash;
    const diskMeta = tab.conflictState.diskMeta;
    tab.draftBody = mergedBody;
    tab.conflictState = {
      ...tab.conflictState,
      mergeSaving: true,
      mergeError: null,
    };

    try {
      const res = await writeNote({
        absPath: tab.absPath,
        body: mergedBody,
        baseBodyHash: diskHash,
      });

      tab.doc.body = mergedBody;
      tab.draftBody = mergedBody;
      tab.baseBodyHash = res.bodyHash;
      tab.doc.bodyHash = res.bodyHash;
      tab.doc.mtimeMs = res.mtimeMs;

      if (res.updatedMeta !== null) {
        tab.doc.meta = res.updatedMeta;
        if (tab.key.startsWith('path:') && res.updatedMeta.id) {
          const oldKey = tab.key;
          const newKey = res.updatedMeta.id;
          tab.key = newKey;
          tab.doc.key = newKey;
          if (activeKey.value === oldKey) {
            activeKey.value = newKey;
          }
          migrateTimerKeys(oldKey, newKey);
          migratePoolKey(oldKey, newKey);
          persistTabsDebounced();
        }
        notifyNotesRefresh();
      } else if (diskMeta !== null) {
        tab.doc.meta = diskMeta;
        notifyNotesRefresh();
      }

      tab.conflictState = null;
      tab.isDirty = false;
      tab.saveStatus = 'saved';
      tab.syncSeq += 1;
      syncTabToPool(tab, { resetState: true, touch: true });
      evictOverflowTabs();
      return true;
    } catch (error) {
      if (isConflictError(error)) {
        const info = parseConflictInfo(error);
        const currentConflict = tab.conflictState;
        if (currentConflict?.kind === 'modified') {
          tab.conflictState = {
            ...currentConflict,
            diskBody: info?.currentBody ?? currentConflict.diskBody,
            diskBodyHash: info?.currentBodyHash ?? currentConflict.diskBodyHash,
            mergeSaving: false,
            mergeError: '合并保存期间磁盘又有新修改，已刷新左栏基线，请复核后再次完成合并',
          };
        }
        return false;
      }
      const msg = extractEditorErrorMessage(error);
      if (tab.conflictState?.kind === 'modified') {
        tab.conflictState = {
          ...tab.conflictState,
          mergeSaving: false,
          mergeError: msg,
        };
      }
      return false;
    }
  }

  async function restoreDeletedTabToDisk(key: string): Promise<boolean> {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || tab.conflictState?.kind !== 'deleted') {
      return false;
    }
    tab.conflictState = null;
    tab.isDirty = true;
    return await saveTab(tab.key);
  }

  async function ensureExternalChangeListener(): Promise<void> {
    if (unlistenExternalChange) {
      return;
    }
    try {
      unlistenExternalChange = await subscribeFsExternalChange((event) => {
        handleExternalChange(event);
      });
    } catch {
      // 非 Tauri 测试环境忽略
    }
  }

  function closeTab(key: string): void {
    const index = tabs.value.findIndex((tab) => tab.key === key);
    if (index < 0) {
      return;
    }
    const target = tabs.value[index];
    if (!target) {
      return;
    }
    if (target.isDirty && target.conflictState === null && !target.isMissing && target.doc) {
      void writeNote({
        absPath: target.absPath,
        body: target.draftBody,
        baseBodyHash: target.baseBodyHash,
      }).catch(() => undefined);
    }
    clearAutoSaveTimers(target.key);
    editorStatePool.delete(target.key);
    pushClosedStack(target.absPath);
    tabs.value.splice(index, 1);

    if (activeKey.value === key) {
      const next = tabs.value[index] ?? tabs.value[index - 1] ?? null;
      activeKey.value = next?.key ?? null;
      if (next) {
        if ((next.evicted || !next.doc) && !next.isMissing) {
          void hydrateTabFromDisk(next).then(() => {
            if (activeKey.value === next.key && next.doc) {
              recordLastActiveDoc(next.doc);
            }
          });
        } else {
          touchTabInPool(next.key);
          if (next.doc) {
            recordLastActiveDoc(next.doc);
          }
        }
      }
    }
    persistTabsDebounced();
  }

  function closeOtherTabs(key: string): void {
    const keep = tabs.value.find((tab) => tab.key === key);
    if (!keep) {
      return;
    }
    for (const tab of tabs.value) {
      if (tab.key !== key) {
        if (tab.isDirty && tab.conflictState === null && !tab.isMissing && tab.doc) {
          void writeNote({
            absPath: tab.absPath,
            body: tab.draftBody,
            baseBodyHash: tab.baseBodyHash,
          }).catch(() => undefined);
        }
        clearAutoSaveTimers(tab.key);
        editorStatePool.delete(tab.key);
        pushClosedStack(tab.absPath);
      }
    }
    tabs.value = [keep];
    activeKey.value = keep.key;
    if ((keep.evicted || !keep.doc) && !keep.isMissing) {
      void hydrateTabFromDisk(keep).then(() => {
        if (activeKey.value === keep.key && keep.doc) {
          recordLastActiveDoc(keep.doc);
        }
      });
    } else {
      touchTabInPool(keep.key);
      if (keep.doc) {
        recordLastActiveDoc(keep.doc);
      }
    }
    persistTabsDebounced();
  }

  function reorderTab(fromKey: string, toKey: string, position: 'before' | 'after'): void {
    const fromIndex = tabs.value.findIndex((tab) => tab.key === fromKey);
    const toIndex = tabs.value.findIndex((tab) => tab.key === toKey);
    if (fromIndex < 0 || toIndex < 0 || fromIndex === toIndex) {
      return;
    }
    const [moved] = tabs.value.splice(fromIndex, 1);
    if (!moved) {
      return;
    }
    const insertAt =
      tabs.value.findIndex((tab) => tab.key === toKey) + (position === 'after' ? 1 : 0);
    tabs.value.splice(insertAt, 0, moved);
    persistTabsDebounced();
  }

  async function reopenLastClosedTab(): Promise<EditorTab | null> {
    const absPath = closedStack.value.pop();
    if (!absPath) {
      return null;
    }
    loading.value = true;
    errorMessage.value = null;
    try {
      const doc = await readNote({ absPath });
      const reopened = upsertOpenedDoc(doc, false);
      recordLastActiveDoc(doc);
      persistTabsDebounced();
      return reopened;
    } catch (error) {
      errorMessage.value = extractEditorErrorMessage(error);
      persistTabsDebounced();
      return null;
    } finally {
      loading.value = false;
    }
  }

  function handleNoteRenamed(oldRelPath: string, updatedDoc: NoteDoc): void {
    const target = tabs.value.find(
      (tab) => tab.relPath === oldRelPath || tab.key === updatedDoc.key,
    );
    if (!target) {
      return;
    }
    target.relPath = updatedDoc.relPath;
    target.absPath = updatedDoc.absPath;
    target.isMissing = false;
    if (!target.evicted) {
      target.doc = updatedDoc;
      if (!target.isDirty) {
        target.draftBody = updatedDoc.body;
        target.baseBodyHash = updatedDoc.bodyHash;
        target.syncSeq += 1;
        syncTabToPool(target, { resetState: true, touch: false });
      } else {
        syncTabToPool(target, { touch: false });
      }
    }
    if (activeKey.value === target.key) {
      recordLastActiveDoc(updatedDoc);
    }
    persistTabsDebounced();
  }

  function handleNoteDeleted(relPath: string): void {
    const index = tabs.value.findIndex((tab) => tab.relPath === relPath);
    if (index < 0) {
      return;
    }
    const target = tabs.value[index];
    if (target) {
      clearAutoSaveTimers(target.key);
      editorStatePool.delete(target.key);
    }
    tabs.value.splice(index, 1);
    if (target && activeKey.value === target.key) {
      const next = tabs.value[index] ?? tabs.value[index - 1] ?? null;
      activeKey.value = next?.key ?? null;
      if (next) {
        if ((next.evicted || !next.doc) && !next.isMissing) {
          void hydrateTabFromDisk(next).then(() => {
            if (activeKey.value === next.key && next.doc) {
              recordLastActiveDoc(next.doc);
            }
          });
        } else {
          touchTabInPool(next.key);
          if (next.doc) {
            recordLastActiveDoc(next.doc);
          }
        }
      }
    }
    persistTabsDebounced();
  }

  async function restoreTabsFromUiState(): Promise<void> {
    loading.value = true;
    errorMessage.value = null;
    try {
      await ensureExternalChangeListener();
      const [uiState, allNotes] = await Promise.all([fetchUiState(), listAllNotes()]);
      const byRelPath = new Map<string, NoteSummary>();
      for (const summary of allNotes) {
        if (summary.relPath) {
          byRelPath.set(summary.relPath, summary);
        }
      }
      const byKey = new Map<string, NoteSummary>(allNotes.map((summary) => [summary.key, summary]));

      closedStack.value = [...(uiState.tabs.closedStack ?? [])].slice(-MAX_CLOSED_STACK_SIZE);
      editorStatePool.clear();

      const restored: EditorTab[] = [];
      let seenPreview = false;

      for (const item of uiState.tabs.items) {
        const matched =
          (item.relPath ? byRelPath.get(item.relPath) : undefined) ?? byKey.get(item.key) ?? null;
        const resolvedRelPath = matched?.relPath ?? item.relPath;
        const resolvedAbsPath = matched?.absPath ?? item.absPath;
        const resolvedKey = matched?.key ?? item.key;

        const isPreview = item.isPreview && !seenPreview;
        if (isPreview) {
          seenPreview = true;
        }

        const isMissing = Boolean(item.relPath !== null && matched === null);

        restored.push({
          key: resolvedKey,
          relPath: resolvedRelPath,
          absPath: resolvedAbsPath,
          isPinned: isPreview ? false : true,
          isPreview,
          scrollTop: item.scrollTop,
          cursorPos: item.cursorPos,
          doc: null,
          isDirty: false,
          isMissing,
          evicted: true,
          lastAccessedAt: Date.now(),
          draftBody: '',
          baseBodyHash: '',
          conflictState: null,
          saveStatus: 'saved',
          cursorLine: 1,
          cursorCol: 1,
          selectionFromLine: 1,
          selectionToLine: 1,
          hasSelection: false,
          syncSeq: 0,
        });
      }

      tabs.value = restored;

      if (restored.length === 0) {
        initialized.value = true;
        const inboxSummary = allNotes.find((item) => item.isInbox || item.relPath === 'inbox.md');
        if (inboxSummary) {
          await openNote(inboxSummary.relPath ?? 'inbox.md', {
            preview: false,
            absPath: inboxSummary.absPath,
          });
        } else {
          await openNote('inbox.md', { preview: false });
        }
        return;
      }

      const preferredKey = uiState.tabs.activeKey;
      const activeTarget =
        (preferredKey ? tabs.value.find((tab) => tab.key === preferredKey) : undefined) ??
        tabs.value[0] ??
        null;
      activeKey.value = activeTarget?.key ?? null;
      if (activeTarget) {
        await hydrateTabFromDisk(activeTarget);
        if (activeTarget.doc) {
          recordLastActiveDoc(activeTarget.doc);
        }
      }
      initialized.value = true;
    } catch (error) {
      errorMessage.value = extractEditorErrorMessage(error);
    } finally {
      loading.value = false;
    }
  }

  async function syncAppendedNote(
    relPath: string,
    options: { openIfMissing?: boolean; jumpToEnd?: boolean } = {},
  ): Promise<void> {
    const openIfMissing = options.openIfMissing ?? false;
    const jumpToEnd = options.jumpToEnd ?? true;
    const existing = tabs.value.find((tab) => tab.relPath === relPath);

    if (existing) {
      if (!existing.isDirty && !existing.isMissing) {
        try {
          const doc = await readNote({ absPath: existing.absPath });
          applyDocToTab(existing, doc);
          if (jumpToEnd) {
            existing.cursorPos = doc.body.length;
          }
          existing.syncSeq += 1;
          if (openIfMissing) {
            activeKey.value = existing.key;
          }
          if (jumpToEnd && activeKey.value === existing.key) {
            const totalLines = Math.max(1, doc.body.split('\n').length);
            requestScrollToLine(totalLines, doc.body.length);
          }
          recordLastActiveDoc(doc);
        } catch (error) {
          errorMessage.value = extractEditorErrorMessage(error);
        }
      } else if (openIfMissing) {
        activeKey.value = existing.key;
      }
      return;
    }

    if (!openIfMissing) {
      return;
    }

    const opened = await openNote(relPath, { preview: false });
    if (opened?.doc && jumpToEnd) {
      opened.cursorPos = opened.doc.body.length;
      opened.syncSeq += 1;
      const totalLines = Math.max(1, opened.doc.body.split('\n').length);
      requestScrollToLine(totalLines, opened.doc.body.length);
    }
  }

  async function handleQuickBarAppendEvent(appendResult: AppendResult): Promise<void> {
    lastQuickBarAppendedRelPath = appendResult.relPath;
    notifyNotesRefresh();
    await syncAppendedNote(appendResult.relPath, {
      openIfMissing: false,
      jumpToEnd: true,
    });
  }

  async function handleMainWindowShown(): Promise<void> {
    try {
      const hasQuickBarAppend = lastQuickBarAppendedRelPath !== null;
      if (!hasQuickBarAppend && tabs.value.length > 0) {
        return;
      }
      const uiState = await fetchUiState();
      const targetRelPath = lastQuickBarAppendedRelPath ?? uiState.lastActiveDoc?.relPath ?? null;
      if (!targetRelPath) {
        return;
      }
      const shouldSync = hasQuickBarAppend || activeTab.value?.relPath !== targetRelPath;
      if (shouldSync) {
        await syncAppendedNote(targetRelPath, {
          openIfMissing: true,
          jumpToEnd: true,
        });
      }
      lastQuickBarAppendedRelPath = null;
    } catch {
      // 非 Tauri 测试环境或无 uiState 时容错
    }
  }

  function updateTabSelection(
    key: string,
    fromLine: number,
    toLine: number,
    hasSelection: boolean,
  ): void {
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab) {
      return;
    }
    tab.selectionFromLine = Math.max(1, Math.min(fromLine, toLine));
    tab.selectionToLine = Math.max(tab.selectionFromLine, Math.max(fromLine, toLine));
    tab.hasSelection = hasSelection;
  }

  function hasConfirmedExternalMeta(absPath: string): boolean {
    return confirmedExternalMetaPaths.value.has(absPath);
  }

  function markExternalMetaConfirmed(absPath: string): void {
    confirmedExternalMetaPaths.value.add(absPath);
  }

  function openInboxExtractModal(): void {
    inboxExtractModalOpen.value = true;
  }

  function closeInboxExtractModal(): void {
    inboxExtractModalOpen.value = false;
  }

  async function updateActiveMeta(patch: NoteMetaPatch): Promise<NoteDoc | null> {
    const tab = activeTab.value;
    if (!tab || !tab.doc) {
      return null;
    }
    errorMessage.value = null;
    try {
      const updatedDoc = await updateNoteMeta({
        absPath: tab.absPath,
        patch,
      });
      // 严格遵守 D-0006（meta + body 分离）：只更新 meta 与 mtimeMs，绝不覆写未保存的 draftBody
      tab.doc = {
        ...tab.doc,
        meta: updatedDoc.meta,
        mtimeMs: updatedDoc.mtimeMs,
      };
      confirmedExternalMetaPaths.value.add(tab.absPath);
      notifyNotesRefresh();
      return tab.doc;
    } catch (error) {
      errorMessage.value = extractEditorErrorMessage(error);
      return null;
    }
  }

  async function extractInboxSelectionToNote(args: {
    newTitle: string;
    targetDirRelPath: string;
    templateName: string;
    fromLine?: number;
    toLine?: number;
  }): Promise<NoteDoc | null> {
    const inboxTab = activeTab.value;
    const doc = inboxTab?.doc;
    if (!inboxTab || !doc || (!doc.isInbox && doc.relPath !== 'inbox.md')) {
      return null;
    }

    errorMessage.value = null;
    try {
      await flushSave(inboxTab.key);
      const fromLine = args.fromLine ?? inboxTab.selectionFromLine;
      const toLine = args.toLine ?? inboxTab.selectionToLine;
      const newDoc = await extractInboxToNote({
        fromLine,
        toLine,
        newTitle: args.newTitle.trim() || '未命名笔记',
        targetDirRelPath: args.targetDirRelPath.trim(),
        templateName: args.templateName || 'note',
      });

      const refreshedInbox = await readNote({ absPath: inboxTab.absPath });
      applyDocToTab(inboxTab, refreshedInbox);
      inboxTab.selectionFromLine = 1;
      inboxTab.selectionToLine = 1;
      inboxTab.hasSelection = false;

      notifyNotesRefresh();
      await openNote(newDoc.relPath ?? newDoc.absPath, {
        preview: false,
        absPath: newDoc.absPath,
      });
      return newDoc;
    } catch (error) {
      errorMessage.value = extractEditorErrorMessage(error);
      return null;
    }
  }

  async function refreshOpenTabsMetaFromDisk(): Promise<void> {
    for (const tab of tabs.value) {
      if (tab.evicted) {
        continue;
      }
      try {
        const latest = await readNote({ absPath: tab.absPath });
        tab.doc = latest;
        if (!tab.isDirty) {
          tab.draftBody = latest.body;
          tab.baseBodyHash = latest.bodyHash;
          tab.saveStatus = 'saved';
          tab.syncSeq += 1;
          syncTabToPool(tab, { resetState: true, touch: false });
        } else {
          syncTabToPool(tab, { touch: false });
        }
      } catch {
        // 忽略单文件刷新失败
      }
    }
  }

  function dispose(): void {
    if (persistTimer !== null) {
      clearTimeout(persistTimer);
      persistTimer = null;
    }
    for (const timer of debounceTimers.values()) {
      clearTimeout(timer);
    }
    debounceTimers.clear();
    for (const timer of maxWaitTimers.values()) {
      clearTimeout(timer);
    }
    maxWaitTimers.clear();
    editorStatePool.clear();
    if (unlistenExternalChange) {
      unlistenExternalChange();
      unlistenExternalChange = null;
    }
  }

  return {
    tabs,
    tabStubs,
    activeKey,
    closedStack,
    loading,
    errorMessage,
    initialized,
    activeTab,
    activeDoc,
    activeTopLine,
    scrollTarget,
    editorStatus,
    setEditorStatus,
    confirmedExternalMetaPaths,
    inboxExtractModalOpen,
    inboxBloatThreshold,
    autoSaveDebounceMs,
    autoSaveMaxWaitMs,
    maxActiveEditorRenders,
    isInboxBloated,
    getPooledEditorState,
    setPooledEditorState,
    setPoolCapacity,
    getPoolCapacity,
    getPoolSize,
    isTabInPool,
    openNote,
    openNoteByAbsPath,
    activateTab,
    activateTabByIndex,
    promoteTabToPinned,
    updateTabBody,
    scheduleAutoSaveIfDirty,
    updateTabCursorAndScroll,
    updateTabSelection,
    hasConfirmedExternalMeta,
    markExternalMetaConfirmed,
    openInboxExtractModal,
    closeInboxExtractModal,
    updateActiveMeta,
    extractInboxSelectionToNote,
    setActiveTopLine,
    requestScrollToLine,
    saveTab,
    flushSave,
    flushAllDirtyTabs,
    handleExternalChange,
    resolveConflictReload,
    resolveConflictOverwrite,
    dismissConflictBanner,
    reopenConflictBanner,
    enterConflictMerge,
    cancelConflictMerge,
    updateMergeDraftBody,
    commitConflictMerge,
    restoreDeletedTabToDisk,
    ensureExternalChangeListener,
    closeTab,
    closeOtherTabs,
    reorderTab,
    reopenLastClosedTab,
    handleNoteRenamed,
    handleNoteDeleted,
    syncAppendedNote,
    handleQuickBarAppendEvent,
    handleMainWindowShown,
    restoreTabsFromUiState,
    refreshOpenTabsMetaFromDisk,
    dispose,
  };
});
