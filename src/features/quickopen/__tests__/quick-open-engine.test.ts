import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ExternalBookmark,
  ExternalDirectoryListing,
  ExternalPathProbeResult,
  NavTreeSnapshot,
  NoteSummary,
  SearchResponse,
} from '@/bindings/types';

const {
  mockSearchNotesAndFiles,
  mockProbeExternalPath,
  mockListExternalDirectory,
  mockSettingsGet,
  mockNotesGetTreeAndTags,
  mockNotesList,
  mockExternalActionRun,
  mockWindowShowMain,
  mockWindowHideQuickBar,
} = vi.hoisted(() => ({
  mockSearchNotesAndFiles: vi.fn(),
  mockProbeExternalPath: vi.fn(),
  mockListExternalDirectory: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockNotesGetTreeAndTags: vi.fn(),
  mockNotesList: vi.fn(),
  mockExternalActionRun: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockWindowHideQuickBar: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  searchNotesAndFiles: mockSearchNotesAndFiles,
  probeExternalPath: mockProbeExternalPath,
  listExternalDirectory: mockListExternalDirectory,
  settingsGet: mockSettingsGet,
  notesGetTreeAndTags: mockNotesGetTreeAndTags,
  notesList: mockNotesList,
  externalActionRun: mockExternalActionRun,
  windowShowMain: mockWindowShowMain,
  windowHideQuickBar: mockWindowHideQuickBar,
}));

import {
  QUICKOPEN_DEBOUNCE_MS,
  computeLongestCommonPrefix,
  consumePendingOpenFile,
  getPathBaseName,
  matchesBookmarkQuery,
  parseQuickOpenQuery,
  sortBookmarks,
  splitPartialExternalPath,
  useQuickOpen,
  writePendingOpenFile,
} from '../index';

function makeSearchResponse(requestId: number, items: SearchResponse['items']): SearchResponse {
  return {
    requestId,
    items,
    truncated: false,
  };
}

function makeProbeResult(
  overrides: Partial<ExternalPathProbeResult> = {},
): ExternalPathProbeResult {
  return {
    rawInput: '/etc/nginx/nginx.conf',
    resolvedPath: '/etc/nginx/nginx.conf',
    exists: true,
    kind: 'file',
    readable: true,
    size: 128,
    isText: true,
    isUtf8: true,
    isHidden: false,
    ...overrides,
  };
}

function makeDirectoryListing(
  rootPath: string,
  entries: ExternalDirectoryListing['entries'],
): ExternalDirectoryListing {
  return {
    rootPath,
    entries,
    truncated: false,
    visitedCount: entries.length,
  };
}

function makeSettingsSnapshot(externalBookmarks: ExternalBookmark[] = []) {
  return {
    settings: {
      externalBookmarks,
    },
  };
}

