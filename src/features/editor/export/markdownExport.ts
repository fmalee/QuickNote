import type { NoteDoc } from '@/bindings/types';

const ALLOWED_SCHEMES = new Set(['http', 'https', 'mailto']);

const HTML_ESCAPE_MAP: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
};

export function escapeHtml(raw: string): string {
  return raw.replace(/[&<>"']/g, (ch) => HTML_ESCAPE_MAP[ch] ?? ch);
}

export function sanitizeHref(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) {
    return null;
  }

  // Strip ASCII control characters and whitespace when checking protocol schemes
  // to prevent obfuscated payloads like `java\tscript:alert(1)`.
  // eslint-disable-next-line no-control-regex
  const normalized = trimmed.replace(/[\u0000-\u0020\u007f]+/g, '');
  if (!normalized) {
    return null;
  }

  if (normalized.startsWith('#')) {
    return trimmed;
  }

  const schemeMatch = /^([a-zA-Z][a-zA-Z0-9+.-]*):/.exec(normalized);
  if (schemeMatch) {
    const scheme = (schemeMatch[1] ?? '').toLowerCase();
    if (!ALLOWED_SCHEMES.has(scheme)) {
      return null;
    }
    return trimmed;
  }

  return trimmed;
}

function sanitizeImageSrc(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) {
    return null;
  }
  // eslint-disable-next-line no-control-regex
  const normalized = trimmed.replace(/[\u0000-\u0020\u007f]+/g, '');
  if (/^data:image\/(png|jpeg|jpg|gif|webp);base64,/i.test(normalized)) {
    return trimmed;
  }
  return sanitizeHref(trimmed);
}

function renderInline(raw: string): string {
  // 1. Extract inline code spans first so emphasis/links inside backticks are not parsed.
  const codeTokens: string[] = [];
  let text = raw.replace(/`([^`\n]+)`/g, (_match, code: string) => {
    const idx = codeTokens.length;
    codeTokens.push(
      `<code style="font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;font-size:0.9em;background:#f6f8fa;padding:2px 5px;border-radius:4px;color:#1f2328;">${escapeHtml(code)}</code>`,
    );
    return `@@QNCODE_${idx}@@`;
  });

  // 2. Extract images ![alt](src) before links [text](href).
  const linkTokens: string[] = [];
  text = text.replace(
    /!\[([^\]]*)\]\(([^)\s]+(?:\s+"[^"]*")?)\)/g,
    (_match, alt: string, rawTarget: string) => {
      const urlPart = rawTarget.replace(/\s+"[^"]*"$/, '').trim();
      const safeSrc = sanitizeImageSrc(urlPart);
      const idx = linkTokens.length;
      if (safeSrc) {
        linkTokens.push(
          `<img src="${escapeHtml(safeSrc)}" alt="${escapeHtml(alt)}" style="max-width:100%;height:auto;border-radius:4px;" />`,
        );
      } else {
        linkTokens.push(escapeHtml(alt ? `[${alt}]` : '[image]'));
      }
      return `@@QNLINK_${idx}@@`;
    },
  );

  // 3. Extract links [label](href).
  text = text.replace(
    /\[([^\]]+)\]\(([^)\s]+(?:\s+"[^"]*")?)\)/g,
    (_match, label: string, rawTarget: string) => {
      const urlPart = rawTarget.replace(/\s+"[^"]*"$/, '').trim();
      const safeHref = sanitizeHref(urlPart);
      const idx = linkTokens.length;
      const renderedLabel = renderInlineFormatting(escapeHtml(label));
      if (safeHref) {
        linkTokens.push(
          `<a href="${escapeHtml(safeHref)}" style="color:#0969da;text-decoration:underline;">${renderedLabel}</a>`,
        );
      } else {
        linkTokens.push(renderedLabel);
      }
      return `@@QNLINK_${idx}@@`;
    },
  );

  // 4. Escape remaining raw HTML and apply inline formatting.
  let html = renderInlineFormatting(escapeHtml(text));

  // 5. Restore link and code tokens.
  html = html.replace(
    /@@QNLINK_(\d+)@@/g,
    (_m, idxStr: string) => linkTokens[Number(idxStr)] ?? '',
  );
  html = html.replace(
    /@@QNCODE_(\d+)@@/g,
    (_m, idxStr: string) => codeTokens[Number(idxStr)] ?? '',
  );

  return html;
}

function renderInlineFormatting(escaped: string): string {
  let out = escaped;
  // Bold (**...** or __...__)
  out = out.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  out = out.replace(/(^|[^\w])__(.+?)__(?=[^\w]|$)/g, '$1<strong>$2</strong>');
  // Strikethrough (~~...~~)
  out = out.replace(/~~(.+?)~~/g, '<del>$1</del>');
  // Italic (*...* or _..._)
  out = out.replace(/(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)/g, '<em>$1</em>');
  out = out.replace(/(^|[^\w])_([^_]+?)_(?=[^\w]|$)/g, '$1<em>$2</em>');
  return out;
}

function splitTableRow(line: string): string[] {
  let trimmed = line.trim();
  if (trimmed.startsWith('|')) {
    trimmed = trimmed.slice(1);
  }
  if (trimmed.endsWith('|')) {
    trimmed = trimmed.slice(0, -1);
  }
  return trimmed.split('|').map((cell) => cell.trim());
}

function isTableSeparatorLine(line: string): boolean {
  const trimmed = line.trim();
  if (!trimmed.includes('|') && !trimmed.includes('-')) {
    return false;
  }
  const cells = splitTableRow(trimmed);
  if (cells.length === 0) {
    return false;
  }
  return cells.every((cell) => /^:?-{3,}:?$/.test(cell));
}

function parseTableAlignments(line: string): Array<'left' | 'center' | 'right'> {
  return splitTableRow(line).map((cell) => {
    const left = cell.startsWith(':');
    const right = cell.endsWith(':');
    if (left && right) return 'center';
    if (right) return 'right';
    return 'left';
  });
}

function isHorizontalRule(line: string): boolean {
  return /^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/.test(line);
}

function parseHeading(line: string): { level: number; text: string } | null {
  const match = /^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
  if (!match) return null;
  return { level: (match[1] ?? '#').length, text: match[2] ?? '' };
}

function parseFenceOpen(line: string): { marker: string; lang: string } | null {
  const match = /^\s{0,3}(`{3,}|~{3,})(.*)$/.exec(line);
  if (!match) return null;
  return { marker: match[1] ?? '```', lang: (match[2] ?? '').trim() };
}

