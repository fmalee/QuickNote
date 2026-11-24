import { StreamLanguage, type StringStream } from '@codemirror/language';
import { EditorSelection, type StateCommand } from '@codemirror/state';

export type DetectedLanguageId =
  | 'markdown'
  | 'yaml'
  | 'json'
  | 'jsonc'
  | 'toml'
  | 'shell'
  | 'dockerfile'
  | 'javascript'
  | 'typescript'
  | 'html'
  | 'css'
  | 'xml'
  | 'python'
  | 'rust'
  | 'go'
  | 'sql'
  | 'ini'
  | 'nginx'
  | 'plaintext';

export interface DetectedLanguage {
  id: DetectedLanguageId;
  label: string;
  isMarkdown: boolean;
}

export interface FileIndentation {
  useTabs: boolean;
  unit: string;
  tabSize: number;
}

const LANGUAGE_LABELS: Record<DetectedLanguageId, string> = {
  markdown: 'Markdown',
  yaml: 'YAML',
  json: 'JSON',
  jsonc: 'JSONC',
  toml: 'TOML',
  shell: 'Shell',
  dockerfile: 'Dockerfile',
  javascript: 'JavaScript',
  typescript: 'TypeScript',
  html: 'HTML',
  css: 'CSS',
  xml: 'XML',
  python: 'Python',
  rust: 'Rust',
  go: 'Go',
  sql: 'SQL',
  ini: 'INI',
  nginx: 'Nginx / Conf',
  plaintext: 'Plain Text',
};

function makeLang(id: DetectedLanguageId): DetectedLanguage {
  return {
    id,
    label: LANGUAGE_LABELS[id],
    isMarkdown: id === 'markdown',
  };
}

function getBasename(absPath: string): string {
  const normalized = absPath.replace(/\\/g, '/');
  const idx = normalized.lastIndexOf('/');
  return (idx >= 0 ? normalized.slice(idx + 1) : normalized).toLowerCase();
}

function getExtension(basename: string): string {
  const dot = basename.lastIndexOf('.');
  return dot > 0 ? basename.slice(dot) : '';
}

function detectByShebang(content: string): DetectedLanguageId | null {
  const firstNewline = content.indexOf('\n');
  const firstLine = (firstNewline >= 0 ? content.slice(0, firstNewline) : content).trim();
  if (!firstLine.startsWith('#!')) {
    return null;
  }
  const lower = firstLine.toLowerCase();
  if (/\b(bash|zsh|sh|ash|dash|ksh)\b/.test(lower)) {
    return 'shell';
  }
  if (/\bpython\d*(\.\d+)?\b/.test(lower)) {
    return 'python';
  }
  if (/\b(ts-node|tsx|deno)\b/.test(lower)) {
    return 'typescript';
  }
  if (/\b(node|nodejs|bun)\b/.test(lower)) {
    return 'javascript';
  }
  return null;
}

/**
 * 按文件名、扩展名及首行 shebang 识别文档语言（覆盖 Markdown + 15 类非 Markdown）。
 */
export function detectDocumentLanguage(absPath: string, content: string): DetectedLanguage {
  const basename = getBasename(absPath);

  if (
    basename === 'dockerfile' ||
    basename.startsWith('dockerfile.') ||
    basename.endsWith('.dockerfile')
  ) {
    return makeLang('dockerfile');
  }
  if (basename === 'cargo.lock') {
    return makeLang('toml');
  }
  if (basename === 'nginx.conf' || basename.endsWith('.conf')) {
    return makeLang('nginx');
  }
  if (/^tsconfig(\..+)?\.json$/.test(basename)) {
    return makeLang('jsonc');
  }
  if (
    basename === '.bashrc' ||
    basename === '.zshrc' ||
    basename === '.profile' ||
    basename === '.zprofile' ||
    basename === '.env' ||
    basename.startsWith('.env.')
  ) {
    return makeLang('shell');
  }
  if (basename === '.gitconfig') {
    return makeLang('ini');
  }

  const ext = getExtension(basename);
  switch (ext) {
    case '.md':
    case '.markdown':
      return makeLang('markdown');
    case '.yml':
    case '.yaml':
      return makeLang('yaml');
    case '.json':
      return makeLang('json');
    case '.jsonc':
    case '.code-workspace':
      return makeLang('jsonc');
    case '.toml':
      return makeLang('toml');
    case '.sh':
    case '.bash':
    case '.zsh':
      return makeLang('shell');
    case '.js':
    case '.mjs':
    case '.cjs':
    case '.jsx':
      return makeLang('javascript');
    case '.ts':
    case '.mts':
    case '.cts':
    case '.tsx':
      return makeLang('typescript');
    case '.html':
    case '.htm':
    case '.vue':
      return makeLang('html');
    case '.css':
    case '.scss':
    case '.less':
      return makeLang('css');
    case '.xml':
    case '.svg':
    case '.plist':
      return makeLang('xml');
    case '.py':
    case '.pyi':
    case '.pyw':
      return makeLang('python');
    case '.rs':
      return makeLang('rust');
    case '.go':
      return makeLang('go');
    case '.sql':
      return makeLang('sql');
    case '.ini':
    case '.cfg':
    case '.properties':
      return makeLang('ini');
    default:
      break;
  }

  const shebangLang = detectByShebang(content);
  if (shebangLang) {
    return makeLang(shebangLang);
  }

  return makeLang('plaintext');
}

