import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NavTreeSnapshot, NoteSummary, PathEntry, SearchResponse } from '@/bindings/types';

const {
  mockSearchNotesAndFiles,
  mockSearchSpotlight,
  mockPathComplete,
  mockNotesGetTreeAndTags,
  mockNotesList,
  mockExternalActionRun,
  mockWindowShowMain,
  mockWindowHideQuickBar,
} = vi.hoisted(() => ({
  mockSearchNotesAndFiles: vi.fn(),
  mockSearchSpotlight: vi.fn(),
  mockPathComplete: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockNotesList: vi.fn(),
  mockExternalActionRun: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockWindowHideQuickBar: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  searchNotesAndFiles: mockSearchNotesAndFiles,
  searchSpotlight: mockSearchSpotlight,
  pathComplete: mockPathComplete,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  notesList: mockNotesList,
  externalActionRun: mockExternalActionRun,
  windowShowMain: mockWindowShowMain,
  windowHideQuickBar: mockWindowHideQuickBar,
}));

import {
  QUICKOPEN_DEBOUNCE_MS,
  computeLongestCommonPrefix,
  parseQuickOpenQuery,
  useQuickOpen,
} from '../index';

function makeSearchResponse(requestId: number, items: SearchResponse['items']): SearchResponse {
  return {
    requestId,
    items,
    truncated: false,
  };
}

