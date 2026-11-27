import { searchKeymap } from '@codemirror/search';
import { EditorSelection, EditorState } from '@codemirror/state';
import { keymap } from '@codemirror/view';
import { describe, expect, it } from 'vitest';
import {
  computeMatchSummary,
  createSearchExtensions,
  validateSearchQuery,
  type SearchOptionsState,
} from '../extensions/search';

const baseOpts: SearchOptionsState = {
  search: '',
  replace: '',
  caseSensitive: false,
  wholeWord: false,
  regexp: false,
};

describe('editor-search 扩展与查询计算', () => {
  it('createSearchExtensions 返回非空扩展数组，且过滤掉 Mod-f 绑定', () => {
    expect(searchKeymap.some((binding) => binding.key === 'Mod-f')).toBe(true);

    const extensions = createSearchExtensions();
    expect(extensions.length).toBeGreaterThan(0);

    const state = EditorState.create({ doc: 'hello', extensions });
    const bindings = state.facet(keymap).flat();
    expect(bindings.some((binding) => binding.key === 'Mod-f')).toBe(false);
    expect(bindings.some((binding) => binding.key === 'Mod-g')).toBe(true);
  });

  it('computeMatchSummary 支持大小写、全字匹配与当前选区索引', () => {
    const doc = 'foo Foo fooBar foo';
    const state = EditorState.create({
      doc,
      selection: EditorSelection.single(15, 18),
    });

    expect(computeMatchSummary(state, { ...baseOpts, search: 'foo' })).toEqual({
      total: 4,
      current: 4,
      valid: true,
      capped: false,
    });

    expect(
      computeMatchSummary(state, {
        ...baseOpts,
        search: 'foo',
        caseSensitive: true,
        wholeWord: true,
      }),
    ).toEqual({
      total: 2,
      current: 2,
      valid: true,
      capped: false,
    });
  });

  it('非法正则表达式安全降级返回 valid=false 而不抛异常', () => {
    const invalidOpts: SearchOptionsState = {
      ...baseOpts,
      search: '[unclosed',
      regexp: true,
    };
    expect(validateSearchQuery(invalidOpts)).toEqual({ valid: false, query: null });

    const state = EditorState.create({ doc: 'hello world' });
    expect(computeMatchSummary(state, invalidOpts)).toEqual({
      total: 0,
      current: 0,
      valid: false,
      capped: false,
    });
  });
});
