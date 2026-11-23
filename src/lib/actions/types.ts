export type ActionCategory = 'global' | 'quickbar' | 'window' | 'editor' | 'external' | 'panel';

export type ActionScope =
  'global' | 'modal' | 'quickbar' | 'terminal' | 'editor' | 'list' | 'main-window';

export type ActionMenuGroup = 'format' | 'view' | 'tools' | 'editor-context';

export interface ActionContext {
  activeScope: ActionScope;
  activeScopes?: ActionScope[];
  hasSelection?: boolean;
  isInbox?: boolean;
  isMarkdown?: boolean;
  hasActiveDoc?: boolean;
  hasTerminalSelection?: boolean;
  linkAtCursor?: string | null;
  payload?: Record<string, unknown>;
}

export interface ActionDef {
  id: string;
  title: string;
  category: ActionCategory;
  defaultKeys: string | null;
  scope: ActionScope;
  when?: (ctx: ActionContext) => boolean;
  swallowWhenDisabled?: boolean;
  menuGroup?: ActionMenuGroup;
  run: (ctx: ActionContext) => void | Promise<void>;
}

export type KeybindingOverrideMap = Record<string, string | null>;

export interface ShortcutConflict {
  actionId: string;
  title: string;
  scope: ActionScope;
  keys: string;
}

export interface ResolvedMenuItem {
  id: string;
  title: string;
  shortcutLabel: string;
  disabled: boolean;
  menuGroup: ActionMenuGroup;
}

export interface DispatchKeyResult {
  handled: boolean;
  swallowed: boolean;
  actionId: string | null;
}

export type SetKeyOverrideResult = { ok: true } | { ok: false; conflicts: ShortcutConflict[] };

export type ImportOverridesResult =
  { ok: true; applied: number } | { ok: false; error: string; conflicts?: ShortcutConflict[] };
