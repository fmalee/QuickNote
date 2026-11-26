import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExternalChangeEvent, NoteDoc, NoteMeta, UiState } from '@/bindings/types';

const {
  mockNoteRead,
  mockNoteWrite,
  mockNotesList,
  mockNotesGetTreeAndTags,
  mockUiStateGet,
  mockUiStateSet,
  mockOnFsExternalChange,
  mockOnIndexStatus,
} = vi.hoisted(() => ({
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockNotesList: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
  mockOnIndexStatus: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: (err: unknown): err is { code: string; message: string; detail?: string } =>
    typeof err === 'object' && err !== null && 'code' in err && 'message' in err,
  noteRead: mockNoteRead,
  noteWrite: mockNoteWrite,
  notesList: mockNotesList,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: mockOnFsExternalChange,
  onIndexStatus: mockOnIndexStatus,
}));

vi.mock('@/features/settings', () => ({
  useSettingsStore: () => ({
    settings: {
      inboxPath: 'inbox.md',
      inboxBloatThresholdLines: 500,
      autoSaveDebounceMs: 600,
      autoSaveMaxWaitMs: 2000,
      maxActiveEditorRenders: 8,
    },
    ensureLoaded: vi.fn().mockResolvedValue(undefined),
  }),
}));

import { useEditorTabsStore } from '../store';

function makeMeta(overrides: Partial<NoteMeta> = {}): NoteMeta {
  return {
    id: '01JTEST0000000000000000000',
    title: '测试文档',
    created: '2026-03-27T00:00:00Z',
    tags: [],
    project: null,
    openDir: null,
    codeWorkspace: null,
    terminalDir: null,
    pinned: false,
    archived: false,
    docType: 'note',
    metaError: null,
    ...overrides,
  };
}

function makeDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  return {
    key: '01JTEST0000000000000000000',
    relPath: 'notes/test.md',
    absPath: '/vault/notes/test.md',
    isInbox: false,
    isLibrary: true,
    meta: makeMeta(),
    body: '# 测试\n\n内容',
    bodyHash: 'hash-v1',
    mtimeMs: 1000,
    sizeBytes: 128,
    ...overrides,
  };
}

function makeUiState(): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    lastActiveDoc: null,
    hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space', selfCheck: 'ok' },
  };
}

describe('editorStatus in useEditorTabsStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([]);
    mockNotesGetTreeAndTags.mockResolvedValue({
      tree: [],
      tags: [],
      totalNotes: 0,
      inboxCount: 0,
      archivedCount: 0,
    });
    mockOnFsExternalChange.mockResolvedValue(() => {});
    mockOnIndexStatus.mockResolvedValue(() => {});
  });

  afterEach(() => {
    useEditorTabsStore().dispose();
  });

  it('setEditorStatus 后 editorStatus 反映新值', () => {
    const store = useEditorTabsStore();
    expect(store.editorStatus).toEqual({
      docKindLabel: '',
      indentLabel: '',
      diagnosticsLabel: '',
    });

    store.setEditorStatus({
      docKindLabel: 'Markdown',
      indentLabel: '2 空格',
      diagnosticsLabel: '语法正常',
    });

    expect(store.editorStatus).toEqual({
      docKindLabel: 'Markdown',
      indentLabel: '2 空格',
      diagnosticsLabel: '语法正常',
    });

    store.setEditorStatus({
      docKindLabel: 'TypeScript',
      indentLabel: '4 空格',
      diagnosticsLabel: '2 个问题 · L3: Unexpected token',
    });

    expect(store.editorStatus.docKindLabel).toBe('TypeScript');
    expect(store.editorStatus.indentLabel).toBe('4 空格');
    expect(store.editorStatus.diagnosticsLabel).toBe('2 个问题 · L3: Unexpected token');
  });

  it('handleExternalChange 等既有流程不会把 editorStatus 置为 undefined', async () => {
    const docV1 = makeDoc({ body: '# v1', bodyHash: 'hash-v1', mtimeMs: 1000 });
    const docV2 = makeDoc({ body: '# v2', bodyHash: 'hash-v2', mtimeMs: 2000 });
    mockNoteRead.mockResolvedValueOnce(docV1).mockResolvedValueOnce(docV2);

    const store = useEditorTabsStore();
    await store.openNote(docV1.key);

    store.setEditorStatus({
      docKindLabel: 'Markdown',
      indentLabel: '2 空格',
      diagnosticsLabel: '语法正常',
    });

    const event: ExternalChangeEvent = {
      kind: 'modified',
      absPath: docV1.absPath,
      newBody: docV2.body,
      newBodyHash: docV2.bodyHash,
      newMeta: docV2.meta,
    };
    await store.handleExternalChange(event);

    expect(store.editorStatus).toBeDefined();
    expect(store.editorStatus).toEqual({
      docKindLabel: 'Markdown',
      indentLabel: '2 空格',
      diagnosticsLabel: '语法正常',
    });

    // 关闭标签后也不应变为 undefined
    store.closeTab(docV1.key);
    expect(store.editorStatus).toBeDefined();
  });
});