function isFenceClose(line: string, marker: string): boolean {
  const ch = marker[0] ?? '`';
  const minLen = marker.length;
  const re = new RegExp(`^\\s{0,3}${ch}{${minLen},}\\s*$`);
  return re.test(line);
}

function parseListItem(
  line: string,
): { ordered: boolean; text: string; todoChecked: boolean | null } | null {
  const todoMatch = /^\s*[-*+]\s+\[([ xX])\]\s+(.*)$/.exec(line);
  if (todoMatch) {
    return {
      ordered: false,
      text: todoMatch[2] ?? '',
      todoChecked: (todoMatch[1] ?? '').toLowerCase() === 'x',
    };
  }
  const ulMatch = /^\s*[-*+]\s+(.*)$/.exec(line);
  if (ulMatch) {
    return { ordered: false, text: ulMatch[1] ?? '', todoChecked: null };
  }
  const olMatch = /^\s*\d+\.\s+(.*)$/.exec(line);
  if (olMatch) {
    return { ordered: true, text: olMatch[1] ?? '', todoChecked: null };
  }
  return null;
}

const HEADING_SIZES: Record<number, string> = {
  1: 'font-size:22px;margin:18px 0 10px;',
  2: 'font-size:18px;margin:16px 0 8px;',
  3: 'font-size:16px;margin:14px 0 6px;',
  4: 'font-size:15px;margin:12px 0 6px;',
  5: 'font-size:14px;margin:10px 0 4px;',
  6: 'font-size:13px;margin:10px 0 4px;color:#57606a;',
};

