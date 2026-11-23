import { EditorSelection, type StateCommand, type Text } from '@codemirror/state';

export interface IndentConfig {
  isMarkdown: boolean;
  tabSizeProse: number;
  tabSizeCode: number;
}

export const DEFAULT_INDENT_CONFIG: IndentConfig = {
  isMarkdown: true,
  tabSizeProse: 2,
  tabSizeCode: 4,
};

const FENCE_OPEN_RE = /^\s*(`{3,}|~{3,})/;
const FENCE_CLOSE_RE = /^\s*(`{3,}|~{3,})\s*$/;
const LIST_MARKER_RE = /^(\s*)([-*+]|\d+\.)(\s+)/;
const BLOCKQUOTE_RE = /^(\s*)(>+)( ?)/;

/**
 * 从第 1 行向下扫描围栏代码块状态，判断指定行是否位于代码块内部（不含围栏开闭行本身）。
 */
export function isLineInFencedCodeBlock(doc: Text, lineNumber: number): boolean {
  if (lineNumber < 1 || lineNumber > doc.lines) {
    return false;
  }

  let activeFenceChar: '`' | '~' | null = null;
  let activeFenceLen = 0;

  for (let i = 1; i <= lineNumber; i += 1) {
    const lineText = doc.line(i).text;

    if (activeFenceChar === null) {
      const openMatch = FENCE_OPEN_RE.exec(lineText);
      const fence = openMatch?.[1];
      if (fence) {
        activeFenceChar = fence[0] as '`' | '~';
        activeFenceLen = fence.length;
        if (i === lineNumber) {
          return false;
        }
      } else if (i === lineNumber) {
        return false;
      }
    } else {
      const closeMatch = FENCE_CLOSE_RE.exec(lineText);
      const closeFence = closeMatch?.[1];
      if (closeFence && closeFence[0] === activeFenceChar && closeFence.length >= activeFenceLen) {
        activeFenceChar = null;
        activeFenceLen = 0;
        if (i === lineNumber) {
          return false;
        }
      } else if (i === lineNumber) {
        return true;
      }
    }
  }

  return false;
}

/**
 * 针对非 Markdown 文件，扫描前 100 行既有缩进：
 * 若 tab 占优返回 '\t'，否则估算常见空格步长（2 或 4）。
 */
export function detectNonMarkdownIndentUnit(doc: Text): string {
  const maxLines = Math.min(doc.lines, 100);
  let tabLines = 0;
  let spaceLines = 0;
  const spaceWidths: number[] = [];

  for (let i = 1; i <= maxLines; i += 1) {
    const text = doc.line(i).text;
    if (text.length === 0) {
      continue;
    }
    if (text[0] === '\t') {
      tabLines += 1;
      continue;
    }
    if (text[0] === ' ') {
      const match = /^( +)/.exec(text);
      const spaces = match?.[1];
      if (spaces) {
        spaceLines += 1;
        spaceWidths.push(spaces.length);
      }
    }
  }

  if (tabLines > spaceLines) {
    return '\t';
  }
  if (spaceLines === 0) {
    return '    ';
  }

  const hasTwoSpaceStep = spaceWidths.some((width) => width % 4 !== 0 && width % 2 === 0);
  return hasTwoSpaceStep ? '  ' : '    ';
}

function getCoveredLineNumbers(
  doc: Text,
  from: number,
  to: number,
  empty: boolean,
): { fromLine: number; toLine: number } {
  const fromLine = doc.lineAt(from).number;
  if (empty) {
    return { fromLine, toLine: fromLine };
  }
  const effectiveTo = to > from && doc.lineAt(to).from === to ? to - 1 : to;
  const toLine = doc.lineAt(effectiveTo).number;
  return { fromLine, toLine };
}

function countLeadingSpacesToRemove(lineText: string, maxSpaces: number): number {
  let count = 0;
  while (count < maxSpaces && count < lineText.length && lineText[count] === ' ') {
    count += 1;
  }
  return count;
}

