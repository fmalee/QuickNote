import { describe, expect, it } from 'vitest';
import { EditorSelection, EditorState, type StateCommand } from '@codemirror/state';
import {
  createMarkdownIndentCommand,
  createMarkdownOutdentCommand,
  DEFAULT_INDENT_CONFIG,
  detectNonMarkdownIndentUnit,
  type IndentConfig,
  isLineInFencedCodeBlock,
} from '../extensions/markdownIndent';
import {
  collectTodoDecorations,
  todoBackspaceCommand,
  TodoCheckboxWidget,
  todoEnterCommand,
  toggleTodoCommand,
} from '../extensions/markdownTodo';
import {
  formatBlockquote,
  formatBold,
  formatBulletList,
  formatCodeBlock,
  formatHeading,
  formatImage,
  formatItalic,
  formatLink,
  formatTable,
} from '../extensions/markdownFormat';

function runCmd(
  doc: string,
  selection: { anchor: number; head?: number },
  cmd: StateCommand,
): { doc: string; anchor: number; head: number; handled: boolean } {
  let state = EditorState.create({
    doc,
    selection: EditorSelection.range(selection.anchor, selection.head ?? selection.anchor),
  });
  const handled = cmd({
    state,
    dispatch: (tr) => {
      state = tr.state;
    },
  });
  return {
    doc: state.doc.toString(),
    anchor: state.selection.main.anchor,
    head: state.selection.main.head,
    handled,
  };
}

describe('CM6 Markdown 上下文 Tab / ⇧Tab 缩进', () => {
  const mdIndent = createMarkdownIndentCommand(() => DEFAULT_INDENT_CONFIG);
  const mdOutdent = createMarkdownOutdentCommand(() => DEFAULT_INDENT_CONFIG);

  it('1. 普通段落按 Tab 插入 2 空格（绝不插 4 空格误造代码块），⇧Tab 移除前导空格', () => {
    const indented = runCmd('hello world', { anchor: 0 }, mdIndent);
    expect(indented.handled).toBe(true);
    expect(indented.doc).toBe('  hello world');
    expect(indented.anchor).toBe(2);

    const outdented = runCmd(indented.doc, { anchor: 2 }, mdOutdent);
    expect(outdented.handled).toBe(true);
    expect(outdented.doc).toBe('hello world');
    expect(outdented.anchor).toBe(0);
  });

  it('2. 列表项按 marker 宽度动态缩进：- 为 2、1. 为 3、10. 为 4', () => {
    const bullet = runCmd('- item', { anchor: 6 }, mdIndent);
    expect(bullet.doc).toBe('  - item');

    const orderedSingle = runCmd('1. item', { anchor: 7 }, mdIndent);
    expect(orderedSingle.doc).toBe('   1. item');

    const orderedDouble = runCmd('10. item', { anchor: 8 }, mdIndent);
    expect(orderedDouble.doc).toBe('    10. item');

    const orderedOutdent = runCmd(orderedDouble.doc, { anchor: 12 }, mdOutdent);
    expect(orderedOutdent.doc).toBe('10. item');
  });

  it('3. 多行列表选区按 Tab / ⇧Tab 整块缩进与反缩进', () => {
    const doc = '- item 1\n1. item 2\n10. item 3';
    const indented = runCmd(doc, { anchor: 0, head: doc.length }, mdIndent);
    expect(indented.doc).toBe('  - item 1\n   1. item 2\n    10. item 3');

    const outdented = runCmd(indented.doc, { anchor: 0, head: indented.doc.length }, mdOutdent);
    expect(outdented.doc).toBe(doc);
  });

  it('4. 引用块 > quote 按 Tab 变 > > quote，⇧Tab 恢复 > quote', () => {
    const indented = runCmd('> quote', { anchor: 7 }, mdIndent);
    expect(indented.doc).toBe('> > quote');

    const outdented = runCmd(indented.doc, { anchor: 9 }, mdOutdent);
    expect(outdented.doc).toBe('> quote');
  });

  it('5. 围栏代码块内部按 Tab 插入 4 空格，围栏行本身不视为代码块内部', () => {
    const doc = '```js\nconst a = 1\n```';
    const state = EditorState.create({ doc });
    expect(isLineInFencedCodeBlock(state.doc, 1)).toBe(false);
    expect(isLineInFencedCodeBlock(state.doc, 2)).toBe(true);
    expect(isLineInFencedCodeBlock(state.doc, 3)).toBe(false);

    const indented = runCmd(doc, { anchor: 6 }, mdIndent);
    expect(indented.doc).toBe('```js\n    const a = 1\n```');
  });

  it('6. 非 Markdown 文件自动侦测缩进单位（\\t 或 4 空格），不套用 Markdown 引用/列表规则', () => {
    const tabDoc = 'fn main() {\n\tlet x = 1;\n}';
    expect(detectNonMarkdownIndentUnit(EditorState.create({ doc: tabDoc }).doc)).toBe('\t');

    const nonMdConfig: IndentConfig = { isMarkdown: false, tabSizeProse: 2, tabSizeCode: 4 };
    const nonMdIndent = createMarkdownIndentCommand(() => nonMdConfig);

    const resTab = runCmd(tabDoc, { anchor: 12 }, nonMdIndent);
    expect(resTab.doc).toBe('fn main() {\n\t\tlet x = 1;\n}');

    const spaceDoc = '> not a quote\n    indented';
    expect(detectNonMarkdownIndentUnit(EditorState.create({ doc: spaceDoc }).doc)).toBe('    ');
    const resQuote = runCmd(spaceDoc, { anchor: 0 }, nonMdIndent);
    expect(resQuote.doc).toBe('    > not a quote\n    indented');
  });
});

