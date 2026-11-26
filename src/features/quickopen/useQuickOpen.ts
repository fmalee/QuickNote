import { computed, ref } from 'vue';
import type {
  ExternalBookmark,
  ExternalDirectoryEntry,
  ExternalPathProbeResult,
  SearchResultItem,
  SearchSource,
  TagNode,
} from '@/bindings/types';
import {
  fetchQuickOpenExternalBookmarks,
  fetchQuickOpenNotesAndFiles,
  fetchQuickOpenNotesList,
  fetchQuickOpenTreeAndTags,
  listQuickOpenExternalDirectory,
  probeQuickOpenExternalPath,
  runQuickOpenExternalAction,
} from './api';
import {
  computeLongestCommonPrefix,
  parseQuickOpenQuery,
  type ParsedQuickOpenQuery,
} from './queryParser';

export const QUICKOPEN_DEBOUNCE_MS = 80;

export type QuickOpenOpenMode = 'open' | 'newTab';

export type QuickOpenResultSource =
  Exclude<SearchSource, 'spotlight'> | 'file_bookmark' | 'dir_bookmark' | 'action' | 'tag';

export interface QuickOpenDirectoryScope {
  dirPath: string;
  title: string;
}

export interface QuickOpenActionItem {
  id: string;
  title: string;
  subtitle?: string;
  shortcutLabel?: string;
  disabled?: boolean;
}

export interface QuickOpenResultItem {
  id: string;
  kind: 'note' | 'file' | 'dir' | 'action' | 'tag';
  title: string;
  subtitle: string;
  absPath: string | null;
  relPath: string | null;
  source: QuickOpenResultSource;
  score: number;
  isDir?: boolean;
  isHidden?: boolean;
  pinned?: boolean;
  tagName?: string;
  actionId?: string;
  shortcutLabel?: string;
  disabled?: boolean;
}

export interface UseQuickOpenOptions {
  debounceMs?: number;
  getActionItems?: () => QuickOpenActionItem[];
  onOpenItem?: (item: QuickOpenResultItem, mode: QuickOpenOpenMode) => void | Promise<void>;
  onExecuteAction?: (actionId: string) => void | Promise<void>;
  onSelectTag?: (tagName: string) => void | Promise<void>;
  onRevealItem?: (item: QuickOpenResultItem) => void | Promise<void>;
  onClose?: () => void;
}

export function getPathBaseName(rawPath: string): string {
  const normalized = rawPath.replace(/\/+$/, '');
  if (!normalized) {
    return '/';
  }
  const idx = normalized.lastIndexOf('/');
  return idx >= 0 ? normalized.slice(idx + 1) : normalized;
}

export function splitPartialExternalPath(
  raw: string,
): { parentDir: string; filterQuery: string } | null {
  const lastSlash = raw.lastIndexOf('/');
  if (lastSlash < 0) {
    return null;
  }
  const parentDir = lastSlash === 0 ? '/' : raw.slice(0, lastSlash);
  const filterQuery = raw.slice(lastSlash + 1);
  if (!parentDir || !filterQuery) {
    return null;
  }
  return { parentDir, filterQuery };
}

export function matchesBookmarkQuery(bookmark: ExternalBookmark, lowerQuery: string): boolean {
  if (!lowerQuery) {
    return true;
  }
  const alias = (bookmark.alias ?? '').toLowerCase();
  const baseName = getPathBaseName(bookmark.path).toLowerCase();
  const fullPath = bookmark.path.toLowerCase();
  return (
    alias.includes(lowerQuery) || baseName.includes(lowerQuery) || fullPath.includes(lowerQuery)
  );
}

export function sortBookmarks(items: ExternalBookmark[]): ExternalBookmark[] {
  return [...items].sort((a, b) => {
    const pinnedDiff = Number(Boolean(b.pinned)) - Number(Boolean(a.pinned));
    if (pinnedDiff !== 0) {
      return pinnedDiff;
    }
    const openedDiff = (b.lastOpenedAt ?? 0) - (a.lastOpenedAt ?? 0);
    if (openedDiff !== 0) {
      return openedDiff;
    }
    return a.path.localeCompare(b.path);
  });
}

function mapSearchResultItem(item: SearchResultItem): QuickOpenResultItem | null {
  if (item.source === 'spotlight') {
    return null;
  }
  return {
    id: `${item.source}:${item.key}:${item.absPath}`,
    kind: item.kind,
    title: item.title,
    subtitle: item.subtitle,
    absPath: item.absPath,
    relPath: item.relPath,
    source: item.source,
    score: item.score,
    isDir: item.kind === 'dir',
  };
}

