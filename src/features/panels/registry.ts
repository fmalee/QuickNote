import type { PanelId } from '@/bindings/types';

export const MIN_PANEL_WIDTH = 220;
export const MIN_EDITOR_WIDTH = 480;
export const BP_COLLAPSE_RIGHT_PANEL = 1200;
export const BP_COLLAPSE_LEFT_SIDEBAR = 1000;
export const BP_OVERLAY_DRAWER = 900;

export const COLLAPSED_PANEL_STRIP_WIDTH = 36;
export const SIDEBAR_EXPANDED_WIDTH = 300;
export const SIDEBAR_COLLAPSED_STRIP_WIDTH = 36;

export interface RightPanelDef {
  id: PanelId;
  title: string;
  shortLabel: string;
  order: number;
  defaultVisible: boolean;
  defaultWidth: number;
  minWidth: number;
  shortcut: string;
  actionId: string;
}

const OUTLINE_PANEL_DEF: RightPanelDef = {
  id: 'outline',
  title: '大纲',
  shortLabel: '纲',
  order: 1,
  defaultVisible: true,
  defaultWidth: 260,
  minWidth: MIN_PANEL_WIDTH,
  shortcut: 'ctrl+cmd+o',
  actionId: 'panel.toggle.outline',
};

export const RIGHT_PANEL_REGISTRY: readonly RightPanelDef[] = [
  OUTLINE_PANEL_DEF,
  {
    id: 'terminal',
    title: '终端',
    shortLabel: '终',
    order: 2,
    defaultVisible: false,
    defaultWidth: 360,
    minWidth: MIN_PANEL_WIDTH,
    shortcut: 'ctrl+cmd+t',
    actionId: 'panel.toggle.terminal',
  },
  {
    id: 'related',
    title: '相关文档',
    shortLabel: '联',
    order: 3,
    defaultVisible: false,
    defaultWidth: 260,
    minWidth: MIN_PANEL_WIDTH,
    shortcut: 'ctrl+cmd+r',
    actionId: 'panel.toggle.related',
  },
  {
    id: 'recent',
    title: '最近编辑',
    shortLabel: '近',
    order: 4,
    defaultVisible: false,
    defaultWidth: 240,
    minWidth: MIN_PANEL_WIDTH,
    shortcut: 'ctrl+cmd+e',
    actionId: 'panel.toggle.recent',
  },
] as const;

export function getRightPanelDef(id: PanelId): RightPanelDef {
  return RIGHT_PANEL_REGISTRY.find((item) => item.id === id) ?? OUTLINE_PANEL_DEF;
}
