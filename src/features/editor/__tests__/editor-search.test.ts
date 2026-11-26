import { searchKeymap } from '@codemirror/search';
import { EditorState } from '@codemirror/state';
import { keymap } from '@codemirror/view';
import { describe, expect, it } from 'vitest';
import { createSearchExtensions } from '../extensions/search';

describe('editor-search 扩展', () => {
  it('createSearchExtensions 返回非空扩展数组，且过滤掉 Mod-f 绑定', () => {
    expect(searchKeymap.some((binding) => binding.key === 'Mod-f')).toBe(true);

    const extensions = createSearchExtensions();
    expect(extensions.length).toBeGreaterThan(0);

    const state = EditorState.create({ doc: 'hello', extensions });
    const bindings = state.facet(keymap).flat();
    expect(bindings.some((binding) => binding.key === 'Mod-f')).toBe(false);
    expect(bindings.some((binding) => binding.key === 'Mod-g')).toBe(true);
  });
});
