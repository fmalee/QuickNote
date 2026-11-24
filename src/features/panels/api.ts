import type { NoteSummary, PanelGroupUiState, RelatedNoteItem, UiState } from '@/bindings/types';
import { notesGetRelated, notesList, uiStateGet, uiStateSet } from '@/lib/ipc/commands';

export const RECENT_NOTES_LIMIT = 20;
export const RELATED_NOTES_LIMIT = 20;

export async function fetchRelatedNotes(relPath: string): Promise<RelatedNoteItem[]> {
  const items = await notesGetRelated({ relPath });
  return items.slice(0, RELATED_NOTES_LIMIT);
}

export async function fetchRecentNotes(): Promise<NoteSummary[]> {
  const items = await notesList({
    tab: 'recent',
    dirRelPath: null,
    tags: [],
    workspaceKey: null,
    filterText: null,
  });
  return items.slice(0, RECENT_NOTES_LIMIT);
}

export async function fetchUiState(): Promise<UiState> {
  return uiStateGet();
}

export async function patchPanelGroupUiState(value: PanelGroupUiState): Promise<UiState> {
  return uiStateSet({
    key: 'panelGroup',
    value,
  });
}
