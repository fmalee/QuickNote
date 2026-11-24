import { StateEffect, StateField } from '@codemirror/state';
import { Decoration, type DecorationSet, EditorView } from '@codemirror/view';
import type { DetectedLanguageId } from './codeLanguages';

export const DIAGNOSTICS_MAX_BYTES = 2 * 1024 * 1024;
export const PLAIN_TEXT_FALLBACK_BYTES = 5 * 1024 * 1024;

export interface EditorDiagnostic {
  line: number; // 1-based
  column: number; // 1-based
  from: number;
  to: number;
  severity: 'error' | 'warning';
  message: string;
}

export interface DiagnosticsResult {
  diagnostics: EditorDiagnostic[];
  disabledReason: string | null;
}

function buildLineStarts(content: string): number[] {
  const starts = [0];
  for (let i = 0; i < content.length; i += 1) {
    if (content[i] === '\n') {
      starts.push(i + 1);
    }
  }
  return starts;
}

function offsetToLineCol(
  lineStarts: number[],
  offset: number,
  maxLen: number,
): { line: number; column: number; from: number; to: number } {
  const clamped = Math.max(0, Math.min(offset, maxLen));
  let low = 0;
  let high = lineStarts.length - 1;
  while (low <= high) {
    const mid = (low + high) >> 1;
    const start = lineStarts[mid] ?? 0;
    if (start <= clamped) {
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  const lineIndex = Math.max(0, high);
  const lineStart = lineStarts[lineIndex] ?? 0;
  const from = clamped;
  const to = Math.min(maxLen, Math.max(from + 1, from));
  return {
    line: lineIndex + 1,
    column: clamped - lineStart + 1,
    from,
    to,
  };
}

function makeLineDiagnostic(
  lineNumber: number,
  column: number,
  lineStartOffset: number,
  lineLength: number,
  severity: 'error' | 'warning',
  message: string,
): EditorDiagnostic {
  const colOffset = Math.max(0, column - 1);
  const from = lineStartOffset + Math.min(colOffset, lineLength);
  const to = Math.max(from + 1, lineStartOffset + lineLength);
  return {
    line: lineNumber,
    column,
    from,
    to,
    severity,
    message,
  };
}

function stripJsonc(input: string): string {
  let out = '';
  let i = 0;
  let inString = false;
  let escaped = false;

  while (i < input.length) {
    const ch = input[i]!;
    const next = input[i + 1];

    if (inString) {
      out += ch;
      if (escaped) {
        escaped = false;
      } else if (ch === '\\') {
        escaped = true;
      } else if (ch === '"') {
        inString = false;
      }
      i += 1;
      continue;
    }

    if (ch === '"') {
      inString = true;
      out += ch;
      i += 1;
      continue;
    }

    if (ch === '/' && next === '/') {
      out += '  ';
      i += 2;
      while (i < input.length && input[i] !== '\n') {
        out += ' ';
        i += 1;
      }
      continue;
    }

    if (ch === '/' && next === '*') {
      out += '  ';
      i += 2;
      while (i < input.length) {
        if (input[i] === '*' && input[i + 1] === '/') {
          out += '  ';
          i += 2;
          break;
        }
        out += input[i] === '\n' ? '\n' : ' ';
        i += 1;
      }
      continue;
    }

    out += ch;
    i += 1;
  }

  // Strip trailing commas before } or ] outside strings
  let cleaned = '';
  inString = false;
  escaped = false;
  for (let j = 0; j < out.length; j += 1) {
    const ch = out[j]!;
    if (inString) {
      cleaned += ch;
      if (escaped) {
        escaped = false;
      } else if (ch === '\\') {
        escaped = true;
      } else if (ch === '"') {
        inString = false;
      }
      continue;
    }
    if (ch === '"') {
      inString = true;
      cleaned += ch;
      continue;
    }
    if (ch === ',') {
      let k = j + 1;
      while (k < out.length && /\s/.test(out[k]!)) {
        k += 1;
      }
      if (out[k] === '}' || out[k] === ']') {
        cleaned += ' ';
        continue;
      }
    }
    cleaned += ch;
  }

  return cleaned;
}

function diagnoseJson(content: string, isJsonc: boolean): EditorDiagnostic[] {
  if (!content.trim()) {
    return [];
  }
  const sourceToParse = isJsonc ? stripJsonc(content) : content;
  try {
    JSON.parse(sourceToParse);
    return [];
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'JSON 语法错误';
    const lineStarts = buildLineStarts(content);

    const lineColMatch = /line\s+(\d+)\s+column\s+(\d+)/i.exec(msg);
    if (lineColMatch) {
      const line = Math.max(1, Number(lineColMatch[1]));
      const column = Math.max(1, Number(lineColMatch[2]));
      const lineStart = lineStarts[line - 1] ?? 0;
      const nextLineStart = lineStarts[line] ?? content.length;
      const lineLen = Math.max(1, nextLineStart - lineStart);
      return [makeLineDiagnostic(line, column, lineStart, lineLen, 'error', msg)];
    }

    const posMatch = /position\s+(\d+)/i.exec(msg);
    if (posMatch) {
      const pos = Number(posMatch[1]);
      const loc = offsetToLineCol(lineStarts, pos, content.length);
      return [{ ...loc, severity: 'error', message: msg }];
    }

    return [
      {
        line: 1,
        column: 1,
        from: 0,
        to: Math.min(1, content.length),
        severity: 'error',
        message: msg,
      },
    ];
  }
}

function findUnclosedQuote(line: string): string | null {
  let quote: string | null = null;
  let escaped = false;
  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i]!;
    if (quote) {
      if (quote === '"' && escaped) {
        escaped = false;
      } else if (quote === '"' && ch === '\\') {
        escaped = true;
      } else if (ch === quote) {
        quote = null;
      }
      continue;
    }
    if (ch === '#' && (i === 0 || /\s/.test(line[i - 1]!))) {
      break;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
    }
  }
  return quote;
}

