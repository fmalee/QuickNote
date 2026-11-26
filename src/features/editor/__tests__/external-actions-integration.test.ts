import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExternalActionId, ExternalActionState, NoteDoc } from '@/bindings/types';

const { mockResolveExternalActions, mockRunExternalAction, mockSearchNotesAndFiles } = vi.hoisted(
  () => ({
    mockResolveExternalActions: vi.fn(),
    mockRunExternalAction: vi.fn(),
    mockSearchNotesAndFiles: vi.fn(),
  }),
);

vi.mock('@/lib/ipc/commands', () => ({
  externalActionsResolve: mockResolveExternalActions,
  externalActionRun: mockRunExternalAction,
  searchNotesAndFiles: mockSearchNotesAndFiles,
  appOpenExternalUrl: vi.fn(),
  inboxExtractToNote: vi.fn(),
  noteRead: vi.fn(),
  noteSaveImage: vi.fn(),
  noteUpdateMeta: vi.fn(),
  noteWrite: vi.fn(),
  notesList: vi.fn().mockResolvedValue([]),
  uiStateGet: vi.fn(),
  uiStateSet: vi.fn(),
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: vi.fn().mockResolvedValue(() => {}),
  listenEvent: vi.fn(),
}));

import {
  buildExternalContextMenuItems,
  DocHeaderBar,
  EXTERNAL_ACTION_SPECS,
  useEditorTabsStore,
  useExternalActions,
} from '@/features/editor';
import { appendExternalContextMenuItems } from '../useExternalActions';
import { useNotesStore } from '@/features/notes';

function makeDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  return {
    key: '/Users/demo/Vault/inbox.md',
    absPath: '/Users/demo/Vault/inbox.md',
    relPath: 'inbox.md',
    isLibrary: true,
    isInbox: true,
    body: '# Inbox\n',
    bodyHash: 'hash-1',
    mtimeMs: 1000,
    sizeBytes: 8,
    meta: {
      id: '01J00000000000000000000000',
      title: 'Inbox',
      docType: 'note',
      tags: [],
      project: null,
      openDir: null,
      codeWorkspace: null,
      terminalDir: null,
      created: '2026-01-01T00:00:00Z',
      pinned: false,
      archived: false,
      metaError: null,
    },
    ...overrides,
  };
}

function makeStates(
  overrides: Partial<Record<ExternalActionId, Partial<ExternalActionState>>> = {},
): ExternalActionState[] {
  const defaults: Record<ExternalActionId, ExternalActionState> = {
    'external.reveal': {
      id: 'external.reveal',
      enabled: true,
      disabledReason: null,
      resolvedPath: '/Users/demo/Vault/inbox.md',
    },
    'external.terminal': {
      id: 'external.terminal',
      enabled: false,
      disabledReason: '当前库内笔记未配置 openDir 或工作区',
      resolvedPath: null,
    },
    'external.vscode': {
      id: 'external.vscode',
      enabled: false,
      disabledReason: '当前库内笔记未配置 openDir 或工作区',
      resolvedPath: null,
    },
    'external.sublime': {
      id: 'external.sublime',
      enabled: false,
      disabledReason: '当前库内笔记未配置 openDir 或工作区',
      resolvedPath: null,
    },
    'external.workspace': {
      id: 'external.workspace',
      enabled: false,
      disabledReason: '未配置 .code-workspace 文件',
      resolvedPath: null,
    },
    'external.default': {
      id: 'external.default',
      enabled: true,
      disabledReason: null,
      resolvedPath: '/Users/demo/Vault/inbox.md',
    },
    'external.copyPath': {
      id: 'external.copyPath',
      enabled: true,
      disabledReason: null,
      resolvedPath: '/Users/demo/Vault/inbox.md',
    },
    'external.pin': {
      id: 'external.pin',
      enabled: true,
      disabledReason: null,
      resolvedPath: '/Users/demo/Vault/inbox.md',
    },
  };

  return EXTERNAL_ACTION_SPECS.map((spec) => ({
    ...defaults[spec.id],
    ...(overrides[spec.id] ?? {}),
  }));
}

