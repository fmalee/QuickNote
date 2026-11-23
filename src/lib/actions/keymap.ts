const MODIFIER_ORDER = ['ctrl', 'alt', 'shift', 'cmd'] as const;
type ModifierName = (typeof MODIFIER_ORDER)[number];

const MODIFIER_ALIASES: Record<string, ModifierName> = {
  ctrl: 'ctrl',
  control: 'ctrl',
  alt: 'alt',
  option: 'alt',
  opt: 'alt',
  shift: 'shift',
  cmd: 'cmd',
  command: 'cmd',
  meta: 'cmd',
  mod: 'cmd',
};

const KEY_ALIASES: Record<string, string> = {
  return: 'enter',
  enter: 'enter',
  esc: 'escape',
  escape: 'escape',
  spacebar: 'space',
  space: 'space',
  ' ': 'space',
  tab: 'tab',
  backspace: 'backspace',
  delete: 'delete',
  del: 'delete',
  up: 'arrowup',
  arrowup: 'arrowup',
  down: 'arrowdown',
  arrowdown: 'arrowdown',
  left: 'arrowleft',
  arrowleft: 'arrowleft',
  right: 'arrowright',
  arrowright: 'arrowright',
  comma: ',',
  period: '.',
};

const CODE_TO_KEY: Record<string, string> = {
  Space: 'space',
  Enter: 'enter',
  NumpadEnter: 'enter',
  Escape: 'escape',
  Tab: 'tab',
  Backspace: 'backspace',
  Delete: 'delete',
  ArrowUp: 'arrowup',
  ArrowDown: 'arrowdown',
  ArrowLeft: 'arrowleft',
  ArrowRight: 'arrowright',
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backslash: '\\',
  BracketLeft: '[',
  BracketRight: ']',
  Semicolon: ';',
  Quote: "'",
  Backquote: '`',
  Minus: '-',
  Equal: '=',
};

const MODIFIER_ONLY_CODES = new Set([
  'MetaLeft',
  'MetaRight',
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  'ShiftLeft',
  'ShiftRight',
]);

const MODIFIER_ONLY_KEYS = new Set(['Meta', 'Control', 'Alt', 'Shift', 'OS']);

const DISPLAY_KEY_MAP: Record<string, string> = {
  space: 'Space',
  enter: '↵',
  escape: 'Esc',
  tab: 'Tab',
  backspace: '⌫',
  delete: '⌦',
  arrowup: '↑',
  arrowdown: '↓',
  arrowleft: '←',
  arrowright: '→',
};

const DISPLAY_MODIFIER_MAP: Record<ModifierName, string> = {
  ctrl: '⌃',
  alt: '⌥',
  shift: '⇧',
  cmd: '⌘',
};

function normalizePrimaryKey(rawKey: string): string | null {
  if (rawKey === ' ') {
    return 'space';
  }
  const trimmed = rawKey.trim().toLowerCase();
  if (!trimmed) {
    return null;
  }
  if (trimmed in MODIFIER_ALIASES) {
    return null;
  }
  return KEY_ALIASES[trimmed] ?? trimmed;
}

/**
 * 规范化组合键为固定顺序 `ctrl+alt+shift+cmd+<key>`（小写）。
 */
export function normalizeKeyCombo(raw: string | null): string | null {
  if (raw === null) {
    return null;
  }
  if (raw === ' ') {
    return 'space';
  }
  const trimmed = raw.trim();
  if (!trimmed) {
    return null;
  }

  const tokens = trimmed.split('+');
  const modifiers = new Set<ModifierName>();
  let primaryKey: string | null = null;

  for (const rawToken of tokens) {
    if (rawToken === ' ' && tokens.length > 1) {
      if (primaryKey !== null) {
        return null;
      }
      primaryKey = 'space';
      continue;
    }

    const token = rawToken.trim().toLowerCase();
    if (!token) {
      return null;
    }

    const mod = MODIFIER_ALIASES[token];
    if (mod) {
      modifiers.add(mod);
      continue;
    }

    const normalized = normalizePrimaryKey(token);
    if (!normalized || primaryKey !== null) {
      return null;
    }
    primaryKey = normalized;
  }

  if (primaryKey === null) {
    return null;
  }

  const parts: string[] = [];
  for (const mod of MODIFIER_ORDER) {
    if (modifiers.has(mod)) {
      parts.push(mod);
    }
  }
  parts.push(primaryKey);
  return parts.join('+');
}

function resolveKeyFromCode(code: string | undefined): string | null {
  if (!code) {
    return null;
  }
  if (MODIFIER_ONLY_CODES.has(code)) {
    return null;
  }
  if (code in CODE_TO_KEY) {
    return CODE_TO_KEY[code] ?? null;
  }
  if (/^Key[A-Z]$/.test(code)) {
    return code.slice(3).toLowerCase();
  }
  if (/^Digit[0-9]$/.test(code)) {
    return code.slice(5);
  }
  if (/^Numpad[0-9]$/.test(code)) {
    return code.slice(6);
  }
  if (/^F([1-9]|1[0-2])$/.test(code)) {
    return code.toLowerCase();
  }
  return null;
}

/**
 * 将浏览器 KeyboardEvent 转换为规范化组合键。
 * 优先使用 event.code，避免 macOS Option 组合键产生特殊字符导致匹配失效。
 */
export function keyboardEventToCombo(event: KeyboardEvent): string | null {
  if (MODIFIER_ONLY_CODES.has(event.code) || MODIFIER_ONLY_KEYS.has(event.key)) {
    return null;
  }

  const primaryKey =
    resolveKeyFromCode(event.code) ??
    (event.key && !MODIFIER_ONLY_KEYS.has(event.key) ? normalizePrimaryKey(event.key) : null);

  if (!primaryKey) {
    return null;
  }

  const parts: string[] = [];
  if (event.ctrlKey) parts.push('ctrl');
  if (event.altKey) parts.push('alt');
  if (event.shiftKey) parts.push('shift');
  if (event.metaKey) parts.push('cmd');
  parts.push(primaryKey);

  return parts.join('+');
}

/**
 * 将规范化组合键格式化为标准 macOS 展示文本（如 `alt+cmd+c` → `⌥⌘C`）。
 */
export function formatKeyComboDisplay(combo: string | null): string {
  const normalized = normalizeKeyCombo(combo);
  if (!normalized) {
    return '';
  }

  const parts = normalized.split('+');
  const key = parts[parts.length - 1];
  if (!key) {
    return '';
  }

  const modifierSymbols = parts
    .slice(0, -1)
    .map((part) => DISPLAY_MODIFIER_MAP[part as ModifierName] ?? '')
    .join('');

  const keyDisplay =
    DISPLAY_KEY_MAP[key] ??
    (key.length === 1 ? key.toUpperCase() : key.charAt(0).toUpperCase() + key.slice(1));

  return `${modifierSymbols}${keyDisplay}`;
}
