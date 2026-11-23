export { default as DocHeaderBar } from './DocHeaderBar.vue';
export { default as DocPropertiesPopover } from './DocPropertiesPopover.vue';
export { default as EditorConflictBanner } from './EditorConflictBanner.vue';
export { default as EditorTabBar } from './EditorTabBar.vue';
export { default as InboxBloatBanner } from './InboxBloatBanner.vue';
export { default as InboxExtractModal } from './InboxExtractModal.vue';
export { default as MarkdownEditor } from './MarkdownEditor.vue';
export {
  DEFAULT_AUTO_SAVE_DEBOUNCE_MS,
  DEFAULT_AUTO_SAVE_MAX_WAIT_MS,
  DEFAULT_INBOX_BLOAT_THRESHOLD_LINES,
  MAX_CLOSED_STACK_SIZE,
  type EditorConflictState,
  type EditorSaveStatus,
  type EditorScrollTarget,
  type EditorTab,
  needsExternalFrontmatterConfirm,
  type OpenNoteOptions,
  useEditorTabsStore,
} from './store';
