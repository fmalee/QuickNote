import type { DirNode, NoteSummary } from '@/bindings/types';

export type DefaultTemplateName = 'note' | 'doc' | 'command-book';

export function inferDefaultTemplateName(
  dirRelPath: string | null | undefined,
): DefaultTemplateName {
  const normalized = (dirRelPath ?? '').trim().replace(/^\/+|\/+$/g, '');
  if (normalized === 'commands' || normalized.startsWith('commands/')) {
    return 'command-book';
  }
  if (normalized === 'projects' || normalized.startsWith('projects/')) {
    return 'doc';
  }
  return 'note';
}

export function deriveTitleFromContent(content: string): string {
  const lines = content.split(/\r?\n/);
  const firstNonEmpty = lines.find((line) => line.trim().length > 0);
  if (!firstNonEmpty) {
    return '未命名笔记';
  }

  let cleaned = firstNonEmpty.trim();
  // 剥离常见 Markdown 列表/待办/标题前缀
  cleaned = cleaned.replace(/^#{1,6}\s+/, '');
  cleaned = cleaned.replace(/^-\s+\[[ xX]\]\s+/, '');
  cleaned = cleaned.replace(/^[-*]\s+/, '');
  // 剥离 Quick Bar 追加的时间戳前缀，如 "14:30 "
  cleaned = cleaned.replace(/^\d{1,2}:\d{2}\s+/, '');
  cleaned = cleaned.trim();

  if (!cleaned) {
    return '未命名笔记';
  }

  const chars = Array.from(cleaned);
  return chars.length > 30 ? chars.slice(0, 30).join('') : cleaned;
}

export function extractTemplateNameFromSummary(summary: NoteSummary): string {
  if (summary.relPath) {
    const parts = summary.relPath.split('/');
    const fileName = parts[parts.length - 1] ?? summary.relPath;
    return fileName.endsWith('.md') ? fileName.slice(0, -3) : fileName;
  }
  return summary.title.trim() || 'note';
}

export function flattenDirRelPaths(dirs: DirNode[]): string[] {
  const result: string[] = [];
  function walk(nodes: DirNode[]): void {
    for (const node of nodes) {
      result.push(node.relPath);
      if (node.children.length > 0) {
        walk(node.children);
      }
    }
  }
  walk(dirs);
  return result;
}