function diagnoseYaml(content: string): EditorDiagnostic[] {
  const diagnostics: EditorDiagnostic[] = [];
  const lines = content.split(/\r?\n/);
  const bracketStack: Array<{ ch: string; line: number; col: number; offset: number }> = [];
  let offset = 0;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i]!;
    const lineNum = i + 1;
    const trimmed = line.trim();

    if (/^ *\t/.test(line)) {
      diagnostics.push(
        makeLineDiagnostic(
          lineNum,
          line.indexOf('\t') + 1,
          offset,
          line.length,
          'error',
          'YAML 缩进禁止使用 Tab 字符，请改用空格',
        ),
      );
    }

    if (trimmed && !trimmed.startsWith('#')) {
      const unclosed = findUnclosedQuote(line);
      if (unclosed) {
        diagnostics.push(
          makeLineDiagnostic(
            lineNum,
            1,
            offset,
            line.length,
            'error',
            `未闭合的字符串引号 (${unclosed})`,
          ),
        );
      }

      if (
        bracketStack.length === 0 &&
        line[0] !== ' ' &&
        line[0] !== '\t' &&
        !trimmed.startsWith('-') &&
        !trimmed.startsWith('---') &&
        !trimmed.startsWith('...') &&
        !trimmed.startsWith('[') &&
        !trimmed.startsWith('{') &&
        !line.includes(':')
      ) {
        diagnostics.push(
          makeLineDiagnostic(
            lineNum,
            1,
            offset,
            line.length,
            'error',
            'YAML 顶层映射缺少冒号分隔符 ":"',
          ),
        );
      }

      let q: string | null = null;
      let esc = false;
      for (let c = 0; c < line.length; c += 1) {
        const ch = line[c]!;
        if (q) {
          if (q === '"' && esc) {
            esc = false;
          } else if (q === '"' && ch === '\\') {
            esc = true;
          } else if (ch === q) {
            q = null;
          }
          continue;
        }
        if (ch === '#') {
          break;
        }
        if (ch === '"' || ch === "'") {
          q = ch;
        } else if (ch === '[' || ch === '{') {
          bracketStack.push({ ch, line: lineNum, col: c + 1, offset });
        } else if (ch === ']' || ch === '}') {
          const top = bracketStack[bracketStack.length - 1];
          const expected = ch === ']' ? '[' : '{';
          if (top && top.ch === expected) {
            bracketStack.pop();
          } else {
            diagnostics.push(
              makeLineDiagnostic(
                lineNum,
                c + 1,
                offset,
                line.length,
                'error',
                `多余的闭合括号 "${ch}"`,
              ),
            );
          }
        }
      }
    }

    offset += line.length + 1;
  }

  for (const unclosed of bracketStack) {
    diagnostics.push({
      line: unclosed.line,
      column: unclosed.col,
      from: unclosed.offset + unclosed.col - 1,
      to: unclosed.offset + unclosed.col,
      severity: 'error',
      message: `未闭合的括号 "${unclosed.ch}"`,
    });
  }

  return diagnostics;
}

