import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UiState } from '@/bindings/types';

const { mockUiStateGet, mockUiStateSet } = vi.hoisted(() => ({
  mockUiStateGet: vi.fn(),
  mockUiStateSet: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: () => false,
  noteRead: vi.fn(),
  noteWrite: vi.fn(),
  notesList: vi.fn().mockResolvedValue([]),
  notesGetRelated: vi.fn().mockResolvedValue([]),
  uiStateGet: mockUiStateGet,
  uiStateSet: mockUiStateSet,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: vi.fn(() => () => undefined),
}));

import { usePanelsStore } from '../store';

function makeDefaultUiState(): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1280, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    leftSidebar: { width: 300, activeView: 'workspace', collapsed: false },
    terminalPanel: { visible: false, height: 240, collapsed: false },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    selectedWorkspaceKey: null,
    lastActiveDoc: null,
    workspaceModes: {},
    hotkey: { combo: 'ctrl+alt+space', backupCombo: 'ctrl+alt+cmd+space', selfCheck: 'ok' },
  };
}

describe('panels store leftSidebar & terminalPanel state', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    const uiState = makeDefaultUiState();
    mockUiStateGet.mockReset();
    mockUiStateSet.mockReset();
    mockUiStateGet.mockResolvedValue(uiState);
    mockUiStateSet.mockImplementation(async (args: { key: string; value: unknown }) => ({
      ...uiState,
      [args.key]: args.value,
    }));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('toggleLeftView toggles between target view and workspace and persists each call', () => {
    const store = usePanelsStore();
    expect(store.leftSidebar.activeView).toBe('workspace');

    store.toggleLeftView('outline');
    expect(store.leftSidebar.activeView).toBe('outline');
    expect(mockUiStateSet).toHaveBeenNthCalledWith(1, {
      key: 'leftSidebar',
      value: { width: 300, activeView: 'outline', collapsed: false },
    });

    store.toggleLeftView('outline');
    expect(store.leftSidebar.activeView).toBe('workspace');
    expect(mockUiStateSet).toHaveBeenNthCalledWith(2, {
      key: 'leftSidebar',
      value: { width: 300, activeView: 'workspace', collapsed: false },
    });

    store.dispose();
  });

  it('toggleTerminalPanel toggles isTerminalPanelShown between true and false', () => {
    const store = usePanelsStore();
    expect(store.isTerminalPanelShown).toBe(false);

    store.toggleTerminalPanel();
    expect(store.isTerminalPanelShown).toBe(true);
    expect(store.terminalPanel.visible).toBe(true);
    expect(store.terminalPanel.collapsed).toBe(false);

    store.toggleTerminalPanel();
    expect(store.isTerminalPanelShown).toBe(false);
    expect(store.terminalPanel.visible).toBe(false);

    store.dispose();
  });

  it('clamps terminalPanel height to [120, 600]', () => {
    const store = usePanelsStore();

    store.setTerminalPanelHeight(9999);
    expect(store.terminalPanel.height).toBe(600);

    store.setTerminalPanelHeight(1);
    expect(store.terminalPanel.height).toBe(120);

    store.dispose();
  });

  it('clamps leftSidebar width minimum to 220', () => {
    const store = usePanelsStore();

    store.setLeftSidebarWidth(50);
    expect(store.leftSidebar.width).toBe(220);
    expect(store.leftSidebarWidth).toBe(220);

    store.dispose();
  });

  it('applies responsive breakpoints for left sidebar collapse and overlay drawer mode', async () => {
    const store = usePanelsStore();
    await store.initialize();

    // 1280px：常规展开
    store.applyViewportWidth(1280);
    expect(store.leftSidebarCollapsed).toBe(false);
    expect(store.isOverlayDrawer).toBe(false);

    // 960px (< 1000)：左侧自动收起为窄图标条
    store.applyViewportWidth(960);
    expect(store.isNarrowLeftBreakpoint).toBe(true);
    expect(store.leftSidebarCollapsed).toBe(true);
    expect(store.isOverlayDrawer).toBe(false);

    // 820px (< 900)：进入覆盖式抽屉模式
    store.applyViewportWidth(820);
    expect(store.isOverlayDrawer).toBe(true);
    expect(store.leftSidebarCollapsed).toBe(true);

    // 回到 1280px：恢复常规展开
    store.applyViewportWidth(1280);
    expect(store.leftSidebarCollapsed).toBe(false);
    expect(store.isOverlayDrawer).toBe(false);

    store.dispose();
  });

  it('enforces overlay drawer and collapses left sidebar when forceOverlayDrawer is enabled', async () => {
    const store = usePanelsStore();
    await store.initialize();

    store.applyViewportWidth(1280);
    expect(store.isOverlayDrawer).toBe(false);

    store.setForceOverlayDrawer(true);
    expect(store.isOverlayDrawer).toBe(true);
    expect(store.leftSidebarCollapsed).toBe(true);

    store.setForceOverlayDrawer(false);
    expect(store.isOverlayDrawer).toBe(false);
    expect(store.leftSidebarCollapsed).toBe(false);

    store.dispose();
  });
});
