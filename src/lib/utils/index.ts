import type { EditorTheme } from '@/bindings/types';

export type WindowKind = 'main' | 'quickbar' | 'toast' | 'settings';

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function resolveWindowKind(search: string): WindowKind {
  const params = new URLSearchParams(search);
  const raw = params.get('window');
  if (raw === 'quickbar' || raw === 'toast' || raw === 'settings') {
    return raw;
  }
  return 'main';
}

export function applyThemeToDocument(theme: EditorTheme, root?: HTMLElement): void {
  const target = root ?? (typeof document !== 'undefined' ? document.documentElement : null);
  if (!target) {
    return;
  }
  if (theme === 'system') {
    target.removeAttribute('data-theme');
  } else {
    target.setAttribute('data-theme', theme);
  }
}

export function formatHotkeyCombo(combo: string): string {
  const parts = combo
    .split('+')
    .map((part) => part.trim().toLowerCase())
    .filter((part) => part.length > 0);

  const symbols: string[] = [];
  for (const part of parts) {
    switch (part) {
      case 'ctrl':
      case 'control':
        symbols.push('⌃');
        break;
      case 'alt':
      case 'option':
        symbols.push('⌥');
        break;
      case 'shift':
        symbols.push('⇧');
        break;
      case 'cmd':
      case 'command':
      case 'meta':
      case 'super':
        symbols.push('⌘');
        break;
      case 'space':
        symbols.push('Space');
        break;
      default:
        symbols.push(part.toUpperCase());
        break;
    }
  }
  return symbols.join('');
}
