export { default as DirTreeSection } from './DirTreeSection.vue';
export { default as DirWorkspaceBindModal } from './DirWorkspaceBindModal.vue';
export { default as NewNoteTemplateModal } from './NewNoteTemplateModal.vue';
export { default as NotesSidebar } from './NotesSidebar.vue';
export { default as PinnedExternalFilesSection } from './PinnedExternalFilesSection.vue';
export { FILTER_DEBOUNCE_MS, type NavTab, useNotesStore } from './store';
export { default as TagCloudSection } from './TagCloudSection.vue';
export { default as TagRenameModal } from './TagRenameModal.vue';
export {
  type DefaultTemplateName,
  deriveTitleFromContent,
  extractTemplateNameFromSummary,
  flattenDirRelPaths,
  inferDefaultTemplateName,
} from './templateUtils';