/**
 * 扫描前 200 行缩进频率，自动识别非 Markdown 文件的 tabs / 2 空格 / 4 空格缩进（默认 2 空格）。
 */
export function detectFileIndentation(content: string): FileIndentation {
  const lines = content.split(/\r?\n/, 200);
  let tabLines = 0;
  let spaceLines = 0;
  let twoSpaceHits = 0;
  let fourSpaceHits = 0;

  for (const line of lines) {
    if (!line || line.trim().length === 0) {
      continue;
    }
    if (line[0] === '\t') {
      tabLines += 1;
      continue;
    }
    if (line[0] === ' ') {
      const match = /^( +)/.exec(line);
      const spaces = match?.[1]?.length ?? 0;
      if (spaces > 0) {
        spaceLines += 1;
        if (spaces % 4 === 0) {
          fourSpaceHits += 1;
        } else if (spaces % 2 === 0) {
          twoSpaceHits += 1;
        }
      }
    }
  }

  if (tabLines > spaceLines) {
    return { useTabs: true, unit: '\t', tabSize: 4 };
  }
  if (spaceLines === 0) {
    return { useTabs: false, unit: '  ', tabSize: 2 };
  }
  if (twoSpaceHits > 0 && twoSpaceHits >= fourSpaceHits / 2) {
    return { useTabs: false, unit: '  ', tabSize: 2 };
  }
  if (fourSpaceHits > 0) {
    return { useTabs: false, unit: '    ', tabSize: 4 };
  }
  return { useTabs: false, unit: '  ', tabSize: 2 };
}

/**
 * 非 Markdown 文件普通缩进命令（按侦测到的 unit 插入缩进）。
 */
export function createCodeIndentCommand(unit: string): StateCommand {
  return ({ state, dispatch }) => {
    const doc = state.doc;
    const changes: Array<{ from: number; to: number; insert: string }> = [];
    const nextRanges = state.selection.ranges.map((range) => {
      if (range.empty) {
        changes.push({ from: range.from, to: range.to, insert: unit });
        return EditorSelection.cursor(range.from + unit.length);
      }
      const fromLine = doc.lineAt(range.from).number;
      const effectiveTo =
        range.to > range.from && doc.lineAt(range.to).from === range.to ? range.to - 1 : range.to;
      const toLine = doc.lineAt(effectiveTo).number;
      let addedBeforeAnchor = 0;
      let addedTotal = 0;
      for (let ln = fromLine; ln <= toLine; ln += 1) {
        const line = doc.line(ln);
        changes.push({ from: line.from, to: line.from, insert: unit });
        if (ln === fromLine && range.from > line.from) {
          addedBeforeAnchor = unit.length;
        }
        addedTotal += unit.length;
      }
      return EditorSelection.range(range.from + addedBeforeAnchor, range.to + addedTotal);
    });
    dispatch(
      state.update({
        changes,
        selection: EditorSelection.create(nextRanges, state.selection.mainIndex),
        userEvent: 'input',
      }),
    );
    return true;
  };
}

/**
 * 非 Markdown 文件普通反缩进命令（按 unit 移除行首缩进）。
 */
export function createCodeOutdentCommand(unit: string): StateCommand {
  return ({ state, dispatch }) => {
    const doc = state.doc;
    const changes: Array<{ from: number; to: number; insert: string }> = [];
    const maxSpaces = unit === '\t' ? 4 : unit.length;
    let changed = false;

    for (const range of state.selection.ranges) {
      const fromLine = doc.lineAt(range.from).number;
      const effectiveTo =
        !range.empty && doc.lineAt(range.to).from === range.to ? range.to - 1 : range.to;
      const toLine = doc.lineAt(effectiveTo).number;
      for (let ln = fromLine; ln <= toLine; ln += 1) {
        const line = doc.line(ln);
        if (line.text.startsWith('\t')) {
          changes.push({ from: line.from, to: line.from + 1, insert: '' });
          changed = true;
        } else {
          let spaces = 0;
          while (spaces < maxSpaces && line.text[spaces] === ' ') {
            spaces += 1;
          }
          if (spaces > 0) {
            changes.push({ from: line.from, to: line.from + spaces, insert: '' });
            changed = true;
          }
        }
      }
    }

    if (!changed) {
      return false;
    }
    dispatch(state.update({ changes, userEvent: 'delete' }));
    return true;
  };
}

