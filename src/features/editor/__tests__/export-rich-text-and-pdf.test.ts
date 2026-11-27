import { createPinia, setActivePinia } from 'pinia';
import { createRenderer, createSSRApp, h, nextTick, ref, ssrContextKey } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NoteDoc } from '@/bindings/types';

const { mockNoteExportPdf, mockExternalActionRun } = vi.hoisted(() => ({
  mockNoteExportPdf: vi.fn(),
  mockExternalActionRun: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  noteExportPdf: mockNoteExportPdf,
  externalActionRun: mockExternalActionRun,
  externalActionsResolve: vi.fn().mockResolvedValue([]),
  noteRead: vi.fn(),
  noteWrite: vi.fn(),
  noteUpdateMeta: vi.fn(),
  inboxExtractToNote: vi.fn(),
  noteSaveImage: vi.fn(),
  appOpenExternalUrl: vi.fn(),
  notesList: vi.fn().mockResolvedValue([]),
  uiStateGet: vi.fn(),
  uiStateSet: vi.fn(),
  isAppError: (err: unknown): err is { code: string; message: string; detail?: string | null } =>
    typeof err === 'object' && err !== null && 'code' in err && 'message' in err,
}));

vi.mock('@/lib/ipc/events', () => ({
  onFsExternalChange: vi.fn().mockResolvedValue(() => undefined),
}));

import ExportPdfModal from '../ExportPdfModal.vue';
import {
  buildDefaultPdfExportPath,
  buildPlainTextFallback,
  buildPrintHtmlDocument,
  copyMarkdownAsRichText,
  escapeHtml,
  renderMarkdownToRichHtml,
  sanitizeHref,
} from '../export/markdownExport';

interface FakeElement {
  tag: string;
  props: Record<string, unknown>;
  children: Array<FakeElement | string>;
  parent: FakeElement | null;
}

function createFakeNode(tag: string): FakeElement {
  return { tag, props: {}, children: [], parent: null };
}

interface ExportPdfModalExposed {
  destPathDraft: string;
  exportedPath: string | null;
  errorMessage: string | null;
  handleSubmit: () => Promise<void>;
  handleRevealInFinder: () => Promise<void>;
}

function mountModalWithFakeRenderer(doc: NoteDoc | null, body: string) {
  const root = createFakeNode('div');
  const modalRef = ref<ExportPdfModalExposed | null>(null);
  const comp = ExportPdfModal as unknown as {
    render?: () => unknown;
    ssrRender?: unknown;
  };
  const prevRender = comp.render;
  const prevSsrRender = comp.ssrRender;
  comp.render = () => h('div', { 'data-testid': 'export-pdf-modal-stub' });
  comp.ssrRender = undefined;

  const { createApp } = createRenderer<FakeElement | string, FakeElement>({
    createElement: (type) => createFakeNode(type),
    createText: (text) => text,
    createComment: () => createFakeNode('comment'),
    setText: () => undefined,
    setElementText: (el, text) => {
      el.children = [text];
    },
    insert: (el, parent) => {
      if (typeof el !== 'string') {
        el.parent = parent;
      }
      parent.children.push(el);
    },
    remove: (el) => {
      if (typeof el === 'string' || !el.parent) return;
      el.parent.children = el.parent.children.filter((item) => item !== el);
      el.parent = null;
    },
    patchProp: (el, key, _prev, next) => {
      if (next === null || next === undefined) {
        delete el.props[key];
      } else {
        el.props[key] = next;
      }
    },
    parentNode: (node) => (typeof node === 'string' ? null : node.parent),
    nextSibling: () => null,
  });

  const pinia = createPinia();
  setActivePinia(pinia);

  const app = createApp({
    render() {
      return h(ExportPdfModal, {
        ref: modalRef,
        open: true,
        doc,
        body,
        onClose: () => undefined,
      });
    },
  });
  app.provide(ssrContextKey, { modules: new Set() });
  app.use(pinia);
  app.mount(root);
  return {
    modalRef,
    unmount: () => {
      app.unmount();
      comp.render = prevRender;
      comp.ssrRender = prevSsrRender;
    },
  };
}

