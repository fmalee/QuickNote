import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NoteDoc, UiState } from '@/bindings/types';

const {
  mockNoteRead,
  mockNoteWrite,
  mockNotesList,
  mockUiStateGet,
  mockUiStateSet,
  mockOnFsExternalChange,
} = vi.hoisted(() => ({
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockNotesList: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: () => false,
  noteRead: mockNoteRead,
  noteWrite: mockNoteWrite,
  notesList: mockNotesList,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: mockOnFsExternalChange,
}));

import { useEditorTabsStore } from '@/features/editor';
import {
  extractMarkdownOutline,
  findActiveHeadingIndex,
  headingToSlug,
  usePanelsStore,
} from '@/features/panels';

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

function makeDoc(body: string): NoteDoc {
  return {
    key: '01JOUTLINEDOC0000000000001',
    relPath: 'notes/outline.md',
    absPath: '/vault/notes/outline.md',
    isInbox: false,
    isLibrary: true,
    meta: {
      id: '01JOUTLINEDOC0000000000001',
      title: '大纲测试',
      tags: [],
      project: null,
      docType: 'note',
      openDir: null,
      codeWorkspace: null,
      terminalDir: null,
      created: '2026-11-24T00:00:00Z',
      pinned: false,
      archived: false,
      metaError: null,
    },
    body,
    bodyHash: 'hash-1',
    mtimeMs: 1000,
    sizeBytes: 128,
  };
}

describe('outlineExtractor & usePanelsStore', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setActivePinia(createPinia());

    mockNoteRead.mockReset();
    mockNoteWrite.mockReset();
    mockNotesList.mockReset();
    mockUiStateGet.mockReset();
    mockUiStateSet.mockReset();
    mockOnFsExternalChange.mockReset();

    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([]);
    mockOnFsExternalChange.mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('extracts headings, skips fenced code blocks, strips inline markdown and builds C8.4 Chinese slugs', () => {
    const markdown = [
      '# **项目** 概述与 [链接](https://example.com)',
      '',
      '```bash',
      '# 这是代码块里的注释，不应该出现在大纲里',
      'echo "hello"',
      '```',
      '',
      '## `API` 设计！',
      '~~~python',
      '# 另一个代码块注释',
      '~~~',
      '## API 设计',
      '### API 设计',
    ].join('\n');

    const items = extractMarkdownOutline(markdown);
    expect(
      items.map((item) => ({
        level: item.level,
        text: item.text,
        slug: item.slug,
        line: item.line,
      })),
    ).toEqual([
      { level: 1, text: '项目 概述与 链接', slug: '项目-概述与-链接', line: 1 },
      { level: 2, text: 'API 设计！', slug: 'api-设计', line: 8 },
      { level: 2, text: 'API 设计', slug: 'api-设计-1', line: 12 },
      { level: 3, text: 'API 设计', slug: 'api-设计-2', line: 13 },
    ]);

    const seen = new Map<string, number>();
    expect(headingToSlug('中文 标题，测试！', seen)).toBe('中文-标题测试');
    expect(headingToSlug('中文 标题，测试！', seen)).toBe('中文-标题测试-1');
    expect(headingToSlug('中文 标题，测试！', seen)).toBe('中文-标题测试-2');

    expect(findActiveHeadingIndex(items, 1)).toBe(0);
    expect(findActiveHeadingIndex(items, 9)).toBe(1);
    expect(findActiveHeadingIndex(items, 13)).toBe(3);
    expect(findActiveHeadingIndex([], 5)).toBe(-1);
  });

  it('debounces outline updates by 150ms and highlights active heading', async () => {
    const doc = makeDoc('# 初始标题\n正文');
    mockNoteRead.mockResolvedValue(doc);

    const editorTabsStore = useEditorTabsStore();
    const panelsStore = usePanelsStore();
    await panelsStore.initialize();
    await editorTabsStore.openNote(doc.relPath!, { preview: false, absPath: doc.absPath });
    await vi.advanceTimersByTimeAsync(0);

    expect(panelsStore.outlineItems.map((i) => i.text)).toEqual(['初始标题']);

    editorTabsStore.updateTabBody(doc.key, '# 初始标题\n正文\n## 新增二级标题');
    await vi.advanceTimersByTimeAsync(149);
    expect(panelsStore.outlineItems.map((i) => i.text)).toEqual(['初始标题']);

    await vi.advanceTimersByTimeAsync(1);
    expect(panelsStore.outlineItems.map((i) => i.text)).toEqual(['初始标题', '新增二级标题']);

    editorTabsStore.setActiveTopLine(3);
    await vi.advanceTimersByTimeAsync(0);
    expect(panelsStore.activeHeadingIndex).toBe(1);
  });
});
