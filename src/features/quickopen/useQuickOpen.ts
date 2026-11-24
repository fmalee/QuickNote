import { computed, ref } from 'vue';
import type { SearchResultItem, SearchSource, TagNode } from '@/bindings/types';
import {
  fetchQuickOpenNotesAndFiles,
  fetchQuickOpenNotesList,
  fetchQuickOpenPathComplete,
  fetchQuickOpenSpotlight,
  fetchQuickOpenTreeAndTags,
  runQuickOpenExternalAction,
} from './api';
import {
  computeLongestCommonPrefix,
  parseQuickOpenQuery,
  type ParsedQuickOpenQuery,
} from './queryParser';

export const QUICKOPEN_DEBOUNCE_MS = 80;

export type QuickOpenOpenMode = 'open' | 'newTab';

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
  source: SearchSource | 'action' | 'tag';
  score: number;
  isDir?: boolean;
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
  onRevealItem?: (item: QuickOpenResultItem) => void | Promise<void>;
  onClose?: () => void;
}

function mapSearchResultItem(item: SearchResultItem): QuickOpenResultItem {
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
  const spotlightLoading = ref<boolean>(false);
  const errorMessage = ref<string | null>(null);

  const parsedQuery = computed<ParsedQuickOpenQuery>(() => parseQuickOpenQuery(query.value));
  const selectedItem = computed<QuickOpenResultItem | null>(
    () => items.value[selectedIndex.value] ?? null,
  );

  let activeRequestId = 0;
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  function clearDebounceTimer(): void {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  function clampSelectedIndex(): void {
    if (items.value.length === 0) {
      selectedIndex.value = 0;
      return;
    }
    if (selectedIndex.value >= items.value.length) {
      selectedIndex.value = items.value.length - 1;
    } else if (selectedIndex.value < 0) {
      selectedIndex.value = 0;
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

  async function executeSearch(reqId: number): Promise<void> {
    if (reqId !== activeRequestId) {
      return;
    }

    const parsed = parseQuickOpenQuery(query.value);
    errorMessage.value = null;

    if (parsed.mode === 'action') {
      loading.value = false;
      spotlightLoading.value = false;
      items.value = filterActions(parsed.term);
      selectedIndex.value = 0;
      return;
    }

    loading.value = true;
    spotlightLoading.value = false;

    try {
      if (parsed.mode === 'path') {
        const entries = await fetchQuickOpenPathComplete({
          prefix: parsed.term,
          requestId: reqId,
        });
        if (reqId !== activeRequestId) {
          return;
        }
        items.value = entries.map((entry, idx) => ({
          id: `path:${entry.absPath}`,
          kind: entry.isDir ? 'dir' : 'file',
          title: entry.name,
          subtitle: entry.absPath,
          absPath: entry.absPath,
          relPath: null,
          source: 'path',
          score: 500 - idx,
          isDir: entry.isDir,
        }));
        selectedIndex.value = 0;
        return;
      }

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
          .map(mapSearchResultItem);
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

        const nextItems: QuickOpenResultItem[] = matchedTags.map((entry, idx) => ({
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

        const exactTag = allTags.find((entry) => entry.tag.toLowerCase() === lower);
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
          for (const note of taggedNotes) {
            nextItems.push({
              id: `tag-note:${note.key}`,
              kind: 'note',
              title: note.title || note.relPath || note.absPath,
              subtitle: note.relPath ?? note.absPath,
              absPath: note.absPath,
              relPath: note.relPath,
              source: 'library',
              score: 500,
            });
          }
        }

        items.value = nextItems;
        selectedIndex.value = 0;
        return;
      }

      // mode === 'all'
      const localRes = await fetchQuickOpenNotesAndFiles({
        query: parsed.term,
        limit: 30,
        requestId: reqId,
      });
      if (reqId !== activeRequestId || localRes.requestId !== activeRequestId) {
        return;
      }

      const localItems = localRes.items.map(mapSearchResultItem);
      items.value = localItems;
      selectedIndex.value = 0;
      loading.value = false;

      if (parsed.term.length >= 2) {
        spotlightLoading.value = true;
        try {
          const spotRes = await fetchQuickOpenSpotlight({
            query: parsed.term,
            requestId: reqId,
          });
          if (reqId !== activeRequestId || spotRes.requestId !== activeRequestId) {
            return;
          }
          const existingPaths = new Set(
            items.value
              .map((item) => item.absPath)
              .filter((p): p is string => typeof p === 'string' && p.length > 0),
          );
          const merged = [...items.value];
          for (const spotItem of spotRes.items) {
            if (spotItem.absPath && existingPaths.has(spotItem.absPath)) {
              continue;
            }
            if (spotItem.absPath) {
              existingPaths.add(spotItem.absPath);
            }
            merged.push(mapSearchResultItem(spotItem));
          }
          items.value = merged;
          clampSelectedIndex();
        } finally {
          if (reqId === activeRequestId) {
            spotlightLoading.value = false;
          }
        }
      }
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

    if (immediate || parseQuickOpenQuery(nextQuery).mode === 'action') {
      void executeSearch(reqId);
      return;
    }

    debounceTimer = setTimeout(() => {
      debounceTimer = null;
      void executeSearch(reqId);
    }, debounceMs);
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
    spotlightLoading.value = false;
    errorMessage.value = null;
  }

  async function handleTabComplete(): Promise<boolean> {
    if (parsedQuery.value.mode !== 'path' || items.value.length === 0) {
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
    if (!item) {
      return;
    }
    if (item.kind === 'action') {
      if (!item.disabled && item.actionId) {
        await options.onExecuteAction?.(item.actionId);
      }
      return;
    }
    if (item.kind === 'tag' && item.tagName) {
      setQuery(`@${item.tagName}`, true);
      return;
    }
    if (item.kind === 'dir' && item.absPath) {
      const dirPrefix = item.absPath.endsWith('/') ? item.absPath : `${item.absPath}/`;
      setQuery(dirPrefix, true);
      return;
    }
    await options.onOpenItem?.(item, openMode);
  }

  function handleKeydown(event: KeyboardEvent): boolean {
    if (event.isComposing || event.keyCode === 229) {
      return false;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      moveSelection(1);
      return true;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      moveSelection(-1);
      return true;
    }
    if (event.key === 'Tab' && parsedQuery.value.mode === 'path') {
      event.preventDefault();
      void handleTabComplete();
      return true;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
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
    spotlightLoading,
    errorMessage,
    setQuery,
    refresh,
    reset,
    setSelectedIndex,
    moveSelection,
    handleTabComplete,
    revealItem,
    confirmItem,
    handleKeydown,
    dispose,
  };
}