function makeSampleDoc(overrides: Partial<NoteDoc> = {}): NoteDoc {
  return {
    key: 'notes/demo.md',
    relPath: 'notes/demo.md',
    absPath: '/Users/demo/QuickNote/notes/demo.md',
    isLibrary: true,
    isInbox: false,
    meta: {
      id: '01JTESTULID000000000000001',
      title: '测试笔记: 导出/验证?',
      docType: 'note',
      tags: ['导出', 'm4'],
      openDir: null,
      codeWorkspace: null,
      terminalDir: null,
      created: '2026-11-24',
      pinned: false,
      archived: false,
      metaError: null,
    },
    body: '# 标题\n\n正文内容',
    bodyHash: 'abc123',
    mtimeMs: 1700000000000,
    sizeBytes: 32,
    ...overrides,
  };
}

describe('M4-04 rich text export, XSS protection, and PDF export', () => {
  const originalNavigator = globalThis.navigator;
  const originalClipboardItem = globalThis.ClipboardItem;

  beforeEach(() => {
    vi.clearAllMocks();
    mockNoteExportPdf.mockResolvedValue(undefined);
    mockExternalActionRun.mockResolvedValue(undefined);
  });

  afterEach(() => {
    Object.defineProperty(globalThis, 'navigator', {
      value: originalNavigator,
      configurable: true,
    });
    Object.defineProperty(globalThis, 'ClipboardItem', {
      value: originalClipboardItem,
      configurable: true,
    });
  });

  it('renders headings, emphasis, code blocks, blockquotes, lists, todos, links, and GFM tables', () => {
    const md = [
      '# 一级标题',
      '## 二级标题',
      '',
      '包含 **加粗**、*斜体*、~~删除线~~ 与 `inlineCode` 以及 [官网](https://example.com)。',
      '',
      '> 引用块内容',
      '',
      '- 无序项',
      '- [ ] 未完成待办',
      '- [x] 已完成待办',
      '',
      '1. 有序项一',
      '2. 有序项二',
      '',
      '```ts',
      'const x = 1 < 2 && "a";',
      '```',
      '',
      '| 名称 | 状态 |',
      '| :--- | ---: |',
      '| 富文本 | 完成 |',
      '',
      '---',
    ].join('\n');

    const html = renderMarkdownToRichHtml(md);

    expect(html).toContain('<h1 ');
    expect(html).toContain('一级标题</h1>');
    expect(html).toContain('<h2 ');
    expect(html).toContain('<strong>加粗</strong>');
    expect(html).toContain('<em>斜体</em>');
    expect(html).toContain('<del>删除线</del>');
    expect(html).toContain('>inlineCode</code>');
    expect(html).toContain('<a href="https://example.com"');
    expect(html).toContain('<blockquote ');
    expect(html).toContain('<ul ');
    expect(html).toContain('<ol ');
    expect(html).toContain('<input type="checkbox" disabled');
    expect(html).toContain('<input type="checkbox" disabled checked');
    expect(html).toContain('data-lang="ts"');
    expect(html).toContain('const x = 1 &lt; 2 &amp;&amp; &quot;a&quot;;');
    expect(html).toContain('<table ');
    expect(html).toContain('text-align:right;');
    expect(html).toContain('<hr ');
  });

  it('escapes raw script tags and blocks dangerous href protocols (XSS protection)', () => {
    expect(escapeHtml('<script>alert("xss")</script>')).toBe(
      '&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;',
    );
    expect(sanitizeHref('javascript:alert(1)')).toBeNull();
    expect(sanitizeHref('java\tscript:alert(1)')).toBeNull();
    expect(sanitizeHref('vbscript:msgbox(1)')).toBeNull();
    expect(sanitizeHref('data:text/html,<script>alert(1)</script>')).toBeNull();
    expect(sanitizeHref('https://example.com/a?b=1')).toBe('https://example.com/a?b=1');
    expect(sanitizeHref('mailto:hi@example.com')).toBe('mailto:hi@example.com');
    expect(sanitizeHref('#section-1')).toBe('#section-1');
    expect(sanitizeHref('./notes/sub.md')).toBe('./notes/sub.md');

    const html = renderMarkdownToRichHtml(
      '<script>alert(1)</script>\n\n[恶意链接](javascript:alert(1)) 和 [正常](https://example.com)',
    );
    expect(html).not.toContain('<script>');
    expect(html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;');
    expect(html).not.toContain('javascript:');
    expect(html).toContain('恶意链接');
    expect(html).toContain('href="https://example.com"');
  });

  it('writes dual-MIME ClipboardItem when supported and falls back to writeText when unavailable', async () => {
    const writeSpy = vi.fn().mockResolvedValue(undefined);
    const writeTextSpy = vi.fn().mockResolvedValue(undefined);

    class MockClipboardItem {
      readonly items: Record<string, Blob>;
      constructor(items: Record<string, Blob>) {
        this.items = items;
      }
    }

    Object.defineProperty(globalThis, 'ClipboardItem', {
      value: MockClipboardItem,
      configurable: true,
    });
    Object.defineProperty(globalThis, 'navigator', {
      value: { clipboard: { write: writeSpy, writeText: writeTextSpy } },
      configurable: true,
    });

    const modeRich = await copyMarkdownAsRichText('# Hello\n\n**World**');
    expect(modeRich).toBe('rich');
    expect(writeSpy).toHaveBeenCalledTimes(1);
    const [items] = writeSpy.mock.calls[0] as [MockClipboardItem[]];
    expect(items[0]?.items['text/html']?.type).toBe('text/html');
    expect(items[0]?.items['text/plain']?.type).toBe('text/plain');
    expect(buildPlainTextFallback('# Hello\r\n')).toBe('# Hello');

    // Fallback when ClipboardItem is undefined
    Object.defineProperty(globalThis, 'ClipboardItem', {
      value: undefined,
      configurable: true,
    });
    const modePlain = await copyMarkdownAsRichText('Fallback text');
    expect(modePlain).toBe('plain');
    expect(writeTextSpy).toHaveBeenCalledWith('Fallback text');
  });

  it('builds complete print-grade HTML document with @page, metadata, and sanitized title', () => {
    const doc = makeSampleDoc({
      meta: {
        ...makeSampleDoc().meta,
        title: '周报 <2026>',
        tags: ['工作', 'a&b'],
        created: '2026-11-24',
      },
    });
    const html = buildPrintHtmlDocument(doc, '## 总结\n\n- 完成导出');

    expect(html).toContain('<!doctype html>');
    expect(html).toContain('@page { size: A4; margin: 18mm 16mm; }');
    expect(html).toContain('<h1 class="doc-title">周报 &lt;2026&gt;</h1>');
    expect(html).toContain('#工作 #a&amp;b');
    expect(html).toContain('创建于: 2026-11-24');
    expect(html).toContain('break-after: avoid');
    expect(html).toContain('完成导出');

    expect(buildDefaultPdfExportPath(makeSampleDoc())).toBe('~/Desktop/测试笔记- 导出-验证-.pdf');
  });

  it('submits ExportPdfModal to note_export_pdf and triggers external.reveal', async () => {
    const doc = makeSampleDoc();

    const ssrApp = createSSRApp(ExportPdfModal, {
      open: true,
      doc,
      body: '# Hello PDF',
    });
    ssrApp.use(createPinia());
    const ssrHtml = await renderToString(ssrApp);
    expect(ssrHtml).toContain('data-testid="export-pdf-modal"');
    expect(ssrHtml).toContain('data-testid="export-pdf-path-input"');
    expect(ssrHtml).toContain('data-testid="export-pdf-submit-btn"');

    const { modalRef, unmount } = mountModalWithFakeRenderer(doc, '# Hello PDF');
    await nextTick();

    const vm = modalRef.value;
    expect(vm).not.toBeNull();
    expect(vm?.destPathDraft).toBe('~/Desktop/测试笔记- 导出-验证-.pdf');

    await vm?.handleSubmit();
    await nextTick();

    expect(mockNoteExportPdf).toHaveBeenCalledTimes(1);
    const args = mockNoteExportPdf.mock.calls[0]?.[0] as { htmlDocument: string; destPath: string };
    expect(args.destPath).toBe('~/Desktop/测试笔记- 导出-验证-.pdf');
    expect(args.htmlDocument).toContain('<!doctype html>');
    expect(args.htmlDocument).toContain('Hello PDF');
    expect(vm?.exportedPath).toBe('~/Desktop/测试笔记- 导出-验证-.pdf');

    await vm?.handleRevealInFinder();

    expect(mockExternalActionRun).toHaveBeenCalledWith({
      id: 'external.reveal',
      absPath: '~/Desktop/测试笔记- 导出-验证-.pdf',
      dirPath: null,
    });

    unmount();
  });
});