export function renderMarkdownToRichHtml(markdown: string): string {
  const lines = markdown.replace(/\r\n?/g, '\n').split('\n');
  const blocks: string[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i] ?? '';

    // Skip blank lines
    if (line.trim() === '') {
      i += 1;
      continue;
    }

    // 1. Fenced code block
    const fence = parseFenceOpen(line);
    if (fence) {
      const codeLines: string[] = [];
      i += 1;
      while (i < lines.length && !isFenceClose(lines[i] ?? '', fence.marker)) {
        codeLines.push(lines[i] ?? '');
        i += 1;
      }
      if (i < lines.length) {
        i += 1;
      }
      const langAttr = fence.lang ? ` data-lang="${escapeHtml(fence.lang)}"` : '';
      blocks.push(
        `<pre style="background:#f6f8fa;border:1px solid #d0d7de;border-radius:6px;padding:12px;margin:12px 0;overflow-x:auto;font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;font-size:13px;line-height:1.5;color:#1f2328;"><code${langAttr}>${escapeHtml(codeLines.join('\n'))}</code></pre>`,
      );
      continue;
    }

    // 2. Horizontal rule
    if (isHorizontalRule(line)) {
      blocks.push('<hr style="border:none;border-top:1px solid #d0d7de;margin:16px 0;" />');
      i += 1;
      continue;
    }

    // 3. ATX Heading
    const heading = parseHeading(line);
    if (heading) {
      const tag = `h${heading.level}`;
      const sizeStyle = HEADING_SIZES[heading.level] ?? HEADING_SIZES[3];
      blocks.push(
        `<${tag} style="${sizeStyle}font-weight:600;line-height:1.35;color:#1f2328;">${renderInline(heading.text)}</${tag}>`,
      );
      i += 1;
      continue;
    }

    // 4. Blockquote
    if (/^\s{0,3}>\s?/.test(line)) {
      const quoteLines: string[] = [];
      while (i < lines.length && /^\s{0,3}>\s?/.test(lines[i] ?? '')) {
        quoteLines.push((lines[i] ?? '').replace(/^\s{0,3}>\s?/, ''));
        i += 1;
      }
      const innerHtml = renderMarkdownToRichHtml(quoteLines.join('\n'));
      blocks.push(
        `<blockquote style="margin:12px 0;padding:4px 12px;border-left:4px solid #d0d7de;color:#57606a;">${innerHtml}</blockquote>`,
      );
      continue;
    }

    // 5. GFM Table (header + separator + rows)
    const nextSepLine = lines[i + 1] ?? '';
    if (line.includes('|') && i + 1 < lines.length && isTableSeparatorLine(nextSepLine)) {
      const headers = splitTableRow(line);
      const alignments = parseTableAlignments(nextSepLine);
      i += 2;
      const bodyRows: string[][] = [];
      while (i < lines.length) {
        const rowLine = lines[i] ?? '';
        if (rowLine.trim() === '' || !rowLine.includes('|')) {
          break;
        }
        bodyRows.push(splitTableRow(rowLine));
        i += 1;
      }

      const ths = headers
        .map((cell, idx) => {
          const align = alignments[idx] ?? 'left';
          return `<th style="border:1px solid #d0d7de;padding:6px 10px;background:#f6f8fa;font-weight:600;text-align:${align};">${renderInline(cell)}</th>`;
        })
        .join('');

      const trs = bodyRows
        .map((row) => {
          const tds = headers
            .map((_h, idx) => {
              const align = alignments[idx] ?? 'left';
              return `<td style="border:1px solid #d0d7de;padding:6px 10px;text-align:${align};">${renderInline(row[idx] ?? '')}</td>`;
            })
            .join('');
          return `<tr>${tds}</tr>`;
        })
        .join('');

      blocks.push(
        `<table style="border-collapse:collapse;width:100%;margin:12px 0;font-size:14px;"><thead><tr>${ths}</tr></thead><tbody>${trs}</tbody></table>`,
      );
      continue;
    }

    // 6. Lists (unordered, ordered, todo)
    const firstListItem = parseListItem(line);
    if (firstListItem) {
      const ordered = firstListItem.ordered;
      const items: string[] = [];
      while (i < lines.length) {
        const item = parseListItem(lines[i] ?? '');
        if (!item || item.ordered !== ordered) {
          break;
        }
        if (item.todoChecked !== null) {
          const checkedAttr = item.todoChecked ? ' checked' : '';
          items.push(
            `<li style="list-style:none;margin:4px 0;"><input type="checkbox" disabled${checkedAttr} style="margin-right:6px;vertical-align:middle;" />${renderInline(item.text)}</li>`,
          );
        } else {
          items.push(`<li style="margin:4px 0;">${renderInline(item.text)}</li>`);
        }
        i += 1;
      }
      const listTag = ordered ? 'ol' : 'ul';
      blocks.push(
        `<${listTag} style="margin:8px 0;padding-left:24px;color:#1f2328;">${items.join('')}</${listTag}>`,
      );
      continue;
    }

    // 7. Paragraph (collect consecutive non-block lines)
    const paraLines: string[] = [];
    while (i < lines.length) {
      const cur = lines[i] ?? '';
      const curNext = lines[i + 1] ?? '';
      if (
        cur.trim() === '' ||
        parseFenceOpen(cur) !== null ||
        isHorizontalRule(cur) ||
        parseHeading(cur) !== null ||
        /^\s{0,3}>\s?/.test(cur) ||
        parseListItem(cur) !== null ||
        (cur.includes('|') && i + 1 < lines.length && isTableSeparatorLine(curNext))
      ) {
        break;
      }
      paraLines.push(cur);
      i += 1;
    }
    const paraHtml = paraLines.map((l) => renderInline(l)).join('<br />');
    blocks.push(`<p style="margin:8px 0;line-height:1.65;color:#1f2328;">${paraHtml}</p>`);
  }

  return blocks.join('\n');
}

export function buildPlainTextFallback(markdown: string): string {
  return markdown.replace(/\r\n?/g, '\n').trimEnd();
}

