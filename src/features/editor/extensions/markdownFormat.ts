import { EditorSelection, type StateCommand, type Text } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import {
  createMarkdownIndentCommand,
  createMarkdownOutdentCommand,
  type IndentConfig,
} from './markdownIndent';
import { toggleTodoCommand } from './markdownTodo';

export type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;

const URL_RE = /^(?:https?:\/\/|mailto:)/i;
const BULLET_PREFIX_RE = /^(\s*)(- )/;
const BLOCKQUOTE_PREFIX_RE = /^(\s*)(> ?)/;
const HEADING_PREFIX_RE = /^(\s*)(#{1,6})(\s+)(.*)$/;
const LEADING_WS_RE = /^(\s*)/;

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

function toggleInlineMarker(marker: string): StateCommand {
  const markerLen = marker.length;

  return ({ state, dispatch }) => {
    const tr = state.changeByRange((range) => {
      if (range.empty) {
        return {
          changes: { from: range.from, to: range.from, insert: `${marker}${marker}` },
          range: EditorSelection.cursor(range.from + markerLen),
        };
      }

      const sel = state.sliceDoc(range.from, range.to);
      const isSingleStar = marker === '*';
      const innerWrapped =
        sel.length >= markerLen * 2 &&
        sel.startsWith(marker) &&
        sel.endsWith(marker) &&
        (!isSingleStar || !sel.startsWith('**') || sel.startsWith('***'));

      if (innerWrapped) {
        const inner = sel.slice(markerLen, sel.length - markerLen);
        return {
          changes: { from: range.from, to: range.to, insert: inner },
          range: EditorSelection.range(range.from, range.from + inner.length),
        };
      }

      const before =
        range.from >= markerLen ? state.sliceDoc(range.from - markerLen, range.from) : '';
      const after =
        range.to + markerLen <= state.doc.length
          ? state.sliceDoc(range.to, range.to + markerLen)
          : '';
      const outerWrapped =
        before === marker &&
        after === marker &&
        (!isSingleStar ||
          ((range.from < 2 || state.sliceDoc(range.from - 2, range.from) !== '**') &&
            (range.to + 2 > state.doc.length || state.sliceDoc(range.to, range.to + 2) !== '**')));

      if (outerWrapped) {
        return {
          changes: [
            { from: range.from - markerLen, to: range.from, insert: '' },
            { from: range.to, to: range.to + markerLen, insert: '' },
          ],
          range: EditorSelection.range(range.from - markerLen, range.to - markerLen),
        };
      }

      return {
        changes: { from: range.from, to: range.to, insert: `${marker}${sel}${marker}` },
        range: EditorSelection.range(range.from + markerLen, range.to + markerLen),
      };
    });

    dispatch(state.update(tr, { userEvent: 'input.format' }));
    return true;
  };
}

export const formatBold: StateCommand = toggleInlineMarker('**');
export const formatItalic: StateCommand = toggleInlineMarker('*');

export const formatCodeBlock: StateCommand = ({ state, dispatch }) => {
  const tr = state.changeByRange((range) => {
    if (range.empty) {
      const line = state.doc.lineAt(range.from);
      if (line.text.trim().length === 0) {
        const block = '```\n\n```';
        return {
          changes: { from: line.from, to: line.to, insert: block },
          range: EditorSelection.cursor(line.from + 4),
        };
      }
      const block = '\n```\n\n```';
      return {
        changes: { from: line.to, to: line.to, insert: block },
        range: EditorSelection.cursor(line.to + 5),
      };
    }

    const sel = state.sliceDoc(range.from, range.to);
    const wrapped = `\`\`\`\n${sel}\n\`\`\``;
    return {
      changes: { from: range.from, to: range.to, insert: wrapped },
      range: EditorSelection.range(range.from + 4, range.from + 4 + sel.length),
    };
  });

  dispatch(state.update(tr, { userEvent: 'input.format' }));
  return true;
};

export const formatTable: StateCommand = ({ state, dispatch }) => {
  const tableText = '| 列1 | 列2 |\n| --- | --- |\n|  |  |';
  const tr = state.changeByRange((range) => ({
    changes: { from: range.from, to: range.to, insert: tableText },
    range: EditorSelection.range(range.from + 2, range.from + 4),
  }));

  dispatch(state.update(tr, { userEvent: 'input.format' }));
  return true;
};

export const formatLink: StateCommand = ({ state, dispatch }) => {
  const tr = state.changeByRange((range) => {
    if (range.empty) {
      return {
        changes: { from: range.from, to: range.to, insert: '[](url)' },
        range: EditorSelection.cursor(range.from + 1),
      };
    }

    const sel = state.sliceDoc(range.from, range.to);
    if (URL_RE.test(sel.trim())) {
      const url = sel.trim();
      return {
        changes: { from: range.from, to: range.to, insert: `[](${url})` },
        range: EditorSelection.cursor(range.from + 1),
      };
    }

    return {
      changes: { from: range.from, to: range.to, insert: `[${sel}]()` },
      range: EditorSelection.cursor(range.from + sel.length + 3),
    };
  });

  dispatch(state.update(tr, { userEvent: 'input.format' }));
  return true;
};

export const formatImage: StateCommand = ({ state, dispatch }) => {
  const tr = state.changeByRange((range) => {
    if (range.empty) {
      return {
        changes: { from: range.from, to: range.to, insert: '![]()' },
        range: EditorSelection.cursor(range.from + 2),
      };
    }

    const sel = state.sliceDoc(range.from, range.to);
    return {
      changes: { from: range.from, to: range.to, insert: `![${sel}]()` },
      range: EditorSelection.cursor(range.from + sel.length + 4),
    };
  });

  dispatch(state.update(tr, { userEvent: 'input.format' }));
  return true;
};

function toggleLinePrefix(prefix: string, matchRe: RegExp): StateCommand {
  return ({ state, dispatch }) => {
    const doc = state.doc;
    const targetLines: number[] = [];
    const seen = new Set<number>();

    for (const range of state.selection.ranges) {
      const { fromLine, toLine } = getCoveredLineNumbers(doc, range.from, range.to, range.empty);
      for (let ln = fromLine; ln <= toLine; ln += 1) {
        if (!seen.has(ln)) {
          seen.add(ln);
          targetLines.push(ln);
        }
      }
    }

    const nonEmptyLines = targetLines.filter((ln) => doc.line(ln).text.trim().length > 0);
    const linesToInspect = nonEmptyLines.length > 0 ? nonEmptyLines : targetLines;
    const allPrefixed =
      linesToInspect.length > 0 && linesToInspect.every((ln) => matchRe.test(doc.line(ln).text));

    const changes: Array<{ from: number; to: number; insert: string }> = [];
    for (const ln of linesToInspect) {
      const line = doc.line(ln);
      if (allPrefixed) {
        const match = matchRe.exec(line.text);
        if (match) {
          const start = line.from + (match[1] ?? '').length;
          changes.push({ from: start, to: start + (match[2] ?? '').length, insert: '' });
        }
      } else {
        const wsMatch = LEADING_WS_RE.exec(line.text);
        const indentLen = wsMatch?.[1]?.length ?? 0;
        changes.push({ from: line.from + indentLen, to: line.from + indentLen, insert: prefix });
      }
    }

    if (changes.length === 0) {
      return false;
    }

    const changeSet = state.changes(changes);
    dispatch(
      state.update({
        changes: changeSet,
        selection: state.selection.map(changeSet, 1),
        userEvent: 'input.format',
      }),
    );
    return true;
  };
}

export const formatBulletList: StateCommand = toggleLinePrefix('- ', BULLET_PREFIX_RE);
export const formatBlockquote: StateCommand = toggleLinePrefix('> ', BLOCKQUOTE_PREFIX_RE);

export function formatHeading(level: HeadingLevel): StateCommand {
  const hashes = '#'.repeat(level);
  return ({ state, dispatch }) => {
    const doc = state.doc;
    const seen = new Set<number>();
    const changes: Array<{ from: number; to: number; insert: string }> = [];

    for (const range of state.selection.ranges) {
      const { fromLine, toLine } = getCoveredLineNumbers(doc, range.from, range.to, range.empty);
      for (let ln = fromLine; ln <= toLine; ln += 1) {
        if (seen.has(ln)) {
          continue;
        }
        seen.add(ln);

        const line = doc.line(ln);
        const match = HEADING_PREFIX_RE.exec(line.text);
        if (match) {
          const indentLen = (match[1] ?? '').length;
          const marker = match[2] ?? '';
          const space = match[3] ?? ' ';
          const currentLevel = marker.length;
          if (currentLevel === level) {
            changes.push({
              from: line.from + indentLen,
              to: line.from + indentLen + marker.length + space.length,
              insert: '',
            });
          } else {
            changes.push({
              from: line.from + indentLen,
              to: line.from + indentLen + marker.length,
              insert: hashes,
            });
          }
        } else {
          const wsMatch = LEADING_WS_RE.exec(line.text);
          const indentLen = wsMatch?.[1]?.length ?? 0;
          changes.push({
            from: line.from + indentLen,
            to: line.from + indentLen,
            insert: `${hashes} `,
          });
        }
      }
    }

    if (changes.length === 0) {
      return false;
    }

    const changeSet = state.changes(changes);
    dispatch(
      state.update({
        changes: changeSet,
        selection: state.selection.map(changeSet, 1),
        userEvent: 'input.format',
      }),
    );
    return true;
  };
}

export const EDITOR_FORMAT_ACTION_IDS = [
  'editor.bold',
  'editor.italic',
  'editor.codeBlock',
  'editor.table',
  'editor.toggleTodo',
  'editor.link',
  'editor.image',
  'editor.bulletList',
  'editor.blockquote',
  'editor.heading1',
  'editor.heading2',
  'editor.heading3',
  'editor.heading4',
  'editor.heading5',
  'editor.heading6',
  'editor.indent',
  'editor.outdent',
] as const;

export function executeEditorFormatAction(
  view: EditorView,
  actionId: string,
  indentConfig: IndentConfig,
): boolean {
  const commandMap: Record<string, StateCommand> = {
    'editor.bold': formatBold,
    'editor.italic': formatItalic,
    'editor.codeBlock': formatCodeBlock,
    'editor.table': formatTable,
    'editor.toggleTodo': toggleTodoCommand,
    'editor.link': formatLink,
    'editor.image': formatImage,
    'editor.bulletList': formatBulletList,
    'editor.blockquote': formatBlockquote,
    'editor.heading1': formatHeading(1),
    'editor.heading2': formatHeading(2),
    'editor.heading3': formatHeading(3),
    'editor.heading4': formatHeading(4),
    'editor.heading5': formatHeading(5),
    'editor.heading6': formatHeading(6),
    'editor.indent': createMarkdownIndentCommand(() => indentConfig),
    'editor.outdent': createMarkdownOutdentCommand(() => indentConfig),
  };

  const cmd = commandMap[actionId];
  if (!cmd) {
    return false;
  }

  return cmd(view);
}
