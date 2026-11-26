import type { CaptureMode, ResolvedTarget, UiState } from '@/bindings/types';
import type { QuickOpenDirectoryScope, QuickOpenResultItem } from '@/features/quickopen';

export const QUICKBAR_DRAFT_STORAGE_KEY = 'quicknote:quickbar-draft-v1';
export const QUICKBAR_SEARCH_DEBOUNCE_MS = 80;

export interface ExplicitQuickBarTarget {
  relPath: string;
  title: string;
}

export interface QuickBarDraftStorage {
  mode: CaptureMode;
  targetRelPath: string | null;
  text: string;
  updatedAt: number;
}

export type QuickBarOpenGroupId =
  'action' | 'note' | 'tag' | 'file_bookmark' | 'dir_bookmark' | 'path_direct';

export type QuickBarOpenBadgeLabel =
  '动作' | '笔记' | '标签' | '文件书签' | '目录书签' | '路径直开';

export type QuickBarOpenActionKind = 'open_file' | 'enter_directory';

export interface QuickBarOpenItem {
  id: string;
  group: QuickBarOpenGroupId;
  badgeLabel: QuickBarOpenBadgeLabel;
  actionKind: QuickBarOpenActionKind;
  title: string;
  subtitle: string;
  absPath: string;
  isHidden?: boolean;
  pinned?: boolean;
  tagName?: string;
  actionId?: string;
  shortcutLabel?: string;
  disabled?: boolean;
  rawIndex?: number;
}

export interface QuickBarOpenGroup {
  id: QuickBarOpenGroupId;
  label: string;
  items: QuickBarOpenItem[];
}

export type QuickBarOpenDirectoryScope = QuickOpenDirectoryScope;

export const QUICKBAR_OPEN_GROUP_ORDER: readonly QuickBarOpenGroupId[] = [
  'action',
  'note',
  'tag',
  'file_bookmark',
  'dir_bookmark',
  'path_direct',
];

export function mapQuickOpenResultToQuickBarItem(
  item: QuickOpenResultItem,
  rawIndex: number,
): QuickBarOpenItem {
  if (item.kind === 'action') {
    return {
      id: item.id,
      group: 'action',
      badgeLabel: '动作',
      actionKind: 'open_file',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      actionId: item.actionId,
      shortcutLabel: item.shortcutLabel,
      disabled: item.disabled,
      rawIndex,
    };
  }
  if (item.kind === 'tag') {
    return {
      id: item.id,
      group: 'tag',
      badgeLabel: '标签',
      actionKind: 'open_file',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      tagName: item.tagName,
      rawIndex,
    };
  }
  if (item.kind === 'note') {
    return {
      id: item.id,
      group: 'note',
      badgeLabel: '笔记',
      actionKind: 'open_file',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      pinned: item.pinned,
      rawIndex,
    };
  }
  if (item.kind === 'file' && item.source === 'file_bookmark') {
    return {
      id: item.id,
      group: 'file_bookmark',
      badgeLabel: '文件书签',
      actionKind: 'open_file',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      pinned: item.pinned,
      rawIndex,
    };
  }
  if (item.kind === 'dir' && item.source === 'dir_bookmark') {
    return {
      id: item.id,
      group: 'dir_bookmark',
      badgeLabel: '目录书签',
      actionKind: 'enter_directory',
      title: item.title,
      subtitle: item.subtitle,
      absPath: item.absPath ?? '',
      pinned: item.pinned,
      rawIndex,
    };
  }
  const isDir = item.kind === 'dir';
  return {
    id: item.id,
    group: 'path_direct',
    badgeLabel: isDir ? '目录书签' : '路径直开',
    actionKind: isDir ? 'enter_directory' : 'open_file',
    title: item.title,
    subtitle: item.subtitle,
    absPath: item.absPath ?? '',
    isHidden: item.isHidden,
    pinned: item.pinned,
    rawIndex,
  };
}

export function createFallbackUiState(): UiState {
  return {
    filters: {
      activeTab: 'recent',
      selectedDir: null,
      selectedTags: [],
      listScrollTop: 0,
    },
    tabs: {
      items: [],
      activeKey: null,
      closedStack: [],
    },
    lastActiveDoc: null,
    leftSidebar: {
      width: 300,
      activeView: 'workspace',
      collapsed: false,
    },
    terminalPanel: {
      visible: false,
      height: 240,
      collapsed: false,
    },
    windows: {
      main: {
        isOpen: false,
        mode: 'standard',
        x: 0,
        y: 0,
        width: 1200,
        height: 800,
      },
    },
    hotkey: {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
  };
}

export function createFallbackResolvedTarget(): ResolvedTarget {
  return {
    scenario: 'S4',
    kind: 'inbox',
    targetRelPath: 'inbox.md',
    targetTitle: '收件箱',
    sectionHint: null,
    recommendedMode: 'list',
    chipLabel: '收件箱 · inbox.md',
  };
}

export function isCaptureMode(value: unknown): value is CaptureMode {
  return value === 'list' || value === 'todo' || value === 'prose';
}

export function getCaptureModeSectionLabel(mode: CaptureMode): string {
  switch (mode) {
    case 'list':
      return '列表';
    case 'todo':
      return '待办';
    case 'prose':
      return '正文';
  }
}

export function readDraftStorage(): QuickBarDraftStorage | null {
  try {
    if (typeof globalThis.localStorage === 'undefined') {
      return null;
    }
    const raw = globalThis.localStorage.getItem(QUICKBAR_DRAFT_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) {
      return null;
    }
    const candidate = parsed as Record<string, unknown>;
    if (typeof candidate.text !== 'string') {
      return null;
    }
    return {
      mode: isCaptureMode(candidate.mode) ? candidate.mode : 'list',
      targetRelPath: typeof candidate.targetRelPath === 'string' ? candidate.targetRelPath : null,
      text: candidate.text,
      updatedAt: typeof candidate.updatedAt === 'number' ? candidate.updatedAt : Date.now(),
    };
  } catch {
    return null;
  }
}

export function writeDraftStorage(payload: QuickBarDraftStorage): void {
  try {
    if (typeof globalThis.localStorage === 'undefined') {
      return;
    }
    globalThis.localStorage.setItem(QUICKBAR_DRAFT_STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // 忽略存储配额或环境异常
  }
}

export function removeDraftStorage(): void {
  try {
    if (typeof globalThis.localStorage === 'undefined') {
      return;
    }
    globalThis.localStorage.removeItem(QUICKBAR_DRAFT_STORAGE_KEY);
  } catch {
    // 忽略环境异常
  }
}
