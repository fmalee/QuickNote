import { createPinia, setActivePinia } from 'pinia';
import { createApp, createSSRApp, nextTick } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NoteSummary, SettingsPatch, SettingsSnapshot, UiState } from '@/bindings/types';
import { actionRegistry } from '@/lib/actions';

const {
  mockSettingsGet,
  mockSettingsUpdate,
  mockUiStateGet,
  mockHotkeyUpdate,
  mockLibrarySetRoot,
  mockNotesList,
  mockNoteCreate,
  mockOnSettingsStatus,
  mockOnWindowState,
} = vi.hoisted(() => ({
  mockSettingsGet: vi.fn(),
  mockSettingsUpdate: vi.fn(),
  mockUiStateGet: vi.fn(),
  mockHotkeyUpdate: vi.fn(),
  mockLibrarySetRoot: vi.fn(),
  mockNotesList: vi.fn(),
  mockNoteCreate: vi.fn(),
  mockOnSettingsStatus: vi.fn(),
  mockOnWindowState: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  settingsGet: mockSettingsGet,
  settingsUpdate: mockSettingsUpdate,
  uiStateGet: mockUiStateGet,
  hotkeyUpdate: mockHotkeyUpdate,
  librarySetRoot: mockLibrarySetRoot,
  notesList: mockNotesList,
  noteCreate: mockNoteCreate,
  isAppError: () => false,
}));

vi.mock('@/lib/ipc/events', () => ({
  onSettingsStatus: mockOnSettingsStatus,
  onWindowState: mockOnWindowState,
  listenEvent: vi.fn(),
}));

import {
  SETTINGS_READONLY_ERROR,
  SettingsShortcutsPanel,
  SettingsView,
  useSettingsStore,
} from '@/features/settings';

function makeValidSettingsSnapshot(overrides?: Partial<SettingsSnapshot>): SettingsSnapshot {
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
        fontSize: 14,
        lineHeight: 1.6,
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
      activeSource: 'file',
      errorLine: null,
      errorColumn: null,
      errorMessage: null,
    },
    ...overrides,
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

