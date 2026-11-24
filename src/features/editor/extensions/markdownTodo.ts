import { EditorSelection, type StateCommand, type Text } from '@codemirror/state';
import {
  Decoration,
  type DecorationSet,
  type EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from '@codemirror/view';
import { isLineInFencedCodeBlock } from './markdownIndent';

export const TODO_LINE_RE = /^(\s*)([-*+])(\s+)\[([ xX])\](?=\s|$)/;
const BULLET_LINE_RE = /^(\s*)([-*+])(\s+)(.*)$/;
const LEADING_WS_RE = /^(\s*)(.*)$/;
const FENCE_OPEN_RE = /^\s{0,3}(`{3,}|~{3,})/;
const FENCE_CLOSE_RE = /^\s{0,3}(`{3,}|~{3,})\s*$/;

export class TodoCheckboxWidget extends WidgetType {
  readonly checked: boolean;

  constructor(checked: boolean) {
    super();
    this.checked = checked;
  }

  eq(other: TodoCheckboxWidget): boolean {
    return other.checked === this.checked;
  }

  toDOM(view: EditorView): HTMLElement {
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.className = 'qn-cm-todo-checkbox';
    input.checked = this.checked;
    input.tabIndex = -1;
    input.setAttribute('aria-label', this.checked ? '标记为未完成' : '标记为已完成');

    input.addEventListener('mousedown', (event) => {
      // 阻止抢占编辑器焦点，避免破坏中文输入法 IME 组合选词
      event.preventDefault();
    });

    input.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();

      try {
        const pos = view.posAtDOM(input);
        const line = view.state.doc.lineAt(pos);
        const match = TODO_LINE_RE.exec(line.text);
        if (!match) {
          return;
        }
        const checkCharOffset =
          (match[1] ?? '').length + (match[2] ?? '').length + (match[3] ?? '').length + 1;
        const from = line.from + checkCharOffset;
        const nextChar = match[4] === ' ' ? 'x' : ' ';
        view.dispatch({
          changes: { from, to: from + 1, insert: nextChar },
        });
      } catch {
        // DOM 尚未挂载或已卸载时容错
      }
    });

    return input;
  }

  ignoreEvent(event: Event): boolean {
    return event.type === 'mousedown' || event.type === 'click' || event.type === 'change';
  }
}

export interface TodoDecorationEntry {
  from: number;
  side: -1;
  checked: boolean;
  decoration: Decoration;
}

export function collectTodoDecorations(
  doc: Text,
  ranges?: readonly { from: number; to: number }[],
): TodoDecorationEntry[] {
  const entries: TodoDecorationEntry[] = [];
  const scanRanges = ranges ?? [{ from: 0, to: doc.length }];
  if (scanRanges.length === 0 || doc.lines === 0) {
    return entries;
  }

  const rawIntervals = scanRanges
    .map(({ from, to }) => {
      const clampedFrom = Math.max(0, Math.min(from, doc.length));
      const clampedTo = Math.max(clampedFrom, Math.min(to, doc.length));
      return {
        startLine: doc.lineAt(clampedFrom).number,
        endLine: doc.lineAt(clampedTo).number,
      };
    })
    .sort((a, b) => a.startLine - b.startLine || a.endLine - b.endLine);

  const mergedIntervals: Array<{ startLine: number; endLine: number }> = [];
  for (const interval of rawIntervals) {
    const prev = mergedIntervals[mergedIntervals.length - 1];
    if (prev && interval.startLine <= prev.endLine + 1) {
      prev.endLine = Math.max(prev.endLine, interval.endLine);
    } else {
      mergedIntervals.push({ ...interval });
    }
  }

  if (mergedIntervals.length === 0) {
    return entries;
  }

  const maxEndLine = mergedIntervals[mergedIntervals.length - 1]!.endLine;
  let rangeIdx = 0;
  let fenceChar: '`' | '~' | null = null;
  let fenceLen = 0;

  for (let ln = 1; ln <= maxEndLine; ln += 1) {
    while (rangeIdx < mergedIntervals.length && ln > mergedIntervals[rangeIdx]!.endLine) {
      rangeIdx += 1;
    }
    if (rangeIdx >= mergedIntervals.length) {
      break;
    }

    const line = doc.line(ln);
    let inFence = false;

    if (fenceChar === null) {
      const openMatch = FENCE_OPEN_RE.exec(line.text);
      const fence = openMatch?.[1];
      if (fence) {
        fenceChar = fence[0] as '`' | '~';
        fenceLen = fence.length;
      }
    } else {
      const closeMatch = FENCE_CLOSE_RE.exec(line.text);
      const closeFence = closeMatch?.[1];
      if (closeFence && closeFence[0] === fenceChar && closeFence.length >= fenceLen) {
        fenceChar = null;
        fenceLen = 0;
      } else {
        inFence = true;
      }
    }

    const currentInterval = mergedIntervals[rangeIdx]!;
    if (inFence || ln < currentInterval.startLine) {
      continue;
    }

    const match = TODO_LINE_RE.exec(line.text);
    if (!match) {
      continue;
    }

    const checked = (match[4] ?? ' ').toLowerCase() === 'x';
    const widgetPos = line.from + (match[1] ?? '').length;
    entries.push({
      from: widgetPos,
      side: -1,
      checked,
      decoration: Decoration.widget({
        widget: new TodoCheckboxWidget(checked),
        side: -1,
      }),
    });
  }

  return entries;
}

function buildTodoDecorations(view: EditorView): DecorationSet {
  const entries = collectTodoDecorations(view.state.doc, view.visibleRanges);
  return Decoration.set(entries.map((item) => item.decoration.range(item.from)));
}

