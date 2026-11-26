import type { EditorState } from '@codemirror/state';
import type { NoteDoc, NoteMeta, TabItemState } from '@/bindings/types';
import { createDocTabStub, type DocTabKind, type DocTabStub } from './docSession';

export const DEFAULT_INBOX_BLOAT_THRESHOLD_LINES = 500;
export const MAX_CLOSED_STACK_SIZE = 20;
export const DEFAULT_AUTO_SAVE_DEBOUNCE_MS = 600;
export const DEFAULT_AUTO_SAVE_MAX_WAIT_MS = 2000;

export type EditorSaveStatus =
  'saved' | 'dirty' | 'saving' | 'conflict' | 'deleted' | 'external-synced' | 'error';

export type EditorConflictState =
  | {
      kind: 'modified';
      diskBody: string;
      diskBodyHash: string;
      diskMeta: NoteMeta | null;
      bannerDismissed?: boolean;
      mergeMode?: boolean;
      mergeSaving?: boolean;
      mergeError?: string | null;
    }
  | {
      kind: 'deleted';
    };

export interface EditorPoolEntry {
  state: EditorState | null;
  syncedSeq: number;
  doc: NoteDoc | null;
  draftBody: string;
  baseBodyHash: string;
}

export interface EditorTab extends TabItemState {
  doc: NoteDoc | null;
  isDirty: boolean;
  isMissing: boolean;
  evicted?: boolean;
  lastAccessedAt?: number;
  draftBody: string;
  baseBodyHash: string;
  conflictState: EditorConflictState | null;
  saveStatus: EditorSaveStatus;
  cursorLine: number;
  cursorCol: number;
  selectionFromLine: number;
  selectionToLine: number;
  hasSelection: boolean;
  syncSeq: number;
}

export interface OpenNoteOptions {
  preview?: boolean;
  absPath?: string;
}

export interface EditorScrollTarget {
  key: string;
  line: number;
  from: number;
  seq: number;
}

export interface EditorStatusInfo {
  docKindLabel: string;
  indentLabel: string;
  diagnosticsLabel: string;
}

export function needsExternalFrontmatterConfirm(doc: NoteDoc): boolean {
  return (
    !doc.isLibrary &&
    doc.meta.id === null &&
    doc.meta.created === null &&
    doc.meta.tags.length === 0 &&
    doc.meta.project === null &&
    doc.meta.openDir === null &&
    doc.meta.codeWorkspace === null &&
    doc.meta.terminalDir === null &&
    !doc.meta.pinned &&
    !doc.meta.archived &&
    doc.meta.docType === 'note' &&
    doc.meta.metaError === null
  );
}

export function toTabItemState(tab: EditorTab): TabItemState {
  return {
    key: tab.key,
    relPath: tab.relPath,
    absPath: tab.absPath,
    isPinned: tab.isPinned,
    isPreview: tab.isPreview,
    scrollTop: tab.scrollTop,
    cursorPos: tab.cursorPos,
  };
}

export function deriveTabTitle(tab: Pick<EditorTab, 'doc' | 'relPath' | 'absPath'>): string {
  const metaTitle = tab.doc?.meta.title?.trim();
  if (metaTitle) {
    return metaTitle;
  }
  const source = tab.relPath || tab.absPath;
  if (!source) {
    return '未命名笔记';
  }
  const segments = source.split('/');
  const base = segments[segments.length - 1] || source;
  return base.replace(/\.md$/i, '') || base;
}

export function deriveTabKind(tab: Pick<EditorTab, 'doc' | 'relPath'>): DocTabKind {
  if (tab.doc) {
    return tab.doc.isLibrary ? 'vault' : 'external';
  }
  return tab.relPath ? 'vault' : 'external';
}

export function toDocTabStub(tab: EditorTab): DocTabStub {
  return createDocTabStub({
    tabKey: tab.key,
    kind: deriveTabKind(tab),
    path: tab.absPath || tab.relPath || '',
    title: deriveTabTitle(tab),
    isDirty: tab.isDirty,
    cursorOffset: tab.cursorPos,
    scrollTop: tab.scrollTop,
    lastAccessedAt: tab.lastAccessedAt ?? 0,
    evicted: Boolean(tab.evicted),
  });
}

export function buildTabFromDoc(doc: NoteDoc, isPreview: boolean): EditorTab {
  return {
    key: doc.key,
    relPath: doc.relPath,
    absPath: doc.absPath,
    isPinned: !isPreview,
    isPreview,
    scrollTop: 0,
    cursorPos: 0,
    doc,
    isDirty: false,
    isMissing: false,
    evicted: false,
    lastAccessedAt: Date.now(),
    draftBody: doc.body,
    baseBodyHash: doc.bodyHash,
    conflictState: null,
    saveStatus: 'saved',
    cursorLine: 1,
    cursorCol: 1,
    selectionFromLine: 1,
    selectionToLine: 1,
    hasSelection: false,
    syncSeq: 0,
  };
}

export function isSameNoteMeta(a: NoteMeta, b: NoteMeta | undefined): boolean {
  if (!b) {
    return false;
  }
  return (
    a.title === b.title &&
    a.docType === b.docType &&
    a.tags.length === b.tags.length &&
    a.tags.every((tag, idx) => tag === b.tags[idx]) &&
    a.project === b.project &&
    a.openDir === b.openDir &&
    a.codeWorkspace === b.codeWorkspace &&
    a.terminalDir === b.terminalDir &&
    a.pinned === b.pinned &&
    a.archived === b.archived
  );
}
