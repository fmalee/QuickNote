export { default as NewNoteTemplateModal } from './NewNoteTemplateModal.vue';
export { default as NotesSidebar } from './NotesSidebar.vue';
export { FILTER_DEBOUNCE_MS, type NavTab, useNotesStore } from './store';
export {
  type DefaultTemplateName,
  deriveTitleFromContent,
  extractTemplateNameFromSummary,
  flattenDirRelPaths,
  inferDefaultTemplateName,
} from './templateUtils';
