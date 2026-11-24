import { computed, ref, watch, type ComputedRef, type Ref } from 'vue';
import type { CommandEntry, ExternalActionState } from '@/bindings/types';
import { writePendingOpenFile } from '@/features/quickopen';
import {
  copyTextToClipboard,
  fetchCommandWorkspaces,
  fetchLibraryRoot,
  hideQuickBarWindow,
  joinLibraryPath,
  openExternalTerminalAtDir,
  resolveCommandExternalActions,
  searchCommands,
  showMainWindow,
  type CommandWorkspaceOption,
} from './api';

export const COMMAND_SEARCH_DEBOUNCE_MS = 80;
export const ALL_SECTIONS_KEY = '__all__';

export interface CommandPaletteContext {
  absPath: string | null;
  workspaceKey: string | null;
}

export interface CommandSectionSummary {
  name: string;
  count: number;
}

export interface UseCommandPaletteOptions {
  getContext?: () => CommandPaletteContext;
  onOpenSource?: (absPath: string, line: number) => void | Promise<void>;
  onRequestClose?: () => void | Promise<void>;
  debounceMs?: number;
}

export interface CommandPaletteEngine {
  query: Ref<string>;
  entries: Ref<CommandEntry[]>;
  selectedSection: Ref<string>;
  selectedIndex: Ref<number>;
  loading: Ref<boolean>;
  feedbackMessage: Ref<string | null>;
  errorMessage: Ref<string | null>;
  terminalState: Ref<ExternalActionState | null>;
  dirPickerOpen: Ref<boolean>;
  manualDirInput: Ref<string>;
  dirPickerError: Ref<string | null>;
  workspaceOptions: Ref<CommandWorkspaceOption[]>;
  sections: ComputedRef<CommandSectionSummary[]>;
  filteredEntries: ComputedRef<CommandEntry[]>;
  selectedEntry: ComputedRef<CommandEntry | null>;
  setQuery: (nextQuery: string) => void;
  setManualDirInput: (nextDir: string) => void;
  refresh: (immediate?: boolean) => Promise<void>;
  selectSection: (section: string) => void;
  selectIndex: (index: number) => void;
  moveSelection: (delta: number) => void;
  copySelected: () => Promise<boolean>;
  copyAndOpenTerminal: () => Promise<boolean>;
  openDirPicker: () => Promise<void>;
  closeDirPicker: () => void;
  confirmManualDir: (dirPath?: string) => Promise<boolean>;
  openSourceFile: () => Promise<boolean>;
  resolveTerminalForSelected: () => Promise<void>;
  dispose: () => void;
}

