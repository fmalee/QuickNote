export type DocTabKind = 'vault' | 'external';

/**
 * 编辑器标签的轻量元数据桩。
 *
 * 严禁在桩对象中持有文档全文或 CodeMirror 6 实例；
 * 当对应 EditorState 被 LRU 池淘汰时，仅保留光标偏移与滚动位置快照用于后续恢复。
 */
export interface DocTabStub {
  tabKey: string;
  kind: DocTabKind;
  path: string;
  title: string;
  isDirty: boolean;
  cursorOffset: number;
  scrollTop: number;
  lastAccessedAt: number;
  evicted: boolean;
}

export interface DocTabViewportSnapshot {
  cursorOffset: number;
  scrollTop: number;
}

export interface CreateDocTabStubInput {
  tabKey: string;
  kind: DocTabKind;
  path: string;
  title: string;
  isDirty?: boolean;
  cursorOffset?: number;
  scrollTop?: number;
  lastAccessedAt?: number;
  evicted?: boolean;
}

export function createDocTabStub(input: CreateDocTabStubInput): DocTabStub {
  return {
    tabKey: input.tabKey,
    kind: input.kind,
    path: input.path,
    title: input.title,
    isDirty: input.isDirty ?? false,
    cursorOffset: input.cursorOffset ?? 0,
    scrollTop: input.scrollTop ?? 0,
    lastAccessedAt: input.lastAccessedAt ?? Date.now(),
    evicted: input.evicted ?? false,
  };
}
