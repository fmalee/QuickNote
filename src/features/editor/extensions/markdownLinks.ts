import { syntaxTree } from '@codemirror/language';
import { type EditorState, RangeSetBuilder } from '@codemirror/state';
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view';
import { extractMarkdownOutline, headingToSlug, type OutlineItem } from '@/features/panels';

export interface ExtractedLink {
  from: number;
  to: number;
  url: string;
  label: string;
}

export interface CurrentDocContext {
  absPath: string;
  relPath: string | null;
  isLibrary: boolean;
}

export type ParsedLinkTarget =
  | { kind: 'external'; url: string }
  | { kind: 'anchor'; anchor: string }
  | {
      kind: 'relativeNote';
      targetAbsPath: string;
      targetRelPath: string | null;
      anchor: string | null;
    }
  | { kind: 'localAbsolute'; path: string; requiresConfirmation: true }
  | { kind: 'invalid'; raw: string; reason: string };

const REF_DEF_RE = /^ {0,3}\[([^\]]+)\]:\s*<?([^\s>]+)>?/gm;
const BARE_URL_RE = /\bhttps?:\/\/[^\s<>()[\]"'`]+/g;
const SCHEME_RE = /^([a-zA-Z][a-zA-Z0-9+.-]*):/;
const ALLOWED_EXTERNAL_SCHEMES = new Set(['http', 'https', 'mailto']);

function safeDecodeURIComponent(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function collectReferenceDefinitions(docText: string): Map<string, string> {
  const refs = new Map<string, string>();
  REF_DEF_RE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = REF_DEF_RE.exec(docText)) !== null) {
    const label = (match[1] ?? '').trim().toLowerCase();
    const url = (match[2] ?? '').trim();
    if (label && url && !refs.has(label)) {
      refs.set(label, url);
    }
  }
  return refs;
}

function unwrapAngleBrackets(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed.startsWith('<') && trimmed.endsWith('>') && trimmed.length >= 2) {
    return trimmed.slice(1, -1).trim();
  }
  return trimmed;
}

function overlapsRange(
  ranges: Array<{ from: number; to: number }>,
  from: number,
  to: number,
): boolean {
  return ranges.some((r) => from < r.to && to > r.from);
}

/**
 * 从 CM6 EditorState 语法树中提取全部 Markdown 链接（行内链接、Autolink、URL 与引用式链接）。
 */
export function extractLinksFromState(state: EditorState): ExtractedLink[] {
  const docText = state.doc.toString();
  const refs = collectReferenceDefinitions(docText);
  const tree = syntaxTree(state);
  const links: ExtractedLink[] = [];
  const excludedRanges: Array<{ from: number; to: number }> = [];

  tree.iterate({
    enter(node) {
      if (
        node.name === 'FencedCode' ||
        node.name === 'CodeBlock' ||
        node.name === 'InlineCode' ||
        node.name === 'Image'
      ) {
        excludedRanges.push({ from: node.from, to: node.to });
        return false;
      }

      if (node.name === 'Link') {
        const urlChild = node.node.getChild('URL');
        if (urlChild) {
          const url = unwrapAngleBrackets(state.sliceDoc(urlChild.from, urlChild.to));
          const rawNodeText = state.sliceDoc(node.from, node.to);
          const bracketMatch = /^\[([^\]]*)\]/.exec(rawNodeText);
          const label = bracketMatch ? (bracketMatch[1] ?? '') : rawNodeText;
          if (url) {
            links.push({ from: node.from, to: node.to, url, label });
            excludedRanges.push({ from: node.from, to: node.to });
          }
          return false;
        }

        // 引用式链接：[text][ref] 或 [ref][]
        const rawNodeText = state.sliceDoc(node.from, node.to);
        const fullRefMatch = /^\[([^\]]+)\]\[([^\]]*)\]$/.exec(rawNodeText);
        const shortcutRefMatch = !fullRefMatch ? /^\[([^\]]+)\]$/.exec(rawNodeText) : null;
        const label = fullRefMatch
          ? (fullRefMatch[1] ?? '')
          : shortcutRefMatch
            ? (shortcutRefMatch[1] ?? '')
            : '';
        const refKey = (fullRefMatch ? fullRefMatch[2] || fullRefMatch[1] || '' : label)
          .trim()
          .toLowerCase();
        const resolvedUrl = refKey ? refs.get(refKey) : undefined;
        if (resolvedUrl) {
          links.push({
            from: node.from,
            to: node.to,
            url: unwrapAngleBrackets(resolvedUrl),
            label,
          });
          excludedRanges.push({ from: node.from, to: node.to });
        }
        return false;
      }

      if (node.name === 'Autolink' || node.name === 'URL') {
        const raw = state.sliceDoc(node.from, node.to);
        const url = unwrapAngleBrackets(raw);
        if (url) {
          links.push({ from: node.from, to: node.to, url, label: url });
          excludedRanges.push({ from: node.from, to: node.to });
        }
        return false;
      }

      return undefined;
    },
  });

  // 若默认 markdown() 未启用 GFM autolink，补充扫描正文中未被语法树节点覆盖的裸 URL
  BARE_URL_RE.lastIndex = 0;
  let bareMatch: RegExpExecArray | null;
  while ((bareMatch = BARE_URL_RE.exec(docText)) !== null) {
    const from = bareMatch.index;
    const raw = bareMatch[0] ?? '';
    const trimmed = raw.replace(/[.,;:!?)]+$/, '');
    const to = from + trimmed.length;
    if (trimmed && !overlapsRange(excludedRanges, from, to)) {
      links.push({ from, to, url: trimmed, label: trimmed });
      excludedRanges.push({ from, to });
    }
  }

  links.sort((a, b) => a.from - b.from || a.to - b.to);
  return links;
}

