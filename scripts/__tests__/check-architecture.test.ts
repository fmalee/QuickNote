import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const SCRIPT_PATH = path.join(REPO_ROOT, 'scripts', 'check-architecture.mjs');

const tempDirs: string[] = [];

function writeRepoFile(repoDir: string, relPath: string, content: string): void {
  const fullPath = path.join(repoDir, relPath);
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });
  fs.writeFileSync(fullPath, content, 'utf8');
}

function createValidRepoFixture(): string {
  const repoDir = fs.mkdtempSync(path.join(os.tmpdir(), 'quicknote-arch-test-'));
  tempDirs.push(repoDir);

  writeRepoFile(
    repoDir,
    'src-tauri/src/domain/mod.rs',
    'pub fn ok() -> usize { let v = Some(1usize); v.unwrap_or_default() }\n',
  );
  writeRepoFile(
    repoDir,
    'src-tauri/src/application/mod.rs',
    'pub fn run() -> Option<i32> { Some(1) }\n',
  );
  writeRepoFile(
    repoDir,
    'src-tauri/tests/sample_test.rs',
    '#[test] fn ok() { Some(1).unwrap(); }\n',
  );
  writeRepoFile(
    repoDir,
    'src/bindings/types.ts',
    '/* GENERATED — DO NOT EDIT. */\nexport type AppInfo = { version: string };\n',
  );
  writeRepoFile(
    repoDir,
    'src/styles/tokens.css',
    ':root { --qn-accent: #246ef0; --qn-z-modal: 400; }\n',
  );
  writeRepoFile(
    repoDir,
    'src/lib/ipc/commands.ts',
    "import { invoke } from '@tauri-apps/api/core';\nexport const ping = () => invoke('ping');\n",
  );
  writeRepoFile(
    repoDir,
    'src/components/BaseButton.vue',
    '<template><button id="bed" class="btn">OK</button></template>\n<style scoped>#bed { color: var(--qn-accent); z-index: var(--qn-z-modal); }</style>\n',
  );
  writeRepoFile(repoDir, 'src/features/notes/index.ts', 'export const notesReady = true;\n');
  writeRepoFile(
    repoDir,
    'src/features/quickbar/QuickBar.vue',
    '<script setup lang="ts">\nimport { notesReady } from \'@/features/notes\';\nvoid notesReady;\n</script>\n<template><div>QuickBar</div></template>\n',
  );

  return repoDir;
}

function runCheck(repoDir: string): { status: number | null; stdout: string; stderr: string } {
  const res = spawnSync('node', [SCRIPT_PATH, '--root', repoDir], { encoding: 'utf8' });
  return {
    status: res.status,
    stdout: res.stdout,
    stderr: res.stderr,
  };
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop();
    if (dir) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  }
});

describe('scripts/check-architecture.mjs', () => {
  it('passes on a clean valid repository fixture and on the real repo', () => {
    const fixture = createValidRepoFixture();
    const res = runCheck(fixture);
    expect(res.status).toBe(0);

    const realRes = runCheck(REPO_ROOT);
    expect(realRes.status).toBe(0);
  });

  it('rejects @tauri-apps/api imports outside src/lib/ipc/**', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src/components/BaseButton.vue',
      '<script setup lang="ts">\nimport { invoke } from \'@tauri-apps/api/core\';\nvoid invoke;\n</script>\n<template><button /></template>\n',
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C1.2-ipc-boundary');
  });

  it('rejects forbidden IO/framework dependencies in src-tauri/src/domain/**', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src-tauri/src/domain/leak.rs',
      'use std::fs;\npub fn read() { let _ = fs::metadata("."); }\n',
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C1.2-rust-domain-isolation');
  });

  it('rejects .unwrap() and .expect() in Rust production sources', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src-tauri/src/application/panic_site.rs',
      'pub fn boom(v: Option<i32>) -> i32 { v.unwrap() }\n',
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C2.1-rust-no-unwrap-expect');
  });

  it('rejects raw hex colors and hardcoded z-index in Vue/CSS outside tokens.css', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src/components/BaseButton.vue',
      '<template><button style="color: #fff">Bad</button></template>\n<style scoped>.btn { z-index: 99; }</style>\n',
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C7.1-no-raw-hex-color');
    expect(res.stderr).toContain('C7.1-no-magic-z-index');
  });

  it('rejects cross-feature deep imports', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src/features/quickbar/QuickBar.vue',
      '<script setup lang="ts">\nimport { secret } from \'@/features/notes/internal/helper\';\nvoid secret;\n</script>\n<template><div /></template>\n',
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C1.2-no-cross-feature-deep-import');
  });

  it('rejects missing header or extra files inside src/bindings/', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(fixture, 'src/bindings/types.ts', 'export type AppInfo = { version: string };\n');
    writeRepoFile(fixture, 'src/bindings/manual.ts', 'export const x = 1;\n');

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C1.4-bindings-header');
    expect(res.stderr).toContain('C1.4-bindings-integrity');
  });
});