export function useCommandPalette(options: UseCommandPaletteOptions = {}): CommandPaletteEngine {
  const debounceMs = options.debounceMs ?? COMMAND_SEARCH_DEBOUNCE_MS;

  const query = ref('');
  const entries = ref<CommandEntry[]>([]);
  const selectedSection = ref<string>(ALL_SECTIONS_KEY);
  const selectedIndex = ref(0);
  const loading = ref(false);
  const feedbackMessage = ref<string | null>(null);
  const errorMessage = ref<string | null>(null);
  const terminalState = ref<ExternalActionState | null>(null);

  const dirPickerOpen = ref(false);
  const manualDirInput = ref('');
  const dirPickerError = ref<string | null>(null);
  const workspaceOptions = ref<CommandWorkspaceOption[]>([]);

  let cachedLibraryRoot: string | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let feedbackTimer: ReturnType<typeof setTimeout> | null = null;
  let searchSeq = 0;
  let resolveSeq = 0;

  const sections = computed<CommandSectionSummary[]>(() => {
    const counts = new Map<string, number>();
    for (const entry of entries.value) {
      const name = entry.section || '未分区';
      counts.set(name, (counts.get(name) ?? 0) + 1);
    }
    return Array.from(counts.entries()).map(([name, count]) => ({ name, count }));
  });

  const filteredEntries = computed<CommandEntry[]>(() => {
    if (selectedSection.value === ALL_SECTIONS_KEY) {
      return entries.value;
    }
    return entries.value.filter((entry) => (entry.section || '未分区') === selectedSection.value);
  });

  const selectedEntry = computed<CommandEntry | null>(() => {
    return filteredEntries.value[selectedIndex.value] ?? null;
  });

  function clearSearchTimer(): void {
    if (searchTimer !== null) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }
  }

  function showFeedback(message: string): void {
    feedbackMessage.value = message;
    if (feedbackTimer !== null) {
      clearTimeout(feedbackTimer);
    }
    feedbackTimer = setTimeout(() => {
      feedbackMessage.value = null;
      feedbackTimer = null;
    }, 2000);
  }

  async function resolveTerminalForSelected(): Promise<void> {
    const entry = selectedEntry.value;
    const seq = ++resolveSeq;
    if (!entry) {
      terminalState.value = null;
      return;
    }
    try {
      const ctx = options.getContext?.() ?? { absPath: null, workspaceKey: null };
      const states = await resolveCommandExternalActions({
        absPath: ctx.absPath,
        workspaceKey: ctx.workspaceKey,
        commandId: entry.id,
      });
      if (seq !== resolveSeq) {
        return;
      }
      terminalState.value = states.find((item) => item.id === 'external.terminal') ?? null;
    } catch {
      if (seq !== resolveSeq) {
        return;
      }
      terminalState.value = null;
    }
  }

  async function executeSearch(q: string): Promise<void> {
    const seq = ++searchSeq;
    loading.value = true;
    errorMessage.value = null;
    try {
      const results = await searchCommands(q);
      if (seq !== searchSeq) {
        return;
      }
      entries.value = results;
      if (
        selectedSection.value !== ALL_SECTIONS_KEY &&
        !results.some((item) => (item.section || '未分区') === selectedSection.value)
      ) {
        selectedSection.value = ALL_SECTIONS_KEY;
      }
      if (filteredEntries.value.length === 0) {
        selectedIndex.value = 0;
      } else if (selectedIndex.value >= filteredEntries.value.length) {
        selectedIndex.value = 0;
      }
      await resolveTerminalForSelected();
    } catch (err) {
      if (seq !== searchSeq) {
        return;
      }
      errorMessage.value = err instanceof Error ? err.message : '命令检索失败';
      entries.value = [];
      terminalState.value = null;
    } finally {
      if (seq === searchSeq) {
        loading.value = false;
      }
    }
  }

  function setQuery(nextQuery: string): void {
    query.value = nextQuery;
    clearSearchTimer();
    searchTimer = setTimeout(() => {
      searchTimer = null;
      void executeSearch(nextQuery);
    }, debounceMs);
  }

  function setManualDirInput(nextDir: string): void {
    manualDirInput.value = nextDir;
    dirPickerError.value = null;
  }

  async function refresh(immediate = true): Promise<void> {
    clearSearchTimer();
    if (immediate) {
      await executeSearch(query.value);
    } else {
      setQuery(query.value);
    }
  }

  function selectSection(section: string): void {
    selectedSection.value = section;
    selectedIndex.value = 0;
  }

  function selectIndex(index: number): void {
    const count = filteredEntries.value.length;
    if (count === 0) {
      selectedIndex.value = 0;
      return;
    }
    const clamped = Math.max(0, Math.min(index, count - 1));
    selectedIndex.value = clamped;
  }

  function moveSelection(delta: number): void {
    const count = filteredEntries.value.length;
    if (count === 0) {
      selectedIndex.value = 0;
      return;
    }
    const next = (selectedIndex.value + delta + count) % count;
    selectedIndex.value = next;
  }

  async function copySelected(): Promise<boolean> {
    const entry = selectedEntry.value;
    if (!entry) {
      return false;
    }
    errorMessage.value = null;
    try {
      await copyTextToClipboard(entry.code);
      showFeedback('已复制命令');
      return true;
    } catch (err) {
      errorMessage.value = err instanceof Error ? err.message : '复制命令失败';
      return false;
    }
  }

  async function openDirPicker(): Promise<void> {
    dirPickerOpen.value = true;
    dirPickerError.value = null;
    manualDirInput.value = '';
    workspaceOptions.value = await fetchCommandWorkspaces();
  }

  function closeDirPicker(): void {
    dirPickerOpen.value = false;
    dirPickerError.value = null;
  }

  async function copyAndOpenTerminal(): Promise<boolean> {
    const entry = selectedEntry.value;
    if (!entry) {
      return false;
    }
    errorMessage.value = null;
    try {
      await copyTextToClipboard(entry.code);
      showFeedback('已复制命令');
    } catch (err) {
      errorMessage.value = err instanceof Error ? err.message : '复制命令失败';
      return false;
    }

    const resolvedDir =
      terminalState.value?.enabled && terminalState.value?.resolvedPath
        ? terminalState.value.resolvedPath
        : null;

    if (!resolvedDir) {
      await openDirPicker();
      return false;
    }

    try {
      await openExternalTerminalAtDir(resolvedDir);
      await options.onRequestClose?.();
      return true;
    } catch (err) {
      errorMessage.value = err instanceof Error ? err.message : '打开外部终端失败';
      return false;
    }
  }

  async function confirmManualDir(dirPath?: string): Promise<boolean> {
    const chosen = (dirPath ?? manualDirInput.value).trim();
    if (!chosen) {
      dirPickerError.value = '请选择工作区或输入有效目录路径';
      return false;
    }
    dirPickerError.value = null;
    try {
      await openExternalTerminalAtDir(chosen);
      dirPickerOpen.value = false;
      await options.onRequestClose?.();
      return true;
    } catch (err) {
      dirPickerError.value = err instanceof Error ? err.message : '打开外部终端失败';
      return false;
    }
  }

  async function openSourceFile(): Promise<boolean> {
    const entry = selectedEntry.value;
    if (!entry) {
      return false;
    }
    errorMessage.value = null;
    if (!cachedLibraryRoot) {
      cachedLibraryRoot = await fetchLibraryRoot();
    }
    const absPath = cachedLibraryRoot
      ? joinLibraryPath(cachedLibraryRoot, entry.noteRelPath)
      : entry.noteRelPath;

    if (options.onOpenSource) {
      await options.onOpenSource(absPath, entry.line);
      return true;
    }

    writePendingOpenFile({
      absPath,
      newTab: false,
      line: entry.line,
      ts: Date.now(),
    });
    await showMainWindow();
    await hideQuickBarWindow('esc');
    return true;
  }

  watch(
    () => selectedEntry.value?.id,
    () => {
      void resolveTerminalForSelected();
    },
  );

  function dispose(): void {
    clearSearchTimer();
    if (feedbackTimer !== null) {
      clearTimeout(feedbackTimer);
      feedbackTimer = null;
    }
  }

  return {
    query,
    entries,
    selectedSection,
    selectedIndex,
    loading,
    feedbackMessage,
    errorMessage,
    terminalState,
    dirPickerOpen,
    manualDirInput,
    dirPickerError,
    workspaceOptions,
    sections,
    filteredEntries,
    selectedEntry,
    setQuery,
    setManualDirInput,
    refresh,
    selectSection,
    selectIndex,
    moveSelection,
    copySelected,
    copyAndOpenTerminal,
    openDirPicker,
    closeDirPicker,
    confirmManualDir,
    openSourceFile,
    resolveTerminalForSelected,
    dispose,
  };
}
