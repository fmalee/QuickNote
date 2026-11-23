export interface OutlineItem {
  level: number;
  text: string;
  slug: string;
  line: number;
  from: number;
}

export function stripInlineMarkdown(raw: string): string {
  return raw
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/(`+)([^`]*)\1/g, '$2')
    .replace(/(\*\*|__)(.*?)\1/g, '$2')
    .replace(/(\*|_)(.*?)\1/g, '$2')
    .replace(/~~(.*?)~~/g, '$1')
    .replace(/[*_`~]/g, '')
    .replace(/\s+#+\s*$/, '')
    .trim();
}

export function headingToSlug(rawHeading: string, seenCounts: Map<string, number>): string {
  const cleaned = stripInlineMarkdown(rawHeading)
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^a-z0-9\u4e00-\u9fff\u3400-\u4dbf\u3040-\u30ff\uac00-\ud7af_-]/g, '')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '');

  const base = cleaned || 'heading';
  const count = seenCounts.get(base) ?? 0;
  seenCounts.set(base, count + 1);
  return count === 0 ? base : `${base}-${count}`;
}

export function extractMarkdownOutline(body: string): OutlineItem[] {
  if (!body) {
    return [];
  }

  const lines = body.split('\n');
  const items: OutlineItem[] = [];
  const seenCounts = new Map<string, number>();

  let fenceChar: '`' | '~' | null = null;
  let fenceLen = 0;
  let offset = 0;

  for (let i = 0; i < lines.length; i += 1) {
    const rawLine = lines[i] ?? '';
    const line = rawLine.endsWith('\r') ? rawLine.slice(0, -1) : rawLine;
    const lineNum = i + 1;

    const fenceMatch = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line);
    if (fenceMatch) {
      const marker = fenceMatch[1] ?? '';
      const rest = fenceMatch[2] ?? '';
      const char = marker[0] as '`' | '~';
      if (fenceChar === null) {
        if (!(char === '`' && rest.includes('`'))) {
          fenceChar = char;
          fenceLen = marker.length;
          offset += rawLine.length + 1;
          continue;
        }
      } else if (char === fenceChar && marker.length >= fenceLen && rest.trim() === '') {
        fenceChar = null;
        fenceLen = 0;
        offset += rawLine.length + 1;
        continue;
      }
    }

    if (fenceChar === null) {
      const headingMatch = /^(#{1,6})\s+(.+)$/.exec(line);
      if (headingMatch) {
        const hashes = headingMatch[1] ?? '#';
        const rawText = headingMatch[2] ?? '';
        const text = stripInlineMarkdown(rawText);
        if (text.length > 0) {
          const slug = headingToSlug(text, seenCounts);
          items.push({
            level: hashes.length,
            text,
            slug,
            line: lineNum,
            from: offset,
          });
        }
      }
    }

    offset += rawLine.length + 1;
  }

  return items;
}

export function findActiveHeadingIndex(items: OutlineItem[], currentLine: number): number {
  if (items.length === 0) {
    return -1;
  }
  let matched = -1;
  for (let i = 0; i < items.length; i += 1) {
    const item = items[i];
    if (item && item.line <= currentLine) {
      matched = i;
    } else {
      break;
    }
  }
  return matched === -1 ? 0 : matched;
}