interface SimpleLangState {
  inBlockComment: boolean;
  inString: string | null;
}

function readQuoted(stream: StringStream, quote: string): boolean {
  let escaped = false;
  while (!stream.eol()) {
    const ch = stream.next();
    if (escaped) {
      escaped = false;
    } else if (ch === '\\') {
      escaped = true;
    } else if (ch === quote) {
      return true;
    }
  }
  return false;
}

interface StreamLangConfig {
  lineComment?: string;
  hashComment?: boolean;
  blockComment?: boolean;
  keywords?: Set<string>;
  atoms?: Set<string>;
  types?: Set<string>;
  keyColon?: boolean;
  keyEquals?: boolean;
  variables?: boolean;
  tags?: boolean;
  sectionBrackets?: boolean;
}

function buildStreamLanguage(cfg: StreamLangConfig): StreamLanguage<SimpleLangState> {
  return StreamLanguage.define<SimpleLangState>({
    startState: () => ({ inBlockComment: false, inString: null }),
    token(stream, state) {
      if (state.inBlockComment) {
        while (!stream.eol()) {
          if (stream.match('*/')) {
            state.inBlockComment = false;
            break;
          }
          stream.next();
        }
        return 'comment';
      }

      if (state.inString) {
        const closed = readQuoted(stream, state.inString);
        if (closed) {
          state.inString = null;
        }
        return 'string';
      }

      if (stream.eatSpace()) {
        return null;
      }

      if (cfg.lineComment && stream.match(cfg.lineComment)) {
        stream.skipToEnd();
        return 'comment';
      }
      if (cfg.hashComment && stream.peek() === '#') {
        stream.skipToEnd();
        return 'comment';
      }
      if (cfg.blockComment && stream.match('/*')) {
        state.inBlockComment = true;
        while (!stream.eol()) {
          if (stream.match('*/')) {
            state.inBlockComment = false;
            break;
          }
          stream.next();
        }
        return 'comment';
      }

      if (cfg.tags && stream.match(/^<\/?[A-Za-z_][\w:-]*/)) {
        return 'tagName';
      }
      if (cfg.tags && stream.match(/^\s*\/?>/)) {
        return 'tagName';
      }

      if (cfg.sectionBrackets && stream.sol() && stream.match(/^\[[^\]]+\]/)) {
        return 'meta';
      }

      const quote = stream.peek();
      if (quote === '"' || quote === "'" || quote === '`') {
        stream.next();
        const closed = readQuoted(stream, quote);
        if (!closed) {
          state.inString = quote;
        }
        if (cfg.keyColon && stream.match(/^\s*(?=:)/, false)) {
          return 'propertyName';
        }
        return 'string';
      }

      if (cfg.variables && stream.match(/^\$(\{[A-Za-z_]\w*\}|[A-Za-z_]\w*|\d+|\?|@|\*)/)) {
        return 'variableName';
      }

      if (stream.match(/^-?(0x[\da-fA-F_]+|0b[01_]+|\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d+)?)/)) {
        return 'number';
      }

      if (stream.match(/^[A-Za-z_][\w.-]*/)) {
        const word = stream.current();
        if (cfg.keyColon && stream.match(/^\s*(?=:)/, false)) {
          return 'propertyName';
        }
        if (cfg.keyEquals && stream.match(/^\s*(?==)/, false)) {
          return 'propertyName';
        }
        if (cfg.atoms?.has(word)) {
          return 'atom';
        }
        if (cfg.keywords?.has(word) || cfg.keywords?.has(word.toUpperCase())) {
          return 'keyword';
        }
        if (cfg.types?.has(word)) {
          return 'typeName';
        }
        return 'variableName';
      }

      if (stream.match(/^[=<>!+\-*/%&|^~?:;,.]+/)) {
        return 'operator';
      }

      stream.next();
      return null;
    },
  });
}

