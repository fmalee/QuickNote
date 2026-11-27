import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LibraryInfo, SettingsSnapshot, WindowState } from '@/bindings/types';

const {
  mockLibraryGetInfo,
  mockLibrarySetRoot,
  mockSettingsGet,
  mockUiStateGet,
  mockWindowShowMain,
  mockWindowHideQuickBar,
  mockOnWindowState,
  mockOnSettingsStatus,
} = vi.hoisted(() => ({
  mockLibraryGetInfo: vi.fn(),
  mockLibrarySetRoot: vi.fn(),
  mockSettingsGet: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockWindowShowMain: vi.fn(),
  mockWindowHideQuickBar: vi.fn(),
  mockOnWindowState: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  libraryGetInfo: mockLibraryGetInfo,
  librarySetRoot: mockLibrarySetRoot,
  settingsGet: mockSettingsGet,
  uiStateGet: mockUiStateGet,
  windowShowMain: mockWindowShowMain,
  windowHideQuickBar: mockWindowHideQuickBar,
  hotkeyUpdate: vi.fn(),
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onWindowState: mockOnWindowState,
  onSettingsStatus: mockOnSettingsStatus,
  listenEvent: vi.fn(),
}));

import { OnboardingView, useOnboardingStore } from '@/features/onboarding';
import { QuickBarView, useQuickBarStore } from '@/features/quickbar';

function makeSettingsSnapshot(libraryRoot: string | null): SettingsSnapshot {
  return {
    settings: {
      libraryRoot,
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
          width: 360,
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
    },
    status: {
      valid: true,
      activeSource: 'default',
      errorLine: null,
      errorColumn: null,
      errorMessage: null,
    },
  };
}

describe('onboarding-state (S1 lock & unlock)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnWindowState.mockResolvedValue(() => {});
    mockOnSettingsStatus.mockResolvedValue(() => {});
  });

  it('locks main workbench when libraryRoot is null and unlocks after library_set_root', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const unconfigured: LibraryInfo = { root: null, noteCount: 0, lastIndexedAt: null };
    const configured: LibraryInfo = {
      root: '/Users/demo/Documents/QuickNote',
      noteCount: 1,
      lastIndexedAt: 1700000000000,
    };
    mockLibraryGetInfo.mockResolvedValueOnce(unconfigured);
    mockLibrarySetRoot.mockResolvedValueOnce(configured);

    const onboardingStore = useOnboardingStore();
    expect(onboardingStore.initialized).toBe(false);
    await onboardingStore.refreshLibraryInfo();

    expect(onboardingStore.initialized).toBe(true);
    expect(onboardingStore.isLocked).toBe(true);
    expect(onboardingStore.isConfigured).toBe(false);

    const lockedApp = createSSRApp(OnboardingView);
    lockedApp.use(pinia);
    const lockedHtml = await renderToString(lockedApp);
    expect(lockedHtml).toContain('欢迎使用 QuickNote');
    expect(lockedHtml).toContain('~/Documents/QuickNote');

    const ok = await onboardingStore.submitRoot('~/Documents/QuickNote', true);
    expect(ok).toBe(true);
    expect(mockLibrarySetRoot).toHaveBeenCalledWith({
      root: '~/Documents/QuickNote',
      createIfMissing: true,
    });
    expect(onboardingStore.isLocked).toBe(false);
    expect(onboardingStore.isConfigured).toBe(true);
    expect(onboardingStore.libraryInfo?.root).toBe('/Users/demo/Documents/QuickNote');
  });

  it('renders Quick Bar S1 disabled state and invokes window_show_main', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    mockLibraryGetInfo.mockResolvedValueOnce({ root: null, noteCount: 0, lastIndexedAt: null });
    mockSettingsGet.mockResolvedValueOnce(makeSettingsSnapshot(null));

    const windowState: WindowState = {
      windowId: 'main',
      visible: true,
      fullscreen: false,
      mode: 'standard',
      reason: null,
    };
    mockWindowShowMain.mockResolvedValueOnce(windowState);

    const quickBarStore = useQuickBarStore();
    await quickBarStore.initialize();

    expect(quickBarStore.isUnconfigured).toBe(true);
    expect(quickBarStore.canSubmit).toBe(false);

    const app = createSSRApp(QuickBarView);
    app.use(pinia);
    const html = await renderToString(app);
    expect(html).toContain('data-testid="quickbar-s1-onboarding"');
    expect(html).toContain('请先选择或创建笔记库');
    expect(html).toContain('打开主窗口完成初始化');

    await quickBarStore.openMainOnboarding();
    expect(mockWindowShowMain).toHaveBeenCalledWith({ mode: null });
  });
});
