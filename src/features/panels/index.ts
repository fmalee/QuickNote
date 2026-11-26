export { default as OutlinePanel } from './OutlinePanel.vue';
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
  BP_OVERLAY_DRAWER,
  COLLAPSED_PANEL_STRIP_WIDTH,
  MIN_EDITOR_WIDTH,
  SIDEBAR_COLLAPSED_STRIP_WIDTH,
  SIDEBAR_EXPANDED_WIDTH,
} from './registry';
export { OUTLINE_DEBOUNCE_MS, type RelatedEmptyReason, usePanelsStore } from './store';
