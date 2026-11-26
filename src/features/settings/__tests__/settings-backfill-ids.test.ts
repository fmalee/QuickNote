import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp, defineComponent, h, nextTick } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BackfillIdsResult, SettingsSnapshot, UiState } from '@/bindings/types';

const {
  mockSettingsGet,
  mockSettingsUpdate,
  mockUiStateGet,
  mockNotesList,
  mockIndexRebuild,
  mockNotesBackfillMissingIds,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockSettingsGet: vi.fn(),
  mockSettingsUpdate: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockNotesList: vi.fn(),
  mockIndexRebuild: vi.fn(),
  mockNotesBackfillMissingIds: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  settingsGet: mockSettingsGet,
  settingsUpdate: mockSettingsUpdate,
  uiStateGet: mockUiStateGet,
  notesList: mockNotesList,
  noteCreate: vi.fn(),
  librarySetRoot: vi.fn(),
  hotkeyUpdate: vi.fn(),
  indexRebuild: mockIndexRebuild,
  notesBackfillMissingIds: mockNotesBackfillMissingIds,
  isAppError: (err: unknown): err is { code: string; message: string; detail?: string | null } =>
    typeof err === 'object' && err !== null && 'code' in err && 'message' in err,
}));

vi.mock('@/lib/ipc/events', () => ({
  onSettingsStatus: mockOnSettingsStatus,
  onWindowState: mockOnWindowState,
  listenEvent: vi.fn(),
}));

import { SettingsLibraryPanel, useSettingsStore } from '@/features/settings';

interface PanelSetupState {
  backfillingIds: boolean;
  backfillIdsFeedback: string | null;
  backfillIdsError: string | null;
  handleBackfillMissingIds: () => Promise<void>;
}

function makeSettingsSnapshot(): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/Users/demo/Documents/QuickNote',
      hotkey: { quickBar: 'ctrl+alt+space', quickBarBackup: 'ctrl+alt+cmd+space' },
      quickBar: {
        defaultTarget: 'last_active_or_inbox',
        lastActiveTtlHours: 24,
        destroyAfterHiddenMinutes: 10,
        autoCreateMissingSection: false,
        pinned: false,
      },
      mainWindow: { mode: 'standard', auxiliaryWidthPercent: 45 },
      editor: {
        theme: 'system',
        fontSize: 15,
        lineHeight: 1.7,
        maxWidth: 900,
        tabSizeProse: 2,
        tabSizeCode: 4,
        autoSaveDebounceMs: 600,
        autoSaveMaxWaitMs: 2000,
        singleClickLink: false,
        showTabCloseButton: false,
      },
      panels: {
        default: 'outline',
        width: 260,
        terminal: {
          visibleByDefault: false,
          width: 380,
          shell: '/bin/zsh',
          idleRecycleMinutes: 30,
          maxConcurrentSessions: 3,
        },
      },
      inbox: { bloatThresholdLines: 500 },
      terminal: { name: 'Terminal.app', bundleId: 'com.apple.Terminal' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: [],
      maxActiveEditorRenders: 5,
    },
    status: {
      valid: true,
      activeSource: 'file',
      errorLine: null,
      errorColumn: null,
      errorMessage: null,
    },
  };
}

function makeUiState(): UiState {
  return {
    windows: {
      main: { x: 100, y: 100, width: 1200, height: 800, mode: 'standard', isOpen: true },
    },
    tabs: { items: [], activeKey: null, closedStack: [] },
    filters: { activeTab: 'inbox', selectedDir: null, selectedTags: [], listScrollTop: 0 },
    lastActiveDoc: null,
    hotkey: {
      combo: 'ctrl+alt+space',
      backupCombo: 'ctrl+alt+cmd+space',
      selfCheck: 'ok',
    },
  };
}