describe('settings-update-and-shortcuts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    actionRegistry.resetAllOverrides();
    mockOnSettingsStatus.mockResolvedValue(() => {});
    mockOnWindowState.mockResolvedValue(() => {});
    mockUiStateGet.mockResolvedValue(makeUiState());
    mockNotesList.mockResolvedValue([]);
  });

  afterEach(() => {
    actionRegistry.resetAllOverrides();
  });

  it('calls settingsUpdate and updates snapshot & DOM data-theme on two-way binding changes', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const initial = makeValidSettingsSnapshot();
    mockSettingsGet.mockResolvedValueOnce(initial);

    let currentSnapshot = structuredClone(initial);
    mockSettingsUpdate.mockImplementation(async (patch: SettingsPatch) => {
      currentSnapshot = {
        ...currentSnapshot,
        settings: {
          ...currentSnapshot.settings,
          editor: {
            ...currentSnapshot.settings.editor,
            theme: patch.editor?.theme ?? currentSnapshot.settings.editor.theme,
            fontSize: patch.editor?.fontSize ?? currentSnapshot.settings.editor.fontSize,
            maxWidth: patch.editor?.maxWidth ?? currentSnapshot.settings.editor.maxWidth,
            singleClickLink:
              patch.editor?.singleClickLink ?? currentSnapshot.settings.editor.singleClickLink,
            showTabCloseButton:
              patch.editor?.showTabCloseButton ??
              currentSnapshot.settings.editor.showTabCloseButton,
          },
          inbox: {
            ...currentSnapshot.settings.inbox,
            bloatThresholdLines:
              patch.inbox?.bloatThresholdLines ??
              currentSnapshot.settings.inbox.bloatThresholdLines,
          },
          quickBar: {
            ...currentSnapshot.settings.quickBar,
            autoCreateMissingSection:
              patch.quickBar?.autoCreateMissingSection ??
              currentSnapshot.settings.quickBar.autoCreateMissingSection,
          },
        },
      };
      return currentSnapshot;
    });

    const store = useSettingsStore();
    await store.initialize();

    expect(store.showTabCloseButton).toBe(false);

    const okEditor = await store.patchSettings({
      editor: { fontSize: 18, maxWidth: 720, singleClickLink: true, showTabCloseButton: true },
      inbox: { bloatThresholdLines: 300 },
      quickBar: { autoCreateMissingSection: true },
    });

    expect(okEditor).toBe(true);
    expect(mockSettingsUpdate).toHaveBeenCalledWith({
      editor: { fontSize: 18, maxWidth: 720, singleClickLink: true, showTabCloseButton: true },
      inbox: { bloatThresholdLines: 300 },
      quickBar: { autoCreateMissingSection: true },
    });
    expect(store.snapshot?.settings.editor.fontSize).toBe(18);
    expect(store.snapshot?.settings.editor.maxWidth).toBe(720);
    expect(store.snapshot?.settings.editor.singleClickLink).toBe(true);
    expect(store.showTabCloseButton).toBe(true);
    expect(store.snapshot?.settings.inbox.bloatThresholdLines).toBe(300);
    expect(store.snapshot?.settings.quickBar.autoCreateMissingSection).toBe(true);

    // 切换主题同时更新 DOM data-theme（如果存在 document）并触发持久化
    const setAttrSpy =
      typeof document !== 'undefined' ? vi.spyOn(document.documentElement, 'setAttribute') : null;
    store.setTheme('dark', { persist: true });
    await nextTick();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(store.themePreference).toBe('dark');
    expect(mockSettingsUpdate).toHaveBeenLastCalledWith({ editor: { theme: 'dark' } });
    if (setAttrSpy) {
      expect(setAttrSpy).toHaveBeenCalledWith('data-theme', 'dark');
      setAttrSpy.mockRestore();
    }

    store.dispose();
  });

  it('blocks patchSettings when settings.yaml is invalid (D-0008 readonly protection)', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    const invalid = makeValidSettingsSnapshot({
      status: {
        valid: false,
        activeSource: 'fallback_default',
        errorLine: 5,
        errorColumn: 2,
        errorMessage: 'bad yaml',
      },
    });
    mockSettingsGet.mockResolvedValueOnce(invalid);

    const store = useSettingsStore();
    await store.initialize();

    expect(store.isSettingsInvalid).toBe(true);

    const ok = await store.patchSettings({ editor: { fontSize: 20 } });
    expect(ok).toBe(false);
    expect(mockSettingsUpdate).not.toHaveBeenCalled();
    expect(store.saveError).toBe(SETTINGS_READONLY_ERROR);

    store.dispose();
  });

  it('records valid shortcut, rejects same-scope conflict, and supports reset & JSON export/import', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    mockSettingsGet.mockResolvedValueOnce(makeValidSettingsSnapshot());
    const store = useSettingsStore();
    await store.initialize();

    // 使用 SSR 验证初始渲染正常且包含核心测试锚点
    store.activeSection = 'shortcuts';
    const ssrApp = createSSRApp(SettingsView);
    ssrApp.use(pinia);
    const ssrHtml = await renderToString(ssrApp);
    expect(ssrHtml).toContain('data-testid="shortcuts-search-input"');
    expect(ssrHtml).toContain('data-testid="shortcut-row-editor.italic"');

    // 验证同作用域冲突拦截与合法录制（直接在 Node/happy-dom 无依赖环境测试面板逻辑）
    if (typeof document !== 'undefined') {
      const host = document.createElement('div');
      document.body.appendChild(host);
      const app = createApp(SettingsShortcutsPanel);
      app.use(pinia);
      const vm = app.mount(host) as unknown as {
        startRecording: (id: string) => void;
        handleRecordKeydown: (e: KeyboardEvent) => void;
      };

      // 1) 尝试把 editor.italic 录制成 cmd+b（与 editor.bold 冲突）
      vm.startRecording('editor.italic');
      await nextTick();
      expect(host.querySelector('[data-testid="shortcut-recording-hint"]')).not.toBeNull();

      vm.handleRecordKeydown(
        new KeyboardEvent('keydown', { key: 'b', code: 'KeyB', metaKey: true }),
      );
      await nextTick();

      const conflictBanner = host.querySelector('[data-testid="shortcut-conflict-banner"]');
      expect(conflictBanner).not.toBeNull();
      expect(conflictBanner?.textContent).toContain('editor.bold');
      // 原键位保持 cmd+i 不变
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('cmd+i');

      // 2) 录制合法无冲突组合键 ctrl+shift+i
      vm.startRecording('editor.italic');
      vm.handleRecordKeydown(
        new KeyboardEvent('keydown', {
          key: 'i',
          code: 'KeyI',
          ctrlKey: true,
          shiftKey: true,
        }),
      );
      await nextTick();
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('ctrl+shift+i');
      expect(host.querySelector('[data-testid="shortcut-conflict-banner"]')).toBeNull();

      // 3) 导出 JSON → 全部重置 → 导入 JSON 恢复
      const exported = actionRegistry.exportOverridesJson();
      expect(exported).toContain('"editor.italic": "ctrl+shift+i"');

      actionRegistry.resetKeyOverride('editor.italic');
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('cmd+i');

      const imported = actionRegistry.importOverridesJson(exported);
      expect(imported.ok).toBe(true);
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('ctrl+shift+i');

      actionRegistry.resetAllOverrides();
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('cmd+i');

      app.unmount();
      host.remove();
    } else {
      // 纯 Node SSR fallback 校验 registry 行为
      const conflictRes = actionRegistry.setKeyOverride('editor.italic', 'cmd+b');
      expect(conflictRes.ok).toBe(false);
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('cmd+i');

      const okRes = actionRegistry.setKeyOverride('editor.italic', 'ctrl+shift+i');
      expect(okRes.ok).toBe(true);
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('ctrl+shift+i');

      const exported = actionRegistry.exportOverridesJson();
      actionRegistry.resetAllOverrides();
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('cmd+i');

      const importRes = actionRegistry.importOverridesJson(exported);
      expect(importRes.ok).toBe(true);
      expect(actionRegistry.getEffectiveKeys('editor.italic')).toBe('ctrl+shift+i');
    }

    store.dispose();
  });

  it('loads template notes via notesList({ tab: "templates" }) and creates custom template', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    mockSettingsGet.mockResolvedValueOnce(makeValidSettingsSnapshot());
    const sampleTemplates: NoteSummary[] = [
      {
        key: '_templates/note.md',
        title: 'note',
        relPath: '_templates/note.md',
        absPath: '/Users/demo/Documents/QuickNote/_templates/note.md',
        docType: 'template',
        tags: [],
        excerpt: '',
        pinned: false,
        archived: false,
        isInbox: false,
        mtimeMs: 1000,
        metaError: null,
      },
      {
        key: '_templates/doc.md',
        title: 'doc',
        relPath: '_templates/doc.md',
        absPath: '/Users/demo/Documents/QuickNote/_templates/doc.md',
        docType: 'template',
        tags: [],
        excerpt: '',
        pinned: false,
        archived: false,
        isInbox: false,
        mtimeMs: 1000,
        metaError: null,
      },
    ];
    mockNotesList.mockResolvedValueOnce(sampleTemplates);

    const store = useSettingsStore();
    await store.initialize();

    const loaded = await store.loadTemplates();
    expect(mockNotesList).toHaveBeenCalledWith({
      tab: 'templates',
      dirRelPath: null,
      tags: [],
      filterText: null,
    });
    expect(loaded).toHaveLength(2);
    expect(store.templates.map((t) => t.title)).toEqual(['note', 'doc']);

    mockNoteCreate.mockResolvedValueOnce({
      key: '_templates/meeting.md',
      relPath: '_templates/meeting.md',
      absPath: '/Users/demo/Documents/QuickNote/_templates/meeting.md',
      isLibrary: true,
      isInbox: false,
      meta: {
        id: '01J00000000000000000000003',
        title: 'meeting',
        docType: 'template',
        tags: [],
        openDir: null,
        codeWorkspace: null,
        terminalDir: null,
        created: null,
        pinned: false,
        archived: false,
        metaError: null,
      },
      body: '# meeting\n',
      bodyHash: 'abc',
      mtimeMs: 2000,
      sizeBytes: 10,
    });
    mockNotesList.mockResolvedValueOnce([
      ...sampleTemplates,
      {
        key: '_templates/meeting.md',
        title: 'meeting',
        relPath: '_templates/meeting.md',
        absPath: '/Users/demo/Documents/QuickNote/_templates/meeting.md',
        docType: 'template',
        tags: [],
        excerpt: '',
        pinned: false,
        archived: false,
        isInbox: false,
        mtimeMs: 2000,
        metaError: null,
      },
    ]);

    const created = await store.addTemplate('meeting');
    expect(mockNoteCreate).toHaveBeenCalledWith({
      title: 'meeting',
      dirRelPath: '_templates',
      templateName: 'note',
      body: null,
    });
    expect(created?.meta.title).toBe('meeting');
    expect(store.templates).toHaveLength(3);

    store.dispose();
  });
});