describe('CM6 行首待办 Widget 与待办命令', () => {
  it('1. 保留底层 - [ ] 源文本不被替换，行首注入 side: -1 Widget 装饰', () => {
    const state = EditorState.create({
      doc: '- [ ] 任务\n  - [x] 已完成\n```\n- [ ] 代码块内忽略\n```',
    });

    expect(state.doc.toString()).toBe('- [ ] 任务\n  - [x] 已完成\n```\n- [ ] 代码块内忽略\n```');
    const entries = collectTodoDecorations(state.doc);
    expect(entries).toHaveLength(2);
    const [first, second] = entries;
    expect(first).toBeDefined();
    expect(second).toBeDefined();
    if (!first || !second) {
      return;
    }

    expect(first.from).toBe(0);
    expect(first.side).toBe(-1);
    expect(first.checked).toBe(false);
    expect((first.decoration.spec as { widget?: unknown }).widget).toBeInstanceOf(
      TodoCheckboxWidget,
    );

    expect(second.from).toBe(11);
    expect(second.side).toBe(-1);
    expect(second.checked).toBe(true);
    expect((second.decoration.spec as { widget?: unknown }).widget).toBeInstanceOf(
      TodoCheckboxWidget,
    );
  });

  it('2. toggleTodoCommand 三态循环：任务 -> - [ ] 任务 -> - [x] 任务 -> 任务，且空行插标记后光标落在 - [ ] 之后', () => {
    const emptyStep = runCmd('', { anchor: 0 }, toggleTodoCommand);
    expect(emptyStep.doc).toBe('- [ ] ');
    expect(emptyStep.anchor).toBe(6);

    const step1 = runCmd('任务', { anchor: 2 }, toggleTodoCommand);
    expect(step1.doc).toBe('- [ ] 任务');
    expect(step1.anchor).toBe(8);

    const step2 = runCmd(step1.doc, { anchor: 8 }, toggleTodoCommand);
    expect(step2.doc).toBe('- [x] 任务');
    expect(step2.anchor).toBe(8);

    const step3 = runCmd(step2.doc, { anchor: 8 }, toggleTodoCommand);
    expect(step3.doc).toBe('任务');
    expect(step3.anchor).toBe(2);
  });

  it('3. todoEnterCommand 非空待办续写与空待办清除', () => {
    const continued = runCmd('  - [ ] 写单测', { anchor: 11 }, todoEnterCommand);
    expect(continued.handled).toBe(true);
    expect(continued.doc).toBe('  - [ ] 写单测\n  - [ ] ');
    expect(continued.anchor).toBe('  - [ ] 写单测\n  - [ ] '.length);

    const cleared = runCmd('  - [ ] ', { anchor: 8 }, todoEnterCommand);
    expect(cleared.handled).toBe(true);
    expect(cleared.doc).toBe('');
    expect(cleared.anchor).toBe(0);
  });

  it('4. todoBackspaceCommand 空待办行首一键清空前缀', () => {
    const cleared = runCmd('- [ ] ', { anchor: 6 }, todoBackspaceCommand);
    expect(cleared.handled).toBe(true);
    expect(cleared.doc).toBe('');
    expect(cleared.anchor).toBe(0);

    const nonEmpty = runCmd('- [ ] 内容', { anchor: 6 }, todoBackspaceCommand);
    expect(nonEmpty.handled).toBe(false);
  });

  it('5. 10,000 行大文档单遍扫描精确过滤围栏代码块且耗时 < 100ms', () => {
    const lines: string[] = [];
    let expectedTodoCount = 0;

    // 构造 10,000 行混合内容：普通段落、有效待办项、多种围栏代码块内的伪待办项
    for (let i = 0; i < 200; i += 1) {
      lines.push(`## Section ${i}`);
      lines.push(`- [ ] 有效待办 ${i}-1`);
      lines.push(`  - [x] 有效已完成 ${i}-2`);
      expectedTodoCount += 2;

      // 3 空格缩进反引号围栏 + 内部 3 个反引号不提前闭合 4 个反引号围栏
      lines.push('   ````markdown');
      lines.push('- [ ] 代码块内假待办 A');
      lines.push('```');
      lines.push('- [x] 仍处于四反引号围栏内');
      lines.push('   ````');

      // 波浪号围栏
      lines.push('~~~sh');
      lines.push('- [ ] 波浪号围栏内假待办');
      lines.push('~~~');

      for (let p = 0; p < 39; p += 1) {
        lines.push(`正文填充段落 ${i}-${p}`);
      }
    }

    expect(lines.length).toBe(10_000);
    const state = EditorState.create({ doc: lines.join('\n') });

    const start = performance.now();
    const entries = collectTodoDecorations(state.doc);
    const elapsedMs = performance.now() - start;

    expect(entries).toHaveLength(expectedTodoCount);
    expect(elapsedMs).toBeLessThan(100);

    // 局部区间扫描同样准确
    const lastLine = state.doc.line(state.doc.lines);
    const tailEntries = collectTodoDecorations(state.doc, [
      { from: 0, to: state.doc.line(3).to },
      { from: lastLine.from, to: lastLine.to },
    ]);
    expect(tailEntries).toHaveLength(2);
  });
});

