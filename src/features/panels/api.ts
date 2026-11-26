import type {
  LeftSidebarUiState,
  RelatedNoteItem,
  TerminalPanelUiState,
  UiState,
} from '@/bindings/types';
import { notesGetRelated, uiStateGet, uiStateSet } from '@/lib/ipc/commands';

export const RELATED_NOTES_LIMIT = 20;

export async function fetchRelatedNotes(relPath: string): Promise<RelatedNoteItem[]> {
  const items = await notesGetRelated({ relPath });
  return items.slice(0, RELATED_NOTES_LIMIT);
}

export async function fetchUiState(): Promise<UiState> {
  return uiStateGet();
}

export async function patchLeftSidebarUiState(value: LeftSidebarUiState): Promise<UiState> {
  return uiStateSet({
    key: 'leftSidebar',
    value,
  });
}

export async function patchTerminalPanelUiState(value: TerminalPanelUiState): Promise<UiState> {
  return uiStateSet({
    key: 'terminalPanel',
    value,
  });
}
