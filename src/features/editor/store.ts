import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type {
  ExternalChangeEvent,
  NoteDoc,
  NoteMeta,
  NoteSummary,
  TabItemState,
  TabsUiState,
} from '@/bindings/types';
import { useNotesStore } from '@/features/notes';
import { useSettingsStore } from '@/features/settings';
import type { UnlistenFn } from '@/lib/ipc/events';
import {
  extractEditorErrorMessage,
  fetchUiState,
  isConflictError,
  isNoteNotFoundError,
  listAllNotes,
  parseConflictInfo,
  patchUiState,
  readNote,
  subscribeFsExternalChange,
  writeNote,
} from './api';

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
    }
  | {
      kind: 'deleted';
    };

export interface EditorTab extends TabItemState {
  doc: NoteDoc | null;
  isDirty: boolean;
  isMissing: boolean;
  draftBody: string;
  baseBodyHash: string;
  conflictState: EditorConflictState | null;
  saveStatus: EditorSaveStatus;
  cursorLine: number;
  cursorCol: number;
  syncSeq: number;
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
    draftBody: doc.body,
    baseBodyHash: doc.bodyHash,
    conflictState: null,
    saveStatus: 'saved',
    cursorLine: 1,
    cursorCol: 1,
    syncSeq: 0,
  };
}

