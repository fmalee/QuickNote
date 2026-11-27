import {
  SearchQuery,
  findNext,
  findPrevious,
  replaceAll,
  replaceNext,
  search,
  searchKeymap,
  setSearchQuery,
} from '@codemirror/search';
import type { EditorState, Extension } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';

export interface SearchOptionsState {
  search: string;
  replace: string;
  caseSensitive: boolean;
  wholeWord: boolean;
  regexp: boolean;
}

export interface SearchMatchSummary {
  total: number;
  current: number;
  valid: boolean;
  capped: boolean;
}

export const MAX_SEARCH_MATCH_COUNT = 999;

/** 查找匹配高亮样式：只用 --qn-* Token（C7.1）。 */
export const qnSearchPanelTheme = EditorView.theme({
  '.cm-searchMatch': { backgroundColor: 'var(--qn-accent-soft)' },
  '.cm-searchMatch.cm-searchMatch-selected': {
    backgroundColor: 'var(--qn-bg-active)',
    outline: '1px solid var(--qn-accent)',
  },
});

function createHiddenSearchPanelHost(): HTMLElement {
  const el = document.createElement('div');
  el.className = 'qn-cm-search-host';
  el.style.display = 'none';
  return el;
}

/** 校验并构造 CodeMirror SearchQuery，非法正则时返回 valid=false。 */
export function validateSearchQuery(opts: SearchOptionsState): {
  valid: boolean;
  query: SearchQuery | null;
} {
  if (opts.regexp && opts.search.length > 0) {
    try {
      new RegExp(opts.search);
    } catch {
      return { valid: false, query: null };
    }
  }
  const query = new SearchQuery({
    search: opts.search,
    replace: opts.replace,
    caseSensitive: opts.caseSensitive,
    wholeWord: opts.wholeWord,
    regexp: opts.regexp,
  });
  return { valid: true, query };
}

/** 将搜索条件同步到 EditorView；若正则非法则保持上次高亮不变并返回 valid=false。 */
export function applySearchQuery(view: EditorView, opts: SearchOptionsState): { valid: boolean } {
  const { valid, query } = validateSearchQuery(opts);
  if (!valid || !query) {
    return { valid: false };
  }
  view.dispatch({ effects: setSearchQuery.of(query) });
  return { valid: true };
}

/** 清空当前搜索查询，移除高亮。 */
export function clearSearchQuery(view: EditorView): void {
  view.dispatch({ effects: setSearchQuery.of(new SearchQuery({ search: '' })) });
}

/** 统计当前文档的匹配总数与当前选区命中的匹配序号（1-based，上限 999+）。 */
export function computeMatchSummary(
  state: EditorState,
  opts: SearchOptionsState,
): SearchMatchSummary {
  if (!opts.search) {
    return { total: 0, current: 0, valid: true, capped: false };
  }
  const { valid, query } = validateSearchQuery(opts);
  if (!valid || !query) {
    return { total: 0, current: 0, valid: false, capped: false };
  }

  const sel = state.selection.main;
  const cursor = query.getCursor(state);
  let total = 0;
  let current = 0;
  let capped = false;

  for (let next = cursor.next(); !next.done; next = cursor.next()) {
    total += 1;
    if (total > MAX_SEARCH_MATCH_COUNT) {
      total = MAX_SEARCH_MATCH_COUNT;
      capped = true;
      break;
    }
    const { from, to } = next.value;
    if (current === 0 && sel.from === from && sel.to === to) {
      current = total;
    }
  }

  return { total, current, valid: true, capped };
}

/** 获取编辑器当前单行选区文本作为查找初始词。 */
export function getSelectedSearchSeed(view: EditorView): string | null {
  const main = view.state.selection.main;
  if (main.empty) {
    return null;
  }
  const text = view.state.sliceDoc(main.from, main.to);
  if (!text || text.includes('\n') || text.includes('\r') || text.length > 200) {
    return null;
  }
  return text;
}

export function findNextMatch(view: EditorView): boolean {
  return findNext(view);
}

export function findPrevMatch(view: EditorView): boolean {
  return findPrevious(view);
}

export function replaceCurrentMatch(view: EditorView): boolean {
  return replaceNext(view);
}

export function replaceAllMatches(view: EditorView): boolean {
  return replaceAll(view);
}

/**
 * 查找扩展集合。内置键位里去掉 `Mod-f`（由统一动作注册表的 `editor.find` 接管），
 * 使用零高度空宿主接管默认面板 DOM，由 Vue 悬浮组件渲染 VS Code 风格查找/替换条。
 */
export function createSearchExtensions(): Extension[] {
  const findKeymap = searchKeymap.filter((binding) => binding.key !== 'Mod-f');
  return [
    search({
      top: true,
      createPanel: () => ({ dom: createHiddenSearchPanelHost() }),
    }),
    qnSearchPanelTheme,
    keymap.of(findKeymap),
  ];
}
