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

  it('2. toggleTodoCommand 三态循环：任务 -> - [ ] 任务 -> - [x] 任务 -> 任务', () => {
    const step1 = runCmd('任务', { anchor: 2 }, toggleTodoCommand);
    expect(step1.doc).toBe('- [ ] 任务');

    const step2 = runCmd(step1.doc, { anchor: 8 }, toggleTodoCommand);
    expect(step2.doc).toBe('- [x] 任务');

    const step3 = runCmd(step2.doc, { anchor: 8 }, toggleTodoCommand);
    expect(step3.doc).toBe('任务');
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

    const bulletOn = runCmd('a\nb', { anchor: 0, head: 3 }, formatBulletList);
    expect(bulletOn.doc).toBe('- a\n- b');
    const bulletOff = runCmd(
      bulletOn.doc,
      { anchor: 0, head: bulletOn.doc.length },
      formatBulletList,
    );
    expect(bulletOff.doc).toBe('a\nb');

    const quoteOn = runCmd('a\nb', { anchor: 0, head: 3 }, formatBlockquote);
    expect(quoteOn.doc).toBe('> a\n> b');

    const h2 = runCmd('标题', { anchor: 0 }, formatHeading(2));
    expect(h2.doc).toBe('## 标题');
    const h4 = runCmd(h2.doc, { anchor: 0 }, formatHeading(4));
    expect(h4.doc).toBe('#### 标题');
    const hOff = runCmd(h4.doc, { anchor: 0 }, formatHeading(4));
    expect(hOff.doc).toBe('标题');
  });
});