export const useEditorTabsStore = defineStore('editorTabs', () => {
  const settingsStore = useSettingsStore();

  const tabs = ref<EditorTab[]>([]);
  const activeKey = ref<string | null>(null);
  const closedStack = ref<string[]>([]);
  const loading = ref<boolean>(false);
  const errorMessage = ref<string | null>(null);
  const initialized = ref<boolean>(false);
  const activeTopLine = ref<number>(1);
  const scrollTarget = ref<EditorScrollTarget | null>(null);

  let persistTimer: ReturnType<typeof setTimeout> | null = null;
  let scrollSeq = 0;
  let unlistenExternalChange: UnlistenFn | null = null;
  const debounceTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const maxWaitTimers = new Map<string, ReturnType<typeof setTimeout>>();

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
      workspaceKey: doc.meta.project ?? null,
      touchedAtMs: Date.now(),
    }).catch(() => undefined);
  }

  function applyDocToTab(tab: EditorTab, doc: NoteDoc): void {
    tab.relPath = doc.relPath;
    tab.absPath = doc.absPath;
    tab.doc = doc;
    tab.isMissing = false;
    tab.draftBody = doc.body;
    tab.baseBodyHash = doc.bodyHash;
    tab.isDirty = false;
    tab.conflictState = null;
    tab.saveStatus = 'saved';
    tab.syncSeq += 1;
  }

  function upsertOpenedDoc(doc: NoteDoc, preview: boolean): EditorTab {
    const existingByKey = tabs.value.find((tab) => tab.key === doc.key);
    if (existingByKey) {
      existingByKey.relPath = doc.relPath;
      existingByKey.absPath = doc.absPath;
      existingByKey.doc = doc;
      existingByKey.isMissing = false;
      if (!existingByKey.isDirty) {
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
      return existingByKey;
    }

    const nextTab = buildTabFromDoc(doc, preview);
    if (preview) {
      const previewIndex = tabs.value.findIndex((tab) => tab.isPreview);
      if (previewIndex >= 0) {
        const replaced = tabs.value[previewIndex];
        if (replaced) {
          clearAutoSaveTimers(replaced.key);
        }
        tabs.value.splice(previewIndex, 1, nextTab);
      } else {
        tabs.value.push(nextTab);
      }
    } else {
      tabs.value.push(nextTab);
    }

    activeKey.value = nextTab.key;
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
      if (!existing.doc && !existing.isMissing) {
        try {
          const doc = await readNote({ absPath: existing.absPath });
          applyDocToTab(existing, doc);
        } catch (error) {
          if (isNoteNotFoundError(error)) {
            existing.isMissing = true;
          } else {
            errorMessage.value = extractEditorErrorMessage(error);
          }
        }
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

  async function activateTab(key: string): Promise<void> {
    const target = tabs.value.find((tab) => tab.key === key);
    if (!target) {
      return;
    }

    if (activeKey.value && activeKey.value !== key) {
      await flushSave(activeKey.value);
    }

    activeKey.value = target.key;

    if (!target.doc && !target.isMissing) {
      try {
        const doc = await readNote({ absPath: target.absPath });
        applyDocToTab(target, doc);
      } catch (error) {
        if (isNoteNotFoundError(error)) {
          target.isMissing = true;
        } else {
          errorMessage.value = extractEditorErrorMessage(error);
        }
      }
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

    if (tab.conflictState === null && !tab.isMissing) {
      tab.saveStatus = 'dirty';
      scheduleAutoSave(tab.key);
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
    const tab = tabs.value.find((item) => item.key === key);
    if (!tab || !tab.doc || tab.isMissing || tab.conflictState !== null) {
      return false;
    }

    clearAutoSaveTimers(key);

    if (!tab.isDirty && tab.draftBody === tab.doc.body) {
      tab.saveStatus = 'saved';
      return true;
    }

    const savingBody = tab.draftBody;
    tab.saveStatus = 'saving';

    try {
      const res = await writeNote({
        absPath: tab.absPath,
        body: savingBody,
        baseBodyHash: tab.baseBodyHash,
      });

      tab.baseBodyHash = res.bodyHash;
      tab.doc.body = savingBody;
      tab.doc.bodyHash = res.bodyHash;
      tab.doc.mtimeMs = res.mtimeMs;

      if (tab.draftBody !== savingBody) {
        tab.isDirty = true;
        tab.saveStatus = 'dirty';
        scheduleAutoSave(tab.key);
      } else {
        tab.isDirty = false;
        tab.saveStatus = 'saved';
      }

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
          persistTabsDebounced();
        }
        notifyNotesRefresh();
      }

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
    }
  }

  async function flushSave(key?: string): Promise<void> {
    const targetKey = key ?? activeKey.value;
    if (!targetKey) {
      return;
    }
    const tab = tabs.value.find((item) => item.key === targetKey);
    if (!tab || !tab.isDirty || tab.conflictState !== null || tab.isMissing) {
      return;
    }
    await saveTab(targetKey);
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
          notifyNotesRefresh();
        } else {
          clearAutoSaveTimers(tab.key);
          tab.conflictState = {
            kind: 'modified',
            diskBody: event.newBody ?? '',
            diskBodyHash: event.newBodyHash ?? '',
            diskMeta: event.newMeta,
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
    pushClosedStack(target.absPath);
    tabs.value.splice(index, 1);

    if (activeKey.value === key) {
      const next = tabs.value[index] ?? tabs.value[index - 1] ?? null;
      activeKey.value = next?.key ?? null;
      if (next?.doc) {
        recordLastActiveDoc(next.doc);
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
        pushClosedStack(tab.absPath);
      }
    }
    tabs.value = [keep];
    activeKey.value = keep.key;
    if (keep.doc) {
      recordLastActiveDoc(keep.doc);
    }
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
    target.doc = updatedDoc;
    target.isMissing = false;
    if (!target.isDirty) {
      target.draftBody = updatedDoc.body;
      target.baseBodyHash = updatedDoc.bodyHash;
      target.syncSeq += 1;
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
    }
    tabs.value.splice(index, 1);
    if (target && activeKey.value === target.key) {
      const next = tabs.value[index] ?? tabs.value[index - 1] ?? null;
      activeKey.value = next?.key ?? null;
      if (next?.doc) {
        recordLastActiveDoc(next.doc);
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

        let doc: NoteDoc | null = null;
        let isMissing = false;

        if (matched) {
          try {
            doc = await readNote({ absPath: resolvedAbsPath });
          } catch (error) {
            if (isNoteNotFoundError(error)) {
              isMissing = true;
            } else {
              errorMessage.value = extractEditorErrorMessage(error);
            }
          }
        } else {
          try {
            doc = await readNote({ absPath: item.absPath });
          } catch {
            isMissing = true;
          }
        }

        restored.push({
          key: doc?.key ?? resolvedKey,
          relPath: doc?.relPath ?? resolvedRelPath,
          absPath: doc?.absPath ?? resolvedAbsPath,
          isPinned: isPreview ? false : true,
          isPreview,
          scrollTop: item.scrollTop,
          cursorPos: item.cursorPos,
          doc,
          isDirty: false,
          isMissing,
          draftBody: doc?.body ?? '',
          baseBodyHash: doc?.bodyHash ?? '',
          conflictState: null,
          saveStatus: 'saved',
          cursorLine: 1,
          cursorCol: 1,
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
        (preferredKey ? restored.find((tab) => tab.key === preferredKey) : undefined) ??
        restored[0] ??
        null;
      activeKey.value = activeTarget?.key ?? null;
      if (activeTarget?.doc) {
        recordLastActiveDoc(activeTarget.doc);
      }
      initialized.value = true;
    } catch (error) {
      errorMessage.value = extractEditorErrorMessage(error);
    } finally {
      loading.value = false;
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
    if (unlistenExternalChange) {
      unlistenExternalChange();
      unlistenExternalChange = null;
    }
  }

  return {
    tabs,
    activeKey,
    closedStack,
    loading,
    errorMessage,
    initialized,
    activeTab,
    activeDoc,
    activeTopLine,
    scrollTarget,
    inboxBloatThreshold,
    autoSaveDebounceMs,
    autoSaveMaxWaitMs,
    isInboxBloated,
    openNote,
    activateTab,
    activateTabByIndex,
    promoteTabToPinned,
    updateTabBody,
    updateTabCursorAndScroll,
    setActiveTopLine,
    requestScrollToLine,
    saveTab,
    flushSave,
    handleExternalChange,
    resolveConflictReload,
    resolveConflictOverwrite,
    dismissConflictBanner,
    restoreDeletedTabToDisk,
    ensureExternalChangeListener,
    closeTab,
    closeOtherTabs,
    reopenLastClosedTab,
    handleNoteRenamed,
    handleNoteDeleted,
    restoreTabsFromUiState,
    dispose,
  };
});
