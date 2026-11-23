import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { mockSettingsInit, mockSettingsDispose, mockOnboardingRefresh, onMountedCallbacks } =
  vi.hoisted(() => ({
    mockSettingsInit: vi.fn(),
    mockSettingsDispose: vi.fn(),
    mockOnboardingRefresh: vi.fn(),
    onMountedCallbacks: [] as Array<() => void>,
  }));

vi.mock('vue', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue')>();
  return {
    ...actual,
    onMounted: (fn: () => void) => {
      onMountedCallbacks.push(fn);
    },
  };
});

vi.mock('@/features/settings', () => ({
  SettingsView: { template: '<div />' },
  SettingsErrorBanner: { template: '<div />' },
  VisualBaselinePreview: { template: '<div />' },
  useSettingsStore: () => ({
    isSettingsInvalid: false,
    loadStatus: null,
    hotkeyStatus: null,
    initialize: mockSettingsInit,
    dispose: mockSettingsDispose,
  }),
}));

vi.mock('@/features/onboarding', () => ({
  OnboardingView: { template: '<div />' },
  useOnboardingStore: () => ({
    isLocked: true,
    libraryInfo: null,
    refreshLibraryInfo: mockOnboardingRefresh,
  }),
}));

vi.mock('@/features/quickbar', () => ({
  QuickBarView: { template: '<div />' },
  useQuickBarStore: () => ({
    initialize: vi.fn().mockResolvedValue(undefined),
    dispose: vi.fn(),
  }),
}));

vi.mock('@/features/toast', () => ({
  ToastView: { template: '<div />' },
  useToastStore: () => ({
    initialize: vi.fn().mockResolvedValue(undefined),
    dispose: vi.fn(),
  }),
}));

vi.mock('@/lib/ipc/commands', () => ({
  isAppError: (err: unknown) =>
    typeof err === 'object' &&
    err !== null &&
    'code' in err &&
    'message' in err &&
    typeof (err as { message: unknown }).message === 'string',
}));

import App from '@/app/App.vue';
import { useAppBootstrapStore } from '@/app/bootstrapStore';

describe('App.vue runtime initialization error handling', () => {
  const originalWindow = globalThis.window;

  beforeEach(() => {
    vi.clearAllMocks();
    onMountedCallbacks.length = 0;
    mockOnboardingRefresh.mockResolvedValue(null);
  });

  afterEach(() => {
    Object.defineProperty(globalThis, 'window', {
      value: originalWindow,
      configurable: true,
      writable: true,
    });
  });

  it('displays error banner in Tauri runtime when initialization rejects', async () => {
    Object.defineProperty(globalThis, 'window', {
      value: {
        __TAURI_INTERNALS__: {},
        location: { search: '' },
      },
      configurable: true,
      writable: true,
    });

    mockSettingsInit.mockRejectedValueOnce({
      code: 'internalError',
      message: '后端配置加载失败',
      detail: 'settings lock poisoned',
    });

    const pinia = createPinia();
    setActivePinia(pinia);

    const probeApp = createSSRApp(App);
    probeApp.use(pinia);
    await renderToString(probeApp);

    expect(onMountedCallbacks).toHaveLength(1);
    onMountedCallbacks[0]?.();
    await new Promise((resolve) => setTimeout(resolve, 0));

    const bootstrapStore = useAppBootstrapStore();
    expect(bootstrapStore.initError).toBe('后端配置加载失败（settings lock poisoned）');

    const renderApp = createSSRApp(App);
    renderApp.use(pinia);
    const html = await renderToString(renderApp);
    expect(html).toContain('data-testid="app-init-error-banner"');
    expect(html).toContain('应用初始化失败');
    expect(html).toContain('后端配置加载失败（settings lock poisoned）');
  });

  it('silently ignores initialization rejection in non-Tauri browser preview', async () => {
    Object.defineProperty(globalThis, 'window', {
      value: {
        location: { search: '' },
      },
      configurable: true,
      writable: true,
    });

    mockSettingsInit.mockRejectedValueOnce(new Error('Tauri IPC unavailable in browser'));

    const pinia = createPinia();
    setActivePinia(pinia);

    const probeApp = createSSRApp(App);
    probeApp.use(pinia);
    await renderToString(probeApp);

    expect(onMountedCallbacks).toHaveLength(1);
    onMountedCallbacks[0]?.();
    await new Promise((resolve) => setTimeout(resolve, 0));

    const bootstrapStore = useAppBootstrapStore();
    expect(bootstrapStore.initError).toBeNull();

    const renderApp = createSSRApp(App);
    renderApp.use(pinia);
    const html = await renderToString(renderApp);
    expect(html).not.toContain('data-testid="app-init-error-banner"');
  });
});
