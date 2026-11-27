export interface FontPresetOption {
  value: string;
  label: string;
}

export const CUSTOM_FONT_SENTINEL = '__custom__';

export const PROSE_FONT_PRESETS: readonly FontPresetOption[] = [
  { value: '', label: '系统默认（SF Pro / 苹方）' },
  { value: 'PingFang SC', label: '苹方（PingFang SC）' },
  { value: 'Hiragino Sans GB', label: '冬青黑体（Hiragino Sans GB）' },
  { value: 'Songti SC', label: '宋体（Songti SC）' },
  { value: 'LXGW WenKai', label: '霞鹜文楷（LXGW WenKai）' },
  { value: 'Inter', label: 'Inter' },
];

export const CODE_FONT_PRESETS: readonly FontPresetOption[] = [
  { value: '', label: '系统默认（SF Mono / Menlo）' },
  { value: 'SF Mono', label: 'SF Mono' },
  { value: 'Menlo', label: 'Menlo' },
  { value: 'JetBrains Mono', label: 'JetBrains Mono' },
  { value: 'Fira Code', label: 'Fira Code' },
  { value: 'Maple Mono', label: 'Maple Mono' },
];

const GENERIC_FAMILIES = new Set([
  'serif',
  'sans-serif',
  'monospace',
  'cursive',
  'fantasy',
  'system-ui',
  'ui-sans-serif',
  'ui-serif',
  'ui-monospace',
  'ui-rounded',
]);

function formatFontFamilyAtom(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) {
    return '';
  }
  if (trimmed.includes(',')) {
    return trimmed;
  }
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'")) ||
    GENERIC_FAMILIES.has(trimmed.toLowerCase())
  ) {
    return trimmed;
  }
  const escaped = trimmed.replace(/'/g, "\\'");
  return `'${escaped}'`;
}

export function isPresetFontValue(value: string, presets: readonly FontPresetOption[]): boolean {
  const trimmed = value.trim();
  return presets.some((opt) => opt.value === trimmed);
}

export function resolveEditorProseFontFamily(value?: string | null): string {
  const formatted = formatFontFamilyAtom(value ?? '');
  if (!formatted) {
    return 'var(--qn-font-sans)';
  }
  return `${formatted}, var(--qn-font-sans)`;
}

export function resolveEditorCodeFontFamily(value?: string | null): string {
  const formatted = formatFontFamilyAtom(value ?? '');
  if (!formatted) {
    return 'var(--qn-font-mono)';
  }
  return `${formatted}, var(--qn-font-mono)`;
}
