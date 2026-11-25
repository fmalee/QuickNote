import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  NavTreeSnapshot,
  NoteDoc,
  NoteSummary,
  TagRenameResult,
  UiState,
} from '@/bindings/types';

const {
  mockNotesGetTreeAndTags,
  mockNotesList,
  mockNoteRead,
  mockTagsBatchRename,
  mockUiStateGet,
  mockUiStateSet,
  mockOnIndexStatus,
} = vi.hoisted(() => ({
  mockNotesGetTreeAndTags: vi.fn(),
  mockNotesList: vi.fn(),
  mockNoteRead: vi.fn(),
  mockTagsBatchRename: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnIndexStatus: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  notesList: mockNotesList,
  noteRead: mockNoteRead,
  noteCreate: vi.fn(),
  noteRename: vi.fn(),
  noteDelete: vi.fn(),
  noteWrite: vi.fn(),
  noteUpdateMeta: vi.fn(),
  inboxExtractToNote: vi.fn(),
  tagsBatchRename: mockTagsBatchRename,
  searchNotesAndFiles: vi.fn(async () => ({ items: [] })),
  externalActionRun: vi.fn(),
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
  settingsGet: vi.fn(async () => ({ settings: null })),
  libraryGetInfo: vi.fn(async () => null),
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onIndexStatus: mockOnIndexStatus,
  onSettingsStatus: vi.fn(async () => () => undefined),
  onFsExternalChange: vi.fn(async () => () => undefined),
  onQuickBarAppend: vi.fn(async () => () => undefined),
  onWindowState: vi.fn(async () => () => undefined),
  onWindowFileDrop: vi.fn(async () => () => undefined),
}));

import { useEditorTabsStore } from '@/features/editor';
import NotesSidebar from '../NotesSidebar.vue';
import { useNotesStore } from '../store';

function makeUiState(): UiState {
  return {
    filters: {
      activeTab: 'tags',
      selectedDir: null,
      selectedTags: ['work', 'work/todo'],
      listScrollTop: 0,
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    selectedWorkspaceKey: null,
  } as unknown as UiState;
}

function makeNavTree(): NavTreeSnapshot {
  return {
    dirs: [],
    tags: [
      {
        tag: 'work',
        count: 3,
        children: [{ tag: 'work/todo', count: 2, children: [] }],
      },
      {
        tag: 'focus',
        count: 1,
        children: [],
      },
    ],
    workspaces: [],
  };
}

function makeNoteSummary(): NoteSummary {
  return {
    key: 'notes/a.md',
    relPath: 'notes/a.md',
    absPath: '/vault/notes/a.md',
    title: 'Note A',
    excerpt: 'excerpt',
    tags: ['work'],
    project: null,
    pinned: false,
    archived: false,
    isInbox: false,
    docType: 'note',
    metaError: null,
    mtimeMs: 1700000000000,
  };
}

function makeNoteDoc(tags: string[]): NoteDoc {
  return {
    key: 'notes/a.md',
    relPath: 'notes/a.md',
    absPath: '/vault/notes/a.md',
    isLibrary: true,
    isInbox: false,
    sizeBytes: 64,
    mtimeMs: 1700000000000,
    body: '# Note A\n',
    bodyHash: 'hash-1',
    meta: {
      id: '01ABCDEF',
      title: 'Note A',
      created: '2026-01-01',
      tags,
      project: null,
      openDir: null,
      codeWorkspace: null,
      terminalDir: null,
      pinned: false,
      archived: false,
      docType: 'note',
      metaError: null,
    },
  };
}

describe('tag batch rename and merge', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockImplementation(async () => makeUiState());
    mockNotesGetTreeAndTags.mockResolvedValue(makeNavTree());
    mockNotesList.mockResolvedValue([makeNoteSummary()]);
    mockOnIndexStatus.mockResolvedValue(async () => undefined);
    mockNoteRead.mockResolvedValue(makeNoteDoc(['focus']));
  });

  it('rejects empty or identical tag names without calling IPC', async () => {
    const notesStore = useNotesStore();
    await notesStore.initialize();

    expect(await notesStore.renameOrMergeTag('work', '   ')).toBe(0);
    expect(notesStore.errorMessage).toContain('不能为空');

    expect(await notesStore.renameOrMergeTag('work', '#work')).toBe(0);
    expect(notesStore.errorMessage).toContain('相同');

    expect(mockTagsBatchRename).not.toHaveBeenCalled();
  });

  it('calls tagsBatchRename, updates selectedTags, refreshes tree/list, and syncs open tabs', async () => {
    const result: TagRenameResult = { updatedFiles: 3 };
    mockTagsBatchRename.mockResolvedValueOnce(result);

    const notesStore = useNotesStore();
    const editorTabsStore = useEditorTabsStore();

    mockNoteRead.mockResolvedValueOnce(makeNoteDoc(['work']));
    await editorTabsStore.openNote('notes/a.md', { preview: false });

    await notesStore.initialize();
    expect(notesStore.selectedTags).toEqual(['work', 'work/todo']);

    mockNotesGetTreeAndTags.mockClear();
    mockNotesList.mockClear();
    mockNoteRead.mockResolvedValueOnce(makeNoteDoc(['focus']));
    const updated = await notesStore.renameOrMergeTag('work', 'focus');

    expect(updated).toBe(3);
    expect(mockTagsBatchRename).toHaveBeenCalledWith({ oldTag: 'work', newTag: 'focus' });
    expect(notesStore.selectedTags).toEqual(['focus', 'focus/todo']);
    expect(mockNotesGetTreeAndTags).toHaveBeenCalledTimes(1);
    expect(mockNotesList).toHaveBeenCalledTimes(1);
    expect(editorTabsStore.activeTab?.doc?.meta.tags).toEqual(['focus']);
  });

  it('renders tag cloud items in NotesSidebar', async () => {
    const notesStore = useNotesStore();
    await notesStore.initialize();

    const app = createSSRApp(NotesSidebar);
    const html = await renderToString(app);

    expect(html).toContain('data-testid="tag-cloud"');
    expect(html).toContain('data-testid="tag-cloud-item-work"');
    expect(html).toContain('data-testid="tag-cloud-item-work/todo"');
    expect(html).toContain('data-testid="tag-cloud-item-focus"');
  });
});