function resolveMarkdownLineIndentWidth(
  doc: Text,
  lineNumber: number,
  tabSizeProse: number,
): { kind: 'list' | 'blockquote' | 'code' | 'prose'; width: number } {
  if (isLineInFencedCodeBlock(doc, lineNumber)) {
    return { kind: 'code', width: 0 };
  }

  const lineText = doc.line(lineNumber).text;
  const listMatch = LIST_MARKER_RE.exec(lineText);
  if (listMatch) {
    const markerWidth = (listMatch[2] ?? '').length + (listMatch[3] ?? '').length;
    return { kind: 'list', width: markerWidth };
  }

  if (BLOCKQUOTE_RE.test(lineText)) {
    return { kind: 'blockquote', width: 2 };
  }

  return { kind: 'prose', width: tabSizeProse };
}

function buildLineOutdentChange(
  lineFrom: number,
  lineText: string,
  maxSpaces: number,
): { from: number; to: number; insert: string } | null {
  if (lineText.startsWith('\t')) {
    return { from: lineFrom, to: lineFrom + 1, insert: '' };
  }
  const removeCount = countLeadingSpacesToRemove(lineText, maxSpaces);
  if (removeCount === 0) {
    return null;
  }
  return { from: lineFrom, to: lineFrom + removeCount, insert: '' };
}

export function createMarkdownIndentCommand(getConfig: () => IndentConfig): StateCommand {
  return ({ state, dispatch }) => {
    const config = getConfig();
    const doc = state.doc;
    const proseSpaces = ' '.repeat(Math.max(1, config.tabSizeProse));
    const codeSpaces = ' '.repeat(Math.max(1, config.tabSizeCode));

    if (!config.isMarkdown) {
      const unit = detectNonMarkdownIndentUnit(doc);
      const tr = state.changeByRange((range) => {
        if (range.empty) {
          return {
            changes: { from: range.from, to: range.from, insert: unit },
            range: EditorSelection.cursor(range.from + unit.length),
          };
        }
        const { fromLine, toLine } = getCoveredLineNumbers(doc, range.from, range.to, false);
        const changes: Array<{ from: number; to: number; insert: string }> = [];
        for (let ln = fromLine; ln <= toLine; ln += 1) {
          const line = doc.line(ln);
          changes.push({ from: line.from, to: line.from, insert: unit });
        }
        const totalAdded = unit.length * (toLine - fromLine + 1);
        const anchorAtStart = range.anchor <= range.head;
        const newFrom =
          range.from === doc.line(fromLine).from ? range.from : range.from + unit.length;
        const newTo = range.to + totalAdded;
        return {
          changes,
          range: anchorAtStart
            ? EditorSelection.range(newFrom, newTo)
            : EditorSelection.range(newTo, newFrom),
        };
      });
      dispatch(state.update(tr, { userEvent: 'input.indent' }));
      return true;
    }

    const tr = state.changeByRange((range) => {
      const { fromLine, toLine } = getCoveredLineNumbers(doc, range.from, range.to, range.empty);
      const isMultiLine = toLine > fromLine;

      if (!isMultiLine && range.empty) {
        const line = doc.line(fromLine);
        const kindInfo = resolveMarkdownLineIndentWidth(doc, fromLine, config.tabSizeProse);

        if (kindInfo.kind === 'list') {
          const spaces = ' '.repeat(kindInfo.width);
          return {
            changes: { from: line.from, to: line.from, insert: spaces },
            range: EditorSelection.cursor(range.from + spaces.length),
          };
        }

        if (kindInfo.kind === 'blockquote') {
          const bqMatch = BLOCKQUOTE_RE.exec(line.text);
          const insertPos = line.from + (bqMatch?.[1]?.length ?? 0);
          return {
            changes: { from: insertPos, to: insertPos, insert: '> ' },
            range: EditorSelection.cursor(range.from + 2),
          };
        }

        const insertStr = kindInfo.kind === 'code' ? codeSpaces : proseSpaces;
        return {
          changes: { from: range.from, to: range.from, insert: insertStr },
          range: EditorSelection.cursor(range.from + insertStr.length),
        };
      }

      const changes: Array<{ from: number; to: number; insert: string }> = [];
      let addedOnFirstLine = 0;
      let totalAdded = 0;

      for (let ln = fromLine; ln <= toLine; ln += 1) {
        const line = doc.line(ln);
        const kindInfo = resolveMarkdownLineIndentWidth(doc, ln, config.tabSizeProse);
        let insertPos = line.from;
        let insertText = proseSpaces;

        if (kindInfo.kind === 'list') {
          insertText = ' '.repeat(kindInfo.width);
        } else if (kindInfo.kind === 'blockquote') {
          const bqMatch = BLOCKQUOTE_RE.exec(line.text);
          insertPos = line.from + (bqMatch?.[1]?.length ?? 0);
          insertText = '> ';
        } else if (kindInfo.kind === 'code') {
          insertText = codeSpaces;
        }

        changes.push({ from: insertPos, to: insertPos, insert: insertText });
        if (ln === fromLine) {
          addedOnFirstLine = insertText.length;
        }
        totalAdded += insertText.length;
      }

      if (range.empty) {
        return {
          changes,
          range: EditorSelection.cursor(range.from + totalAdded),
        };
      }

      const anchorAtStart = range.anchor <= range.head;
      const firstLineStart = doc.line(fromLine).from;
      const newFrom = range.from === firstLineStart ? range.from : range.from + addedOnFirstLine;
      const newTo = range.to + totalAdded;
      return {
        changes,
        range: anchorAtStart
          ? EditorSelection.range(newFrom, newTo)
          : EditorSelection.range(newTo, newFrom),
      };
    });

    dispatch(state.update(tr, { userEvent: 'input.indent' }));
    return true;
  };
}

