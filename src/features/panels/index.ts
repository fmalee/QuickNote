export { default as OutlinePanel } from './OutlinePanel.vue';
export { default as RecentNotesPanel } from './RecentNotesPanel.vue';
export { default as RelatedNotesPanel } from './RelatedNotesPanel.vue';
export {
  type OutlineItem,
  extractMarkdownOutline,
  findActiveHeadingIndex,
  headingToSlug,
  stripInlineMarkdown,
} from './outlineExtractor';
export {
  BP_COLLAPSE_LEFT_SIDEBAR,
  BP_COLLAPSE_RIGHT_PANEL,
  BP_OVERLAY_DRAWER,
  COLLAPSED_PANEL_STRIP_WIDTH,
  MIN_EDITOR_WIDTH,
  MIN_PANEL_WIDTH,
  RIGHT_PANEL_REGISTRY,
  type RightPanelDef,
  SIDEBAR_COLLAPSED_STRIP_WIDTH,
  SIDEBAR_EXPANDED_WIDTH,
  getRightPanelDef,
} from './registry';
export {
  DEFAULT_PANEL_WIDTH,
  OUTLINE_DEBOUNCE_MS,
  PANEL_WIDTH_PERSIST_DEBOUNCE_MS,
  type RelatedEmptyReason,
  usePanelsStore,
} from './store';
