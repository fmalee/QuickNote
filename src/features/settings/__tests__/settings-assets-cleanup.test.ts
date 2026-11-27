import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp, defineComponent, h, nextTick } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SettingsSnapshot, UiState } from '@/bindings/types';

const {
  mockSettingsGet,
  mockSettingsUpdate,
  mockUiStateGet,
  mockNotesList,
  mockAssetsScanUnused,
  mockAssetsCleanUnused,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockSettingsGet: vi.fn(),
  mockSettingsUpdate: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockNotesList: vi.fn(),
  mockAssetsScanUnused: vi.fn(),
  mockAssetsCleanUnused: vi.fn(),
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
  indexRebuild: vi.fn(),
  notesBackfillMissingIds: vi.fn(),
  assetsScanUnused: mockAssetsScanUnused,
  assetsCleanUnused: mockAssetsCleanUnused,
  isAppError: (err: unknown): err is { code: string; message: string; detail?: string | null } =>
    typeof err === 'object' && err !== null && 'code' in err && 'message' in err,
}));

vi.mock('@/lib/ipc/events', () => ({
  onSettingsStatus: mockOnSettingsStatus,
  onWindowState: mockOnWindowState,
}));

import SettingsLibraryPanel from '../SettingsLibraryPanel.vue';
import { useSettingsStore } from '../store';

function makeSnapshot(autoCleanUnusedAssets = true): SettingsSnapshot {
  return {
    settings: {
      libraryRoot: '/tmp/vault',
      hotkey: { quickBar: 'ctrl+alt+space', quickBarBackup: 'ctrl+alt+cmd+space' },
      quickBar: {
        defaultTarget: 'workspace_last_active_or_inbox',
        lastActiveTtlHours: 24,
        destroyAfterHiddenMinutes: 10,
        autoCreateMissingSection: false,
        pinned: false,
      },
      mainWindow: { mode: 'standard', auxiliaryWidthPercent: 45 },
      editor: {
        theme: 'system',
        fontFamily: 'system-ui',
        codeFontFamily: 'monospace',
        fontSize: 15,
        lineHeight: 1.65,
        maxWidth: 760,
        tabSizeProse: 2,
        tabSizeCode: 4,
        autoSaveDebounceMs: 1200,
        autoSaveMaxWaitMs: 6000,
        singleClickLink: false,
        showTabCloseButton: true,
      },
      panels: {
        default: 'outline',
        width: 280,
        terminal: {
          visibleByDefault: false,
          width: 360,
          shell: '/bin/zsh',
          idleRecycleMinutes: 30,
          maxConcurrentSessions: 2,
        },
      },
      inbox: { bloatThresholdLines: 500 },
      terminal: { name: 'iTerm', bundleId: 'com.googlecode.iterm2' },
      editors: {
        vscode: { name: 'Visual Studio Code', bundleId: 'com.microsoft.VSCode' },
        sublime: { name: 'Sublime Text', bundleId: 'com.sublimetext.4' },
      },
      searchRoots: [],
      maxActiveEditorRenders: 5,
      externalBookmarks: [],
      scanHiddenFiles: true,
      autoCleanUnusedAssets,
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

interface PanelSetupState {
  autoCleanUnusedAssets: boolean;
  unusedAssetsFeedback: string | null;
  unusedAssetsError: string | null;
  onToggleAutoCleanUnusedAssets: (event: Event) => Promise<void>;
  handleScanUnusedAssets: () => Promise<void>;
  handleCleanUnusedAssets: () => Promise<void>;
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

describe('settings-assets-cleanup', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnSettingsStatus.mockResolvedValue(() => {});
    mockOnWindowState.mockResolvedValue(() => {});
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([]);
    mockSettingsGet.mockResolvedValue(makeSnapshot(true));
    mockSettingsUpdate.mockResolvedValue(makeSnapshot(false));
    mockAssetsScanUnused.mockResolvedValue({
      items: [
        { relPath: 'assets/2026-11/unused-a.png', sizeBytes: 2048, mtimeMs: 1000 },
        { relPath: 'assets/2026-11/unused-b.webp', sizeBytes: 4096, mtimeMs: 1000 },
      ],
      totalBytes: 6144,
      skippedRecentCount: 0,
    });
    mockAssetsCleanUnused.mockResolvedValue({
      deletedPaths: ['assets/2026-11/unused-a.png', 'assets/2026-11/unused-b.webp'],
      freedBytes: 6144,
      skippedReferencedCount: 0,
      skippedRecentCount: 0,
      failedCount: 0,
    });
  });

  it('支持切换自动清理开关、扫描未引用图片并一键移至系统废纸篓', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const store = useSettingsStore();
    await store.initialize();

    const { html, state, renderCurrentHtml } = await mountPanelSsr(pinia);
    expect(html).toContain('data-testid="library-assets-cleanup-card"');
    expect(html).toContain('data-testid="auto-clean-unused-assets-checkbox"');
    expect(html).toContain('data-testid="scan-unused-assets-btn"');
    expect(state.autoCleanUnusedAssets).toBe(true);

    // 1. Toggle autoCleanUnusedAssets
    await state.onToggleAutoCleanUnusedAssets({
      target: { checked: false },
    } as unknown as Event);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({ autoCleanUnusedAssets: false });

    // 2. Scan unused assets
    await state.handleScanUnusedAssets();
    expect(mockAssetsScanUnused).toHaveBeenCalledWith({ gracePeriodSeconds: 0 });
    expect(state.unusedAssetsFeedback).toContain('发现 2 张未引用图片');

    const scannedHtml = await renderCurrentHtml();
    expect(scannedHtml).toContain('assets/2026-11/unused-a.png');
    expect(scannedHtml).toContain('data-testid="clean-unused-assets-btn"');

    // 3. Clean unused assets
    await state.handleCleanUnusedAssets();
    expect(mockAssetsCleanUnused).toHaveBeenCalledWith({
      relPaths: ['assets/2026-11/unused-a.png', 'assets/2026-11/unused-b.webp'],
      gracePeriodSeconds: 0,
    });
    expect(state.unusedAssetsFeedback).toContain('已将 2 张未引用图片移至系统废纸篓');

    const cleanedHtml = await renderCurrentHtml();
    expect(cleanedHtml).not.toContain('data-testid="unused-assets-list"');
  });
});