async function mountPanelSsr(pinia: ReturnType<typeof createPinia>): Promise<{
  html: string;
  state: PanelSetupState;
  renderCurrentHtml: () => Promise<string>;
}> {
  let capturedState: PanelSetupState | null = null;
  let capturedCtx: unknown = null;

  const Wrapper = defineComponent({
    setup() {
      return () => h(SettingsLibraryPanel);
    },
  });

  const app = createSSRApp(Wrapper);
  app.use(pinia);
  app.mixin({
    created(this: {
      $?: { type?: { __name?: string }; setupState?: PanelSetupState; proxy?: unknown };
    }) {
      if (this.$?.type?.__name === 'SettingsLibraryPanel' && this.$.setupState) {
        capturedState = this.$.setupState;
        capturedCtx = this.$;
      }
    },
  });

  const html = await renderToString(app);
  if (!capturedState || !capturedCtx) {
    throw new Error('Failed to capture SettingsLibraryPanel setupState');
  }

  const ssrRender = (
    SettingsLibraryPanel as unknown as {
      ssrRender: (
        ctx: unknown,
        push: (chunk: string) => void,
        parent: unknown,
        attrs: Record<string, unknown>,
        props: Record<string, unknown>,
        setupState: PanelSetupState,
        data: Record<string, unknown>,
        options: Record<string, unknown>,
      ) => void;
    }
  ).ssrRender;

  const state = capturedState as PanelSetupState;
  const instance = capturedCtx as { proxy?: unknown };

  const renderCurrentHtml = async (): Promise<string> => {
    await nextTick();
    let out = '';
    ssrRender(
      instance.proxy ?? {},
      (chunk: string) => {
        out += chunk;
      },
      instance,
      {},
      {},
      state,
      {},
      {},
    );
    return out;
  };

  return { html, state, renderCurrentHtml };
}

describe('settings-backfill-ids', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnSettingsStatus.mockResolvedValue(() => {});
    mockOnWindowState.mockResolvedValue(() => {});
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([]);
    mockSettingsGet.mockResolvedValue(makeSettingsSnapshot());
  });

  it('triggers notesBackfillMissingIds, guards re-entry, and displays summary feedback', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const store = useSettingsStore();
    await store.initialize();

    let resolveBackfill!: (value: BackfillIdsResult) => void;
    mockNotesBackfillMissingIds.mockImplementationOnce(
      () =>
        new Promise<BackfillIdsResult>((resolve) => {
          resolveBackfill = resolve;
        }),
    );

    const { html, state, renderCurrentHtml } = await mountPanelSsr(pinia);
    expect(html).toContain('data-testid="rebuild-index-btn"');
    expect(html).toContain('data-testid="backfill-ids-btn"');
    expect(html).toContain('为缺失 ID 的笔记补全 ULID');
    expect(html).not.toContain('data-testid="backfill-ids-feedback"');

    // Start backfill: enters loading/disabled state and blocks re-entry.
    const firstRun = state.handleBackfillMissingIds();
    expect(state.backfillingIds).toBe(true);
    expect(mockNotesBackfillMissingIds).toHaveBeenCalledTimes(1);

    await state.handleBackfillMissingIds();
    expect(mockNotesBackfillMissingIds).toHaveBeenCalledTimes(1);

    resolveBackfill({
      scanned: 6,
      updated: 2,
      skippedExisting: 3,
      skippedBrokenYaml: 1,
      failed: 0,
    });
    await firstRun;

    expect(state.backfillingIds).toBe(false);
    expect(state.backfillIdsError).toBeNull();
    expect(state.backfillIdsFeedback).toBe(
      '扫描 6 篇：已补全 2 篇，已有 ID 跳过 3 篇，YAML 异常跳过 1 篇。',
    );

    const afterHtml = await renderCurrentHtml();
    expect(afterHtml).toContain('data-testid="backfill-ids-feedback"');
    expect(afterHtml).toContain('扫描 6 篇：已补全 2 篇，已有 ID 跳过 3 篇，YAML 异常跳过 1 篇。');

    store.dispose();
  });

  it('displays error feedback when notesBackfillMissingIds rejects', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const store = useSettingsStore();
    await store.initialize();

    mockNotesBackfillMissingIds.mockRejectedValueOnce({
      code: 'libraryNotConfigured',
      message: 'Library root is not configured',
    });

    const { state, renderCurrentHtml } = await mountPanelSsr(pinia);
    await state.handleBackfillMissingIds();

    expect(state.backfillingIds).toBe(false);
    expect(state.backfillIdsFeedback).toBeNull();
    expect(state.backfillIdsError).toContain('Library root is not configured');

    const afterHtml = await renderCurrentHtml();
    expect(afterHtml).toContain('data-testid="backfill-ids-error"');
    expect(afterHtml).toContain('Library root is not configured');

    store.dispose();
  });
});
