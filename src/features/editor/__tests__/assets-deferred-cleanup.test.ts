import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NoteDoc, SettingsSnapshot } from '@/bindings/types';
import { extractAssetRefsFromMarkdown } from '../extensions/markdownImages';

const {
  mockNoteRead,
  mockNoteWrite,
  mockAssetsCleanUnused,
  mockNotesList,
  mockNotesGetTreeAndTags,
  mockUiStateGet,
  mockUiStateSet,
  mockSettingsGet,
  mockOnFsExternalChange,
  mockOnIndexStatus,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockAssetsCleanUnused: vi.fn(),
  mockNotesList: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
  mockOnIndexStatus: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: (err: unknown): err is { code: string; message: string; detail?: string } =>
    typeof err === 'object' && err !== null && 'code' in err && 'message' in err,
  noteRead: mockNoteRead,
  noteWrite: mockNoteWrite,
  assetsCleanUnused: mockAssetsCleanUnused,
  notesList: mockNotesList,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  settingsGet: mockSettingsGet,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: mockOnFsExternalChange,
  onIndexStatus: mockOnIndexStatus,
  onSettingsStatus: mockOnSettingsStatus,
  onWindowState: mockOnWindowState,
}));

import { useEditorTabsStore } from '@/features/editor';
import { useSettingsStore } from '@/features/settings';

function makeDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  return {
    key: '01TESTDOCA',
    relPath: 'notes/a.md',
    absPath: '/vault/notes/a.md',
    isLibrary: true,
    isInbox: false,
    meta: {
      id: '01TESTDOCA',
      title: 'Doc A',
      docType: 'note',
      tags: [],
      openDir: null,
      codeWorkspace: null,
      terminalDir: null,
      created: '2026-11-27T00:00:00Z',
      pinned: false,
      archived: false,
      metaError: null,
    },
    body: '# Doc A\n\n![one](assets/2026-11/one.png)\n![two](assets/2026-11/two.png)\n',
    bodyHash: 'hash-a-1',
    mtimeMs: 1000,
    sizeBytes: 120,
    ...overrides,
  };
}

function makeSettingsSnapshot(autoCleanUnusedAssets = true): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/vault',
      hotkey: { quickBar: 'ctrl+alt+space', quickBarBackup: 'ctrl+alt+cmd+space' },
      quickBar: {
        defaultTarget: 'workspace_last_active_or_inbox',
        lastActiveTtlHours: 24,
        destroyAfterHiddenMinutes: 10,
        autoCreateMissingSection: false,
        pinned: false,
      },
      mainWindow: { mode: 'standard', auxiliaryWidthPercent: 45 },
      editor: {
        theme: 'system',
        fontFamily: 'system-ui',
        codeFontFamily: 'monospace',
        fontSize: 15,
        lineHeight: 1.65,
        maxWidth: 760,
        tabSizeProse: 2,
        tabSizeCode: 4,
        autoSaveDebounceMs: 1200,
        autoSaveMaxWaitMs: 6000,
        singleClickLink: false,
        showTabCloseButton: true,
      },
      panels: {
        default: 'outline',
        width: 280,
        terminal: {
          visibleByDefault: false,
          width: 360,
          shell: '/bin/zsh',
          idleRecycleMinutes: 30,
          maxConcurrentSessions: 2,
        },
      },
      inbox: { bloatThresholdLines: 500 },
      terminal: { name: 'iTerm', bundleId: 'com.googlecode.iterm2' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: [],
      maxActiveEditorRenders: 5,
      externalBookmarks: [],
      scanHiddenFiles: true,
      autoCleanUnusedAssets,
    },
    status: {
      valid: true,
      activeSource: 'file',
      errorLine: null,
      errorColumn: null,
      errorMessage: null,
    },
  };
}

