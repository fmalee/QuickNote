export type QuickOpenQueryMode = 'all' | 'path' | 'fulltext' | 'tag' | 'action';

export interface ParsedQuickOpenQuery {
  mode: QuickOpenQueryMode;
  raw: string;
  term: string;
}

/**
 * 解析快速打开输入框的前缀模态：
 * - `/` 或 `~/` 开头 -> `path`（路径补全）
 * - `#` 开头 -> `fulltext`（库内全文检索）
 * - `@` 开头 -> `tag`（标签与带标签笔记检索）
 * - `>` 开头 -> `action`（已注册动作面板）
 * - 其余 -> `all`（默认混合检索 + 异步 Spotlight）
 */
export function parseQuickOpenQuery(rawInput: string): ParsedQuickOpenQuery {
  const trimmedStart = rawInput.trimStart();
  if (trimmedStart.startsWith('/') || trimmedStart.startsWith('~/')) {
    return {
      mode: 'path',
      raw: rawInput,
      term: trimmedStart,
    };
  }
  if (trimmedStart.startsWith('#')) {
    return {
      mode: 'fulltext',
      raw: rawInput,
      term: trimmedStart.slice(1).trim(),
    };
  }
  if (trimmedStart.startsWith('@')) {
    return {
      mode: 'tag',
      raw: rawInput,
      term: trimmedStart.slice(1).trim(),
    };
  }
  if (trimmedStart.startsWith('>')) {
    return {
      mode: 'action',
      raw: rawInput,
      term: trimmedStart.slice(1).trim(),
    };
  }
  return {
    mode: 'all',
    raw: rawInput,
    term: rawInput.trim(),
  };
}

/** 计算非空字符串列表的最长公共前缀，用于路径模式 `Tab` 多项补全。 */
export function computeLongestCommonPrefix(values: readonly string[]): string {
  if (values.length === 0) {
    return '';
  }
  let prefix = values[0] ?? '';
  for (let i = 1; i < values.length; i += 1) {
    const current = values[i] ?? '';
    let idx = 0;
    const maxLen = Math.min(prefix.length, current.length);
    while (idx < maxLen && prefix[idx] === current[idx]) {
      idx += 1;
    }
    prefix = prefix.slice(0, idx);
    if (!prefix) {
      break;
    }
  }
  return prefix;
}