function mapDirectoryEntryItem(
  entry: ExternalDirectoryEntry,
  idPrefix: string,
  useRelSubtitle: boolean,
): QuickOpenResultItem | null {
  if (entry.kind !== 'file' && entry.kind !== 'directory') {
    return null;
  }
  const isDir = entry.kind === 'directory';
  return {
    id: `${idPrefix}:${entry.kind}:${entry.path}`,
    kind: isDir ? 'dir' : 'file',
    title: entry.name,
    subtitle: useRelSubtitle ? entry.relPath || entry.path : entry.path,
    absPath: entry.path,
    relPath: entry.relPath || null,
    source: 'path',
    score: entry.matchScore,
    isDir,
    isHidden: entry.isHidden,
  };
}

function flattenTagNodes(nodes: TagNode[]): Array<{ tag: string; count: number }> {
  const flat: Array<{ tag: string; count: number }> = [];
  const seen = new Set<string>();
  const walk = (list: TagNode[]) => {
    for (const node of list) {
      if (!seen.has(node.tag)) {
        seen.add(node.tag);
        flat.push({ tag: node.tag, count: node.count });
      }
      if (node.children.length > 0) {
        walk(node.children);
      }
    }
  };
  walk(nodes);
  return flat;
}

function extractErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return '检索失败，请稍后重试。';
}