describe('extractAssetRefsFromMarkdown', () => {
  it('提取 Markdown、HTML、相对路径与 URL 编码的 assets 引用并忽略外链', () => {
    const body = [
      '![img1](assets/2026-11/pic%201.png)',
      '![img2](../assets/2026-11/sub.webp)',
      '<img src="./assets/2026-11/html.jpg" />',
      '[ref]: assets/2026-11/ref.png',
      '![remote](https://example.com/assets/2026-11/remote.png)',
    ].join('\n');

    expect(extractAssetRefsFromMarkdown(body, 'projects/doc.md')).toEqual([
      'assets/2026-11/html.jpg',
      'assets/2026-11/pic 1.png',
      'assets/2026-11/ref.png',
      'assets/2026-11/sub.webp',
    ]);
  });
});

describe('editor store deferred asset cleanup', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mockNoteWrite.mockResolvedValue({
      bodyHash: 'hash-a-2',
      mtimeMs: 2000,
      updatedMeta: null,
    });
    mockAssetsCleanUnused.mockResolvedValue({
      deletedPaths: ['assets/2026-11/two.png'],
      freedBytes: 1024,
      skippedReferencedCount: 1,
      skippedRecentCount: 0,
      failedCount: 0,
    });
    mockUiStateSet.mockResolvedValue({});
    mockNotesList.mockResolvedValue([]);
    mockNotesGetTreeAndTags.mockResolvedValue({ tree: [], tags: [] });
  });

  it('编辑删除图片后保存时不立即删图，关闭标签页时才触发延迟清理并保护其他标签页草稿', async () => {
    const settingsStore = useSettingsStore();
    settingsStore.snapshot = makeSettingsSnapshot(true);

    const store = useEditorTabsStore();
    const docA = makeDoc();
    const docB = makeDoc({
      key: '01TESTDOCB',
      relPath: 'notes/b.md',
      absPath: '/vault/notes/b.md',
      body: '# Doc B\n',
      bodyHash: 'hash-b-1',
    });

    mockNoteRead.mockResolvedValueOnce(docA).mockResolvedValueOnce(docB);
    await store.openNote('/vault/notes/a.md', { preview: false });
    await store.openNote('/vault/notes/b.md', { preview: false });
    expect(store.tabs).toHaveLength(2);

    // Remove both images from doc A, and paste `one.png` into doc B's unsaved draft.
    store.updateTabBody(docA.key, '# Doc A\n\nNo images anymore.\n');
    store.updateTabBody(docB.key, '# Doc B\n\n![pasted](assets/2026-11/one.png)\n');

    // Saving doc A must NOT clean assets immediately (keeps Cmd+Z undo intact).
    await store.saveTab(docA.key);
    expect(mockAssetsCleanUnused).not.toHaveBeenCalled();

    // Closing doc A triggers deferred cleanup with doc B's unsaved draft protected.
    store.closeTab(docA.key);
    await Promise.resolve();
    await Promise.resolve();

    expect(mockAssetsCleanUnused).toHaveBeenCalledTimes(1);
    expect(mockAssetsCleanUnused).toHaveBeenCalledWith({
      relPaths: ['assets/2026-11/one.png', 'assets/2026-11/two.png'],
      protectedRelPaths: ['assets/2026-11/one.png'],
      gracePeriodSeconds: 0,
    });
  });

  it('当 autoCleanUnusedAssets 关闭时，关闭标签页不会触发自动清理', async () => {
    const settingsStore = useSettingsStore();
    settingsStore.snapshot = makeSettingsSnapshot(false);

    const store = useEditorTabsStore();
    const docA = makeDoc();
    mockNoteRead.mockResolvedValueOnce(docA);

    await store.openNote('/vault/notes/a.md', { preview: false });
    store.updateTabBody(docA.key, '# Doc A\n');
    store.closeTab(docA.key);
    await Promise.resolve();
    await Promise.resolve();

    expect(mockAssetsCleanUnused).not.toHaveBeenCalled();
  });
});
