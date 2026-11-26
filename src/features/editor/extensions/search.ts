import { openSearchPanel, search, searchKeymap } from '@codemirror/search';
import type { Extension } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';

/** 查找 / 替换面板样式：只用 --qn-* Token（C7.1）。 */
export const qnSearchPanelTheme = EditorView.theme({
  '.cm-panels': {
    backgroundColor: 'var(--qn-bg-elevated)',
    color: 'var(--qn-text-primary)',
    borderBottom: '1px solid var(--qn-border-subtle)',
    fontSize: 'var(--qn-text-sm)',
  },
  '.cm-panels.cm-panels-top': { borderBottom: '1px solid var(--qn-border-subtle)' },
  '.cm-search': {
    padding: 'var(--qn-space-2)',
    display: 'flex',
    flexWrap: 'wrap',
    gap: 'var(--qn-space-2)',
    alignItems: 'center',
  },
  '.cm-search input': {
    backgroundColor: 'var(--qn-bg-base)',
    color: 'var(--qn-text-primary)',
    border: '1px solid var(--qn-border-strong)',
    borderRadius: 'var(--qn-radius-sm)',
    padding: 'var(--qn-space-1) var(--qn-space-2)',
    font: 'inherit',
  },
  '.cm-search button': {
    backgroundColor: 'transparent',
    color: 'var(--qn-text-secondary)',
    border: '1px solid var(--qn-border-subtle)',
    borderRadius: 'var(--qn-radius-sm)',
    padding: 'var(--qn-space-1) var(--qn-space-2)',
    font: 'inherit',
  },
  '.cm-search button:hover': {
    backgroundColor: 'var(--qn-bg-hover)',
    color: 'var(--qn-text-primary)',
  },
  '.cm-searchMatch': { backgroundColor: 'var(--qn-accent-soft)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'var(--qn-bg-active)' },
});

/** 打开查找面板（⌘F）。 */
export function openFindPanel(view: EditorView): void {
  openSearchPanel(view);
}

/** 打开查找面板并聚焦「替换为」输入框（⌥⌘F）。 */
export function openReplacePanel(view: EditorView): void {
  openSearchPanel(view);
  const replaceInput = view.dom.querySelector<HTMLInputElement>('.cm-search input[name="replace"]');
  replaceInput?.focus();
}

/**
 * 查找扩展集合。内置键位里去掉 `Mod-f`（由统一动作注册表的 `editor.find` 接管），
 * 其余（⌘G 下一个、⇧⌘G 上一个、F3、回车下一个、⇧回车上一个、替换确认等）保留。
 */
export function createSearchExtensions(): Extension[] {
  const findKeymap = searchKeymap.filter((binding) => binding.key !== 'Mod-f');
  return [search({ top: true }), qnSearchPanelTheme, keymap.of(findKeymap)];
}
