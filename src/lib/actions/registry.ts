import { formatKeyComboDisplay, keyboardEventToCombo, normalizeKeyCombo } from './keymap';
import type {
  ActionContext,
  ActionDef,
  ActionMenuGroup,
  ActionScope,
  DispatchKeyResult,
  ImportOverridesResult,
  KeybindingOverrideMap,
  ResolvedMenuItem,
  SetKeyOverrideResult,
  ShortcutConflict,
} from './types';

export const ACTION_KEYBINDINGS_STORAGE_KEY = 'quicknote:action-keybindings-v1';

export const SCOPE_PRIORITY: readonly ActionScope[] = [
  'modal',
  'quickbar',
  'terminal',
  'editor',
  'list',
  'main-window',
  'global',
];

export const NATIVE_MENU_ACTION_IDS = [
  'window.newNote',
  'window.quickOpen',
  'window.commandBook',
  'editor.bold',
  'editor.italic',
  'editor.codeBlock',
  'editor.table',
  'editor.toggleTodo',
  'editor.link',
  'editor.image',
  'editor.bulletList',
  'editor.blockquote',
  'editor.heading1',
  'editor.heading2',
  'editor.heading3',
  'editor.heading4',
  'editor.heading5',
  'editor.heading6',
  'panel.toggle.outline',
  'panel.toggle.related',
  'panel.toggle.terminal',
] as const;

const FIXED_NATIVE_MENU_ACCELERATORS: Readonly<Record<string, string>> = {
  menu_settings: 'cmd+,',
};

/**
 * 收集需要同步至 macOS 原生菜单栏的 accelerator 映射表；
 * `clear=true` 时全部置 `null`（用于设置页录制快捷键防抢键）。
 */
export function collectNativeMenuAccelerators(
  registry: ActionRegistry,
  clear = false,
): Record<string, string | null> {
  const entries: Record<string, string | null> = {};
  for (const id of NATIVE_MENU_ACTION_IDS) {
    entries[id] = clear ? null : registry.getEffectiveKeys(id);
  }
  for (const [id, combo] of Object.entries(FIXED_NATIVE_MENU_ACCELERATORS)) {
    entries[id] = clear ? null : combo;
  }
  return entries;
}

/**
 * 判断两个作用域是否可能同时处于活跃状态并产生快捷键冲突。
 */
export function scopesCanConflict(a: ActionScope, b: ActionScope): boolean {
  if (a === b) {
    return true;
  }
  if (a === 'global' || b === 'global') {
    return true;
  }
  if (
    (a === 'editor' && b === 'main-window') ||
    (a === 'main-window' && b === 'editor') ||
    (a === 'list' && b === 'main-window') ||
    (a === 'main-window' && b === 'list')
  ) {
    return true;
  }
  return false;
}

const SCOPE_RANK: Record<ActionScope, number> = {
  modal: 0,
  quickbar: 1,
  terminal: 2,
  editor: 3,
  list: 4,
  'main-window': 5,
  global: 6,
};

export interface ActionRegistryOptions {
  storageKey?: string;
  storage?: Storage | null;
}

export interface BindHandlerOptions {
  when?: ActionDef['when'];
  swallowWhenDisabled?: boolean;
}

export interface ActionRegistry {
  register(def: ActionDef): () => void;
  registerMany(defs: ActionDef[]): () => void;
  bindHandler(id: string, handler: ActionDef['run'], options?: BindHandlerOptions): () => void;
  getAll(): ActionDef[];
  getById(id: string): ActionDef | undefined;
  getEffectiveKeys(id: string): string | null;
  detectConflicts(actionId: string, candidateKeys: string | null): ShortcutConflict[];
  setKeyOverride(actionId: string, keys: string | null): SetKeyOverrideResult;
  resetKeyOverride(actionId: string): void;
  resetAllOverrides(): void;
  exportOverridesJson(): string;
  importOverridesJson(rawJson: string): ImportOverridesResult;
  syncFromStorage(): void;
  dispatchKeyEvent(event: KeyboardEvent, ctx: ActionContext): DispatchKeyResult;
  executeById(id: string, ctx: ActionContext): Promise<boolean>;
  getMenuItems(menuGroup: ActionMenuGroup, ctx: ActionContext): ResolvedMenuItem[];
}

