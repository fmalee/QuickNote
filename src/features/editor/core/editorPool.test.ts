import { describe, expect, it, vi } from 'vitest';
import { createDocTabStub, type DocTabStub } from './docSession';
import {
  DEFAULT_EDITOR_STATE_POOL_CAPACITY,
  EditorStatePool,
  MIN_EDITOR_STATE_POOL_CAPACITY,
} from './editorPool';

interface MockEditorState {
  cursorOffset: number;
  scrollTop: number;
  revision: number;
}

describe('EditorStatePool', () => {
  it('defaults capacity to 5 and clamps setCapacity to a minimum of 2', () => {
    const pool = new EditorStatePool<MockEditorState>();
    expect(DEFAULT_EDITOR_STATE_POOL_CAPACITY).toBe(5);
    expect(MIN_EDITOR_STATE_POOL_CAPACITY).toBe(2);
    expect(pool.getCapacity()).toBe(5);

    expect(pool.setCapacity(1)).toBe(2);
    expect(pool.getCapacity()).toBe(2);

    expect(pool.setCapacity(0)).toBe(2);
    expect(pool.setCapacity(-10)).toBe(2);
    expect(pool.setCapacity(3.9)).toBe(3);
    expect(pool.getCapacity()).toBe(3);

    expect(pool.setCapacity(Number.NaN)).toBe(5);
    expect(pool.getCapacity()).toBe(5);

    const lowInitPool = new EditorStatePool<MockEditorState>(1);
    expect(lowInitPool.getCapacity()).toBe(2);
  });

  it('tracks LRU order across set, get, touch, delete, and clear', () => {
    const pool = new EditorStatePool<MockEditorState>(3);
    pool.set('a', { cursorOffset: 1, scrollTop: 10, revision: 1 });
    pool.set('b', { cursorOffset: 2, scrollTop: 20, revision: 1 });
    pool.set('c', { cursorOffset: 3, scrollTop: 30, revision: 1 });

    expect(pool.size()).toBe(3);
    expect(pool.keys()).toEqual(['a', 'b', 'c']);

    // get() 刷新访问顺序
    expect(pool.get('a')).toEqual({ cursorOffset: 1, scrollTop: 10, revision: 1 });
    expect(pool.keys()).toEqual(['b', 'c', 'a']);

    // touch() 刷新访问顺序，不存在的 key 返回 false
    expect(pool.touch('b')).toBe(true);
    expect(pool.touch('missing')).toBe(false);
    expect(pool.keys()).toEqual(['c', 'a', 'b']);

    // set() 覆写已存在 key 同样移至队尾
    pool.set('c', { cursorOffset: 30, scrollTop: 300, revision: 2 });
    expect(pool.keys()).toEqual(['a', 'b', 'c']);
    expect(pool.get('c')?.revision).toBe(2);

    // delete() 与 clear()
    expect(pool.delete('b')).toBe(true);
    expect(pool.delete('b')).toBe(false);
    expect(pool.size()).toBe(2);

    pool.clear();
    expect(pool.size()).toBe(0);
    expect(pool.get('a')).toBeUndefined();
  });

  it('evicts least recently used clean non-active entries on overflow', () => {
    const pool = new EditorStatePool<MockEditorState>(3);
    pool.set('t1', { cursorOffset: 10, scrollTop: 100, revision: 1 });
    pool.set('t2', { cursorOffset: 20, scrollTop: 200, revision: 1 });
    pool.set('t3', { cursorOffset: 30, scrollTop: 300, revision: 1 });
    pool.set('t4', { cursorOffset: 40, scrollTop: 400, revision: 1 });
    pool.set('t5', { cursorOffset: 50, scrollTop: 500, revision: 1 });

    // 访问 t1 使其成为最近使用，当前 LRU 顺序: t2, t3, t4, t5, t1
    pool.touch('t1');

    const evicted = pool.evictOverflow('t1', () => false);
    expect(evicted).toEqual(['t2', 't3']);
    expect(pool.size()).toBe(3);
    expect(pool.keys()).toEqual(['t4', 't5', 't1']);
  });

  it('never evicts the active tab even when it is least recently used', () => {
    const pool = new EditorStatePool<MockEditorState>(2);
    pool.set('active', { cursorOffset: 5, scrollTop: 50, revision: 1 });
    pool.set('older-clean', { cursorOffset: 15, scrollTop: 150, revision: 1 });
    pool.set('newer-clean', { cursorOffset: 25, scrollTop: 250, revision: 1 });

    // active 处于队头（最久未访问），必须跳过 active 淘汰 older-clean
    const evicted = pool.evictOverflow('active', () => false);
    expect(evicted).toEqual(['older-clean']);
    expect(pool.has('active')).toBe(true);
    expect(pool.has('newer-clean')).toBe(true);
    expect(pool.size()).toBe(2);
  });

  it('never evicts dirty tabs and allows temporary overflow until dirtiness clears', () => {
    const pool = new EditorStatePool<MockEditorState>(2);
    const dirtySet = new Set<string>(['dirty-1', 'dirty-2']);

    pool.set('dirty-1', { cursorOffset: 1, scrollTop: 10, revision: 1 });
    pool.set('dirty-2', { cursorOffset: 2, scrollTop: 20, revision: 1 });
    pool.set('clean-1', { cursorOffset: 3, scrollTop: 30, revision: 1 });
    pool.set('active', { cursorOffset: 4, scrollTop: 40, revision: 1 });

    // 仅 clean-1 可淘汰；其余 3 个（2 个脏 + 1 个激活）受保护，池大小保持为 3
    const firstEvict = pool.evictOverflow('active', (key) => dirtySet.has(key));
    expect(firstEvict).toEqual(['clean-1']);
    expect(pool.size()).toBe(3);
    expect(pool.keys()).toEqual(['dirty-1', 'dirty-2', 'active']);

    // 再次触发淘汰不会误删任何受保护条目
    expect(pool.evictOverflow('active', (key) => dirtySet.has(key))).toEqual([]);
    expect(pool.size()).toBe(3);

    // 当 dirty-1 保存干净后，下一次淘汰会按 LRU 移除 dirty-1
    dirtySet.delete('dirty-1');
    const secondEvict = pool.evictOverflow('active', (key) => dirtySet.has(key));
    expect(secondEvict).toEqual(['dirty-1']);
    expect(pool.size()).toBe(2);
    expect(pool.keys()).toEqual(['dirty-2', 'active']);
  });

  it('evicts excess entries when shrinking capacity via setCapacity', () => {
    const pool = new EditorStatePool<MockEditorState>(5);
    for (let i = 1; i <= 5; i += 1) {
      pool.set(`tab-${i}`, { cursorOffset: i * 10, scrollTop: i * 100, revision: 1 });
    }
    expect(pool.size()).toBe(5);

    // 缩容到 2，保护激活的 tab-5 和脏的 tab-2
    pool.setCapacity(2);
    const evicted = pool.evictOverflow('tab-5', (key) => key === 'tab-2');
    expect(evicted).toEqual(['tab-1', 'tab-3', 'tab-4']);
    expect(pool.size()).toBe(2);
    expect(pool.keys()).toEqual(['tab-2', 'tab-5']);
  });

  it('records cursor and scroll snapshots onto DocTabStub in onEvict callback', () => {
    const pool = new EditorStatePool<MockEditorState>(2);
    const stubs = new Map<string, DocTabStub>([
      [
        'vault:notes/a.md',
        createDocTabStub({
          tabKey: 'vault:notes/a.md',
          kind: 'vault',
          path: 'notes/a.md',
          title: 'A',
          lastAccessedAt: 1000,
        }),
      ],
      [
        'external:/tmp/b.md',
        createDocTabStub({
          tabKey: 'external:/tmp/b.md',
          kind: 'external',
          path: '/tmp/b.md',
          title: 'B',
          lastAccessedAt: 2000,
        }),
      ],
      [
        'vault:notes/c.md',
        createDocTabStub({
          tabKey: 'vault:notes/c.md',
          kind: 'vault',
          path: 'notes/c.md',
          title: 'C',
          lastAccessedAt: 3000,
        }),
      ],
    ]);

    pool.set('vault:notes/a.md', { cursorOffset: 128, scrollTop: 420, revision: 1 });
    pool.set('external:/tmp/b.md', { cursorOffset: 64, scrollTop: 180, revision: 1 });
    pool.set('vault:notes/c.md', { cursorOffset: 12, scrollTop: 0, revision: 1 });

    const onEvict = vi.fn((key: string, state: MockEditorState) => {
      const stub = stubs.get(key);
      if (stub) {
        stub.cursorOffset = state.cursorOffset;
        stub.scrollTop = state.scrollTop;
        stub.evicted = true;
      }
    });

    const evicted = pool.evictOverflow(
      'vault:notes/c.md',
      (key) => stubs.get(key)?.isDirty ?? false,
      onEvict,
    );

    expect(evicted).toEqual(['vault:notes/a.md']);
    expect(onEvict).toHaveBeenCalledTimes(1);

    const evictedStub = stubs.get('vault:notes/a.md');
    expect(evictedStub).toEqual({
      tabKey: 'vault:notes/a.md',
      kind: 'vault',
      path: 'notes/a.md',
      title: 'A',
      isDirty: false,
      cursorOffset: 128,
      scrollTop: 420,
      lastAccessedAt: 1000,
      evicted: true,
    });
    expect(pool.has('vault:notes/a.md')).toBe(false);
  });
});
