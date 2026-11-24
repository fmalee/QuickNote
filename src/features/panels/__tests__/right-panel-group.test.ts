import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NoteDoc, NoteSummary, RelatedNoteItem, UiState } from '@/bindings/types';

const {
  mockNoteRead,
  mockNoteWrite,
  mockNotesList,
  mockNotesGetRelated,
  mockUiStateGet,
  mockUiStateSet,
  mockOnFsExternalChange,
} = vi.hoisted(() => ({
  mockNoteRead: vi.fn(),
  mockNoteWrite: vi.fn(),
  mockNotesList: vi.fn(),
  mockNotesGetRelated: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
  mockOnFsExternalChange: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: () => false,
  noteRead: mockNoteRead,
  noteWrite: mockNoteWrite,
  notesList: mockNotesList,
  notesGetRelated: mockNotesGetRelated,
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: mockOnFsExternalChange,
}));

import { useEditorTabsStore } from '@/features/editor';
import {
  BP_COLLAPSE_LEFT_SIDEBAR,
  BP_COLLAPSE_RIGHT_PANEL,
  BP_OVERLAY_DRAWER,
  MIN_EDITOR_WIDTH,
  MIN_PANEL_WIDTH,
  RIGHT_PANEL_REGISTRY,
  RightPanelGroup,
  usePanelsStore,
} from '@/features/panels';
import { actionRegistry } from '@/lib/actions';

function makeUiState(): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1280, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    panelGroup: { visible: true, activeId: 'outline', width: 260, collapsed: false },
    selectedWorkspaceKey: null,
    lastActiveDoc: null,
    workspaceModes: {},
    hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space', selfCheck: 'ok' },
  };
}

function makeDoc(key: string, relPath: string | null, absPath: string, isLibrary = true): NoteDoc {
  return {
    key,
    relPath,
    absPath,
    isInbox: false,
    isLibrary,
    meta: {
      id: key,
      title: relPath ?? absPath,
      tags: ['dev'],
      project: 'quicknote',
      docType: 'note',
      openDir: null,
      codeWorkspace: null,
      terminalDir: null,
      created: '2026-11-24T00:00:00Z',
      pinned: false,
      archived: false,
      metaError: null,
    },
    body: `# ${relPath ?? '外部文件'}\n正文内容`,
    bodyHash: `hash-${key}`,
    mtimeMs: 1000,
    sizeBytes: 128,
  };
}

function makeSummary(index: number): NoteSummary {
  const key = `01JRECENT000000000000000${String(index).padStart(2, '0')}`;
  return {
    key,
    relPath: `notes/recent-${index}.md`,
    absPath: `/vault/notes/recent-${index}.md`,
    title: `最近笔记 ${index}`,
    docType: 'note',
    tags: [],
    project: 'quicknote',
    excerpt: '摘要',
    pinned: false,
    archived: false,
    isInbox: false,
    mtimeMs: Date.now() - index * 60_000,
    metaError: null,
  };
}

let activePinia = createPinia();

async function renderRightPanelHtml(): Promise<string> {
  const app = createSSRApp(RightPanelGroup);
  app.use(activePinia);
  return renderToString(app);
}

