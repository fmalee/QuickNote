import { describe, expect, it, vi } from 'vitest';
import {
  ACTION_KEYBINDINGS_STORAGE_KEY,
  SCOPE_PRIORITY,
  createActionRegistry,
  formatKeyComboDisplay,
  keyboardEventToCombo,
  normalizeKeyCombo,
  registerDefaultActionCatalog,
  scopesCanConflict,
  type ActionContext,
} from '../index';

function createMemoryStorage(): Storage {
  const store = new Map<string, string>();
  return {
    get length() {
      return store.size;
    },
    clear() {
      store.clear();
    },
    getItem(key: string) {
      return store.get(key) ?? null;
    },
    key(index: number) {
      return Array.from(store.keys())[index] ?? null;
    },
    removeItem(key: string) {
      store.delete(key);
    },
    setItem(key: string, value: string) {
      store.set(key, value);
    },
  };
}

function createFakeKeyboardEvent(init: {
  key: string;
  code?: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  metaKey?: boolean;
}): {
  event: KeyboardEvent;
  preventDefault: ReturnType<typeof vi.fn>;
  stopPropagation: ReturnType<typeof vi.fn>;
} {
  const preventDefault = vi.fn();
  const stopPropagation = vi.fn();
  const event = {
    key: init.key,
    code: init.code ?? '',
    ctrlKey: Boolean(init.ctrlKey),
    altKey: Boolean(init.altKey),
    shiftKey: Boolean(init.shiftKey),
    metaKey: Boolean(init.metaKey),
    preventDefault,
    stopPropagation,
  } as unknown as KeyboardEvent;
  return { event, preventDefault, stopPropagation };
}

describe('keymap 规范化与 macOS Option 键解析', () => {
  it('规范化固定修饰键顺序并解析常见别名', () => {
    expect(normalizeKeyCombo('Meta+Shift+Alt+Control+Spacebar')).toBe('ctrl+alt+shift+cmd+space');
    expect(normalizeKeyCombo('Option+Command+C')).toBe('alt+cmd+c');
    expect(normalizeKeyCombo('mod+return')).toBe('cmd+enter');
    expect(normalizeKeyCombo('esc')).toBe('escape');
    expect(normalizeKeyCombo(' ')).toBe('space');
    expect(normalizeKeyCombo('Meta')).toBeNull();
    expect(normalizeKeyCombo('cmd+a+b')).toBeNull();
  });

  it('macOS 下优先基于 event.code 解析 Option 组合键（如 ⌥⌘C 产生 ç）', () => {
    const { event: optCmdC } = createFakeKeyboardEvent({
      key: 'ç',
      code: 'KeyC',
      altKey: true,
      metaKey: true,
    });
    expect(keyboardEventToCombo(optCmdC)).toBe('alt+cmd+c');

    const { event: optCmd1 } = createFakeKeyboardEvent({
      key: '¡',
      code: 'Digit1',
      altKey: true,
      metaKey: true,
    });
    expect(keyboardEventToCombo(optCmd1)).toBe('alt+cmd+1');

    const { event: onlyModifier } = createFakeKeyboardEvent({
      key: 'Meta',
      code: 'MetaLeft',
      metaKey: true,
    });
    expect(keyboardEventToCombo(onlyModifier)).toBeNull();
  });

  it('格式化 macOS 标准展示文本', () => {
    expect(formatKeyComboDisplay('alt+cmd+c')).toBe('⌥⌘C');
    expect(formatKeyComboDisplay('shift+tab')).toBe('⇧Tab');
    expect(formatKeyComboDisplay('cmd+enter')).toBe('⌘↵');
    expect(formatKeyComboDisplay('ctrl+cmd+o')).toBe('⌃⌘O');
    expect(formatKeyComboDisplay('ctrl+alt+space')).toBe('⌃⌥Space');
    expect(formatKeyComboDisplay(null)).toBe('');
  });
});