describe('external-actions-integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('disables unconfigured workspace actions with disabledReason and never triggers externalActionRun', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const doc = makeDoc();
    mockResolveExternalActions.mockResolvedValueOnce(makeStates());

    const externalActions = useExternalActions();
    await externalActions.refreshForDoc(doc);

    const menuItems = buildExternalContextMenuItems(externalActions.states);
    const terminalItem = menuItems.find(
      (item): item is Extract<(typeof menuItems)[number], { type: 'item' }> =>
        item.type === 'item' && item.id === 'external.terminal',
    );
    expect(terminalItem?.disabled).toBe(true);
    expect(terminalItem?.disabledReason).toBe('当前库内笔记未配置 openDir 或工作区');

    const executed = await externalActions.executeAction('external.terminal');
    expect(executed).toBe(false);
    expect(mockRunExternalAction).not.toHaveBeenCalled();
    expect(externalActions.feedback?.message).toBe('当前库内笔记未配置 openDir 或工作区');

    // Verify DocHeaderBar renders the external actions trigger button
    const tabsStore = useEditorTabsStore();
    tabsStore.tabs = [
      {
        key: doc.absPath,
        absPath: doc.absPath,
        relPath: doc.relPath,
        isPreview: false,
        isPinned: true,
        isMissing: false,
        isDirty: false,
        saveStatus: 'saved',
        hasSelection: false,
        selectionFromLine: 1,
        selectionToLine: 1,
        cursorLine: 1,
        cursorCol: 1,
        scrollTop: 0,
        cursorPos: 0,
        draftBody: doc.body,
        baseBodyHash: doc.bodyHash,
        syncSeq: 1,
        doc,
        conflictState: null,
      },
    ];
    tabsStore.activeKey = doc.absPath;

    const app = createSSRApp(DocHeaderBar);
    app.use(pinia);
    const html = await renderToString(app);
    expect(html).toContain('data-testid="doc-header-external-btn"');
  });

  it('runs reveal, terminal, vscode, and pin with exact absPath/dirPath for workspace/external docs', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const projectDoc = makeDoc({
      key: '/Users/demo/Sites/App/README.md',
      absPath: '/Users/demo/Sites/App/README.md',
      relPath: null,
      isLibrary: false,
      isInbox: false,
      meta: {
        ...makeDoc().meta,
        project: 'app',
      },
    });

    mockResolveExternalActions.mockResolvedValueOnce(
      makeStates({
        'external.reveal': {
          enabled: true,
          disabledReason: null,
          resolvedPath: '/Users/demo/Sites/App/README.md',
        },
        'external.terminal': {
          enabled: true,
          disabledReason: null,
          resolvedPath: '/Users/demo/Sites/App',
        },
        'external.vscode': {
          enabled: true,
          disabledReason: null,
          resolvedPath: '/Users/demo/Sites/App',
        },
        'external.pin': {
          enabled: true,
          disabledReason: null,
          resolvedPath: '/Users/demo/Sites/App/README.md',
        },
      }),
    );
    mockRunExternalAction.mockResolvedValue(undefined);

    const externalActions = useExternalActions();
    await externalActions.refreshForDoc(projectDoc);

    expect(mockResolveExternalActions).toHaveBeenCalledWith({
      absPath: '/Users/demo/Sites/App/README.md',
      commandId: null,
    });

    await externalActions.executeAction('external.reveal');
    expect(mockRunExternalAction).toHaveBeenCalledWith({
      id: 'external.reveal',
      absPath: '/Users/demo/Sites/App/README.md',
      dirPath: null,
    });

    await externalActions.executeAction('external.terminal');
    expect(mockRunExternalAction).toHaveBeenCalledWith({
      id: 'external.terminal',
      absPath: null,
      dirPath: '/Users/demo/Sites/App',
    });

    await externalActions.executeAction('external.vscode');
    expect(mockRunExternalAction).toHaveBeenCalledWith({
      id: 'external.vscode',
      absPath: null,
      dirPath: '/Users/demo/Sites/App',
    });

    await externalActions.executeAction('external.pin', { pinned: false });
    expect(mockRunExternalAction).toHaveBeenCalledWith({
      id: 'external.pin',
      absPath: '/Users/demo/Sites/App/README.md',
      dirPath: null,
    });
    expect(externalActions.feedback?.message).toBe('已固定到常用文件');
  });

  it('silently discards stale externalActionsResolve responses when switching docs rapidly (C2.2)', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    let resolveFirst!: (val: ExternalActionState[]) => void;
    let resolveSecond!: (val: ExternalActionState[]) => void;

    mockResolveExternalActions
      .mockImplementationOnce(
        () =>
          new Promise<ExternalActionState[]>((resolve) => {
            resolveFirst = resolve;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise<ExternalActionState[]>((resolve) => {
            resolveSecond = resolve;
          }),
      );

    const docA = makeDoc({ absPath: '/Users/demo/Vault/a.md', relPath: 'a.md' });
    const docB = makeDoc({ absPath: '/Users/demo/Vault/b.md', relPath: 'b.md' });

    const externalActions = useExternalActions();
    const p1 = externalActions.refreshForDoc(docA);
    const p2 = externalActions.refreshForDoc(docB);

    // Second request resolves first with docB's path
    resolveSecond(
      makeStates({
        'external.reveal': {
          enabled: true,
          disabledReason: null,
          resolvedPath: '/Users/demo/Vault/b.md',
        },
      }),
    );
    await p2;
    expect(externalActions.resolvedAbsPath).toBe('/Users/demo/Vault/b.md');

    // First (stale) request resolves later with docA's path -> must be discarded
    resolveFirst(
      makeStates({
        'external.reveal': {
          enabled: true,
          disabledReason: null,
          resolvedPath: '/Users/demo/Vault/a.md',
        },
      }),
    );
    await p1;

    expect(externalActions.resolvedAbsPath).toBe('/Users/demo/Vault/b.md');
    const revealState = externalActions.states.find((item) => item.id === 'external.reveal');
    expect(revealState?.resolvedPath).toBe('/Users/demo/Vault/b.md');
  });

  it('syncs open tab meta.pinned and notesStore.refreshPinnedExternalFiles on external.pin, and restricts non-Markdown DocHeaderBar editing', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const confDoc = makeDoc({
      key: 'abs:/etc/nginx/nginx.conf',
      absPath: '/etc/nginx/nginx.conf',
      relPath: null,
      isLibrary: false,
      isInbox: false,
      body: 'worker_processes 1;\n',
      meta: {
        ...makeDoc().meta,
        id: null,
        title: 'nginx.conf',
        created: null,
        pinned: false,
      },
    });

    const tabsStore = useEditorTabsStore();
    tabsStore.tabs = [
      {
        key: confDoc.key,
        absPath: confDoc.absPath,
        relPath: confDoc.relPath,
        isPreview: false,
        isPinned: true,
        isMissing: false,
        isDirty: false,
        saveStatus: 'saved',
        hasSelection: false,
        selectionFromLine: 1,
        selectionToLine: 1,
        cursorLine: 1,
        cursorCol: 1,
        scrollTop: 0,
        cursorPos: 0,
        draftBody: confDoc.body,
        baseBodyHash: confDoc.bodyHash,
        syncSeq: 1,
        doc: confDoc,
        conflictState: null,
      },
    ];
    tabsStore.activeKey = confDoc.key;

    const notesStore = useNotesStore();
    const refreshSpy = vi.spyOn(notesStore, 'refreshPinnedExternalFiles').mockResolvedValue();

    mockResolveExternalActions.mockResolvedValueOnce(
      makeStates({
        'external.pin': {
          enabled: true,
          disabledReason: null,
          resolvedPath: '/etc/nginx/nginx.conf',
        },
      }),
    );
    mockRunExternalAction.mockResolvedValue(undefined);

    const externalActions = useExternalActions();
    await externalActions.refreshForDoc(confDoc);

    // 1. Pinning syncs open tab doc.meta.pinned = true and refreshes sidebar pinned list
    await externalActions.executeAction('external.pin', { pinned: false });
    expect(tabsStore.activeDoc?.meta.pinned).toBe(true);
    expect(externalActions.feedback?.message).toBe('已固定到常用文件');
    expect(refreshSpy).toHaveBeenCalledTimes(1);

    // 2. Non-Markdown DocHeaderBar hides frontmatter controls and renders readonly filename + external controls
    const appPinned = createSSRApp(DocHeaderBar);
    appPinned.use(pinia);
    const htmlPinned = await renderToString(appPinned);
    expect(htmlPinned).toContain('data-testid="doc-header-title-readonly"');
    expect(htmlPinned).toContain('nginx.conf');
    expect(htmlPinned).toContain('data-testid="doc-header-external-badge"');
    expect(htmlPinned).toContain('data-testid="doc-header-pin-external-btn"');
    expect(htmlPinned).toContain('取消固定');
    expect(htmlPinned).toContain('data-testid="doc-header-external-btn"');
    expect(htmlPinned).not.toContain('data-testid="doc-header-title-input"');
    expect(htmlPinned).not.toContain('data-testid="doc-header-type-select"');
    expect(htmlPinned).not.toContain('data-testid="doc-header-tag-input"');
    expect(htmlPinned).not.toContain('data-testid="doc-header-properties-btn"');

    // 3. Unpinning syncs open tab doc.meta.pinned = false
    await externalActions.executeAction('external.pin', { pinned: true });
    expect(tabsStore.activeDoc?.meta.pinned).toBe(false);
    expect(externalActions.feedback?.message).toBe('已取消固定常用文件');
    expect(refreshSpy).toHaveBeenCalledTimes(2);
  });

  it('resolveForTarget returns enabled/disabled states and buildExternalContextMenuItems outputs all 8 actions with disabled flag', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const resolvedStates = makeStates({
      'external.reveal': {
        enabled: true,
        disabledReason: null,
        resolvedPath: '/Users/demo/Sites/App/notes.md',
      },
      'external.terminal': {
        enabled: false,
        disabledReason: '未配置工作区目录',
        resolvedPath: null,
      },
    });
    mockResolveExternalActions.mockResolvedValueOnce(resolvedStates);

    const externalActions = useExternalActions();
    const states = await externalActions.resolveForTarget({
      absPath: '/Users/demo/Sites/App/notes.md',
    });

    expect(mockResolveExternalActions).toHaveBeenCalledWith({
      absPath: '/Users/demo/Sites/App/notes.md',
      commandId: null,
    });

    const items = buildExternalContextMenuItems(states);
    expect(items).toHaveLength(8);
    expect(items.map((item) => item.id)).toEqual(EXTERNAL_ACTION_SPECS.map((spec) => spec.id));

    const revealItem = items.find(
      (item): item is Extract<(typeof items)[number], { type: 'item' }> =>
        item.type === 'item' && item.id === 'external.reveal',
    );
    expect(revealItem?.disabled).toBe(false);

    const terminalItem = items.find(
      (item): item is Extract<(typeof items)[number], { type: 'item' }> =>
        item.type === 'item' && item.id === 'external.terminal',
    );
    expect(terminalItem?.disabled).toBe(true);
    expect(terminalItem?.disabledReason).toBe('未配置工作区目录');
  });

  it('appendExternalContextMenuItems places all 8 external actions after separator when base entries exist', () => {
    const baseEntries = [
      { type: 'item' as const, id: 'editor.bold', title: '加粗' },
      { type: 'item' as const, id: 'editor.copy', title: '复制' },
    ];
    const states = makeStates();

    const merged = appendExternalContextMenuItems(baseEntries, states);
    expect(merged).toHaveLength(baseEntries.length + 1 + 8);
    expect(merged[2]).toEqual({ type: 'separator', id: 'ctx-ext-sep' });
    expect(merged.slice(3).map((item) => item.id)).toEqual(
      EXTERNAL_ACTION_SPECS.map((spec) => spec.id),
    );
  });
});
