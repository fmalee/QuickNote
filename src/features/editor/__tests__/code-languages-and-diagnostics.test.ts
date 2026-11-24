import { EditorState } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import {
  DIAGNOSTICS_MAX_BYTES,
  diagnosticsField,
  PLAIN_TEXT_FALLBACK_BYTES,
  runLightweightDiagnostics,
  setDiagnosticsEffect,
} from '../extensions/codeDiagnostics';
import {
  createCodeIndentCommand,
  createCodeOutdentCommand,
  detectDocumentLanguage,
  detectFileIndentation,
  type DetectedLanguageId,
  getStreamLanguageExtension,
} from '../extensions/codeLanguages';

describe('codeLanguages', () => {
  it('detects all 16 language categories by filename, extension, and shebang', () => {
    const cases: Array<{ path: string; content: string; expected: DetectedLanguageId }> = [
      { path: '/vault/note.md', content: '# Hi', expected: 'markdown' },
      { path: '/vault/config.yaml', content: 'key: val', expected: 'yaml' },
      { path: '/vault/package.json', content: '{"a":1}', expected: 'json' },
      { path: '/vault/tsconfig.json', content: '{ /* comment */ }', expected: 'jsonc' },
      { path: '/vault/Cargo.toml', content: '[package]\nname = "app"', expected: 'toml' },
      { path: '/vault/deploy.sh', content: 'echo hi', expected: 'shell' },
      { path: '/vault/Dockerfile', content: 'FROM node:20', expected: 'dockerfile' },
      { path: '/vault/index.mjs', content: 'export const x = 1;', expected: 'javascript' },
      { path: '/vault/main.tsx', content: 'const x: number = 1;', expected: 'typescript' },
      { path: '/vault/index.html', content: '<div>hi</div>', expected: 'html' },
      { path: '/vault/styles.scss', content: 'body { color: red; }', expected: 'css' },
      { path: '/vault/Info.plist', content: '<plist></plist>', expected: 'xml' },
      { path: '/vault/app.py', content: 'def main(): pass', expected: 'python' },
      { path: '/vault/lib.rs', content: 'fn main() {}', expected: 'rust' },
      { path: '/vault/main.go', content: 'package main', expected: 'go' },
      { path: '/vault/schema.sql', content: 'SELECT * FROM notes;', expected: 'sql' },
      { path: '/vault/.gitconfig', content: '[user]\nname = Test', expected: 'ini' },
      { path: '/vault/nginx.conf', content: 'server { listen 80; }', expected: 'nginx' },
      // Shebang fallbacks
      { path: '/vault/script', content: '#!/usr/bin/env python3\nprint(1)', expected: 'python' },
      { path: '/vault/run', content: '#!/bin/bash\necho ok', expected: 'shell' },
      {
        path: '/vault/cli',
        content: '#!/usr/bin/env node\nconsole.log(1)',
        expected: 'javascript',
      },
      {
        path: '/vault/tscli',
        content: '#!/usr/bin/env ts-node\nconst a: number = 1',
        expected: 'typescript',
      },
      { path: '/vault/unknown.xyz', content: 'plain text', expected: 'plaintext' },
    ];

    for (const item of cases) {
      const detected = detectDocumentLanguage(item.path, item.content);
      expect(detected.id).toBe(item.expected);
      expect(detected.isMarkdown).toBe(item.expected === 'markdown');
    }
  });

  it('creates CM6 EditorState without error for all 15 non-Markdown StreamLanguages', () => {
    const nonMdLangs: DetectedLanguageId[] = [
      'yaml',
      'json',
      'jsonc',
      'toml',
      'shell',
      'dockerfile',
      'javascript',
      'typescript',
      'html',
      'css',
      'xml',
      'python',
      'rust',
      'go',
      'sql',
      'ini',
      'nginx',
    ];

    for (const id of nonMdLangs) {
      const ext = getStreamLanguageExtension(id);
      expect(ext).not.toBeNull();
      const state = EditorState.create({
        doc: 'key = "value"\n# comment\n',
        extensions: [ext!],
      });
      expect(state.doc.lines).toBe(3);
    }
  });

  it('detectFileIndentation identifies tabs, 2 spaces, and 4 spaces', () => {
    expect(detectFileIndentation('func main() {\n\tfmt.Println()\n\treturn\n}')).toEqual({
      useTabs: true,
      unit: '\t',
      tabSize: 4,
    });
    expect(detectFileIndentation('root:\n  child:\n    leaf: 1\n')).toEqual({
      useTabs: false,
      unit: '  ',
      tabSize: 2,
    });
    expect(detectFileIndentation('def foo():\n    x = 1\n        y = 2\n')).toEqual({
      useTabs: false,
      unit: '    ',
      tabSize: 4,
    });
    expect(detectFileIndentation('plain')).toEqual({
      useTabs: false,
      unit: '  ',
      tabSize: 2,
    });
  });

  it('indents and outdents non-Markdown code using detected unit', () => {
    let state = EditorState.create({
      doc: 'line1\nline2',
      selection: { anchor: 0 },
    });
    const indent = createCodeIndentCommand('  ');
    const outdent = createCodeOutdentCommand('  ');

    indent({
      state,
      dispatch: (tr) => {
        state = tr.state;
      },
    });
    expect(state.doc.toString()).toBe('  line1\nline2');

    outdent({
      state,
      dispatch: (tr) => {
        state = tr.state;
      },
    });
    expect(state.doc.toString()).toBe('line1\nline2');
  });
});