describe('quick-open-engine (M3-03 / v5 Refactor)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    mockSearchNotesAndFiles.mockImplementation(async (args: { requestId: number }) =>
      makeSearchResponse(args.requestId, []),
    );
    mockProbeExternalPath.mockResolvedValue(
      makeProbeResult({ exists: false, kind: 'not_found', readable: false }),
    );
    mockListExternalDirectory.mockResolvedValue(makeDirectoryListing('/', []));
    mockSettingsGet.mockResolvedValue(makeSettingsSnapshot([]));
    mockNotesGetTreeAndTags.mockResolvedValue({
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

  it('debounces 80ms, merges vault notes with external bookmarks, filters spotlight items, and discards stale responses', async () => {
    let resolveOldSearch!: (res: SearchResponse) => void;
    const oldSearchPromise = new Promise<SearchResponse>((resolve) => {
      resolveOldSearch = resolve;
    });

    mockSettingsGet.mockResolvedValue(
      makeSettingsSnapshot([
        {
          path: '/etc/nginx/nginx.conf',
          kind: 'file',
          alias: 'Nginx 主配置',
          pinned: true,
          lastOpenedAt: 200,
        },
        {
          path: '/usr/local/etc/nginx',
          kind: 'directory',
          alias: 'Nginx 目录',
          pinned: false,
          lastOpenedAt: 100,
        },
      ]),
    );

    mockSearchNotesAndFiles.mockImplementationOnce(() => oldSearchPromise);
    mockSearchNotesAndFiles.mockImplementationOnce(
      async (args: { query: string; requestId: number }) =>
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
          // Spotlight legacy results must be ignored
          {
            kind: 'file',
            key: 'spot:legacy',
            title: 'legacy-spotlight.conf',
            subtitle: '/etc/legacy-spotlight.conf',
            relPath: null,
            absPath: '/etc/legacy-spotlight.conf',
            score: 80,
            source: 'spotlight',
          },
        ]),
    );

    const engine = useQuickOpen();

    // Query 1: "ng" -> wait 80ms so it fires and stays in-flight
    engine.setQuery('ng');
    expect(mockSearchNotesAndFiles).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(QUICKOPEN_DEBOUNCE_MS);
    expect(mockSearchNotesAndFiles).toHaveBeenCalledTimes(1);
    const firstReqId = mockSearchNotesAndFiles.mock.calls[0]?.[0]?.requestId as number;

    // Query 2: "nginx" -> fires after 80ms and finishes
    engine.setQuery('nginx');
    await vi.advanceTimersByTimeAsync(QUICKOPEN_DEBOUNCE_MS);
    await Promise.resolve();
    await Promise.resolve();

    expect(engine.items.value.map((i) => `${i.source}:${i.absPath}`)).toEqual([
      'library:/vault/nginx.md',
      'file_bookmark:/etc/nginx/nginx.conf',
      'dir_bookmark:/usr/local/etc/nginx',
    ]);

    // Now stale response from query 1 arrives late
    resolveOldSearch(
      makeSearchResponse(firstReqId, [
        {
          kind: 'note',
          key: 'note:stale',
          title: 'stale.md',
          subtitle: 'stale.md',
          relPath: 'stale.md',
          absPath: '/vault/stale.md',
          score: 99,
          source: 'library',
        },
      ]),
    );
    await Promise.resolve();
    await Promise.resolve();

    // Stale response must NOT overwrite current results
    expect(engine.items.value.map((i) => i.absPath)).toEqual([
      '/vault/nginx.md',
      '/etc/nginx/nginx.conf',
      '/usr/local/etc/nginx',
    ]);

    engine.dispose();
  });

  it('supports direct external path probing, Tab completion, and directory drill-down with Backspace return', async () => {
    const onOpenItem = vi.fn();
    const engine = useQuickOpen({ onOpenItem });

    // 1. Direct file probe for `/etc/nginx/nginx.conf`
    mockProbeExternalPath.mockResolvedValueOnce(
      makeProbeResult({
        rawInput: '/etc/nginx/nginx.conf',
        resolvedPath: '/etc/nginx/nginx.conf',
        exists: true,
        kind: 'file',
        readable: true,
      }),
    );
    engine.setQuery('/etc/nginx/nginx.conf', true);
    await Promise.resolve();
    await Promise.resolve();

    expect(mockProbeExternalPath).toHaveBeenCalledWith('/etc/nginx/nginx.conf');
    expect(engine.items.value).toHaveLength(1);
    expect(engine.items.value[0]).toEqual(
      expect.objectContaining({
        kind: 'file',
        source: 'path',
        title: '直接打开外部文件：nginx.conf',
        absPath: '/etc/nginx/nginx.conf',
      }),
    );

    // 2. Partial path listing + multi-match Tab LCP completion
    mockProbeExternalPath.mockResolvedValueOnce(
      makeProbeResult({
        rawInput: '/etc/nginx/ng',
        resolvedPath: '/etc/nginx/ng',
        exists: false,
        kind: 'not_found',
        readable: false,
      }),
    );
    mockListExternalDirectory.mockResolvedValueOnce(
      makeDirectoryListing('/etc/nginx', [
        {
          name: 'nginx.conf',
          path: '/etc/nginx/nginx.conf',
          relPath: 'nginx.conf',
          kind: 'file',
          depth: 0,
          size: 120,
          isHidden: false,
          matchScore: 90,
        },
        {
          name: 'nginx.d',
          path: '/etc/nginx/nginx.d',
          relPath: 'nginx.d',
          kind: 'directory',
          depth: 0,
          size: null,
          isHidden: false,
          matchScore: 80,
        },
      ]),
    );

    engine.setQuery('/etc/nginx/ng');
    await vi.advanceTimersByTimeAsync(QUICKOPEN_DEBOUNCE_MS);
    await Promise.resolve();
    await Promise.resolve();

    expect(computeLongestCommonPrefix(engine.items.value.map((e) => e.absPath ?? ''))).toBe(
      '/etc/nginx/nginx.',
    );

    mockProbeExternalPath.mockResolvedValueOnce(
      makeProbeResult({
        rawInput: '/etc/nginx/nginx.',
        resolvedPath: '/etc/nginx/nginx.',
        exists: false,
        kind: 'not_found',
        readable: false,
      }),
    );
    mockListExternalDirectory.mockResolvedValueOnce(
      makeDirectoryListing('/etc/nginx', [
        {
          name: 'nginx.conf',
          path: '/etc/nginx/nginx.conf',
          relPath: 'nginx.conf',
          kind: 'file',
          depth: 0,
          size: 120,
          isHidden: false,
          matchScore: 90,
        },
      ]),
    );
    await engine.handleTabComplete();
    expect(engine.query.value).toBe('/etc/nginx/nginx.');
    await Promise.resolve();
    await Promise.resolve();

    // Single match Tab completion
    mockProbeExternalPath.mockResolvedValueOnce(
      makeProbeResult({
        rawInput: '/etc/nginx/nginx.conf',
        resolvedPath: '/etc/nginx/nginx.conf',
        exists: true,
        kind: 'file',
        readable: true,
      }),
    );
    await engine.handleTabComplete();
    expect(engine.query.value).toBe('/etc/nginx/nginx.conf');

    // 3. Directory probe -> Enter directory enters scope -> filter -> Enter file opens -> Backspace exits scope
    mockProbeExternalPath.mockResolvedValueOnce(
      makeProbeResult({
        rawInput: '/etc/nginx/',
        resolvedPath: '/etc/nginx',
        exists: true,
        kind: 'directory',
        readable: true,
      }),
    );
    mockListExternalDirectory.mockResolvedValueOnce(
      makeDirectoryListing('/etc/nginx', [
        {
          name: 'sites-enabled',
          path: '/etc/nginx/sites-enabled',
          relPath: 'sites-enabled',
          kind: 'directory',
          depth: 0,
          size: null,
          isHidden: false,
          matchScore: 100,
        },
        {
          name: 'mime.types',
          path: '/etc/nginx/mime.types',
          relPath: 'mime.types',
          kind: 'file',
          depth: 0,
          size: 50,
          isHidden: false,
          matchScore: 90,
        },
      ]),
    );
    engine.setQuery('/etc/nginx/', true);
    await Promise.resolve();
    await Promise.resolve();

    mockListExternalDirectory.mockResolvedValueOnce(
      makeDirectoryListing('/etc/nginx/sites-enabled', [
        {
          name: 'default.conf',
          path: '/etc/nginx/sites-enabled/default.conf',
          relPath: 'default.conf',
          kind: 'file',
          depth: 0,
          size: 80,
          isHidden: false,
          matchScore: 100,
        },
      ]),
    );
    await engine.confirmItem(engine.items.value[0], 'open');
    expect(engine.activeDirectoryScope.value).toEqual({
      dirPath: '/etc/nginx/sites-enabled',
      title: 'sites-enabled',
    });
    expect(engine.query.value).toBe('');
    expect(onOpenItem).not.toHaveBeenCalled();

    await Promise.resolve();
    await Promise.resolve();

    // Index 0 is now file -> Enter opens it in newTab
    await engine.confirmItem(engine.items.value[0], 'newTab');
    expect(onOpenItem).toHaveBeenCalledWith(
      expect.objectContaining({ absPath: '/etc/nginx/sites-enabled/default.conf' }),
      'newTab',
    );

    // Empty input Backspace returns to parent scope
    const handledBackspace = engine.handleKeydown({
      key: 'Backspace',
      isComposing: false,
      keyCode: 8,
      altKey: false,
      metaKey: false,
      preventDefault: vi.fn(),
    } as unknown as KeyboardEvent);
    expect(handledBackspace).toBe(true);
    expect(engine.activeDirectoryScope.value).toBeNull();

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
    await vi.advanceTimersByTimeAsync(0);

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

  it('exports deduplicated path/bookmark pure helpers and extended pending payloads (M5-06b)', () => {
    expect(getPathBaseName('/usr/local/etc/')).toBe('etc');
    expect(getPathBaseName('/')).toBe('/');
    expect(splitPartialExternalPath('/usr/lo')).toEqual({
      parentDir: '/usr',
      filterQuery: 'lo',
    });
    expect(splitPartialExternalPath('/usr/')).toBeNull();

    const bookmarks: ExternalBookmark[] = [
      { path: '/b/file.conf', kind: 'file', alias: 'Beta', pinned: false, lastOpenedAt: 10 },
      { path: '/a/file.conf', kind: 'file', alias: 'Alpha', pinned: true, lastOpenedAt: 5 },
    ];
    expect(sortBookmarks(bookmarks).map((b) => b.path)).toEqual(['/a/file.conf', '/b/file.conf']);
    expect(matchesBookmarkQuery(bookmarks[0]!, 'beta')).toBe(true);
    expect(matchesBookmarkQuery(bookmarks[0]!, 'gamma')).toBe(false);

    const store = new Map<string, string>();
    Object.defineProperty(globalThis, 'localStorage', {
      value: {
        getItem: (k: string) => store.get(k) ?? null,
        setItem: (k: string, v: string) => {
          store.set(k, v);
        },
        removeItem: (k: string) => {
          store.delete(k);
        },
      },
      configurable: true,
      writable: true,
    });

    writePendingOpenFile({ kind: 'tag', tag: 'work', ts: 123 });
    expect(consumePendingOpenFile()).toEqual({ kind: 'tag', tag: 'work', ts: 123 });

    writePendingOpenFile({ kind: 'action', actionId: 'window.newNote', ts: 456 });
    expect(consumePendingOpenFile()).toEqual({
      kind: 'action',
      actionId: 'window.newNote',
      ts: 456,
    });
  });

  it('completes unique prefix match without regressing when deeper subdir matches exist and dedupes duplicate paths (M5-06b)', async () => {
    const engine = useQuickOpen();

    mockProbeExternalPath.mockResolvedValueOnce(
      makeProbeResult({
        rawInput: '/usr/loca',
        resolvedPath: '/usr/loca',
        exists: false,
        kind: 'not_found',
        readable: false,
      }),
    );
    mockListExternalDirectory.mockResolvedValueOnce(
      makeDirectoryListing('/usr', [
        {
          name: 'local',
          path: '/usr/local',
          relPath: 'local',
          kind: 'directory',
          depth: 0,
          size: null,
          isHidden: false,
          matchScore: 100,
        },
        {
          name: 'locale',
          path: '/usr/bin/locale',
          relPath: 'bin/locale',
          kind: 'file',
          depth: 1,
          size: 100,
          isHidden: false,
          matchScore: 80,
        },
        {
          name: 'locale',
          path: '/usr/bin/locale',
          relPath: 'bin/locale',
          kind: 'file',
          depth: 1,
          size: 100,
          isHidden: false,
          matchScore: 80,
        },
      ]),
    );

    engine.setQuery('/usr/loca', true);
    await engine.waitForPendingSearch();

    // Duplicate absPath entries must be deduplicated
    expect(engine.items.value.map((item) => item.absPath)).toEqual([
      '/usr/local',
      '/usr/bin/locale',
    ]);

    // Tab must complete the unique prefix match `/usr/local/` instead of regressing to `/usr/`
    mockProbeExternalPath.mockResolvedValueOnce(
      makeProbeResult({
        rawInput: '/usr/local/',
        resolvedPath: '/usr/local',
        exists: true,
        kind: 'directory',
        readable: true,
      }),
    );
    mockListExternalDirectory.mockResolvedValueOnce(makeDirectoryListing('/usr/local', []));
    const completed = await engine.handleTabComplete();
    expect(completed).toBe(true);
    expect(engine.query.value).toBe('/usr/local/');
    await engine.waitForPendingSearch();

    // When no candidate starts with the current term, Tab must NOT shorten/regress the query
    mockProbeExternalPath.mockResolvedValueOnce(
      makeProbeResult({
        rawInput: '/usr/xyz',
        resolvedPath: '/usr/xyz',
        exists: false,
        kind: 'not_found',
        readable: false,
      }),
    );
    mockListExternalDirectory.mockResolvedValueOnce(
      makeDirectoryListing('/usr', [
        {
          name: 'xyz1',
          path: '/usr/bin/xyz1',
          relPath: 'bin/xyz1',
          kind: 'file',
          depth: 1,
          size: 10,
          isHidden: false,
          matchScore: 50,
        },
        {
          name: 'xyz2',
          path: '/usr/share/xyz2',
          relPath: 'share/xyz2',
          kind: 'file',
          depth: 1,
          size: 10,
          isHidden: false,
          matchScore: 40,
        },
      ]),
    );
    engine.setQuery('/usr/xyz', true);
    await engine.waitForPendingSearch();

    const noRegress = await engine.handleTabComplete();
    expect(noRegress).toBe(false);
    expect(engine.query.value).toBe('/usr/xyz');

    engine.dispose();
  });
});