describe('ActionRegistry 作用域优先级、显式吞键与冲突矩阵', () => {
  it('1. 严格按 modal > quickbar > terminal > editor > list > main-window > global 优先级命中', () => {
    expect(SCOPE_PRIORITY).toEqual([
      'modal',
      'quickbar',
      'terminal',
      'editor',
      'list',
      'main-window',
      'global',
    ]);

    const registry = createActionRegistry({ storage: null });
    const hits: string[] = [];

    registry.registerMany([
      {
        id: 'test.global',
        title: 'Global',
        category: 'global',
        defaultKeys: 'cmd+k',
        scope: 'global',
        run: () => {
          hits.push('global');
        },
      },
      {
        id: 'test.main',
        title: 'Main',
        category: 'window',
        defaultKeys: 'cmd+k',
        scope: 'main-window',
        run: () => {
          hits.push('main-window');
        },
      },
      {
        id: 'test.editor',
        title: 'Editor',
        category: 'editor',
        defaultKeys: 'cmd+k',
        scope: 'editor',
        run: () => {
          hits.push('editor');
        },
      },
      {
        id: 'test.terminal',
        title: 'Terminal',
        category: 'panel',
        defaultKeys: 'cmd+k',
        scope: 'terminal',
        run: () => {
          hits.push('terminal');
        },
      },
      {
        id: 'test.modal',
        title: 'Modal',
        category: 'window',
        defaultKeys: 'cmd+k',
        scope: 'modal',
        run: () => {
          hits.push('modal');
        },
      },
    ]);

    const { event, preventDefault } = createFakeKeyboardEvent({
      key: 'k',
      code: 'KeyK',
      metaKey: true,
    });

    const result = registry.dispatchKeyEvent(event, {
      activeScope: 'modal',
      activeScopes: ['main-window', 'editor', 'terminal', 'modal', 'global'],
    });

    expect(result).toEqual({ handled: true, swallowed: false, actionId: 'test.modal' });
    expect(hits).toEqual(['modal']);
    expect(preventDefault).toHaveBeenCalledTimes(1);
  });

  it('2. when 未命中时放行不阻止默认行为；terminal.copySelection 仅在有选区时拦截', async () => {
    const registry = createActionRegistry({ storage: null });
    registerDefaultActionCatalog(registry);

    const moveRun = vi.fn();
    registry.bindHandler('editor.moveSelectionToNewNote', moveRun, {
      when: (ctx) => Boolean(ctx.isInbox && ctx.hasSelection),
    });

    const executedNoSelection = await registry.executeById('editor.moveSelectionToNewNote', {
      activeScope: 'editor',
      isInbox: true,
      hasSelection: false,
    });
    expect(executedNoSelection).toBe(false);
    expect(moveRun).not.toHaveBeenCalled();

    const termCopyRun = vi.fn();
    registry.bindHandler('terminal.copySelection', termCopyRun);

    const noTermSel = createFakeKeyboardEvent({ key: 'c', code: 'KeyC', metaKey: true });
    const resNoSel = registry.dispatchKeyEvent(noTermSel.event, {
      activeScope: 'terminal',
      activeScopes: ['terminal', 'main-window'],
      hasTerminalSelection: false,
    });
    expect(resNoSel).toEqual({ handled: false, swallowed: false, actionId: null });
    expect(noTermSel.preventDefault).not.toHaveBeenCalled();
    expect(termCopyRun).not.toHaveBeenCalled();

    const withTermSel = createFakeKeyboardEvent({ key: 'c', code: 'KeyC', metaKey: true });
    const resWithSel = registry.dispatchKeyEvent(withTermSel.event, {
      activeScope: 'terminal',
      activeScopes: ['terminal', 'main-window'],
      hasTerminalSelection: true,
    });
    expect(resWithSel).toEqual({
      handled: true,
      swallowed: false,
      actionId: 'terminal.copySelection',
    });
    expect(withTermSel.preventDefault).toHaveBeenCalledTimes(1);
    expect(termCopyRun).toHaveBeenCalledTimes(1);
  });

  it('3. 焦点在 terminal 时 ⌥⌘C 被显式吞键阻断穿透至 editor.codeBlock，未注册 ⌃C 放行直传 PTY', () => {
    const registry = createActionRegistry({ storage: null });
    registerDefaultActionCatalog(registry);

    const codeBlockRun = vi.fn();
    registry.bindHandler('editor.codeBlock', codeBlockRun);

    const optCmdC = createFakeKeyboardEvent({
      key: 'ç',
      code: 'KeyC',
      altKey: true,
      metaKey: true,
    });

    const swallowRes = registry.dispatchKeyEvent(optCmdC.event, {
      activeScope: 'terminal',
      activeScopes: ['terminal', 'editor', 'main-window'],
    });

    expect(swallowRes).toEqual({
      handled: false,
      swallowed: true,
      actionId: 'terminal.swallowCodeBlock',
    });
    expect(optCmdC.preventDefault).toHaveBeenCalledTimes(1);
    expect(optCmdC.stopPropagation).toHaveBeenCalledTimes(1);
    expect(codeBlockRun).not.toHaveBeenCalled();

    const ctrlC = createFakeKeyboardEvent({
      key: 'c',
      code: 'KeyC',
      ctrlKey: true,
    });
    const passthroughRes = registry.dispatchKeyEvent(ctrlC.event, {
      activeScope: 'terminal',
      activeScopes: ['terminal', 'editor', 'main-window'],
    });
    expect(passthroughRes).toEqual({ handled: false, swallowed: false, actionId: null });
    expect(ctrlC.preventDefault).not.toHaveBeenCalled();
  });

  it('4. 同作用域冲突检测拒绝覆盖，跨作用域允许使用相同快捷键', () => {
    const registry = createActionRegistry({ storage: null });
    registerDefaultActionCatalog(registry);

    const conflictResult = registry.setKeyOverride('editor.italic', 'cmd+b');
    expect(conflictResult).toEqual({
      ok: false,
      conflicts: [
        {
          actionId: 'editor.bold',
          title: '加粗',
          scope: 'editor',
          keys: 'cmd+b',
        },
      ],
    });
    expect(registry.getEffectiveKeys('editor.italic')).toBe('cmd+i');

    const crossScopeResult = registry.setKeyOverride('quickbar.mode.open', 'cmd+b');
    expect(crossScopeResult).toEqual({ ok: true });
    expect(registry.getEffectiveKeys('quickbar.mode.open')).toBe('cmd+b');

    // 主窗口共现作用域（main-window 与 editor/list）以及 global 必须命中冲突检测
    expect(scopesCanConflict('main-window', 'editor')).toBe(true);
    expect(scopesCanConflict('editor', 'main-window')).toBe(true);
    expect(scopesCanConflict('main-window', 'list')).toBe(true);
    expect(scopesCanConflict('global', 'editor')).toBe(true);
    expect(scopesCanConflict('quickbar', 'editor')).toBe(false);
    expect(scopesCanConflict('terminal', 'editor')).toBe(false);
    expect(scopesCanConflict('editor', 'list')).toBe(false);

    const mainWindowConflict = registry.setKeyOverride('window.newNote', 'cmd+b');
    expect(mainWindowConflict).toEqual({
      ok: false,
      conflicts: [
        {
          actionId: 'editor.bold',
          title: '加粗',
          scope: 'editor',
          keys: 'cmd+b',
        },
      ],
    });
    expect(registry.getEffectiveKeys('window.newNote')).toBe('cmd+n');

    const editorConflictWithMain = registry.setKeyOverride('editor.bold', 'cmd+s');
    expect(editorConflictWithMain).toEqual({
      ok: false,
      conflicts: [
        {
          actionId: 'window.save',
          title: '保存当前笔记',
          scope: 'main-window',
          keys: 'cmd+s',
        },
      ],
    });
  });

  it('5. 覆写持久化、跨实例 syncFromStorage、重置与 JSON 导入导出校验', () => {
    const storage = createMemoryStorage();
    const registryA = createActionRegistry({ storage });
    registerDefaultActionCatalog(registryA);

    expect(registryA.setKeyOverride('editor.bold', 'shift+cmd+b')).toEqual({ ok: true });
    expect(storage.getItem(ACTION_KEYBINDINGS_STORAGE_KEY)).toContain('shift+cmd+b');

    const registryB = createActionRegistry({ storage });
    registerDefaultActionCatalog(registryB);
    expect(registryB.getEffectiveKeys('editor.bold')).toBe('shift+cmd+b');

    const exported = registryA.exportOverridesJson();
    expect(JSON.parse(exported)).toEqual({ 'editor.bold': 'shift+cmd+b' });

    registryA.resetKeyOverride('editor.bold');
    expect(registryA.getEffectiveKeys('editor.bold')).toBe('cmd+b');

    registryB.syncFromStorage();
    expect(registryB.getEffectiveKeys('editor.bold')).toBe('cmd+b');

    expect(registryA.importOverridesJson('{invalid')).toMatchObject({ ok: false });
    expect(
      registryA.importOverridesJson(JSON.stringify({ 'editor.italic': 'cmd+b' })),
    ).toMatchObject({
      ok: false,
      conflicts: [{ actionId: 'editor.bold', keys: 'cmd+b', scope: 'editor' }],
    });
    expect(registryA.getEffectiveKeys('editor.italic')).toBe('cmd+i');

    expect(registryA.importOverridesJson(exported)).toEqual({ ok: true, applied: 1 });
    expect(registryA.getEffectiveKeys('editor.bold')).toBe('shift+cmd+b');

    registryA.resetAllOverrides();
    expect(registryA.getEffectiveKeys('editor.bold')).toBe('cmd+b');
  });

  it('6. getMenuItems 同源生成格式与上下文菜单并反映自定义快捷键与 disabled 态', () => {
    const registry = createActionRegistry({ storage: null });
    registerDefaultActionCatalog(registry);

    expect(registry.setKeyOverride('editor.bold', 'shift+cmd+b')).toEqual({ ok: true });

    const ctx: ActionContext = {
      activeScope: 'editor',
      hasActiveDoc: true,
      isInbox: true,
      hasSelection: false,
    };

    const formatItems = registry.getMenuItems('format', ctx);
    const boldItem = formatItems.find((item) => item.id === 'editor.bold');
    expect(boldItem).toEqual({
      id: 'editor.bold',
      title: '加粗',
      shortcutLabel: '⇧⌘B',
      disabled: false,
      menuGroup: 'format',
    });

    const contextDisabled = registry.getMenuItems('editor-context', ctx);
    expect(contextDisabled).toEqual([
      {
        id: 'editor.moveSelectionToNewNote',
        title: '将选中内容移至新笔记',
        shortcutLabel: '',
        disabled: true,
        menuGroup: 'editor-context',
      },
    ]);

    const contextEnabled = registry.getMenuItems('editor-context', {
      ...ctx,
      hasSelection: true,
    });
    expect(contextEnabled[0]?.disabled).toBe(false);
  });

  it('7. 无激活文档时 editor.* 菜单项禁用且快捷键放行不拦截', async () => {
    const registry = createActionRegistry({ storage: null });
    registerDefaultActionCatalog(registry);

    const boldRun = vi.fn();
    registry.bindHandler('editor.bold', boldRun);

    const noDocCtx: ActionContext = {
      activeScope: 'editor',
      activeScopes: ['editor', 'main-window', 'global'],
      hasActiveDoc: false,
      isInbox: true,
      hasSelection: true,
    };

    const formatItems = registry.getMenuItems('format', noDocCtx);
    expect(formatItems.length).toBeGreaterThan(0);
    expect(formatItems.every((item) => item.disabled)).toBe(true);

    const contextItems = registry.getMenuItems('editor-context', noDocCtx);
    expect(contextItems[0]?.disabled).toBe(true);
    expect(await registry.executeById('editor.moveSelectionToNewNote', noDocCtx)).toBe(false);

    const cmdB = createFakeKeyboardEvent({ key: 'b', code: 'KeyB', metaKey: true });
    const dispatchRes = registry.dispatchKeyEvent(cmdB.event, noDocCtx);
    expect(dispatchRes).toEqual({ handled: false, swallowed: false, actionId: null });
    expect(cmdB.preventDefault).not.toHaveBeenCalled();
    expect(boldRun).not.toHaveBeenCalled();
  });
});