export function createMarkdownOutdentCommand(getConfig: () => IndentConfig): StateCommand {
  return ({ state, dispatch }) => {
    const config = getConfig();
    const doc = state.doc;
    const nonMdUnit = config.isMarkdown ? '' : detectNonMarkdownIndentUnit(doc);

    const tr = state.changeByRange((range) => {
      const { fromLine, toLine } = getCoveredLineNumbers(doc, range.from, range.to, range.empty);
      const changes: Array<{ from: number; to: number; insert: string }> = [];
      let removedOnFirstLine = 0;
      let totalRemoved = 0;

      for (let ln = fromLine; ln <= toLine; ln += 1) {
        const line = doc.line(ln);
        let change: { from: number; to: number; insert: string } | null = null;

        if (!config.isMarkdown) {
          const maxSpaces = nonMdUnit === '\t' ? config.tabSizeCode : nonMdUnit.length;
          change = buildLineOutdentChange(line.from, line.text, maxSpaces);
        } else {
          const kindInfo = resolveMarkdownLineIndentWidth(doc, ln, config.tabSizeProse);
          if (kindInfo.kind === 'blockquote') {
            const bqMatch = BLOCKQUOTE_RE.exec(line.text);
            if (bqMatch) {
              const startPos = line.from + (bqMatch[1] ?? '').length;
              const removeLen = 1 + (bqMatch[3] === ' ' ? 1 : 0);
              change = { from: startPos, to: startPos + removeLen, insert: '' };
            }
          } else if (kindInfo.kind === 'list') {
            change = buildLineOutdentChange(line.from, line.text, kindInfo.width);
          } else if (kindInfo.kind === 'code') {
            change = buildLineOutdentChange(line.from, line.text, config.tabSizeCode);
          } else {
            change = buildLineOutdentChange(line.from, line.text, config.tabSizeProse);
          }
        }

        if (change) {
          const removed = change.to - change.from;
          changes.push(change);
          if (ln === fromLine) {
            removedOnFirstLine = removed;
          }
          totalRemoved += removed;
        }
      }

      if (changes.length === 0) {
        return { range };
      }

      if (range.empty) {
        const firstLineStart = doc.line(fromLine).from;
        const nextPos = Math.max(firstLineStart, range.from - removedOnFirstLine);
        return {
          changes,
          range: EditorSelection.cursor(nextPos),
        };
      }

      const anchorAtStart = range.anchor <= range.head;
      const firstLineStart = doc.line(fromLine).from;
      const newFrom = Math.max(firstLineStart, range.from - removedOnFirstLine);
      const newTo = Math.max(newFrom, range.to - totalRemoved);
      return {
        changes,
        range: anchorAtStart
          ? EditorSelection.range(newFrom, newTo)
          : EditorSelection.range(newTo, newFrom),
      };
    });

    if (tr.changes.empty) {
      return false;
    }

    dispatch(state.update(tr, { userEvent: 'delete.dedent' }));
    return true;
  };
}