export const todoCheckboxPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = buildTodoDecorations(view);
    }

    update(update: ViewUpdate): void {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildTodoDecorations(update.view);
      }
    }
  },
  {
    decorations: (value) => value.decorations,
  },
);

/**
 * ⌥⌘X 三态循环切换待办命令：
 * 普通文本 / 普通列表 -> `- [ ] ` -> `- [x] ` -> 普通文本
 */
export const toggleTodoCommand: StateCommand = ({ state, dispatch }) => {
  const doc = state.doc;
  const processedLines = new Set<number>();
  const changes: Array<{ from: number; to: number; insert: string }> = [];

  for (const range of state.selection.ranges) {
    const fromLine = doc.lineAt(range.from).number;
    const effectiveTo =
      !range.empty && range.to > range.from && doc.lineAt(range.to).from === range.to
        ? range.to - 1
        : range.to;
    const toLine = doc.lineAt(effectiveTo).number;

    for (let ln = fromLine; ln <= toLine; ln += 1) {
      if (processedLines.has(ln)) {
        continue;
      }
      processedLines.add(ln);

      const line = doc.line(ln);
      const todoMatch = TODO_LINE_RE.exec(line.text);
      if (todoMatch) {
        const indentLen = (todoMatch[1] ?? '').length;
        const bulletLen = (todoMatch[2] ?? '').length;
        const spaceLen = (todoMatch[3] ?? '').length;
        const isChecked = (todoMatch[4] ?? ' ').toLowerCase() === 'x';
        const bracketCharPos = line.from + indentLen + bulletLen + spaceLen + 1;

        if (!isChecked) {
          changes.push({ from: bracketCharPos, to: bracketCharPos + 1, insert: 'x' });
        } else {
          const prefixEndInLine = indentLen + bulletLen + spaceLen + 3;
          const hasTrailingSpace = line.text[prefixEndInLine] === ' ';
          changes.push({
            from: line.from + indentLen,
            to: line.from + prefixEndInLine + (hasTrailingSpace ? 1 : 0),
            insert: '',
          });
        }
        continue;
      }

      const bulletMatch = BULLET_LINE_RE.exec(line.text);
      if (bulletMatch) {
        const indentLen = (bulletMatch[1] ?? '').length;
        const bullet = bulletMatch[2] ?? '-';
        const bulletAndSpaceLen = bullet.length + (bulletMatch[3] ?? '').length;
        changes.push({
          from: line.from + indentLen,
          to: line.from + indentLen + bulletAndSpaceLen,
          insert: `${bullet} [ ] `,
        });
        continue;
      }

      const wsMatch = LEADING_WS_RE.exec(line.text);
      const indentLen = wsMatch?.[1]?.length ?? 0;
      changes.push({
        from: line.from + indentLen,
        to: line.from + indentLen,
        insert: '- [ ] ',
      });
    }
  }

  if (changes.length === 0) {
    return false;
  }

  const changeSet = state.changes(changes);
  const tr = state.update({
    changes: changeSet,
    selection: state.selection.map(changeSet, 1),
    userEvent: 'input.todo',
  });
  dispatch(tr);
  return true;
};

/**
 * 待办行 Enter 续写 / 空待办清除命令
 */
export const todoEnterCommand: StateCommand = ({ state, dispatch }) => {
  if (state.selection.ranges.length !== 1 || !state.selection.main.empty) {
    return false;
  }

  const pos = state.selection.main.head;
  const line = state.doc.lineAt(pos);
  if (isLineInFencedCodeBlock(state.doc, line.number)) {
    return false;
  }

  const match = TODO_LINE_RE.exec(line.text);
  if (!match) {
    return false;
  }

  const indent = match[1] ?? '';
  const bullet = match[2] ?? '-';
  const prefixLen = indent.length + bullet.length + (match[3] ?? '').length + 3;
  const afterBracket = line.text.slice(prefixLen);

  if (afterBracket.trim().length === 0) {
    dispatch(
      state.update({
        changes: { from: line.from, to: line.to, insert: '' },
        selection: EditorSelection.cursor(line.from),
        userEvent: 'delete.todo',
      }),
    );
    return true;
  }

  if (pos < line.from + prefixLen) {
    return false;
  }

  const nextLinePrefix = `\n${indent}${bullet} [ ] `;
  dispatch(
    state.update({
      changes: { from: pos, to: pos, insert: nextLinePrefix },
      selection: EditorSelection.cursor(pos + nextLinePrefix.length),
      userEvent: 'input.todo',
    }),
  );
  return true;
};

/**
 * 空待办行首前缀 Backspace 一键清除命令
 */
export const todoBackspaceCommand: StateCommand = ({ state, dispatch }) => {
  if (state.selection.ranges.length !== 1 || !state.selection.main.empty) {
    return false;
  }

  const pos = state.selection.main.head;
  const line = state.doc.lineAt(pos);
  if (isLineInFencedCodeBlock(state.doc, line.number)) {
    return false;
  }

  const match = TODO_LINE_RE.exec(line.text);
  if (!match) {
    return false;
  }

  const basePrefixLen =
    (match[1] ?? '').length + (match[2] ?? '').length + (match[3] ?? '').length + 3;
  const hasTrailingSpace = line.text[basePrefixLen] === ' ';
  const prefixEndOffset = basePrefixLen + (hasTrailingSpace ? 1 : 0);

  if (pos !== line.from + prefixEndOffset || line.text.slice(prefixEndOffset).length > 0) {
    return false;
  }

  dispatch(
    state.update({
      changes: { from: line.from, to: line.to, insert: '' },
      selection: EditorSelection.cursor(line.from),
      userEvent: 'delete.todo',
    }),
  );
  return true;
};
