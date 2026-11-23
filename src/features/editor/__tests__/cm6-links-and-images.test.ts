import { describe, expect, it, vi } from 'vitest';
import { markdown } from '@codemirror/lang-markdown';
import { EditorState } from '@codemirror/state';
import {
  arrayBufferToBase64,
  buildMarkdownImageSnippet,
  type CurrentDocContext,
  extractImageExtFromFile,
  extractLinksFromState,
  findLinkAtPos,
  handleImageInsert,
  isLinkTriggerEvent,
  parseLinkTarget,
  resolveHeadingAnchor,
} from '../index';

function createMarkdownState(doc: string): EditorState {
  return EditorState.create({
    doc,
    extensions: [markdown()],
  });
}

describe('CM6 语法树链接提取', () => {
  it('准确提取行内链接、裸 URL 与引用式链接', () => {
    const doc = [
      '请查看 [文档](./a.md#配置) 与 https://example.com/docs 以及 [规范][spec]。',
      '',
      '[spec]: ./spec.md',
    ].join('\n');
    const state = createMarkdownState(doc);
    const links = extractLinksFromState(state);

    expect(links.length).toBeGreaterThanOrEqual(3);
    expect(links.map((item) => item.url)).toEqual(
      expect.arrayContaining(['./a.md#配置', 'https://example.com/docs', './spec.md']),
    );

    const inlineOffset = doc.indexOf('[文档](./a.md#配置)') + 2;
    const hit = findLinkAtPos(state, inlineOffset);
    expect(hit).not.toBeNull();
    expect(hit?.url).toBe('./a.md#配置');

    const plainOffset = doc.indexOf('请查看');
    expect(findLinkAtPos(state, plainOffset)).toBeNull();
  });

  it('忽略代码块内的 URL 与链接文本', () => {
    const doc = [
      '```ts',
      'const url = "https://ignored.example.com";',
      '```',
      '',
      '`[x](ignored.md)`',
    ].join('\n');
    const state = createMarkdownState(doc);
    const links = extractLinksFromState(state);
    expect(links).toHaveLength(0);
  });
});

describe('中文 Slug 锚点匹配（C8.4）', () => {
  const sampleDoc = [
    '# 实现方案',
    '第一段内容',
    '',
    '# 实现方案',
    '第二段重复标题',
    '',
    '## CJK + **混合**分词',
    '第三段内容',
  ].join('\n');

  it('精确命中首个标题、重复标题（-1）与 CJK 混合内联格式标题', () => {
    const first = resolveHeadingAnchor(sampleDoc, '实现方案');
    expect(first).not.toBeNull();
    expect(first?.line).toBe(1);

    const second = resolveHeadingAnchor(sampleDoc, '实现方案-1');
    expect(second).not.toBeNull();
    expect(second?.line).toBe(4);

    const mixed = resolveHeadingAnchor(sampleDoc, 'cjk-混合分词');
    expect(mixed).not.toBeNull();
    expect(mixed?.line).toBe(7);
  });

  it('失效锚点返回 null 不抛异常', () => {
    expect(resolveHeadingAnchor(sampleDoc, '不存在的标题')).toBeNull();
    expect(resolveHeadingAnchor('', '实现方案')).toBeNull();
  });
});

describe('链接目标分流与安全确认（C2.3）', () => {
  const docCtx: CurrentDocContext = {
    absPath: '/Users/demo/Vault/notes/daily.md',
    relPath: 'notes/daily.md',
    isLibrary: true,
  };
  const libraryRoot = '/Users/demo/Vault';

  it('正确分流 external、anchor 与 relativeNote', () => {
    expect(parseLinkTarget('https://example.com', docCtx, libraryRoot)).toEqual({
      kind: 'external',
      url: 'https://example.com',
    });

    expect(parseLinkTarget('mailto:test@example.com', docCtx, libraryRoot)).toEqual({
      kind: 'external',
      url: 'mailto:test@example.com',
    });

    expect(parseLinkTarget('#中文标题', docCtx, libraryRoot)).toEqual({
      kind: 'anchor',
      anchor: '中文标题',
    });

    expect(parseLinkTarget('./sub/note.md#章节', docCtx, libraryRoot)).toEqual({
      kind: 'relativeNote',
      targetAbsPath: '/Users/demo/Vault/notes/sub/note.md',
      targetRelPath: 'notes/sub/note.md',
      anchor: '章节',
    });

    expect(parseLinkTarget('../guide.md', docCtx, libraryRoot)).toEqual({
      kind: 'relativeNote',
      targetAbsPath: '/Users/demo/Vault/guide.md',
      targetRelPath: 'guide.md',
      anchor: null,
    });
  });

  it('本地绝对路径与 file:// 强制 requiresConfirmation === true', () => {
    expect(parseLinkTarget('/etc/hosts', docCtx, libraryRoot)).toEqual({
      kind: 'localAbsolute',
      path: '/etc/hosts',
      requiresConfirmation: true,
    });

    expect(parseLinkTarget('file:///Users/a/b.md', docCtx, libraryRoot)).toEqual({
      kind: 'localAbsolute',
      path: '/Users/a/b.md',
      requiresConfirmation: true,
    });
  });

  it('拦截 javascript: 等危险协议与空链接', () => {
    const jsResult = parseLinkTarget('javascript:alert(1)', docCtx, libraryRoot);
    expect(jsResult.kind).toBe('invalid');

    const emptyResult = parseLinkTarget('   ', docCtx, libraryRoot);
    expect(emptyResult.kind).toBe('invalid');
  });
});