function diagnoseToml(content: string): EditorDiagnostic[] {
  const diagnostics: EditorDiagnostic[] = [];
  const lines = content.split(/\r?\n/);
  let offset = 0;
  let arrayDepth = 0;
  let multilineQuote: '"""' | "'''" | null = null;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i]!;
    const lineNum = i + 1;
    const trimmed = line.trim();

    if (multilineQuote) {
      if (line.includes(multilineQuote)) {
        multilineQuote = null;
      }
      offset += line.length + 1;
      continue;
    }

    if (!trimmed || trimmed.startsWith('#')) {
      offset += line.length + 1;
      continue;
    }

    if (arrayDepth === 0 && trimmed.startsWith('[')) {
      const isDouble = trimmed.startsWith('[[');
      const closed = isDouble ? /\]\]\s*(#.*)?$/.test(trimmed) : /\]\s*(#.*)?$/.test(trimmed);
      if (!closed) {
        diagnostics.push(
          makeLineDiagnostic(
            lineNum,
            1,
            offset,
            line.length,
            'error',
            'TOML 节标题缺少闭合括号 "]"',
          ),
        );
      }
      offset += line.length + 1;
      continue;
    }

    if (line.includes('"""') && line.indexOf('"""') === line.lastIndexOf('"""')) {
      multilineQuote = '"""';
    } else if (line.includes("'''") && line.indexOf("'''") === line.lastIndexOf("'''")) {
      multilineQuote = "'''";
    } else {
      const unclosed = findUnclosedQuote(line);
      if (unclosed) {
        diagnostics.push(
          makeLineDiagnostic(
            lineNum,
            1,
            offset,
            line.length,
            'error',
            `TOML 未闭合的字符串引号 (${unclosed})`,
          ),
        );
      }
    }

    if (arrayDepth === 0 && !trimmed.includes('=')) {
      diagnostics.push(
        makeLineDiagnostic(lineNum, 1, offset, line.length, 'error', 'TOML 语句缺少 "=" 赋值'),
      );
    }

    for (const ch of trimmed) {
      if (ch === '#') {
        break;
      }
      if (ch === '[') {
        arrayDepth += 1;
      } else if (ch === ']') {
        arrayDepth = Math.max(0, arrayDepth - 1);
      }
    }

    offset += line.length + 1;
  }

  if (arrayDepth > 0) {
    diagnostics.push({
      line: lines.length,
      column: 1,
      from: Math.max(0, content.length - 1),
      to: content.length,
      severity: 'error',
      message: 'TOML 数组缺少闭合括号 "]"',
    });
  }

  return diagnostics;
}

function diagnoseShell(content: string): EditorDiagnostic[] {
  const diagnostics: EditorDiagnostic[] = [];
  const lines = content.split(/\r?\n/);
  const blockStack: Array<{
    open: string;
    expect: string;
    line: number;
    offset: number;
    len: number;
  }> = [];
  let offset = 0;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i]!;
    const lineNum = i + 1;
    const trimmed = line.trim();

    if (trimmed && !trimmed.startsWith('#')) {
      const unclosed = findUnclosedQuote(line);
      if (unclosed) {
        diagnostics.push(
          makeLineDiagnostic(
            lineNum,
            1,
            offset,
            line.length,
            'error',
            `Shell 未闭合的引号 (${unclosed})`,
          ),
        );
      }

      const stripped = trimmed.replace(/(["'])(?:\\.|(?!\1).)*\1/g, '');
      const tokens = stripped.split(/[\s;]+/).filter(Boolean);
      for (const tok of tokens) {
        if (tok === 'if') {
          blockStack.push({ open: 'if', expect: 'fi', line: lineNum, offset, len: line.length });
        } else if (tok === 'do') {
          blockStack.push({ open: 'do', expect: 'done', line: lineNum, offset, len: line.length });
        } else if (tok === 'case') {
          blockStack.push({
            open: 'case',
            expect: 'esac',
            line: lineNum,
            offset,
            len: line.length,
          });
        } else if (tok === 'fi' || tok === 'done' || tok === 'esac') {
          const top = blockStack[blockStack.length - 1];
          if (top && top.expect === tok) {
            blockStack.pop();
          } else {
            diagnostics.push(
              makeLineDiagnostic(
                lineNum,
                1,
                offset,
                line.length,
                'error',
                `意外的块结束关键字 "${tok}"`,
              ),
            );
          }
        }
      }
    }

    offset += line.length + 1;
  }

  for (const unclosed of blockStack) {
    diagnostics.push(
      makeLineDiagnostic(
        unclosed.line,
        1,
        unclosed.offset,
        unclosed.len,
        'error',
        `Shell "${unclosed.open}" 块缺少对应的 "${unclosed.expect}"`,
      ),
    );
  }

  return diagnostics;
}

function diagnoseNginx(content: string): EditorDiagnostic[] {
  const diagnostics: EditorDiagnostic[] = [];
  const lines = content.split(/\r?\n/);
  const braceStack: Array<{ line: number; col: number; offset: number }> = [];
  let offset = 0;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i]!;
    const lineNum = i + 1;
    const withoutComment = line.replace(/#.*$/, '').trim();

    if (withoutComment) {
      let inString: string | null = null;
      for (let c = 0; c < line.length; c += 1) {
        const ch = line[c]!;
        if (inString) {
          if (ch === inString && line[c - 1] !== '\\') {
            inString = null;
          }
          continue;
        }
        if (ch === '#') {
          break;
        }
        if (ch === '"' || ch === "'") {
          inString = ch;
        } else if (ch === '{') {
          braceStack.push({ line: lineNum, col: c + 1, offset });
        } else if (ch === '}') {
          if (braceStack.length > 0) {
            braceStack.pop();
          } else {
            diagnostics.push(
              makeLineDiagnostic(
                lineNum,
                c + 1,
                offset,
                line.length,
                'error',
                '多余的闭合花括号 "}"',
              ),
            );
          }
        }
      }

      const lastChar = withoutComment[withoutComment.length - 1];
      if (lastChar !== '{' && lastChar !== '}' && lastChar !== ';' && lastChar !== '\\') {
        diagnostics.push(
          makeLineDiagnostic(
            lineNum,
            line.length,
            offset,
            line.length,
            'warning',
            'Nginx 指令末尾可能缺少分号 ";"',
          ),
        );
      }
    }

    offset += line.length + 1;
  }

  for (const unclosed of braceStack) {
    diagnostics.push({
      line: unclosed.line,
      column: unclosed.col,
      from: unclosed.offset + unclosed.col - 1,
      to: unclosed.offset + unclosed.col,
      severity: 'error',
      message: '未闭合的花括号 "{"',
    });
  }

  return diagnostics;
}

