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

  it('rejects raw hex and magic zIndex after nested <template #slot> and in dynamic :style / v-bind:style', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src/components/BaseButton.vue',
      `<template>
  <div class="wrap">
    <ModalShell>
      <template #header><span>Header</span></template>
      <div style="color: #fff">After slot</div>
      <div :style="{ backgroundColor: '#123456', zIndex: 99 }">Dynamic</div>
      <div v-bind:style="{ 'z-index': -1 }">VBind</div>
    </ModalShell>
  </div>
</template>
`,
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C7.1-no-raw-hex-color');
    expect(res.stderr).toContain('C7.1-no-magic-z-index');
    expect(res.stderr).toContain('#fff');
    expect(res.stderr).toContain('#123456');
    expect(res.stderr).toContain('"99"');
    expect(res.stderr).toContain('"-1"');
  });

  it('rejects reverse crate dependencies from domain into application/infra/commands', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src-tauri/src/domain/reverse.rs',
      'use crate::application::persistence;\nuse crate::infra::fs;\nuse crate::commands::app;\n',
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C1.2-rust-domain-isolation');
    expect(res.stderr).toContain('crate::application');
    expect(res.stderr).toContain('crate::infra');
    expect(res.stderr).toContain('crate::commands');
  });

  it('rejects reverse crate dependencies from infra into application/commands', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src-tauri/src/infra/reverse.rs',
      'use crate::application::window;\nuse crate::commands::settings;\n',
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C1.2-rust-infra-isolation');
    expect(res.stderr).toContain('crate::application');
    expect(res.stderr).toContain('crate::commands');
  });

  it("handles char literals like '\"' and rejects Option::unwrap, Result::unwrap, unwrap_err, expect_err, and panic!", () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src-tauri/src/application/char_and_panics.rs',
      `pub fn ok_quote<'a>(_s: &'a str) -> char { '"' }
pub fn bad_calls(a: Option<i32>, b: Result<i32, i32>) -> i32 {
    let q = '"';
    let _ = (q, a.map(Option::unwrap), b.map_err(|_| 0));
    let _ = Result::<i32, i32>::unwrap(Ok(1));
    let _ = Err::<i32, i32>(1).unwrap_err();
    let _ = Err::<i32, i32>(1).expect_err("boom");
    panic!("fatal");
}
`,
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C2.1-rust-no-unwrap-expect');
    expect(res.stderr).toContain('char_and_panics.rs:4');
    expect(res.stderr).toContain('char_and_panics.rs:5');
    expect(res.stderr).toContain('char_and_panics.rs:6');
    expect(res.stderr).toContain('char_and_panics.rs:7');
    expect(res.stderr).toContain('char_and_panics.rs:8');
  });

  it('rejects broken runtime wiring invariants (startup reconcile, switch_vault_db, notes external watch, editor pool draft guard)', () => {
    const fixture = createValidRepoFixture();
    writeRepoFile(
      fixture,
      'src-tauri/src/lib.rs',
      'pub fn run() { let _ = VaultWatcher::start(None, |_| {}); }\n',
    );
    writeRepoFile(
      fixture,
      'src-tauri/src/application/library.rs',
      'pub fn set_library_root() { let _ = 1; }\n',
    );
    writeRepoFile(
      fixture,
      'src/features/notes/store.ts',
      'export function init() { void subscribeIndexStatus(() => {}); }\n',
    );
    writeRepoFile(
      fixture,
      'src/features/editor/MarkdownEditor.vue',
      '<script setup lang="ts">\nconst p = store.getPooledEditorState(tab.key);\nvoid p;\n</script>\n<template><div /></template>\n',
    );

    const res = runCheck(fixture);
    expect(res.status).not.toBe(0);
    expect(res.stderr).toContain('C5.1-startup-reconcile');
    expect(res.stderr).toContain('C8.5-watch-unified-handler');
    expect(res.stderr).toContain('C6.1-library-switch-vault-db');
    expect(res.stderr).toContain('C8.5-notes-subscribe-external-change');
    expect(res.stderr).toContain('C7.5-editor-pool-draft-guard');
  });
});
