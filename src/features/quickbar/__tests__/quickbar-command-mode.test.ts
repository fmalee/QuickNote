import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  CommandEntry,
  ExternalActionState,
  LibraryInfo,
  NavTreeSnapshot,
  ResolvedTarget,
  SearchResponse,
  SettingsSnapshot,
  UiState,
  WindowState,
} from '@/bindings/types';

const {
  mockLibraryGetInfo,
  mockSettingsGet,
  mockUiStateGet,
  mockUiStateSet,
  mockCaptureResolveTarget,
  mockNoteAppend,
  mockNoteRead,
  mockNoteWrite,
  mockSearchNotesAndFiles,
  mockPathComplete,
  mockNotesGetTreeAndTags,
  mockNotesList,
  mockCommandsSearch,
  mockExternalActionsResolve,
  mockExternalActionRun,
  mockWindowShowMain,
  mockWindowHideQuickBar,
  mockOnWindowState,
  mockOnExternalChange,
  mockOnIndexStatus,
} = vi.hoisted(() => ({
  mockLibraryGetInfo: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockCaptureResolveTarget: vi.fn(),
  mockNoteAppend: vi.fn(),
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockSearchNotesAndFiles: vi.fn(),
  mockPathComplete: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockNotesList: vi.fn(),
  mockCommandsSearch: vi.fn(),
  mockExternalActionsResolve: vi.fn(),
  mockExternalActionRun: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockWindowHideQuickBar: vi.fn(),
  mockOnWindowState: vi.fn(),
  mockOnExternalChange: vi.fn(),
  mockOnIndexStatus: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: () => false,
  libraryGetInfo: mockLibraryGetInfo,
  settingsGet: mockSettingsGet,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  captureResolveTarget: mockCaptureResolveTarget,
  noteAppend: mockNoteAppend,
  noteRead: mockNoteRead,
  noteWrite: mockNoteWrite,
  searchNotesAndFiles: mockSearchNotesAndFiles,
  pathComplete: mockPathComplete,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  notesList: mockNotesList,
  commandsSearch: mockCommandsSearch,
  externalActionsResolve: mockExternalActionsResolve,
  externalActionRun: mockExternalActionRun,
  windowShowMain: mockWindowShowMain,
  windowHideQuickBar: mockWindowHideQuickBar,
}));

vi.mock('@/lib/ipc/events', () => ({
  onWindowState: mockOnWindowState,
  onExternalChange: mockOnExternalChange,
  onIndexStatus: mockOnIndexStatus,
}));

import { useCommandPalette } from '@/features/commands';
import QuickBarView from '@/features/quickbar/QuickBarView.vue';
import { useQuickBarStore } from '@/features/quickbar/store';
import { PENDING_OPEN_FILE_STORAGE_KEY } from '@/features/quickopen';
import { actionRegistry } from '@/lib/actions';

function createMemoryStorage() {
  const store = new Map<string, string>();
  return {
    getItem(key: string): string | null {
      return store.get(key) ?? null;
    },
    setItem(key: string, value: string): void {
      store.set(key, value);
    },
    removeItem(key: string): void {
      store.delete(key);
    },
    clear(): void {
      store.clear();
    },
  };
}

