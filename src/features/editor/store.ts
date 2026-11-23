import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type { NoteDoc, NoteSummary, TabItemState, TabsUiState } from '@/bindings/types';
import { useSettingsStore } from '@/features/settings';
import {
  extractEditorErrorMessage,
  fetchUiState,
  isNoteNotFoundError,
  listAllNotes,
  patchUiState,
  readNote,
} from './api';

export const DEFAULT_INBOX_BLOAT_THRESHOLD_LINES = 500;
export const MAX_CLOSED_STACK_SIZE = 20;
const TABS_PERSIST_DEBOUNCE_MS = 250;

export interface EditorTab extends TabItemState {
  doc: NoteDoc | null;
  isDirty: boolean;
  isMissing: boolean;
}

export interface OpenNoteOptions {
  preview?: boolean;
  absPath?: string;
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

  let persistTimer: ReturnType<typeof setTimeout> | null = null;

  const activeTab = computed<EditorTab | null>(
    () => tabs.value.find((tab) => tab.key === activeKey.value) ?? null,
  );

  const activeDoc = computed<NoteDoc | null>(() => activeTab.value?.doc ?? null);

  const inboxBloatThreshold = computed<number>(
    () =>
      settingsStore.snapshot?.settings?.inbox.bloatThresholdLines ??
      DEFAULT_INBOX_BLOAT_THRESHOLD_LINES,
  );

  const isInboxBloated = computed<boolean>(() => {
    const doc = activeDoc.value;
    if (!doc) {
      return false;
    }
    if (!doc.isInbox && doc.relPath !== 'inbox.md') {
      return false;
    }
    const lineCount = doc.body.split('\n').length;
    return lineCount > inboxBloatThreshold.value;
  });

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

  function upsertOpenedDoc(doc: NoteDoc, preview: boolean): EditorTab {
    const existingByKey = tabs.value.find((tab) => tab.key === doc.key);
    if (existingByKey) {
      existingByKey.relPath = doc.relPath;
      existingByKey.absPath = doc.absPath;
      existingByKey.doc = doc;
      existingByKey.isMissing = false;
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
          existing.doc = doc;
          existing.isMissing = false;
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
    activeKey.value = target.key;

    if (!target.doc && !target.isMissing) {
      try {
        const doc = await readNote({ absPath: target.absPath });
        target.doc = doc;
        target.isMissing = false;
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

  function closeTab(key: string): void {
    const index = tabs.value.findIndex((tab) => tab.key === key);
    if (index < 0) {
      return;
    }
    const target = tabs.value[index];
    if (!target) {
      return;
    }
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
    inboxBloatThreshold,
    isInboxBloated,
    openNote,
    activateTab,
    activateTabByIndex,
    promoteTabToPinned,
    closeTab,
    closeOtherTabs,
    reopenLastClosedTab,
    handleNoteRenamed,
    handleNoteDeleted,
    restoreTabsFromUiState,
    dispose,
  };
});