describe('RightPanelGroup & responsive breakpoints', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    activePinia = createPinia();
    setActivePinia(activePinia);

    mockNoteRead.mockReset();
    mockNoteWrite.mockReset();
    mockNotesList.mockReset();
    mockNotesGetRelated.mockReset();
    mockUiStateGet.mockReset();
    mockUiStateSet.mockReset();
    mockOnFsExternalChange.mockReset();

    mockUiStateGet.mockResolvedValue(makeUiState());
    mockUiStateSet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([]);
    mockNotesGetRelated.mockResolvedValue([]);
    mockOnFsExternalChange.mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('registers all 4 built-in panels in C7.4 order with matching actionRegistry shortcuts', () => {
    expect(RIGHT_PANEL_REGISTRY.map((item) => item.id)).toEqual([
      'outline',
      'terminal',
      'related',
      'recent',
    ]);
    expect(RIGHT_PANEL_REGISTRY.map((item) => item.defaultWidth)).toEqual([260, 360, 260, 240]);
    expect(RIGHT_PANEL_REGISTRY.every((item) => item.minWidth === MIN_PANEL_WIDTH)).toBe(true);
    expect(MIN_PANEL_WIDTH).toBe(220);
    expect(MIN_EDITOR_WIDTH).toBe(480);
    expect(BP_COLLAPSE_RIGHT_PANEL).toBe(1200);
    expect(BP_COLLAPSE_LEFT_SIDEBAR).toBe(1000);
    expect(BP_OVERLAY_DRAWER).toBe(900);

    for (const def of RIGHT_PANEL_REGISTRY) {
      const action = actionRegistry.getById(def.actionId);
      expect(action).toBeDefined();
      expect(action?.defaultKeys).toBe(def.shortcut);
    }
  });

  it('loads related notes with reason badges, handles outside-library docs, and drops stale async responses', async () => {
    const docA = makeDoc('01JDOCA0000000000000000001', 'notes/a.md', '/vault/notes/a.md', true);
    const docB = makeDoc('01JDOCB0000000000000000002', 'notes/b.md', '/vault/notes/b.md', true);
    const docTarget = makeDoc(
      '01JTARGET0000000000000003',
      'notes/target.md',
      '/vault/notes/target.md',
      true,
    );
    const externalDoc = makeDoc('01JEXT0000000000000000004', null, '/tmp/external.md', false);

    let resolveA!: (value: RelatedNoteItem[]) => void;
    let resolveB!: (value: RelatedNoteItem[]) => void;

    mockNoteRead.mockImplementation(async ({ absPath }: { absPath: string }) => {
      if (absPath.endsWith('a.md')) return docA;
      if (absPath.endsWith('b.md')) return docB;
      if (absPath.endsWith('target.md')) return docTarget;
      return externalDoc;
    });

    mockNotesGetRelated.mockImplementation(({ relPath }: { relPath: string }) => {
      if (relPath === 'notes/a.md') {
        return new Promise<RelatedNoteItem[]>((resolve) => {
          resolveA = resolve;
        });
      }
      if (relPath === 'notes/b.md') {
        return new Promise<RelatedNoteItem[]>((resolve) => {
          resolveB = resolve;
        });
      }
      return Promise.resolve([]);
    });

    const editorTabsStore = useEditorTabsStore();
    const panelsStore = usePanelsStore();
    await panelsStore.initialize();

    // 打开文档 A 并切到相关文档面板
    await editorTabsStore.openNote('notes/a.md', { preview: false, absPath: docA.absPath });
    panelsStore.togglePanel('related');
    await vi.advanceTimersByTimeAsync(0);

    // 紧接着切到文档 B（B 先返回，A 后返回）
    await editorTabsStore.openNote('notes/b.md', { preview: false, absPath: docB.absPath });
    await vi.advanceTimersByTimeAsync(0);

    const bResults: RelatedNoteItem[] = [
      {
        key: '01JTARGET0000000000000003',
        relPath: 'notes/target.md',
        title: '目标关联文档',
        reason: 'backlink',
        score: 100,
      },
      {
        key: '01JTAG0000000000000000005',
        relPath: 'notes/shared-tag.md',
        title: '共有标签文档',
        reason: 'shared_tags',
        score: 60,
      },
      {
        key: '01JPROJ000000000000000006',
        relPath: 'notes/same-proj.md',
        title: '同项目文档',
        reason: 'same_project',
        score: 20,
      },
    ];
    resolveB(bResults);
    await vi.advanceTimersByTimeAsync(0);

    // 慢返回的 A 请求必须被丢弃，不污染 B 的结果
    resolveA([
      {
        key: '01JSTALE00000000000000007',
        relPath: 'notes/stale-from-a.md',
        title: '过期文档 A',
        reason: 'backlink',
        score: 99,
      },
    ]);
    await vi.advanceTimersByTimeAsync(0);

    expect(panelsStore.relatedItems.map((i) => i.relPath)).toEqual([
      'notes/target.md',
      'notes/shared-tag.md',
      'notes/same-proj.md',
    ]);
    const html = await renderRightPanelHtml();
    expect(html).toContain('反向链接');
    expect(html).toContain('共有标签');
    expect(html).toContain('同项目');
    expect(html).toContain('data-testid="related-item-01JTARGET0000000000000003"');
    expect(html).not.toContain('过期文档 A');

    // 打开关联条目跳转
    const firstRelated = panelsStore.relatedItems[0];
    expect(firstRelated).toBeDefined();
    if (firstRelated) {
      await editorTabsStore.openNote(firstRelated.relPath, { preview: false });
    }
    expect(editorTabsStore.activeDoc?.relPath).toBe('notes/target.md');

    // 库外文件显示明确提示
    await editorTabsStore.openNote(externalDoc.absPath, {
      preview: false,
      absPath: externalDoc.absPath,
    });
    await vi.advanceTimersByTimeAsync(0);
    const externalHtml = await renderRightPanelHtml();
    expect(externalHtml).toContain('data-testid="related-outside-library"');
    expect(externalHtml).toContain('库外文件暂不计算库内关联');
  });

  it('caps recent notes at 20 items and opens selected note', async () => {
    const recent25 = Array.from({ length: 25 }, (_, i) => makeSummary(i + 1));
    mockNotesList.mockResolvedValue(recent25);
    mockNoteRead.mockImplementation(async ({ absPath }: { absPath: string }) =>
      makeDoc('01JRECENT00000000000000003', 'notes/recent-3.md', absPath, true),
    );

    const editorTabsStore = useEditorTabsStore();
    const panelsStore = usePanelsStore();
    await panelsStore.initialize();

    panelsStore.togglePanel('recent');
    await vi.advanceTimersByTimeAsync(0);

    expect(panelsStore.recentItems).toHaveLength(20);
    const html = await renderRightPanelHtml();
    const itemMatches = html.match(/data-testid="recent-item-/g) ?? [];
    expect(itemMatches).toHaveLength(20);

    const thirdRecent = panelsStore.recentItems[2];
    expect(thirdRecent).toBeDefined();
    if (thirdRecent) {
      await editorTabsStore.openNoteByAbsPath(thirdRecent.absPath, { preview: false });
    }
    expect(editorTabsStore.activeDoc?.absPath).toBe('/vault/notes/recent-3.md');
  });

  it('clamps panel width between 220px and editor >= 480px and persists to uiStateSet', async () => {
    const panelsStore = usePanelsStore();
    await panelsStore.initialize();

    // 1280px 视口：左侧 300 + 编辑区 >= 480 => 右侧最大 500
    panelsStore.applyViewportWidth(1280);
    const tooSmall = panelsStore.setPanelWidth(120, 1280);
    expect(tooSmall).toBe(220);

    const tooLarge = panelsStore.setPanelWidth(700, 1280);
    expect(tooLarge).toBe(500);

    await vi.advanceTimersByTimeAsync(200);
    expect(mockUiStateSet).toHaveBeenLastCalledWith({
      key: 'panelGroup',
      value: {
        visible: true,
        activeId: 'outline',
        width: 500,
        collapsed: false,
      },
    });
  });

  it('applies 1200 / 1000 / 900 responsive breakpoints and overlay drawer mode accurately', async () => {
    const panelsStore = usePanelsStore();
    await panelsStore.initialize();

    // 1280px：常规展开
    panelsStore.applyViewportWidth(1280);
    await vi.advanceTimersByTimeAsync(0);
    expect(panelsStore.isPanelGroupExpanded).toBe(true);
    expect(panelsStore.leftSidebarCollapsed).toBe(false);
    expect(panelsStore.isOverlayDrawer).toBe(false);
    const expandedHtml = await renderRightPanelHtml();
    expect(expandedHtml).toContain('data-testid="right-panel-group"');

    // 1150px (< 1200)：右侧自动收起为窄图标条
    panelsStore.applyViewportWidth(1150);
    await vi.advanceTimersByTimeAsync(0);
    expect(panelsStore.isPanelGroupExpanded).toBe(false);
    expect(panelsStore.leftSidebarCollapsed).toBe(false);
    const collapsedHtml = await renderRightPanelHtml();
    expect(collapsedHtml).toContain('data-testid="right-panel-collapsed-strip"');

    // 960px (< 1000)：左侧进入可收起窄图标条态
    panelsStore.applyViewportWidth(960);
    await vi.advanceTimersByTimeAsync(0);
    expect(panelsStore.isNarrowLeftBreakpoint).toBe(true);
    expect(panelsStore.leftSidebarCollapsed).toBe(true);
    expect(panelsStore.isOverlayDrawer).toBe(false);

    // 820px (< 900)：右侧展开为覆盖式抽屉，不压缩编辑区宽度
    panelsStore.applyViewportWidth(820);
    await vi.advanceTimersByTimeAsync(0);
    expect(panelsStore.isOverlayDrawer).toBe(true);
    expect(panelsStore.isPanelGroupExpanded).toBe(false);

    // 手动展开终端面板 -> 以抽屉形式展开
    panelsStore.selectPanel('terminal');
    await vi.advanceTimersByTimeAsync(0);
    const drawerHtml = await renderRightPanelHtml();
    expect(drawerHtml).toContain('qn-right-panel-group--drawer');
    expect(drawerHtml).toContain('data-testid="right-panel-drawer-backdrop"');
    expect(drawerHtml).toContain('data-testid="terminal-placeholder-panel"');

    // 回到 1280px：左右栏均恢复常规展开
    panelsStore.applyViewportWidth(1280);
    await vi.advanceTimersByTimeAsync(0);
    expect(panelsStore.leftSidebarCollapsed).toBe(false);
    expect(panelsStore.isOverlayDrawer).toBe(false);
    const restoredHtml = await renderRightPanelHtml();
    expect(restoredHtml).not.toContain('qn-right-panel-group--drawer');
  });
});