describe('codeDiagnostics', () => {
  it('validates JSON and JSONC with comments and trailing commas', () => {
    expect(runLightweightDiagnostics('json', '{"ok": true}', null, 12).diagnostics).toHaveLength(0);

    const badJson = runLightweightDiagnostics('json', '{\n  "a": 1,\n}', null, 14);
    expect(badJson.diagnostics.length).toBeGreaterThan(0);

    const goodJsonc = runLightweightDiagnostics(
      'jsonc',
      '{\n  // line comment\n  /* block */\n  "a": 1,\n}',
      null,
      48,
    );
    expect(goodJsonc.diagnostics).toHaveLength(0);

    const badJsonc = runLightweightDiagnostics('jsonc', '{\n  // ok\n  "a": \n}', null, 20);
    expect(badJsonc.diagnostics.length).toBeGreaterThan(0);
  });

  it('catches YAML, TOML, Shell, Nginx errors and Markdown metaError', () => {
    const badYaml = runLightweightDiagnostics('yaml', 'root:\n\tchild: 1\n"unclosed', null, 30);
    expect(badYaml.diagnostics.length).toBeGreaterThanOrEqual(2);

    const badToml = runLightweightDiagnostics('toml', '[section\nkey = "unclosed', null, 30);
    expect(badToml.diagnostics.length).toBeGreaterThanOrEqual(2);

    const badShell = runLightweightDiagnostics('shell', 'if true; then\n  echo "hi\n', null, 25);
    expect(badShell.diagnostics.length).toBeGreaterThanOrEqual(2);

    const badNginx = runLightweightDiagnostics('nginx', 'server {\n  listen 80\n', null, 25);
    expect(badNginx.diagnostics.length).toBeGreaterThanOrEqual(2);

    const mdMetaErr = runLightweightDiagnostics(
      'markdown',
      '# Title\nBody',
      'YAML parse error',
      20,
    );
    expect(mdMetaErr.diagnostics).toHaveLength(1);
    expect(mdMetaErr.diagnostics[0]?.message).toBe('YAML parse error');

    const mdOk = runLightweightDiagnostics('markdown', '# Title\nBody', null, 20);
    expect(mdOk.diagnostics).toHaveLength(0);
  });

  it('enforces 2MB and 5MB size thresholds', () => {
    const at2Mb = runLightweightDiagnostics('json', '{bad', null, DIAGNOSTICS_MAX_BYTES);
    expect(at2Mb.disabledReason).toBeNull();
    expect(at2Mb.diagnostics.length).toBeGreaterThan(0);

    const over2Mb = runLightweightDiagnostics('json', '{bad', null, DIAGNOSTICS_MAX_BYTES + 1);
    expect(over2Mb.diagnostics).toHaveLength(0);
    expect(over2Mb.disabledReason).toContain('2MB');

    const over5Mb = runLightweightDiagnostics('json', '{bad', null, PLAIN_TEXT_FALLBACK_BYTES + 1);
    expect(over5Mb.diagnostics).toHaveLength(0);
    expect(over5Mb.disabledReason).toContain('5MB');
  });

  it('applies diagnostic decorations in CM6 StateField', () => {
    let state = EditorState.create({
      doc: '{bad}',
      extensions: [diagnosticsField],
    });
    state = state.update({
      effects: setDiagnosticsEffect.of([
        { line: 1, column: 2, from: 1, to: 4, severity: 'error', message: 'Syntax error' },
      ]),
    }).state;

    const decoSet = state.field(diagnosticsField);
    expect(decoSet.size).toBe(1);
  });

  it('allows multiline Nginx directives without false-positive semicolon warnings', () => {
    const validMultilineNginx = [
      'http {',
      "  log_format main '$remote_addr - $remote_user [$time_local] '",
      '                  \'"$request" $status $body_bytes_sent\';',
      '  server {',
      '    listen 80;',
      '    location / {',
      '      add_header Content-Security-Policy',
      '        "default-src \'self\'"',
      '        always;',
      '      proxy_set_header X-Forwarded-For',
      '        $proxy_add_x_forwarded_for;',
      '    }',
      '  }',
      '}',
    ].join('\n');

    const ok = runLightweightDiagnostics('nginx', validMultilineNginx, null, 400);
    expect(ok.diagnostics).toHaveLength(0);

    const missingSemicolonBeforeNewDirective = [
      'server {',
      '  listen 80',
      '  server_name example.com;',
      '}',
    ].join('\n');
    const warn = runLightweightDiagnostics('nginx', missingSemicolonBeforeNewDirective, null, 80);
    expect(warn.diagnostics).toHaveLength(1);
    expect(warn.diagnostics[0]?.severity).toBe('warning');
    expect(warn.diagnostics[0]?.line).toBe(2);
  });

  it('supports YAML block scalars (| / >) and multiline quoted strings without false positives', () => {
    const validMultilineYaml = [
      'script: |-',
      '  if [ -f /tmp/a ]; then',
      '    echo "unclosed quote in block scalar is literal',
      '    no colon needed here',
      '  fi',
      'summary: >+',
      '  first folded line without colon',
      '  second folded line',
      'items:',
      '  - |',
      '    list item block scalar line',
      'multiline_double: "hello',
      '  world"',
      "multiline_single: 'foo",
      "  bar''baz'",
      'next_key: ok',
    ].join('\n');

    const ok = runLightweightDiagnostics('yaml', validMultilineYaml, null, 400);
    expect(ok.diagnostics).toHaveLength(0);

    const invalidYaml = ['name: test', 'bad_top_level_line', 'quote: "never closed'].join('\n');
    const err = runLightweightDiagnostics('yaml', invalidYaml, null, 100);
    expect(err.diagnostics).toHaveLength(2);
    expect(err.diagnostics[0]?.line).toBe(2);
    expect(err.diagnostics[1]?.line).toBe(3);
  });
});