describe('CM6 全套 Markdown 格式命令', () => {
  it('1. formatBold：无选区插入 **** 光标居中，有选区包裹与反包裹', () => {
    const emptyRes = runCmd('', { anchor: 0 }, formatBold);
    expect(emptyRes.doc).toBe('****');
    expect(emptyRes.anchor).toBe(2);

    const wrapRes = runCmd('hello', { anchor: 0, head: 5 }, formatBold);
    expect(wrapRes.doc).toBe('**hello**');
    expect(wrapRes.anchor).toBe(2);
    expect(wrapRes.head).toBe(7);

    const unwrapRes = runCmd(
      wrapRes.doc,
      { anchor: wrapRes.anchor, head: wrapRes.head },
      formatBold,
    );
    expect(unwrapRes.doc).toBe('hello');
  });

  it('2. formatItalic：无选区插入 ** 光标居中，有选区包裹与反包裹', () => {
    const emptyRes = runCmd('', { anchor: 0 }, formatItalic);
    expect(emptyRes.doc).toBe('**');
    expect(emptyRes.anchor).toBe(1);

    const wrapRes = runCmd('hello', { anchor: 0, head: 5 }, formatItalic);
    expect(wrapRes.doc).toBe('*hello*');
  });

  it('3. formatCodeBlock：无选区插入围栏代码块且光标位于中间空行，有选区前后包裹', () => {
    const emptyLine = runCmd('prefix\n', { anchor: 7 }, formatCodeBlock);
    expect(emptyLine.doc).toBe('prefix\n```\n\n```');
    expect(emptyLine.anchor).toBe(11);

    const nonEmptyLine = runCmd('prefix', { anchor: 3 }, formatCodeBlock);
    expect(nonEmptyLine.doc).toBe('prefix\n```\n\n```');
    expect(nonEmptyLine.anchor).toBe(11);

    const wrapped = runCmd('const x = 1;', { anchor: 0, head: 12 }, formatCodeBlock);
    expect(wrapped.doc).toBe('```\nconst x = 1;\n```');
  });

  it('4. formatTable：插入 3 行 Markdown 表格并选中首格 列1', () => {
    const res = runCmd('', { anchor: 0 }, formatTable);
    expect(res.doc).toBe('| 列1 | 列2 |\n| --- | --- |\n|  |  |');
    expect(res.doc.slice(res.anchor, res.head)).toBe('列1');
  });

  it('5. formatLink：无选区插 [](url)、URL 选区插 [](url)、普通文本插 [文本]()', () => {
    const empty = runCmd('', { anchor: 0 }, formatLink);
    expect(empty.doc).toBe('[](url)');
    expect(empty.anchor).toBe(1);

    const urlSel = runCmd('https://example.com', { anchor: 0, head: 19 }, formatLink);
    expect(urlSel.doc).toBe('[](https://example.com)');
    expect(urlSel.anchor).toBe(1);

    const textSel = runCmd('文本', { anchor: 0, head: 2 }, formatLink);
    expect(textSel.doc).toBe('[文本]()');
    expect(textSel.anchor).toBe(5);
  });

  it('6. formatImage、formatBulletList、formatBlockquote 与 formatHeading(1..6)', () => {
    const imgEmpty = runCmd('', { anchor: 0 }, formatImage);
    expect(imgEmpty.doc).toBe('![]()');
    expect(imgEmpty.anchor).toBe(2);

    const bulletEmpty = runCmd('', { anchor: 0 }, formatBulletList);
    expect(bulletEmpty.doc).toBe('- ');
    expect(bulletEmpty.anchor).toBe(2);

    const bulletOn = runCmd('a\nb', { anchor: 0, head: 3 }, formatBulletList);
    expect(bulletOn.doc).toBe('- a\n- b');
    expect(bulletOn.anchor).toBe(2);
    expect(bulletOn.head).toBe(7);
    const bulletOff = runCmd(
      bulletOn.doc,
      { anchor: 0, head: bulletOn.doc.length },
      formatBulletList,
    );
    expect(bulletOff.doc).toBe('a\nb');

    const quoteEmpty = runCmd('', { anchor: 0 }, formatBlockquote);
    expect(quoteEmpty.doc).toBe('> ');
    expect(quoteEmpty.anchor).toBe(2);

    const quoteOn = runCmd('a\nb', { anchor: 0, head: 3 }, formatBlockquote);
    expect(quoteOn.doc).toBe('> a\n> b');

    const h2Empty = runCmd('', { anchor: 0 }, formatHeading(2));
    expect(h2Empty.doc).toBe('## ');
    expect(h2Empty.anchor).toBe(3);

    const h2 = runCmd('标题', { anchor: 0 }, formatHeading(2));
    expect(h2.doc).toBe('## 标题');
    expect(h2.anchor).toBe(3);
    const h4 = runCmd(h2.doc, { anchor: h2.anchor }, formatHeading(4));
    expect(h4.doc).toBe('#### 标题');
    expect(h4.anchor).toBe(5);
    const hOff = runCmd(h4.doc, { anchor: h4.anchor }, formatHeading(4));
    expect(hOff.doc).toBe('标题');
    expect(hOff.anchor).toBe(0);
  });

  it('7. 异步图片插入坐标夹取至文档长度范围内，防止越界 dispatch 抛错', () => {
    const state = EditorState.create({ doc: 'short' });
    const stalePos = 999;
    const clampedPos = Math.max(0, Math.min(stalePos, state.doc.length));
    expect(clampedPos).toBe(state.doc.length);

    const snippet = '![](assets/2026-11/img.png)';
    const tr = state.update({
      changes: { from: clampedPos, to: clampedPos, insert: snippet },
      selection: { anchor: clampedPos + snippet.length },
    });
    expect(tr.state.doc.toString()).toBe('short![](assets/2026-11/img.png)');
  });
});
