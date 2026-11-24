export { default as DocHeaderBar } from './DocHeaderBar.vue';
export { default as DocPropertiesPopover } from './DocPropertiesPopover.vue';
export { default as EditorConflictBanner } from './EditorConflictBanner.vue';
export { default as EditorTabBar } from './EditorTabBar.vue';
export { default as InboxBloatBanner } from './InboxBloatBanner.vue';
export { default as InboxExtractModal } from './InboxExtractModal.vue';
export { default as MarkdownEditor } from './MarkdownEditor.vue';
export {
  buildExternalContextMenuItems,
  buildExternalRunArgs,
  EXTERNAL_ACTION_SPECS,
  type ExternalActionFeedback,
  type ExternalActionSpec,
  isExternalActionId,
  useExternalActions,
} from './useExternalActions';
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
export {
  createMarkdownIndentCommand,
  createMarkdownOutdentCommand,
  DEFAULT_INDENT_CONFIG,
  detectNonMarkdownIndentUnit,
  type IndentConfig,
  isLineInFencedCodeBlock,
} from './extensions/markdownIndent';
export {
  TODO_LINE_RE,
  todoBackspaceCommand,
  todoCheckboxPlugin,
  TodoCheckboxWidget,
  todoEnterCommand,
  toggleTodoCommand,
} from './extensions/markdownTodo';
export {
  EDITOR_FORMAT_ACTION_IDS,
  executeEditorFormatAction,
  formatBlockquote,
  formatBold,
  formatBulletList,
  formatCodeBlock,
  formatHeading,
  formatImage,
  formatItalic,
  formatLink,
  formatTable,
  type HeadingLevel,
} from './extensions/markdownFormat';
export {
  type CurrentDocContext,
  type ExtractedLink,
  extractLinksFromState,
  findLinkAtPos,
  isLinkTriggerEvent,
  markdownLinkPlugin,
  type ParsedLinkTarget,
  parseLinkTarget,
  resolveHeadingAnchor,
} from './extensions/markdownLinks';
export {
  arrayBufferToBase64,
  buildMarkdownImageSnippet,
  extractImageExtFromFile,
  extractImageFileFromDataTransfer,
  handleImageInsert,
  type HandleImageInsertOptions,
  type ImageInputFile,
  type ImageInsertOutcome,
  MAX_IMAGE_SIZE_BYTES,
} from './extensions/markdownImages';