/**
 * 查找指定文档偏移量处命中的链接。
 */
export function findLinkAtPos(state: EditorState, pos: number): ExtractedLink | null {
  const links = extractLinksFromState(state);
  for (const link of links) {
    if (pos >= link.from && pos <= link.to) {
      return link;
    }
  }
  return null;
}

function dirnamePosix(absPath: string): string {
  const normalized = absPath.replace(/\\/g, '/');
  const idx = normalized.lastIndexOf('/');
  if (idx < 0) {
    return '/';
  }
  if (idx === 0) {
    return '/';
  }
  return normalized.slice(0, idx);
}

function normalizePosixPath(baseDir: string, relPart: string): string {
  const isAbs = relPart.startsWith('/');
  const combined = isAbs ? relPart : `${baseDir}/${relPart}`;
  const parts = combined.split('/');
  const stack: string[] = [];
  for (const part of parts) {
    if (!part || part === '.') {
      continue;
    }
    if (part === '..') {
      stack.pop();
    } else {
      stack.push(part);
    }
  }
  return `/${stack.join('/')}`;
}

function computeRelPath(absPath: string, libraryRoot: string | null): string | null {
  if (!libraryRoot) {
    return null;
  }
  const cleanRoot = libraryRoot.replace(/\\/g, '/').replace(/\/+$/, '');
  if (!cleanRoot) {
    return null;
  }
  if (absPath.startsWith(`${cleanRoot}/`)) {
    return absPath.slice(cleanRoot.length + 1);
  }
  return null;
}

/**
 * 纯链接目标解析器：将原始链接文本分流为外部链接、页内锚点、库内/相对笔记、本地绝对路径或非法链接。
 */