describe('quick-open-engine (M3-03)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    mockSearchNotesAndFiles.mockImplementation(async (args: { requestId: number }) =>
      makeSearchResponse(args.requestId, []),
    );
    mockSearchSpotlight.mockImplementation(async (args: { requestId: number }) =>
      makeSearchResponse(args.requestId, []),
    );
    mockPathComplete.mockResolvedValue([]);
    mockNotesGetTreeAndTags.mockResolvedValue({
      inboxCount: 0,
      dirs: [],
      tags: [{ tag: 'work', count: 2, children: [] }],
      workspaces: [],
    } satisfies NavTreeSnapshot);
    mockNotesList.mockResolvedValue([]);
    mockExternalActionRun.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('parses all 5 prefix modes accurately', () => {
    expect(parseQuickOpenQuery('nginx')).toEqual({
      mode: 'all',
      raw: 'nginx',
      term: 'nginx',
    });
    expect(parseQuickOpenQuery('/etc/nginx')).toEqual({
      mode: 'path',
      raw: '/etc/nginx',
      term: '/etc/nginx',
    });
    expect(parseQuickOpenQuery('~/Documents')).toEqual({
      mode: 'path',
      raw: '~/Documents',
      term: '~/Documents',
    });
    expect(parseQuickOpenQuery('#部署脚本')).toEqual({
      mode: 'fulltext',
      raw: '#部署脚本',
      term: '部署脚本',
    });
    expect(parseQuickOpenQuery('@work')).toEqual({
      mode: 'tag',
      raw: '@work',
      term: 'work',
    });
    expect(parseQuickOpenQuery('>新建笔记')).toEqual({
      mode: 'action',
      raw: '>新建笔记',
      term: '新建笔记',
    });
  });

  it('debounces 80ms and discards stale Spotlight responses with outdated requestId', async () => {
    let resolveOldSpotlight!: (res: SearchResponse) => void;
    const oldSpotlightPromise = new Promise<SearchResponse>((resolve) => {
      resolveOldSpotlight = resolve;
    });

    mockSearchNotesAndFiles.mockImplementation(async (args: { query: string; requestId: number }) =>
      makeSearchResponse(args.requestId, [
        {
          kind: 'note',
          key: `note:${args.query}`,
          title: `本地-${args.query}`,
          subtitle: `${args.query}.md`,
          relPath: `${args.query}.md`,
          absPath: `/vault/${args.query}.md`,
          score: 100,
          source: 'library',
        },
      ]),
    );

    mockSearchSpotlight.mockImplementationOnce(() => oldSpotlightPromise);
    mockSearchSpotlight.mockImplementationOnce(async (args: { requestId: number }) =>
      makeSearchResponse(args.requestId, [
        {
          kind: 'file',
          key: 'spot:new',
          title: 'new-spotlight.conf',
          subtitle: '/etc/new-spotlight.conf',
          relPath: null,
          absPath: '/etc/new-spotlight.conf',
          score: 80,
          source: 'spotlight',
        },
        // Duplicate absPath of local item should be deduplicated
        {
          kind: 'file',
          key: 'spot:dup',
          title: '本地-nginxx',
          subtitle: '/vault/nginxx.md',
          relPath: null,
          absPath: '/vault/nginxx.md',
          score: 70,
          source: 'spotlight',
        },
      ]),
    );

    const engine = useQuickOpen();

    // Query 1: "ng" -> wait 80ms so it fires and Spotlight stays in-flight
    engine.setQuery('ng');
    expect(mockSearchNotesAndFiles).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(QUICKOPEN_DEBOUNCE_MS);
    expect(mockSearchNotesAndFiles).toHaveBeenCalledTimes(1);
    expect(mockSearchSpotlight).toHaveBeenCalledTimes(1);
    const firstReqId = mockSearchSpotlight.mock.calls[0]?.[0]?.requestId as number;

    // Query 2: "nginxx" -> fires after 80ms and finishes Spotlight
    engine.setQuery('nginxx');
    await vi.advanceTimersByTimeAsync(QUICKOPEN_DEBOUNCE_MS);
    await Promise.resolve();
    await Promise.resolve();

    expect(engine.items.value.map((i) => i.absPath)).toEqual([
      '/vault/nginxx.md',
      '/etc/new-spotlight.conf',
    ]);

    // Now the stale Spotlight response from query 1 arrives late
    resolveOldSpotlight(
      makeSearchResponse(firstReqId, [
        {
          kind: 'file',
          key: 'spot:stale',
          title: 'stale.conf',
          subtitle: '/etc/stale.conf',
          relPath: null,
          absPath: '/etc/stale.conf',
          score: 99,
          source: 'spotlight',
        },
      ]),
    );
    await Promise.resolve();
    await Promise.resolve();

    // Stale response must NOT overwrite or pollute current results
    expect(engine.items.value.map((i) => i.absPath)).toEqual([
      '/vault/nginxx.md',
      '/etc/new-spotlight.conf',
    ]);

    engine.dispose();
  });

  it('supports Tab completion (single match & common prefix) and Enter dir/file navigation in path mode', async () => {
    const onOpenItem = vi.fn();
    const engine = useQuickOpen({ onOpenItem });

    // Multi-match common prefix completion
    const multiEntries: PathEntry[] = [
      { name: 'nginx.conf', absPath: '/etc/nginx/nginx.conf', isDir: false },
      { name: 'nginx.d', absPath: '/etc/nginx/nginx.d', isDir: true },
    ];
    mockPathComplete.mockResolvedValueOnce(multiEntries);
    engine.setQuery('/etc/nginx/ng');
    await vi.advanceTimersByTimeAsync(QUICKOPEN_DEBOUNCE_MS);

    expect(computeLongestCommonPrefix(multiEntries.map((e) => e.absPath))).toBe(
      '/etc/nginx/nginx.',
    );

    mockPathComplete.mockResolvedValueOnce([
      { name: 'nginx.conf', absPath: '/etc/nginx/nginx.conf', isDir: false },
    ]);
    await engine.handleTabComplete();
    expect(engine.query.value).toBe('/etc/nginx/nginx.');
    await Promise.resolve();

    // Single match Tab completion
    mockPathComplete.mockResolvedValueOnce([]);
    await engine.handleTabComplete();
    expect(engine.query.value).toBe('/etc/nginx/nginx.conf');

    // Enter on directory enters directory; Enter on file opens file
    mockPathComplete.mockResolvedValueOnce([
      { name: 'sites-enabled', absPath: '/etc/nginx/sites-enabled', isDir: true },
      { name: 'mime.types', absPath: '/etc/nginx/mime.types', isDir: false },
    ]);
    engine.setQuery('/etc/nginx/', true);
    await Promise.resolve();
    await Promise.resolve();

    // Index 0 is dir -> Enter should append '/' and refresh
    mockPathComplete.mockResolvedValueOnce([
      { name: 'default.conf', absPath: '/etc/nginx/sites-enabled/default.conf', isDir: false },
    ]);
    await engine.confirmItem(engine.items.value[0], 'open');
    expect(engine.query.value).toBe('/etc/nginx/sites-enabled/');
    expect(onOpenItem).not.toHaveBeenCalled();

    await Promise.resolve();
    await Promise.resolve();
    // Index 0 is now file -> Enter opens it
    await engine.confirmItem(engine.items.value[0], 'newTab');
    expect(onOpenItem).toHaveBeenCalledWith(
      expect.objectContaining({ absPath: '/etc/nginx/sites-enabled/default.conf' }),
      'newTab',
    );

    engine.dispose();
  });

  it('filters and executes registered actions in > mode, and supports tag & fulltext modes', async () => {
    const onExecuteAction = vi.fn();
    const engine = useQuickOpen({
      getActionItems: () => [
        { id: 'window.newNote', title: '新建笔记', shortcutLabel: '⌘N' },
        { id: 'window.save', title: '保存当前笔记', shortcutLabel: '⌘S' },
      ],
      onExecuteAction,
    });

    engine.setQuery('>保存');
    expect(engine.items.value).toHaveLength(1);
    expect(engine.items.value[0]?.actionId).toBe('window.save');

    engine.handleKeydown({
      key: 'Enter',
      isComposing: false,
      keyCode: 13,
      altKey: false,
      metaKey: false,
      preventDefault: vi.fn(),
    } as unknown as KeyboardEvent);

    expect(onExecuteAction).toHaveBeenCalledWith('window.save');

    // Tag mode: exact match with notes places real notes first and removes exact tag placeholder
    mockNotesList.mockResolvedValueOnce([
      {
        key: 'n1',
        relPath: 'work/plan.md',
        absPath: '/vault/work/plan.md',
        title: '工作计划',
        excerpt: '',
        tags: ['work'],
        docType: 'note',
        project: null,
        pinned: false,
        archived: false,
        mtimeMs: 1,
        metaError: null,
        isInbox: false,
      } satisfies NoteSummary,
    ]);
    engine.setQuery('@work', true);
    await Promise.resolve();
    await Promise.resolve();
    expect(engine.items.value[0]).toEqual(
      expect.objectContaining({ kind: 'note', title: '工作计划' }),
    );
    expect(engine.items.value.some((i) => i.kind === 'tag' && i.tagName === 'work')).toBe(false);

    // Fulltext mode
    engine.setQuery('#关键词', true);
    await Promise.resolve();
    expect(mockSearchNotesAndFiles).toHaveBeenCalledWith(
      expect.objectContaining({ query: '关键词' }),
    );

    engine.dispose();
  });

  it('triggers external.reveal on Alt+Enter and ignores IME composing key events', async () => {
    const engine = useQuickOpen();
    mockSearchNotesAndFiles.mockResolvedValueOnce(
      makeSearchResponse(1, [
        {
          kind: 'file',
          key: 'abs:/tmp/demo.conf',
          title: 'demo.conf',
          subtitle: '/tmp/demo.conf',
          relPath: null,
          absPath: '/tmp/demo.conf',
          score: 100,
          source: 'recent',
        },
      ]),
    );

    engine.setQuery('demo', true);
    await Promise.resolve();
    await Promise.resolve();

    // IME composing Enter must be ignored
    const handledIme = engine.handleKeydown({
      key: 'Enter',
      isComposing: true,
      keyCode: 229,
      altKey: true,
      metaKey: false,
      preventDefault: vi.fn(),
    } as unknown as KeyboardEvent);
    expect(handledIme).toBe(false);
    expect(mockExternalActionRun).not.toHaveBeenCalled();

    // Real Alt+Enter triggers external.reveal
    const handledAltEnter = engine.handleKeydown({
      key: 'Enter',
      isComposing: false,
      keyCode: 13,
      altKey: true,
      metaKey: false,
      preventDefault: vi.fn(),
    } as unknown as KeyboardEvent);
    expect(handledAltEnter).toBe(true);
    expect(mockExternalActionRun).toHaveBeenCalledWith({
      id: 'external.reveal',
      absPath: '/tmp/demo.conf',
      dirPath: null,
    });

    engine.dispose();
  });

  it('places real notes first and keeps sub-tags after notes on exact @tag match (M3-qa)', async () => {
    const onOpenItem = vi.fn();
    mockNotesGetTreeAndTags.mockResolvedValueOnce({
      inboxCount: 0,
      dirs: [],
      tags: [
        {
          tag: '测试',
          count: 2,
          children: [{ tag: '测试/子标签', count: 1, children: [] }],
        },
      ],
      workspaces: [],
    } satisfies NavTreeSnapshot);
    mockNotesList.mockResolvedValueOnce([
      {
        key: 'n-test-1',
        relPath: 'notes/test-1.md',
        absPath: '/vault/notes/test-1.md',
        title: '第一篇测试笔记',
        excerpt: '',
        tags: ['测试'],
        docType: 'note',
        project: null,
        pinned: false,
        archived: false,
        mtimeMs: 2,
        metaError: null,
        isInbox: false,
      } satisfies NoteSummary,
      {
        key: 'n-test-2',
        relPath: 'notes/test-2.md',
        absPath: '/vault/notes/test-2.md',
        title: '第二篇测试笔记',
        excerpt: '',
        tags: ['测试'],
        docType: 'note',
        project: null,
        pinned: false,
        archived: false,
        mtimeMs: 1,
        metaError: null,
        isInbox: false,
      } satisfies NoteSummary,
    ]);

    const engine = useQuickOpen({ onOpenItem });
    engine.setQuery('@测试', true);
    await Promise.resolve();
    await Promise.resolve();

    expect(engine.items.value.map((item) => `${item.kind}:${item.title}`)).toEqual([
      'note:第一篇测试笔记',
      'note:第二篇测试笔记',
      'tag:#测试/子标签',
    ]);
    expect(engine.selectedIndex.value).toBe(0);
    expect(engine.selectedItem.value?.kind).toBe('note');
    expect(engine.selectedItem.value?.absPath).toBe('/vault/notes/test-1.md');

    engine.handleKeydown({
      key: 'Enter',
      isComposing: false,
      keyCode: 13,
      altKey: false,
      metaKey: false,
      preventDefault: vi.fn(),
    } as unknown as KeyboardEvent);
    expect(onOpenItem).toHaveBeenCalledWith(
      expect.objectContaining({ absPath: '/vault/notes/test-1.md' }),
      'open',
    );

    engine.dispose();
  });
});