export function useQuickOpen(options: UseQuickOpenOptions = {}) {
  const debounceMs = options.debounceMs ?? QUICKOPEN_DEBOUNCE_MS;

  const query = ref<string>('');
  const items = ref<QuickOpenResultItem[]>([]);
  const selectedIndex = ref<number>(0);
  const loading = ref<boolean>(false);
  const errorMessage = ref<string | null>(null);
  const externalBookmarks = ref<ExternalBookmark[]>([]);
  const directoryStack = ref<QuickOpenDirectoryScope[]>([]);
  const pathProbeResult = ref<ExternalPathProbeResult | null>(null);

  const parsedQuery = computed<ParsedQuickOpenQuery>(() => parseQuickOpenQuery(query.value));
  const activeDirectoryScope = computed<QuickOpenDirectoryScope | null>(() => {
    if (directoryStack.value.length === 0) {
      return null;
    }
    return directoryStack.value[directoryStack.value.length - 1] ?? null;
  });
  const selectedItem = computed<QuickOpenResultItem | null>(
    () => items.value[selectedIndex.value] ?? null,
  );

  let activeRequestId = 0;
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingSearchPromise: Promise<void> = Promise.resolve();

  function trackSearch(reqId: number): Promise<void> {
    const run = executeSearch(reqId);
    pendingSearchPromise = run;
    return run;
  }

  function waitForPendingSearch(): Promise<void> {
    return pendingSearchPromise;
  }

  function clearDebounceTimer(): void {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  function moveSelection(delta: 1 | -1): void {
    const len = items.value.length;
    if (len === 0) {
      selectedIndex.value = 0;
      return;
    }
    selectedIndex.value = (selectedIndex.value + delta + len) % len;
  }

  function setSelectedIndex(index: number): void {
    if (items.value.length === 0) {
      selectedIndex.value = 0;
      return;
    }
    if (index < 0) {
      selectedIndex.value = 0;
    } else if (index >= items.value.length) {
      selectedIndex.value = items.value.length - 1;
    } else {
      selectedIndex.value = index;
    }
  }

  function filterActions(term: string): QuickOpenResultItem[] {
    const rawActions = options.getActionItems?.() ?? [];
    const lower = term.toLowerCase();
    return rawActions
      .filter((action) => {
        if (!lower) {
          return true;
        }
        return (
          action.title.toLowerCase().includes(lower) ||
          action.id.toLowerCase().includes(lower) ||
          (action.shortcutLabel?.toLowerCase().includes(lower) ?? false)
        );
      })
      .map((action, index) => ({
        id: `action:${action.id}`,
        kind: 'action' as const,
        title: action.title,
        subtitle: action.subtitle ?? action.id,
        absPath: null,
        relPath: null,
        source: 'action' as const,
        score: 1000 - index,
        actionId: action.id,
        shortcutLabel: action.shortcutLabel,
        disabled: action.disabled,
      }));
  }

  function buildBookmarkResultItems(trimmedQuery: string): QuickOpenResultItem[] {
    const lower = trimmedQuery.toLowerCase();
    const sorted = sortBookmarks(externalBookmarks.value);
    const fileItems: QuickOpenResultItem[] = [];
    const dirItems: QuickOpenResultItem[] = [];

    for (const [idx, bookmark] of sorted.entries()) {
      if (!matchesBookmarkQuery(bookmark, lower)) {
        continue;
      }
      const title = bookmark.alias?.trim() || getPathBaseName(bookmark.path);
      if (bookmark.kind === 'file') {
        fileItems.push({
          id: `file-bookmark:${bookmark.path}`,
          kind: 'file',
          title,
          subtitle: bookmark.path,
          absPath: bookmark.path,
          relPath: null,
          source: 'file_bookmark',
          score: 600 - idx,
          isDir: false,
          pinned: Boolean(bookmark.pinned),
        });
      } else {
        dirItems.push({
          id: `dir-bookmark:${bookmark.path}`,
          kind: 'dir',
          title,
          subtitle: bookmark.path,
          absPath: bookmark.path,
          relPath: null,
          source: 'dir_bookmark',
          score: 500 - idx,
          isDir: true,
          pinned: Boolean(bookmark.pinned),
        });
      }
    }

    return [...fileItems, ...dirItems];
  }

  async function executeSearch(reqId: number): Promise<void> {
    if (reqId !== activeRequestId) {
      return;
    }

    const scope = activeDirectoryScope.value;
    const parsed = parseQuickOpenQuery(query.value);
    errorMessage.value = null;

    if (!scope && parsed.mode === 'action') {
      pathProbeResult.value = null;
      loading.value = false;
      items.value = filterActions(parsed.term);
      selectedIndex.value = 0;
      return;
    }

    loading.value = true;

    try {
      // 1. Directory bookmark / directory drill-down scope
      if (scope) {
        pathProbeResult.value = null;
        const trimmed = query.value.trim();
        const listing = await listQuickOpenExternalDirectory(scope.dirPath, {
          query: trimmed.length > 0 ? trimmed : null,
          includeHidden: true,
        });
        if (reqId !== activeRequestId) {
          return;
        }
        items.value = listing.entries
          .map((entry) => mapDirectoryEntryItem(entry, 'dir-scope', true))
          .filter((item): item is QuickOpenResultItem => item !== null);
        selectedIndex.value = 0;
        return;
      }

      // 2. Direct path mode (`/` or `~/`)
      if (parsed.mode === 'path') {
        const trimmed = parsed.term.trim();
        const probe = await probeQuickOpenExternalPath(trimmed);
        if (reqId !== activeRequestId) {
          return;
        }
        pathProbeResult.value = probe;

        if (probe.exists && probe.kind === 'file' && probe.readable) {
          const fileName = getPathBaseName(probe.resolvedPath);
          items.value = [
            {
              id: `path-direct:${probe.resolvedPath}`,
              kind: 'file',
              title: `直接打开外部文件：${fileName}`,
              subtitle: probe.resolvedPath,
              absPath: probe.resolvedPath,
              relPath: null,
              source: 'path',
              score: 1000,
              isDir: false,
              isHidden: probe.isHidden,
            },
          ];
          selectedIndex.value = 0;
          return;
        }

        if (probe.exists && probe.kind === 'directory' && probe.readable) {
          const listing = await listQuickOpenExternalDirectory(probe.resolvedPath, {
            query: null,
            includeHidden: true,
          });
          if (reqId !== activeRequestId) {
            return;
          }
          items.value = listing.entries
            .map((entry) => mapDirectoryEntryItem(entry, 'path', false))
            .filter((item): item is QuickOpenResultItem => item !== null);
          selectedIndex.value = 0;
          return;
        }

        const partial = splitPartialExternalPath(trimmed);
        if (partial) {
          const listing = await Promise.resolve(
            listQuickOpenExternalDirectory(partial.parentDir, {
              query: partial.filterQuery,
              includeHidden: true,
            }),
          ).catch(() => null);
          if (reqId !== activeRequestId) {
            return;
          }
          items.value = (listing?.entries ?? [])
            .map((entry) => mapDirectoryEntryItem(entry, 'path', false))
            .filter((item): item is QuickOpenResultItem => item !== null);
          selectedIndex.value = 0;
          return;
        }

        items.value = [];
        selectedIndex.value = 0;
        return;
      }

      pathProbeResult.value = null;

      if (parsed.mode === 'fulltext') {
        const res = await fetchQuickOpenNotesAndFiles({
          query: parsed.term,
          limit: 40,
          requestId: reqId,
        });
        if (reqId !== activeRequestId || res.requestId !== activeRequestId) {
          return;
        }
        items.value = res.items
          .filter((item) => item.source === 'library')
          .map(mapSearchResultItem)
          .filter((item): item is QuickOpenResultItem => item !== null);
        selectedIndex.value = 0;
        return;
      }

      if (parsed.mode === 'tag') {
        const tree = await fetchQuickOpenTreeAndTags();
        if (reqId !== activeRequestId) {
          return;
        }
        const allTags = flattenTagNodes(tree.tags);
        const lower = parsed.term.toLowerCase();
        const matchedTags = allTags.filter((entry) =>
          lower ? entry.tag.toLowerCase().includes(lower) : true,
        );

        const exactTag = lower
          ? allTags.find((entry) => entry.tag.toLowerCase() === lower)
          : undefined;
        let noteItems: QuickOpenResultItem[] = [];
        if (exactTag) {
          const taggedNotes = await fetchQuickOpenNotesList({
            tab: 'tags',
            dirRelPath: null,
            tags: [exactTag.tag],
            workspaceKey: null,
            filterText: null,
          });
          if (reqId !== activeRequestId) {
            return;
          }
          noteItems = taggedNotes.map((note, idx) => ({
            id: `tag-note:${note.key}`,
            kind: 'note' as const,
            title: note.title || note.relPath || note.absPath,
            subtitle: note.relPath ?? note.absPath,
            absPath: note.absPath,
            relPath: note.relPath,
            source: 'library' as const,
            score: 1000 - idx,
          }));
        }

        if (exactTag && noteItems.length > 0) {
          const remainingTags = matchedTags.filter((entry) => entry.tag.toLowerCase() !== lower);
          const tagItems: QuickOpenResultItem[] = remainingTags.map((entry, idx) => ({
            id: `tag:${entry.tag}`,
            kind: 'tag',
            title: `#${entry.tag}`,
            subtitle: `${entry.count} 篇笔记`,
            absPath: null,
            relPath: null,
            source: 'tag',
            score: 500 - idx,
            tagName: entry.tag,
          }));
          items.value = [...noteItems, ...tagItems];
        } else {
          const tagItems: QuickOpenResultItem[] = matchedTags.map((entry, idx) => ({
            id: `tag:${entry.tag}`,
            kind: 'tag',
            title: `#${entry.tag}`,
            subtitle: `${entry.count} 篇笔记`,
            absPath: null,
            relPath: null,
            source: 'tag',
            score: 1000 - idx,
            tagName: entry.tag,
          }));
          items.value = [...tagItems, ...noteItems];
        }

        selectedIndex.value = 0;
        return;
      }

      // 3. Default mixed mode: vault notes + external file bookmarks + external directory bookmarks
      const [localRes, latestBookmarks] = await Promise.all([
        fetchQuickOpenNotesAndFiles({
          query: parsed.term,
          limit: 30,
          requestId: reqId,
        }),
        fetchQuickOpenExternalBookmarks().catch(() => null),
      ]);
      if (reqId !== activeRequestId || localRes.requestId !== activeRequestId) {
        return;
      }

      if (latestBookmarks !== null) {
        externalBookmarks.value = latestBookmarks;
      }

      const localItems = localRes.items
        .map(mapSearchResultItem)
        .filter((item): item is QuickOpenResultItem => item !== null);

      const bookmarkItems = buildBookmarkResultItems(parsed.term);
      const existingPaths = new Set(
        localItems
          .map((item) => item.absPath)
          .filter((p): p is string => typeof p === 'string' && p.length > 0),
      );

      const merged = [...localItems];
      for (const bmItem of bookmarkItems) {
        if (bmItem.absPath && existingPaths.has(bmItem.absPath)) {
          continue;
        }
        if (bmItem.absPath) {
          existingPaths.add(bmItem.absPath);
        }
        merged.push(bmItem);
      }

      items.value = merged;
      selectedIndex.value = 0;
    } catch (error) {
      if (reqId === activeRequestId) {
        errorMessage.value = extractErrorMessage(error);
      }
    } finally {
      if (reqId === activeRequestId) {
        loading.value = false;
      }
    }
  }

  function setQuery(nextQuery: string, immediate = false): void {
    query.value = nextQuery;
    clearDebounceTimer();
    const reqId = ++activeRequestId;

    if (
      immediate ||
      (!activeDirectoryScope.value && parseQuickOpenQuery(nextQuery).mode === 'action')
    ) {
      void trackSearch(reqId);
      return;
    }

    debounceTimer = setTimeout(() => {
      debounceTimer = null;
      void trackSearch(reqId);
    }, debounceMs);
  }

  function enterDirectory(dirPath: string, title?: string): void {
    directoryStack.value = [
      ...directoryStack.value,
      { dirPath, title: title?.trim() || getPathBaseName(dirPath) },
    ];
    selectedIndex.value = 0;
    setQuery('', true);
  }

  function exitDirectory(): boolean {
    if (directoryStack.value.length === 0) {
      return false;
    }
    directoryStack.value = directoryStack.value.slice(0, -1);
    selectedIndex.value = 0;
    setQuery('', true);
    return true;
  }

  function refresh(immediate = true): void {
    setQuery(query.value, immediate);
  }

  function reset(): void {
    clearDebounceTimer();
    activeRequestId += 1;
    query.value = '';
    items.value = [];
    selectedIndex.value = 0;
    loading.value = false;
    errorMessage.value = null;
    directoryStack.value = [];
    pathProbeResult.value = null;
  }

  async function handleTabComplete(): Promise<boolean> {
    if (
      activeDirectoryScope.value !== null ||
      parsedQuery.value.mode !== 'path' ||
      items.value.length === 0
    ) {
      return false;
    }

    if (items.value.length === 1) {
      const only = items.value[0];
      if (!only?.absPath) {
        return false;
      }
      const nextPrefix = only.isDir
        ? only.absPath.endsWith('/')
          ? only.absPath
          : `${only.absPath}/`
        : only.absPath;
      setQuery(nextPrefix, true);
      return true;
    }

    const paths = items.value
      .map((item) => item.absPath)
      .filter((p): p is string => typeof p === 'string' && p.length > 0);
    const lcp = computeLongestCommonPrefix(paths);
    if (lcp && lcp !== parsedQuery.value.term) {
      setQuery(lcp, true);
      return true;
    }
    return false;
  }

  async function revealItem(item: QuickOpenResultItem | null = selectedItem.value): Promise<void> {
    if (!item?.absPath) {
      return;
    }
    if (options.onRevealItem) {
      await options.onRevealItem(item);
      return;
    }
    await runQuickOpenExternalAction({
      id: 'external.reveal',
      absPath: item.absPath,
      dirPath: null,
    });
  }

  async function confirmItem(
    item: QuickOpenResultItem | null = selectedItem.value,
    openMode: QuickOpenOpenMode = 'open',
  ): Promise<void> {
    let target = item;
    if (!target && !activeDirectoryScope.value && parsedQuery.value.mode === 'path') {
      clearDebounceTimer();
      const reqId = ++activeRequestId;
      await executeSearch(reqId);
      target = items.value[0] ?? null;
    }
    if (!target) {
      return;
    }
    if (target.kind === 'action') {
      if (!target.disabled && target.actionId) {
        await options.onExecuteAction?.(target.actionId);
      }
      return;
    }
    if (target.kind === 'tag' && target.tagName) {
      if (options.onSelectTag) {
        await options.onSelectTag(target.tagName);
      } else {
        setQuery(`@${target.tagName}`, true);
      }
      return;
    }
    if (target.kind === 'dir' && target.absPath) {
      enterDirectory(target.absPath, target.title);
      return;
    }
    await options.onOpenItem?.(target, openMode);
  }

  function handleKeydown(event: KeyboardEvent): boolean {
    if (event.isComposing || event.keyCode === 229) {
      return false;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      event.stopPropagation?.();
      moveSelection(1);
      return true;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      event.stopPropagation?.();
      moveSelection(-1);
      return true;
    }
    if (
      event.key === 'Backspace' &&
      query.value.length === 0 &&
      activeDirectoryScope.value !== null
    ) {
      event.preventDefault();
      event.stopPropagation?.();
      exitDirectory();
      return true;
    }
    if (
      event.key === 'Tab' &&
      activeDirectoryScope.value === null &&
      parsedQuery.value.mode === 'path'
    ) {
      event.preventDefault();
      event.stopPropagation?.();
      void handleTabComplete();
      return true;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation?.();
      if (event.altKey) {
        void revealItem(selectedItem.value);
        return true;
      }
      const openMode: QuickOpenOpenMode = event.metaKey ? 'newTab' : 'open';
      void confirmItem(selectedItem.value, openMode);
      return true;
    }
    if (event.key === 'Escape' && options.onClose) {
      event.preventDefault();
      event.stopPropagation?.();
      options.onClose();
      return true;
    }

    return false;
  }

  function dispose(): void {
    clearDebounceTimer();
    activeRequestId += 1;
  }

  return {
    query,
    parsedQuery,
    items,
    selectedIndex,
    selectedItem,
    loading,
    errorMessage,
    externalBookmarks,
    directoryStack,
    activeDirectoryScope,
    pathProbeResult,
    setQuery,
    enterDirectory,
    exitDirectory,
    refresh,
    reset,
    setSelectedIndex,
    moveSelection,
    handleTabComplete,
    revealItem,
    confirmItem,
    handleKeydown,
    waitForPendingSearch,
    dispose,
  };
}