const STREAM_LANGUAGES: Partial<Record<DetectedLanguageId, StreamLanguage<SimpleLangState>>> = {
  yaml: buildStreamLanguage({
    hashComment: true,
    keyColon: true,
    atoms: new Set(['true', 'false', 'null', 'yes', 'no', 'on', 'off', '~']),
  }),
  json: buildStreamLanguage({
    keyColon: true,
    atoms: new Set(['true', 'false', 'null']),
  }),
  jsonc: buildStreamLanguage({
    lineComment: '//',
    blockComment: true,
    keyColon: true,
    atoms: new Set(['true', 'false', 'null']),
  }),
  toml: buildStreamLanguage({
    hashComment: true,
    keyEquals: true,
    sectionBrackets: true,
    atoms: new Set(['true', 'false']),
  }),
  shell: buildStreamLanguage({
    hashComment: true,
    keyEquals: true,
    variables: true,
    keywords: new Set([
      'if',
      'then',
      'else',
      'elif',
      'fi',
      'for',
      'while',
      'until',
      'do',
      'done',
      'case',
      'esac',
      'in',
      'function',
      'return',
      'exit',
      'export',
      'local',
      'readonly',
      'source',
      'echo',
      'set',
      'unset',
      'shift',
      'break',
      'continue',
    ]),
    atoms: new Set(['true', 'false']),
  }),
  dockerfile: buildStreamLanguage({
    hashComment: true,
    variables: true,
    keywords: new Set([
      'FROM',
      'AS',
      'RUN',
      'CMD',
      'LABEL',
      'MAINTAINER',
      'EXPOSE',
      'ENV',
      'ADD',
      'COPY',
      'ENTRYPOINT',
      'VOLUME',
      'USER',
      'WORKDIR',
      'ARG',
      'ONBUILD',
      'STOPSIGNAL',
      'HEALTHCHECK',
      'SHELL',
    ]),
  }),
  javascript: buildStreamLanguage({
    lineComment: '//',
    blockComment: true,
    keywords: new Set([
      'async',
      'await',
      'break',
      'case',
      'catch',
      'class',
      'const',
      'continue',
      'debugger',
      'default',
      'delete',
      'do',
      'else',
      'export',
      'extends',
      'finally',
      'for',
      'from',
      'function',
      'if',
      'import',
      'in',
      'instanceof',
      'let',
      'new',
      'of',
      'return',
      'static',
      'super',
      'switch',
      'this',
      'throw',
      'try',
      'typeof',
      'var',
      'void',
      'while',
      'with',
      'yield',
    ]),
    atoms: new Set(['true', 'false', 'null', 'undefined', 'NaN', 'Infinity']),
  }),
  typescript: buildStreamLanguage({
    lineComment: '//',
    blockComment: true,
    keywords: new Set([
      'abstract',
      'as',
      'async',
      'await',
      'break',
      'case',
      'catch',
      'class',
      'const',
      'continue',
      'debugger',
      'declare',
      'default',
      'delete',
      'do',
      'else',
      'enum',
      'export',
      'extends',
      'finally',
      'for',
      'from',
      'function',
      'if',
      'implements',
      'import',
      'in',
      'instanceof',
      'interface',
      'is',
      'keyof',
      'let',
      'namespace',
      'new',
      'of',
      'private',
      'protected',
      'public',
      'readonly',
      'return',
      'satisfies',
      'static',
      'super',
      'switch',
      'this',
      'throw',
      'try',
      'type',
      'typeof',
      'var',
      'void',
      'while',
      'with',
      'yield',
    ]),
    types: new Set(['string', 'number', 'boolean', 'unknown', 'never', 'any', 'object', 'void']),
    atoms: new Set(['true', 'false', 'null', 'undefined', 'NaN', 'Infinity']),
  }),
  html: buildStreamLanguage({
    tags: true,
    blockComment: true,
    keyEquals: true,
    keywords: new Set(['DOCTYPE', 'html', 'head', 'body', 'script', 'style', 'template']),
  }),
  css: buildStreamLanguage({
    lineComment: '//',
    blockComment: true,
    keyColon: true,
    keywords: new Set(['important', 'from', 'to', 'and', 'or', 'not', 'only', 'screen']),
    atoms: new Set([
      'auto',
      'none',
      'inherit',
      'initial',
      'unset',
      'flex',
      'grid',
      'block',
      'inline',
    ]),
  }),
  xml: buildStreamLanguage({
    tags: true,
    keyEquals: true,
  }),
  python: buildStreamLanguage({
    hashComment: true,
    keywords: new Set([
      'and',
      'as',
      'assert',
      'async',
      'await',
      'break',
      'class',
      'continue',
      'def',
      'del',
      'elif',
      'else',
      'except',
      'finally',
      'for',
      'from',
      'global',
      'if',
      'import',
      'in',
      'is',
      'lambda',
      'nonlocal',
      'not',
      'or',
      'pass',
      'raise',
      'return',
      'try',
      'while',
      'with',
      'yield',
      'match',
      'case',
    ]),
    atoms: new Set(['True', 'False', 'None']),
    types: new Set(['int', 'str', 'float', 'bool', 'list', 'dict', 'set', 'tuple']),
  }),
  rust: buildStreamLanguage({
    lineComment: '//',
    blockComment: true,
    keywords: new Set([
      'as',
      'async',
      'await',
      'break',
      'const',
      'continue',
      'crate',
      'dyn',
      'else',
      'enum',
      'extern',
      'fn',
      'for',
      'if',
      'impl',
      'in',
      'let',
      'loop',
      'match',
      'mod',
      'move',
      'mut',
      'pub',
      'ref',
      'return',
      'self',
      'Self',
      'static',
      'struct',
      'super',
      'trait',
      'type',
      'unsafe',
      'use',
      'where',
      'while',
    ]),
    types: new Set([
      'bool',
      'char',
      'str',
      'String',
      'Vec',
      'Option',
      'Result',
      'usize',
      'isize',
      'u8',
      'u16',
      'u32',
      'u64',
      'i8',
      'i16',
      'i32',
      'i64',
      'f32',
      'f64',
    ]),
    atoms: new Set(['true', 'false', 'Some', 'None', 'Ok', 'Err']),
  }),
  go: buildStreamLanguage({
    lineComment: '//',
    blockComment: true,
    keywords: new Set([
      'break',
      'case',
      'chan',
      'const',
      'continue',
      'default',
      'defer',
      'else',
      'fallthrough',
      'for',
      'func',
      'go',
      'goto',
      'if',
      'import',
      'interface',
      'map',
      'package',
      'range',
      'return',
      'select',
      'struct',
      'switch',
      'type',
      'var',
    ]),
    types: new Set([
      'bool',
      'byte',
      'complex64',
      'complex128',
      'error',
      'float32',
      'float64',
      'int',
      'int8',
      'int16',
      'int32',
      'int64',
      'rune',
      'string',
      'uint',
      'uint8',
      'uint16',
      'uint32',
      'uint64',
      'uintptr',
    ]),
    atoms: new Set(['true', 'false', 'nil', 'iota']),
  }),
  sql: buildStreamLanguage({
    lineComment: '--',
    blockComment: true,
    keywords: new Set([
      'SELECT',
      'FROM',
      'WHERE',
      'INSERT',
      'INTO',
      'VALUES',
      'UPDATE',
      'SET',
      'DELETE',
      'CREATE',
      'TABLE',
      'INDEX',
      'ALTER',
      'DROP',
      'JOIN',
      'LEFT',
      'RIGHT',
      'INNER',
      'OUTER',
      'ON',
      'AS',
      'AND',
      'OR',
      'NOT',
      'IN',
      'IS',
      'NULL',
      'ORDER',
      'BY',
      'GROUP',
      'HAVING',
      'LIMIT',
      'OFFSET',
      'DISTINCT',
      'UNION',
      'ALL',
      'CASE',
      'WHEN',
      'THEN',
      'ELSE',
      'END',
      'PRIMARY',
      'KEY',
    ]),
    atoms: new Set(['TRUE', 'FALSE', 'NULL']),
  }),
  ini: buildStreamLanguage({
    hashComment: true,
    lineComment: ';',
    keyEquals: true,
    sectionBrackets: true,
    atoms: new Set(['true', 'false', 'yes', 'no', 'on', 'off']),
  }),
  nginx: buildStreamLanguage({
    hashComment: true,
    variables: true,
    keywords: new Set([
      'http',
      'server',
      'location',
      'events',
      'upstream',
      'listen',
      'server_name',
      'root',
      'index',
      'proxy_pass',
      'proxy_set_header',
      'try_files',
      'return',
      'rewrite',
      'include',
      'ssl_certificate',
      'ssl_certificate_key',
      'error_page',
      'access_log',
      'error_log',
      'worker_processes',
      'worker_connections',
      'gzip',
    ]),
    atoms: new Set(['on', 'off']),
  }),
};

/**
 * 获取对应非 Markdown 语言的 CM6 StreamLanguage 扩展；Markdown 或 plaintext 返回 null。
 */
export function getStreamLanguageExtension(
  id: DetectedLanguageId,
): StreamLanguage<SimpleLangState> | null {
  return STREAM_LANGUAGES[id] ?? null;
}
