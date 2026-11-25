export const MIN_EDITOR_STATE_POOL_CAPACITY = 2;
export const DEFAULT_EDITOR_STATE_POOL_CAPACITY = 5;

function normalizeCapacity(limit: number): number {
  if (!Number.isFinite(limit)) {
    return DEFAULT_EDITOR_STATE_POOL_CAPACITY;
  }
  return Math.max(MIN_EDITOR_STATE_POOL_CAPACITY, Math.floor(limit));
}

/**
 * 与框架无关的泛型 EditorState LRU 池。
 *
 * 内部利用 Map 插入序维护访问顺序（队头为最久未访问，队尾为最近访问）。
 * 淘汰时严格保护当前激活标签与脏数据标签，绝不淘汰 activeKey 或 isDirtyFn(key) === true 的条目。
 */
export class EditorStatePool<TState> {
  private readonly entries = new Map<string, TState>();
  private capacity: number;

  constructor(capacity: number = DEFAULT_EDITOR_STATE_POOL_CAPACITY) {
    this.capacity = normalizeCapacity(capacity);
  }

  getCapacity(): number {
    return this.capacity;
  }

  setCapacity(limit: number): number {
    this.capacity = normalizeCapacity(limit);
    return this.capacity;
  }

  get(key: string): TState | undefined {
    if (!this.entries.has(key)) {
      return undefined;
    }
    const state = this.entries.get(key) as TState;
    this.entries.delete(key);
    this.entries.set(key, state);
    return state;
  }

  has(key: string): boolean {
    return this.entries.has(key);
  }

  set(key: string, state: TState): void {
    if (this.entries.has(key)) {
      this.entries.delete(key);
    }
    this.entries.set(key, state);
  }

  touch(key: string): boolean {
    if (!this.entries.has(key)) {
      return false;
    }
    const state = this.entries.get(key) as TState;
    this.entries.delete(key);
    this.entries.set(key, state);
    return true;
  }

  delete(key: string): boolean {
    return this.entries.delete(key);
  }

  evictOverflow(
    activeKey: string | null | undefined,
    isDirtyFn: (key: string) => boolean,
    onEvict?: (key: string, state: TState) => void,
  ): string[] {
    if (this.entries.size <= this.capacity) {
      return [];
    }

    const evictedKeys: string[] = [];
    const candidates = Array.from(this.entries.keys());

    for (const key of candidates) {
      if (this.entries.size <= this.capacity) {
        break;
      }
      if (activeKey != null && key === activeKey) {
        continue;
      }
      if (isDirtyFn(key)) {
        continue;
      }
      if (!this.entries.has(key)) {
        continue;
      }
      const state = this.entries.get(key) as TState;
      this.entries.delete(key);
      evictedKeys.push(key);
      onEvict?.(key, state);
    }

    return evictedKeys;
  }

  clear(): void {
    this.entries.clear();
  }

  size(): number {
    return this.entries.size;
  }

  keys(): string[] {
    return Array.from(this.entries.keys());
  }
}