describe('链接点击触发模式判定', () => {
  it('默认 singleClickLink = false 时仅 metaKey 左键单击触发', () => {
    expect(
      isLinkTriggerEvent({ button: 0, metaKey: false, ctrlKey: false, altKey: false }, false),
    ).toBe(false);
    expect(
      isLinkTriggerEvent({ button: 0, metaKey: true, ctrlKey: false, altKey: false }, false),
    ).toBe(true);
    expect(
      isLinkTriggerEvent({ button: 0, metaKey: true, ctrlKey: true, altKey: false }, false),
    ).toBe(false);
  });

  it('singleClickLink = true 时普通左键单击即可触发', () => {
    expect(
      isLinkTriggerEvent({ button: 0, metaKey: false, ctrlKey: false, altKey: false }, true),
    ).toBe(true);
    expect(
      isLinkTriggerEvent({ button: 2, metaKey: false, ctrlKey: false, altKey: false }, true),
    ).toBe(false);
  });
});

describe('图片粘贴与拖入落盘', () => {
  it('extractImageExtFromFile、arrayBufferToBase64 与 buildMarkdownImageSnippet 正确工作', () => {
    expect(extractImageExtFromFile({ type: 'image/png', name: 'shot.png' })).toBe('png');
    expect(extractImageExtFromFile({ type: 'image/jpeg', name: 'photo.jpeg' })).toBe('jpg');
    expect(extractImageExtFromFile({ type: '', name: 'diagram.WEBP' })).toBe('webp');
    expect(extractImageExtFromFile({ type: 'application/pdf', name: 'doc.pdf' })).toBeNull();

    const bytes = new Uint8Array([72, 105, 33]).buffer;
    expect(arrayBufferToBase64(bytes)).toBe('SGkh');
    expect(buildMarkdownImageSnippet('assets/2026-11/abc.png', '截图')).toBe(
      '![截图](assets/2026-11/abc.png)',
    );
    expect(buildMarkdownImageSnippet('assets/2026-11/abc.png')).toBe(
      '![image](assets/2026-11/abc.png)',
    );
  });

  it('库内文档直接调用 saveImage 并插入 Markdown 引用', async () => {
    const saveImage = vi.fn().mockResolvedValue({ relPath: 'assets/2026-11/pic.png' });
    const confirmExternalSave = vi.fn().mockResolvedValue(true);
    const insertSnippet = vi.fn();
    const onError = vi.fn();

    const outcome = await handleImageInsert({
      file: {
        name: 'clipboard.png',
        type: 'image/png',
        size: 4,
        arrayBuffer: async () => new Uint8Array([1, 2, 3, 4]).buffer,
      },
      isLibrary: true,
      contextRelPath: 'notes/demo.md',
      saveImage,
      confirmExternalSave,
      insertSnippet,
      onError,
    });

    expect(outcome).toBe('inserted');
    expect(confirmExternalSave).not.toHaveBeenCalled();
    expect(saveImage).toHaveBeenCalledWith({
      sourcePath: null,
      bytesBase64: 'AQIDBA==',
      ext: 'png',
      contextRelPath: 'notes/demo.md',
    });
    expect(insertSnippet).toHaveBeenCalledWith('![clipboard](assets/2026-11/pic.png)');
    expect(onError).not.toHaveBeenCalled();
  });

  it('库外文档须先经过二次确认，取消时不落盘也不插入', async () => {
    const saveImage = vi.fn().mockResolvedValue({ relPath: 'assets/2026-11/ext.png' });
    const confirmExternalSave = vi.fn().mockResolvedValue(false);
    const insertSnippet = vi.fn();
    const onError = vi.fn();

    const outcome = await handleImageInsert({
      file: {
        name: 'ext.png',
        type: 'image/png',
        size: 4,
        arrayBuffer: async () => new Uint8Array([1, 2, 3, 4]).buffer,
      },
      isLibrary: false,
      contextRelPath: null,
      saveImage,
      confirmExternalSave,
      insertSnippet,
      onError,
    });

    expect(outcome).toBe('cancelled');
    expect(confirmExternalSave).toHaveBeenCalledTimes(1);
    expect(saveImage).not.toHaveBeenCalled();
    expect(insertSnippet).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
  });

  it('非法格式或 IPC 失败优雅提示且不破坏正文', async () => {
    const saveImage = vi.fn().mockRejectedValue(new Error('未配置笔记库'));
    const confirmExternalSave = vi.fn().mockResolvedValue(true);
    const insertSnippet = vi.fn();
    const onError = vi.fn();

    const badFormat = await handleImageInsert({
      file: {
        name: 'archive.zip',
        type: 'application/zip',
        size: 4,
        arrayBuffer: async () => new Uint8Array([1, 2]).buffer,
      },
      isLibrary: true,
      contextRelPath: 'notes/demo.md',
      saveImage,
      confirmExternalSave,
      insertSnippet,
      onError,
    });
    expect(badFormat).toBe('error');
    expect(onError).toHaveBeenCalledTimes(1);
    expect(insertSnippet).not.toHaveBeenCalled();

    const ipcFail = await handleImageInsert({
      file: {
        name: 'shot.png',
        type: 'image/png',
        size: 4,
        arrayBuffer: async () => new Uint8Array([1, 2, 3, 4]).buffer,
      },
      isLibrary: true,
      contextRelPath: 'notes/demo.md',
      saveImage,
      confirmExternalSave,
      insertSnippet,
      onError,
    });
    expect(ipcFail).toBe('error');
    expect(onError).toHaveBeenCalledWith('未配置笔记库');
    expect(insertSnippet).not.toHaveBeenCalled();
  });
});