function createSampleUiState(): UiState {
  return {
    filters: {
      activeTab: 'recent',
      selectedDir: null,
      selectedTags: [],
      listScrollTop: 0,
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    lastActiveDoc: {
      relPath: 'projects/quicknote.md',
      touchedAtMs: 1700000000000,
    },
    windows: {
      main: { isOpen: false, mode: 'standard', x: 0, y: 0, width: 1200, height: 800 },
    },
    hotkey: {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
  };
}

function createSampleCommand(): CommandEntry {
  return {
    id: 'cmd-undo',
    noteRelPath: 'commands/常用命令.md',
    section: 'Git',
    title: '撤销提交',
    description: '回退一次 commit',
    code: 'git reset --soft HEAD~1',
    language: 'bash',
    keywords: ['undo'],
    isDanger: false,
    line: 19,
  };
}

describe('quickbar-command-mode', () => {
  const originalLocalStorage = globalThis.localStorage;

  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: createMemoryStorage(),
    });

    mockLibraryGetInfo.mockResolvedValue({
      root: '/Users/Joey/Notes/QuickNote',
    } as LibraryInfo);
    mockSettingsGet.mockResolvedValue({
      status: { valid: true },
      settings: { workspaces: {} },
    } as unknown as SettingsSnapshot);
    mockUiStateGet.mockResolvedValue(createSampleUiState());
    mockUiStateSet.mockResolvedValue(createSampleUiState());
    mockCaptureResolveTarget.mockResolvedValue({
      scenario: 'S4',
      kind: 'note',
      targetRelPath: 'projects/quicknote.md',
      targetTitle: 'QuickNote',
      sectionHint: null,
      workspaceKey: 'quicknote',
      workspaceName: 'QuickNote',
      recommendedMode: 'list',
      chipLabel: 'QuickNote · projects/quicknote.md',
    } as ResolvedTarget);
    mockSearchNotesAndFiles.mockResolvedValue({ items: [] } as unknown as SearchResponse);
    mockNotesList.mockResolvedValue([]);
    mockNotesGetTreeAndTags.mockResolvedValue({
      tree: [],
      tags: [],
    } as unknown as NavTreeSnapshot);
    mockCommandsSearch.mockResolvedValue([createSampleCommand()]);
    mockExternalActionsResolve.mockResolvedValue([
      {
        id: 'external.terminal',
        enabled: true,
        disabledReason: null,
        resolvedPath: '/Users/Joey/Sites/Projects/QuickNote',
      },
    ] as ExternalActionState[]);
    mockWindowShowMain.mockResolvedValue({
      windowId: 'main',
      visible: true,
      fullscreen: false,
      mode: 'standard',
      reason: null,
    } as WindowState);
    mockWindowHideQuickBar.mockResolvedValue({
      windowId: 'quickbar',
      visible: false,
      fullscreen: false,
      mode: null,
      reason: 'esc',
    } as WindowState);
    mockOnWindowState.mockResolvedValue(() => {});
    mockOnExternalChange.mockResolvedValue(() => {});
    mockOnIndexStatus.mockResolvedValue(() => {});
  });

  afterEach(() => {
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: originalLocalStorage,
    });
  });

  it('switches to command mode via Cmd+Shift+K, open mode via Cmd+K, and Esc returns to capture before hiding', async () => {
    const store = useQuickBarStore();
    await store.refreshState();

    const disposers = [
      actionRegistry.bindHandler('quickbar.mode.command', () => {
        store.mode = 'command';
      }),
      actionRegistry.bindHandler('quickbar.mode.open', () => {
        store.mode = 'open';
      }),
    ];

    try {
      expect(store.mode).toBe('capture');

      // Cmd+Shift+K -> command mode
      const cmdShiftK = {
        key: 'k',
        code: 'KeyK',
        metaKey: true,
        shiftKey: true,
        ctrlKey: false,
        altKey: false,
        isComposing: false,
        keyCode: 75,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as KeyboardEvent;
      const handledCommand = actionRegistry.dispatchKeyEvent(cmdShiftK, {
        activeScope: 'quickbar',
        activeScopes: ['quickbar', 'global'],
      });
      expect(handledCommand.handled).toBe(true);
      expect(store.mode).toBe('command');

      // Cmd+K -> open mode
      const cmdK = {
        key: 'k',
        code: 'KeyK',
        metaKey: true,
        shiftKey: false,
        ctrlKey: false,
        altKey: false,
        isComposing: false,
        keyCode: 75,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as KeyboardEvent;
      const handledOpen = actionRegistry.dispatchKeyEvent(cmdK, {
        activeScope: 'quickbar',
        activeScopes: ['quickbar', 'global'],
      });
      expect(handledOpen.handled).toBe(true);
      expect(store.mode).toBe('open');

      // 回到 command 模式后，按 Esc 直接隐藏窗口并将模式复位为 capture
      store.mode = 'command';
      await store.handleEscape();
      expect(store.mode).toBe('capture');
      expect(mockWindowHideQuickBar).toHaveBeenCalledWith({ reason: 'esc' });
    } finally {
      disposers.forEach((dispose) => dispose());
    }
  });

  it('writes PENDING_OPEN_FILE_STORAGE_KEY with line, shows main window and hides Quick Bar on Cmd+O', async () => {
    const store = useQuickBarStore();
    await store.refreshState();

    const engine = useCommandPalette({
      onOpenSource: async (absPath, line) => {
        await store.openFileInMain(absPath, false, line);
      },
    });
    await engine.refresh(true);

    const opened = await engine.openSourceFile();
    expect(opened).toBe(true);

    const raw = globalThis.localStorage.getItem(PENDING_OPEN_FILE_STORAGE_KEY);
    expect(raw).not.toBeNull();
    const payload = JSON.parse(raw as string) as {
      absPath: string;
      newTab: boolean;
      line: number | null;
    };
    expect(payload.absPath).toBe('/Users/Joey/Notes/QuickNote/commands/常用命令.md');
    expect(payload.newTab).toBe(false);
    expect(payload.line).toBe(19);
    expect(mockWindowShowMain).toHaveBeenCalledTimes(1);
    expect(mockWindowHideQuickBar).toHaveBeenCalledWith({ reason: 'esc' });

    engine.dispose();
  });

  it('renders CommandBookPalette inside QuickBarView when mode is command', async () => {
    const store = useQuickBarStore();
    await store.refreshState();
    store.mode = 'command';

    const html = await renderToString(createSSRApp(QuickBarView));
    expect(html).toContain('quickbar-command-mode');
    expect(html).toContain('command-book-palette');
  });
});