function resolveDefaultStorage(): Storage | null {
  try {
    if (typeof globalThis !== 'undefined' && 'localStorage' in globalThis) {
      return globalThis.localStorage;
    }
  } catch {
    // 沙盒或无 DOM 环境容错
  }
  return null;
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function resolveOrderedScopes(ctx: ActionContext): ActionScope[] {
  const rawScopes: ActionScope[] =
    ctx.activeScopes && ctx.activeScopes.length > 0
      ? ctx.activeScopes
      : [ctx.activeScope, 'main-window', 'global'];

  const unique = new Set<ActionScope>();
  for (const scope of rawScopes) {
    if (scope in SCOPE_RANK) {
      unique.add(scope);
    }
  }

  return Array.from(unique).sort((a, b) => SCOPE_RANK[a] - SCOPE_RANK[b]);
}

function parseOverridesPayload(raw: unknown): KeybindingOverrideMap | null {
  if (!isPlainRecord(raw)) {
    return null;
  }

  const candidateMap = 'overrides' in raw && isPlainRecord(raw.overrides) ? raw.overrides : raw;

  const parsed: KeybindingOverrideMap = {};
  for (const [actionId, value] of Object.entries(candidateMap)) {
    if (!actionId.trim()) {
      return null;
    }
    if (value === null) {
      parsed[actionId] = null;
      continue;
    }
    if (typeof value !== 'string') {
      return null;
    }
    const normalized = normalizeKeyCombo(value);
    if (normalized === null) {
      return null;
    }
    parsed[actionId] = normalized;
  }
  return parsed;
}

export function createActionRegistry(options?: ActionRegistryOptions): ActionRegistry {
  const storageKey = options?.storageKey ?? ACTION_KEYBINDINGS_STORAGE_KEY;
  const storage = options?.storage === undefined ? resolveDefaultStorage() : options.storage;

  const actions = new Map<string, ActionDef>();
  let overrides: KeybindingOverrideMap = {};

  function readStorageOverrides(): KeybindingOverrideMap {
    if (!storage) {
      return {};
    }
    try {
      const raw = storage.getItem(storageKey);
      if (!raw) {
        return {};
      }
      const parsedJson: unknown = JSON.parse(raw);
      return parseOverridesPayload(parsedJson) ?? {};
    } catch {
      return {};
    }
  }

  function writeStorageOverrides(): void {
    if (!storage) {
      return;
    }
    try {
      storage.setItem(storageKey, JSON.stringify(overrides));
    } catch {
      // 存储不可用时静默降级
    }
  }

  function getEffectiveKeysWithMap(id: string, map: KeybindingOverrideMap): string | null {
    if (Object.prototype.hasOwnProperty.call(map, id)) {
      return map[id] ?? null;
    }
    return actions.get(id)?.defaultKeys ?? null;
  }

  function detectConflictsWithMap(
    actionId: string,
    candidateKeys: string | null,
    map: KeybindingOverrideMap,
  ): ShortcutConflict[] {
    const normalized = normalizeKeyCombo(candidateKeys);
    if (!normalized) {
      return [];
    }
    const target = actions.get(actionId);
    if (!target) {
      return [];
    }

    const conflicts: ShortcutConflict[] = [];
    for (const other of actions.values()) {
      if (other.id === actionId || !scopesCanConflict(other.scope, target.scope)) {
        continue;
      }
      if (getEffectiveKeysWithMap(other.id, map) === normalized) {
        conflicts.push({
          actionId: other.id,
          title: other.title,
          scope: other.scope,
          keys: normalized,
        });
      }
    }
    return conflicts;
  }

  function syncFromStorage(): void {
    overrides = readStorageOverrides();
  }

  syncFromStorage();

  function register(def: ActionDef): () => void {
    const normalizedDef: ActionDef = {
      ...def,
      defaultKeys: normalizeKeyCombo(def.defaultKeys),
    };
    actions.set(normalizedDef.id, normalizedDef);
    return () => {
      const current = actions.get(normalizedDef.id);
      if (current === normalizedDef) {
        actions.delete(normalizedDef.id);
      }
    };
  }

  function registerMany(defs: ActionDef[]): () => void {
    const disposers = defs.map((def) => register(def));
    return () => {
      for (const dispose of disposers) {
        dispose();
      }
    };
  }

  function bindHandler(
    id: string,
    handler: ActionDef['run'],
    bindOptions?: BindHandlerOptions,
  ): () => void {
    const existing = actions.get(id);
    if (!existing) {
      return () => {};
    }

    const prevRun = existing.run;
    const prevWhen = existing.when;
    const prevSwallow = existing.swallowWhenDisabled;

    existing.run = handler;
    if (bindOptions && 'when' in bindOptions) {
      existing.when = bindOptions.when;
    }
    if (bindOptions && 'swallowWhenDisabled' in bindOptions) {
      existing.swallowWhenDisabled = bindOptions.swallowWhenDisabled;
    }

    return () => {
      const current = actions.get(id);
      if (!current || current.run !== handler) {
        return;
      }
      current.run = prevRun;
      current.when = prevWhen;
      current.swallowWhenDisabled = prevSwallow;
    };
  }

  function getAll(): ActionDef[] {
    return Array.from(actions.values());
  }

  function getById(id: string): ActionDef | undefined {
    return actions.get(id);
  }

  function getEffectiveKeys(id: string): string | null {
    return getEffectiveKeysWithMap(id, overrides);
  }

  function detectConflicts(actionId: string, candidateKeys: string | null): ShortcutConflict[] {
    return detectConflictsWithMap(actionId, candidateKeys, overrides);
  }

  function setKeyOverride(actionId: string, keys: string | null): SetKeyOverrideResult {
    const target = actions.get(actionId);
    if (!target) {
      return { ok: false, conflicts: [] };
    }

    let normalized: string | null = null;
    if (keys !== null) {
      normalized = normalizeKeyCombo(keys);
      if (normalized === null) {
        return { ok: false, conflicts: [] };
      }
      const conflicts = detectConflictsWithMap(actionId, normalized, overrides);
      if (conflicts.length > 0) {
        return { ok: false, conflicts };
      }
    }

    const nextOverrides: KeybindingOverrideMap = { ...overrides };
    if (normalized === target.defaultKeys) {
      delete nextOverrides[actionId];
    } else {
      nextOverrides[actionId] = normalized;
    }

    overrides = nextOverrides;
    writeStorageOverrides();
    return { ok: true };
  }

  function resetKeyOverride(actionId: string): void {
    if (!Object.prototype.hasOwnProperty.call(overrides, actionId)) {
      return;
    }
    const nextOverrides: KeybindingOverrideMap = { ...overrides };
    delete nextOverrides[actionId];
    overrides = nextOverrides;
    writeStorageOverrides();
  }

  function resetAllOverrides(): void {
    overrides = {};
    writeStorageOverrides();
  }

  function exportOverridesJson(): string {
    return JSON.stringify(overrides, null, 2);
  }

  function importOverridesJson(rawJson: string): ImportOverridesResult {
    let parsedJson: unknown;
    try {
      parsedJson = JSON.parse(rawJson);
    } catch {
      return { ok: false, error: '无效的 JSON 格式' };
    }

    const candidateMap = parseOverridesPayload(parsedJson);
    if (!candidateMap) {
      return { ok: false, error: '快捷键配置格式不合法' };
    }

    const conflicts: ShortcutConflict[] = [];
    for (const [actionId, keys] of Object.entries(candidateMap)) {
      if (keys === null) {
        continue;
      }
      const detected = detectConflictsWithMap(actionId, keys, candidateMap);
      for (const item of detected) {
        conflicts.push(item);
      }
    }

    if (conflicts.length > 0) {
      return {
        ok: false,
        error: '导入的快捷键配置存在同作用域冲突',
        conflicts,
      };
    }

    overrides = { ...candidateMap };
    writeStorageOverrides();
    return { ok: true, applied: Object.keys(candidateMap).length };
  }

  function dispatchKeyEvent(event: KeyboardEvent, ctx: ActionContext): DispatchKeyResult {
    const combo = keyboardEventToCombo(event);
    if (!combo) {
      return { handled: false, swallowed: false, actionId: null };
    }

    const scopes = resolveOrderedScopes(ctx);
    for (const scope of scopes) {
      for (const action of actions.values()) {
        if (action.scope !== scope) {
          continue;
        }
        if (getEffectiveKeys(action.id) !== combo) {
          continue;
        }

        const isAllowed = action.when === undefined || action.when(ctx) === true;
        if (isAllowed) {
          event.preventDefault();
          event.stopPropagation();
          void action.run(ctx);
          return { handled: true, swallowed: false, actionId: action.id };
        }

        if (action.swallowWhenDisabled === true) {
          event.preventDefault();
          event.stopPropagation();
          return { handled: false, swallowed: true, actionId: action.id };
        }
      }
    }

    return { handled: false, swallowed: false, actionId: null };
  }

  async function executeById(id: string, ctx: ActionContext): Promise<boolean> {
    const action = actions.get(id);
    if (!action) {
      return false;
    }
    if (action.when && !action.when(ctx)) {
      return false;
    }
    await action.run(ctx);
    return true;
  }

  function getMenuItems(menuGroup: ActionMenuGroup, ctx: ActionContext): ResolvedMenuItem[] {
    const items: ResolvedMenuItem[] = [];
    for (const action of actions.values()) {
      if (action.menuGroup !== menuGroup) {
        continue;
      }
      const disabled = action.when ? !action.when(ctx) : false;
      items.push({
        id: action.id,
        title: action.title,
        shortcutLabel: formatKeyComboDisplay(getEffectiveKeys(action.id)),
        disabled,
        menuGroup,
      });
    }
    return items;
  }

  return {
    register,
    registerMany,
    bindHandler,
    getAll,
    getById,
    getEffectiveKeys,
    detectConflicts,
    setKeyOverride,
    resetKeyOverride,
    resetAllOverrides,
    exportOverridesJson,
    importOverridesJson,
    syncFromStorage,
    dispatchKeyEvent,
    executeById,
    getMenuItems,
  };
}