export async function copyMarkdownAsRichText(markdown: string): Promise<'rich' | 'plain'> {
  const html = renderMarkdownToRichHtml(markdown);
  const plain = buildPlainTextFallback(markdown);

  if (
    typeof ClipboardItem !== 'undefined' &&
    typeof navigator !== 'undefined' &&
    navigator.clipboard &&
    typeof navigator.clipboard.write === 'function'
  ) {
    try {
      const item = new ClipboardItem({
        'text/html': new Blob([html], { type: 'text/html' }),
        'text/plain': new Blob([plain], { type: 'text/plain' }),
      });
      await navigator.clipboard.write([item]);
      return 'rich';
    } catch {
      // Fall back to plain text clipboard write when rich MIME is rejected.
    }
  }

  if (
    typeof navigator !== 'undefined' &&
    navigator.clipboard &&
    typeof navigator.clipboard.writeText === 'function'
  ) {
    await navigator.clipboard.writeText(plain);
    return 'plain';
  }

  throw new Error('当前环境不支持剪贴板写入');
}

function isMarkdownDocument(doc: NoteDoc): boolean {
  const lower = doc.absPath.toLowerCase();
  if (lower.endsWith('.md') || lower.endsWith('.markdown')) {
    return true;
  }
  const baseName = lower.split('/').pop() ?? '';
  return !baseName.includes('.');
}

function resolveDocTitle(doc: NoteDoc): string {
  const trimmed = doc.meta.title.trim();
  if (trimmed) {
    return trimmed;
  }
  const baseName = doc.absPath.split('/').pop() ?? '';
  return baseName || '未命名笔记';
}

export function buildPrintHtmlDocument(doc: NoteDoc, body: string): string {
  const title = resolveDocTitle(doc);
  const escapedTitle = escapeHtml(title);

  const metaItems: string[] = [`<span class="meta-badge">${escapeHtml(doc.meta.docType)}</span>`];
  if (doc.meta.project) {
    metaItems.push(`<span class="meta-item">项目: ${escapeHtml(doc.meta.project)}</span>`);
  }
  if (doc.meta.tags.length > 0) {
    const tagsHtml = doc.meta.tags.map((t) => `#${escapeHtml(t)}`).join(' ');
    metaItems.push(`<span class="meta-item">标签: ${tagsHtml}</span>`);
  }
  if (doc.meta.created) {
    metaItems.push(`<span class="meta-item">创建于: ${escapeHtml(doc.meta.created)}</span>`);
  }

  const contentHtml = isMarkdownDocument(doc)
    ? renderMarkdownToRichHtml(body)
    : `<pre class="plain-doc"><code>${escapeHtml(body)}</code></pre>`;

  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <title>${escapedTitle}</title>
  <style>
    @page { size: A4; margin: 18mm 16mm; }
    * { box-sizing: border-box; }
    html, body {
      margin: 0;
      padding: 0;
      background: #ffffff;
      color: #1f2328;
      font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "PingFang SC", "Helvetica Neue", Arial, sans-serif;
      font-size: 14px;
      line-height: 1.65;
    }
    .doc-header {
      border-bottom: 1px solid #d0d7de;
      padding-bottom: 12px;
      margin-bottom: 18px;
    }
    .doc-title {
      margin: 0 0 8px 0;
      font-size: 24px;
      font-weight: 700;
      line-height: 1.3;
      color: #1f2328;
    }
    .doc-meta {
      display: flex;
      flex-wrap: wrap;
      gap: 8px 14px;
      align-items: center;
      font-size: 12px;
      color: #57606a;
    }
    .meta-badge {
      display: inline-block;
      padding: 1px 8px;
      border-radius: 999px;
      background: #f6f8fa;
      border: 1px solid #d0d7de;
      color: #1f2328;
      font-weight: 500;
    }
    .plain-doc {
      background: #f6f8fa;
      border: 1px solid #d0d7de;
      border-radius: 6px;
      padding: 12px;
      white-space: pre-wrap;
      word-break: break-word;
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      font-size: 12.5px;
      line-height: 1.55;
      color: #1f2328;
    }
    h1, h2, h3 { break-after: avoid; }
    pre, blockquote, table { break-inside: avoid; }
  </style>
</head>
<body>
  <header class="doc-header">
    <h1 class="doc-title">${escapedTitle}</h1>
    <div class="doc-meta">${metaItems.join('')}</div>
  </header>
  <main class="doc-body">
    ${contentHtml}
  </main>
</body>
</html>`;
}

export function buildDefaultPdfExportPath(doc: NoteDoc): string {
  const rawTitle =
    doc.meta.title.trim() ||
    (doc.absPath.split('/').pop() ?? '').replace(/\.[^.]+$/, '') ||
    'untitled';
  const safeTitle = rawTitle
    .replace(/[/\\:*?"<>|]/g, '-')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/^\.+/, '');
  const finalName = safeTitle || 'untitled';
  return `~/Desktop/${finalName}.pdf`;
}
