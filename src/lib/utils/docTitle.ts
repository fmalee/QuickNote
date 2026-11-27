const MARKDOWN_EXT_RE = /\.(md|markdown)$/i;

/**
 * 判断路径是否为 Markdown 文件（`.md` / `.markdown`，忽略大小写）。
 */
export function isMarkdownPath(path: string): boolean {
  return MARKDOWN_EXT_RE.test(path.trim());
}

/**
 * 从文件路径推导兜底显示标题：
 * - Markdown 文件：文件名去掉 `.md` / `.markdown` 扩展名；
 * - 其他类型文件：保留完整文件名（含扩展名）。
 */
export function deriveFallbackTitleFromPath(path: string | null | undefined): string {
  const source = path?.trim() ?? '';
  if (!source) {
    return '未命名笔记';
  }
  const normalized = source.replace(/\\/g, '/');
  const segments = normalized.split('/');
  const base = segments[segments.length - 1] || source;
  if (isMarkdownPath(base)) {
    return base.replace(MARKDOWN_EXT_RE, '') || base;
  }
  return base;
}