export function parseLinkTarget(
  rawUrl: string,
  currentDoc: CurrentDocContext,
  libraryRoot: string | null,
): ParsedLinkTarget {
  const trimmed = rawUrl.trim();
  if (!trimmed) {
    return { kind: 'invalid', raw: rawUrl, reason: '链接目标为空' };
  }

  const schemeMatch = SCHEME_RE.exec(trimmed);
  if (schemeMatch) {
    const scheme = (schemeMatch[1] ?? '').toLowerCase();
    if (ALLOWED_EXTERNAL_SCHEMES.has(scheme)) {
      return { kind: 'external', url: trimmed };
    }
    if (scheme === 'file') {
      let withoutScheme = trimmed.slice('file:'.length);
      if (withoutScheme.startsWith('//localhost/')) {
        withoutScheme = withoutScheme.slice('//localhost'.length);
      } else if (withoutScheme.startsWith('//')) {
        withoutScheme = withoutScheme.slice(2);
      }
      if (!withoutScheme.startsWith('/')) {
        withoutScheme = `/${withoutScheme}`;
      }
      const decodedPath = safeDecodeURIComponent(withoutScheme);
      if (!decodedPath || decodedPath === '/') {
        return { kind: 'invalid', raw: rawUrl, reason: '本地文件路径无效' };
      }
      return {
        kind: 'localAbsolute',
        path: decodedPath,
        requiresConfirmation: true,
      };
    }
    return {
      kind: 'invalid',
      raw: rawUrl,
      reason: `不支持的链接协议：${scheme}:`,
    };
  }

  if (trimmed.startsWith('#')) {
    const anchor = safeDecodeURIComponent(trimmed.slice(1)).trim();
    if (!anchor) {
      return { kind: 'invalid', raw: rawUrl, reason: '锚点名称为空' };
    }
    return { kind: 'anchor', anchor };
  }

  if (trimmed.startsWith('/')) {
    const decodedPath = safeDecodeURIComponent(trimmed);
    return {
      kind: 'localAbsolute',
      path: decodedPath,
      requiresConfirmation: true,
    };
  }

  const hashIdx = trimmed.indexOf('#');
  const rawPathPart = hashIdx >= 0 ? trimmed.slice(0, hashIdx) : trimmed;
  const rawAnchorPart = hashIdx >= 0 ? trimmed.slice(hashIdx + 1) : '';
  const decodedPathPart = safeDecodeURIComponent(rawPathPart).trim();
  const decodedAnchor = safeDecodeURIComponent(rawAnchorPart).trim();

  if (!decodedPathPart) {
    return { kind: 'invalid', raw: rawUrl, reason: '相对路径为空' };
  }

  const baseDir = dirnamePosix(currentDoc.absPath || '/');
  const targetAbsPath = normalizePosixPath(baseDir, decodedPathPart);
  const targetRelPath = computeRelPath(targetAbsPath, libraryRoot);

  return {
    kind: 'relativeNote',
    targetAbsPath,
    targetRelPath,
    anchor: decodedAnchor || null,
  };
}

/**
 * 中文 Slug 锚点匹配（遵循 C8.4 规范）。
 */
export function resolveHeadingAnchor(body: string, rawAnchor: string): OutlineItem | null {
  if (!body || !rawAnchor.trim()) {
    return null;
  }
  const cleanedRaw = safeDecodeURIComponent(rawAnchor.trim().replace(/^#/, ''));
  if (!cleanedRaw) {
    return null;
  }

  const items = extractMarkdownOutline(body);
  if (items.length === 0) {
    return null;
  }

  const exact = items.find(
    (item) => item.slug === cleanedRaw || item.slug.toLowerCase() === cleanedRaw.toLowerCase(),
  );
  if (exact) {
    return exact;
  }

  const normalizedSlug = headingToSlug(cleanedRaw, new Map());
  const byNormalized = items.find((item) => item.slug === normalizedSlug);
  if (byNormalized) {
    return byNormalized;
  }

  return null;
}

/**
 * 判定鼠标点击是否应触发链接跳转。
 */
export function isLinkTriggerEvent(
  event: Pick<MouseEvent, 'button' | 'metaKey' | 'ctrlKey' | 'altKey'>,
  singleClickLink: boolean,
): boolean {
  if (event.button !== 0 || event.ctrlKey || event.altKey) {
    return false;
  }
  if (singleClickLink) {
    return true;
  }
  return event.metaKey;
}

function buildLinkDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const links = extractLinksFromState(view.state);
  for (const link of links) {
    builder.add(
      link.from,
      link.to,
      Decoration.mark({
        class: 'qn-cm-link',
        attributes: { 'data-qn-link': link.url },
      }),
    );
  }
  return builder.finish();
}

export const markdownLinkPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = buildLinkDecorations(view);
    }

    update(update: ViewUpdate): void {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildLinkDecorations(update.view);
      }
    }
  },
  {
    decorations: (plugin) => plugin.decorations,
  },
);