/**
 * 轻量静态语法诊断，含 2MB/5MB 大文件体积分级短路保护。
 */
export function runLightweightDiagnostics(
  language: DetectedLanguageId,
  content: string,
  metaError: string | null,
  sizeBytes: number,
): DiagnosticsResult {
  if (sizeBytes > PLAIN_TEXT_FALLBACK_BYTES) {
    return {
      diagnostics: [],
      disabledReason: '文件超过 5MB，已降级为纯文本模式以保持流畅',
    };
  }
  if (sizeBytes > DIAGNOSTICS_MAX_BYTES) {
    return {
      diagnostics: [],
      disabledReason: '文件超过 2MB，已自动关闭语法诊断',
    };
  }

  if (language === 'markdown') {
    if (!metaError) {
      return { diagnostics: [], disabledReason: null };
    }
    const firstLineEnd = content.indexOf('\n');
    const lineLen = firstLineEnd >= 0 ? firstLineEnd : content.length;
    return {
      diagnostics: [
        {
          line: 1,
          column: 1,
          from: 0,
          to: Math.max(1, lineLen),
          severity: 'error',
          message: metaError,
        },
      ],
      disabledReason: null,
    };
  }

  switch (language) {
    case 'json':
      return { diagnostics: diagnoseJson(content, false), disabledReason: null };
    case 'jsonc':
      return { diagnostics: diagnoseJson(content, true), disabledReason: null };
    case 'yaml':
      return { diagnostics: diagnoseYaml(content), disabledReason: null };
    case 'toml':
      return { diagnostics: diagnoseToml(content), disabledReason: null };
    case 'shell':
      return { diagnostics: diagnoseShell(content), disabledReason: null };
    case 'nginx':
      return { diagnostics: diagnoseNginx(content), disabledReason: null };
    default:
      return { diagnostics: [], disabledReason: null };
  }
}

export const setDiagnosticsEffect = StateEffect.define<EditorDiagnostic[]>();

export const diagnosticsField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, tr) {
    for (const effect of tr.effects) {
      if (effect.is(setDiagnosticsEffect)) {
        const ranges = effect.value
          .filter((item) => item.from < item.to && item.to <= tr.state.doc.length)
          .sort((a, b) => a.from - b.from || a.to - b.to)
          .map((item) =>
            Decoration.mark({
              class:
                item.severity === 'warning' ? 'qn-cm-diagnostic-warning' : 'qn-cm-diagnostic-error',
              attributes: { title: item.message },
            }).range(item.from, item.to),
          );
        return Decoration.set(ranges, true);
      }
    }
    if (tr.docChanged) {
      return decorations.map(tr.changes);
    }
    return decorations;
  },
  provide: (field) => EditorView.decorations.from(field),
});

export function dispatchDiagnostics(view: EditorView, diagnostics: EditorDiagnostic[]): void {
  view.dispatch({
    effects: setDiagnosticsEffect.of(diagnostics),
  });
}
