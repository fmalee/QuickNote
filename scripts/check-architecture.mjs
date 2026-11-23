import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

function normalizeRel(p) {
  return p.split(path.sep).join('/');
}

function walkFiles(dir, predicate, out = []) {
  if (!fs.existsSync(dir)) {
    return out;
  }
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name === 'node_modules' || entry.name === 'target' || entry.name === 'dist') {
      continue;
    }
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walkFiles(fullPath, predicate, out);
    } else if (entry.isFile() && predicate(fullPath)) {
      out.push(fullPath);
    }
  }
  return out;
}

/**
 * Strips comments (and optionally string literals) while preserving newlines so line numbers stay accurate.
 */
function stripRust(source, { stripStrings = true } = {}) {
  let out = '';
  let i = 0;
  const len = source.length;

  while (i < len) {
    const ch = source[i];
    const next = source[i + 1];

    // Line comment
    if (ch === '/' && next === '/') {
      out += '  ';
      i += 2;
      while (i < len && source[i] !== '\n') {
        out += ' ';
        i += 1;
      }
      continue;
    }

    // Block comment (supports Rust nested block comments)
    if (ch === '/' && next === '*') {
      let depth = 1;
      out += '  ';
      i += 2;
      while (i < len && depth > 0) {
        if (source[i] === '/' && source[i + 1] === '*') {
          depth += 1;
          out += '  ';
          i += 2;
        } else if (source[i] === '*' && source[i + 1] === '/') {
          depth -= 1;
          out += '  ';
          i += 2;
        } else {
          out += source[i] === '\n' ? '\n' : ' ';
          i += 1;
        }
      }
      continue;
    }

    if (stripStrings) {
      // Raw string literal: r#"..."# or br#"..."#
      if (
        (ch === 'r' || (ch === 'b' && next === 'r')) &&
        ((ch === 'r' && (next === '"' || next === '#')) ||
          (ch === 'b' && (source[i + 2] === '"' || source[i + 2] === '#')))
      ) {
        let j = ch === 'b' ? i + 2 : i + 1;
        let hashes = 0;
        while (j < len && source[j] === '#') {
          hashes += 1;
          j += 1;
        }
        if (j < len && source[j] === '"') {
          out += ' '.repeat(j - i + 1);
          i = j + 1;
          while (i < len) {
            if (source[i] === '"') {
              let k = i + 1;
              let matched = 0;
              while (matched < hashes && k < len && source[k] === '#') {
                matched += 1;
                k += 1;
              }
              if (matched === hashes) {
                out += ' '.repeat(1 + hashes);
                i = k;
                break;
              }
            }
            out += source[i] === '\n' ? '\n' : ' ';
            i += 1;
          }
          continue;
        }
      }

      // Standard or byte string literal: "..." or b"..."
      if (ch === '"' || (ch === 'b' && next === '"')) {
        if (ch === 'b') {
          out += '  ';
          i += 2;
        } else {
          out += ' ';
          i += 1;
        }
        while (i < len) {
          if (source[i] === '\\' && i + 1 < len) {
            out += source[i + 1] === '\n' ? ' \n' : '  ';
            i += 2;
            continue;
          }
          if (source[i] === '"') {
            out += ' ';
            i += 1;
            break;
          }
          out += source[i] === '\n' ? '\n' : ' ';
          i += 1;
        }
        continue;
      }
    }

    out += ch;
    i += 1;
  }

  return out;
}

function stripJsComments(source) {
  let out = '';
  let i = 0;
  const len = source.length;
  let quote = null;

  while (i < len) {
    const ch = source[i];
    const next = source[i + 1];

    if (quote) {
      if (ch === '\\' && i + 1 < len) {
        out += ch + source[i + 1];
        i += 2;
        continue;
      }
      if (ch === quote) {
        quote = null;
      }
      out += ch;
      i += 1;
      continue;
    }

    if (ch === '/' && next === '/') {
      out += '  ';
      i += 2;
      while (i < len && source[i] !== '\n') {
        out += ' ';
        i += 1;
      }
      continue;
    }

    if (ch === '/' && next === '*') {
      out += '  ';
      i += 2;
      while (i < len) {
        if (source[i] === '*' && source[i + 1] === '/') {
          out += '  ';
          i += 2;
          break;
        }
        out += source[i] === '\n' ? '\n' : ' ';
        i += 1;
      }
      continue;
    }

    if (ch === '"' || ch === "'" || ch === '`') {
      quote = ch;
    }
    out += ch;
    i += 1;
  }

  return out;
}

function stripCssComments(source) {
  let out = '';
  let i = 0;
  const len = source.length;
  while (i < len) {
    if (source[i] === '/' && source[i + 1] === '*') {
      out += '  ';
      i += 2;
      while (i < len) {
        if (source[i] === '*' && source[i + 1] === '/') {
          out += '  ';
          i += 2;
          break;
        }
        out += source[i] === '\n' ? '\n' : ' ';
        i += 1;
      }
      continue;
    }
    out += source[i];
    i += 1;
  }
  return out;
}

function lineNumberAt(text, index) {
  let line = 1;
  for (let i = 0; i < index && i < text.length; i += 1) {
    if (text[i] === '\n') {
      line += 1;
    }
  }
  return line;
}

function checkRustDomain(rootDir, violations) {
  const domainDir = path.join(rootDir, 'src-tauri', 'src', 'domain');
  const files = walkFiles(domainDir, (f) => f.endsWith('.rs'));
  const forbiddenPatterns = [
    { name: 'tauri', regex: /\btauri\b/g },
    { name: 'std::fs', regex: /\bstd\s*::\s*fs\b/g },
    { name: 'std::{...fs...}', regex: /\buse\s+std\s*::\s*\{[^}]*\bfs\b[^}]*\}/g },
    { name: 'std::process', regex: /\bstd\s*::\s*process\b/g },
    { name: 'std::{...process...}', regex: /\buse\s+std\s*::\s*\{[^}]*\bprocess\b[^}]*\}/g },
    { name: 'rusqlite', regex: /\brusqlite\b/g },
    { name: 'notify', regex: /\bnotify\b/g },
    { name: 'portable_pty', regex: /\bportable_pty\b/g },
  ];

  for (const file of files) {
    const raw = fs.readFileSync(file, 'utf8');
    const cleaned = stripRust(raw, { stripStrings: true });
    const rel = normalizeRel(path.relative(rootDir, file));

    for (const rule of forbiddenPatterns) {
      rule.regex.lastIndex = 0;
      let match;
      while ((match = rule.regex.exec(cleaned)) !== null) {
        violations.push({
          rule: 'C1.2-rust-domain-isolation',
          file: rel,
          line: lineNumberAt(cleaned, match.index),
          message: `domain module must not reference forbidden dependency "${rule.name}"`,
        });
      }
    }
  }
}

function checkRustNoPanic(rootDir, violations) {
  const srcDir = path.join(rootDir, 'src-tauri', 'src');
  const files = walkFiles(srcDir, (f) => f.endsWith('.rs'));
  const panicRegex = /\.\s*(?:unwrap|expect)\s*\(/g;

  for (const file of files) {
    const raw = fs.readFileSync(file, 'utf8');
    const cleaned = stripRust(raw, { stripStrings: true });
    const rel = normalizeRel(path.relative(rootDir, file));

    panicRegex.lastIndex = 0;
    let match;
    while ((match = panicRegex.exec(cleaned)) !== null) {
      violations.push({
        rule: 'C2.1-rust-no-unwrap-expect',
        file: rel,
        line: lineNumberAt(cleaned, match.index),
        message: 'production Rust code under src-tauri/src/** must not call .unwrap() or .expect()',
      });
    }
  }
}

function extractVueScripts(vueSource) {
  const blocks = [];
  const scriptRegex = /<script\b[^>]*>([\s\S]*?)<\/script>/gi;
  let match;
  while ((match = scriptRegex.exec(vueSource)) !== null) {
    const contentStart = match.index + match[0].indexOf('>') + 1;
    blocks.push({ content: match[1] ?? '', offset: contentStart });
  }
  return blocks;
}

function extractVueStyles(vueSource) {
  const blocks = [];
  const styleRegex = /<style\b[^>]*>([\s\S]*?)<\/style>/gi;
  let match;
  while ((match = styleRegex.exec(vueSource)) !== null) {
    const contentStart = match.index + match[0].indexOf('>') + 1;
    blocks.push({ content: match[1] ?? '', offset: contentStart });
  }
  return blocks;
}

function extractImports(code) {
  const results = [];
  const cleaned = stripJsComments(code);
  const patterns = [
    /\b(?:import|export)\s+(?:type\s+)?(?:[\s\S]*?\s+from\s+)?['"]([^'"]+)['"]/g,
    /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g,
  ];
  for (const regex of patterns) {
    regex.lastIndex = 0;
    let match;
    while ((match = regex.exec(cleaned)) !== null) {
      results.push({ specifier: match[1] ?? '', index: match.index });
    }
  }
  return { cleaned, imports: results };
}

function resolveFrontendSpecifier(specifier, fileRel) {
  if (specifier.startsWith('@/')) {
    return `src/${specifier.slice(2)}`;
  }
  if (specifier.startsWith('.')) {
    const dir = path.posix.dirname(fileRel);
    return path.posix.normalize(path.posix.join(dir, specifier));
  }
  return null;
}

function checkFrontendBoundaries(rootDir, violations) {
  const srcDir = path.join(rootDir, 'src');
  const files = walkFiles(srcDir, (f) => /\.(ts|tsx|js|mjs|cjs|vue)$/.test(f));

  for (const file of files) {
    const rel = normalizeRel(path.relative(rootDir, file));
    const raw = fs.readFileSync(file, 'utf8');
    const isIpcLayer = rel.startsWith('src/lib/ipc/');

    const chunks = rel.endsWith('.vue') ? extractVueScripts(raw) : [{ content: raw, offset: 0 }];

    for (const chunk of chunks) {
      const { cleaned, imports } = extractImports(chunk.content);

      // Direct @tauri-apps/api usage guard
      if (!isIpcLayer) {
        const directTauriRegex = /@tauri-apps\/api/g;
        let match;
        while ((match = directTauriRegex.exec(cleaned)) !== null) {
          violations.push({
            rule: 'C1.2-ipc-boundary',
            file: rel,
            line: lineNumberAt(raw, chunk.offset + match.index),
            message: 'only src/lib/ipc/** may import @tauri-apps/api',
          });
        }
      }

      for (const imp of imports) {
        const target = resolveFrontendSpecifier(imp.specifier, rel);
        if (!target) {
          continue;
        }
        const line = lineNumberAt(raw, chunk.offset + imp.index);

        // Cross-feature deep import guard
        const srcFeatureMatch = /^src\/features\/([^/]+)\//.exec(rel);
        const targetFeatureMatch = /^src\/features\/([^/]+)(?:\/(.*))?$/.exec(target);
        if (srcFeatureMatch && targetFeatureMatch) {
          const srcFeature = srcFeatureMatch[1];
          const targetFeature = targetFeatureMatch[1];
          const subpath = targetFeatureMatch[2] ?? '';
          const isPublicEntry = subpath === '' || /^index(?:\.(?:ts|tsx|js|mjs))?$/.test(subpath);
          if (srcFeature !== targetFeature && !isPublicEntry) {
            violations.push({
              rule: 'C1.2-no-cross-feature-deep-import',
              file: rel,
              line,
              message: `cross-feature deep import "${imp.specifier}" into feature "${targetFeature}" is forbidden`,
            });
          }
        }

        // Unidirectional layer checks: lib -> components -> features -> app
        if (rel.startsWith('src/components/')) {
          if (
            target.startsWith('src/lib/ipc') ||
            target.startsWith('src/features') ||
            target.startsWith('src/app')
          ) {
            violations.push({
              rule: 'C1.2-frontend-layering',
              file: rel,
              line,
              message: `components layer must not import "${imp.specifier}"`,
            });
          }
        } else if (rel.startsWith('src/lib/')) {
          if (
            target.startsWith('src/components') ||
            target.startsWith('src/features') ||
            target.startsWith('src/app')
          ) {
            violations.push({
              rule: 'C1.2-frontend-layering',
              file: rel,
              line,
              message: `lib layer must not import "${imp.specifier}"`,
            });
          }
        } else if (rel.startsWith('src/features/')) {
          if (target.startsWith('src/app')) {
            violations.push({
              rule: 'C1.2-frontend-layering',
              file: rel,
              line,
              message: `features layer must not import "${imp.specifier}"`,
            });
          }
        }
      }
    }
  }
}

function checkBindingsDir(rootDir, violations) {
  const bindingsDir = path.join(rootDir, 'src', 'bindings');
  if (!fs.existsSync(bindingsDir) || !fs.statSync(bindingsDir).isDirectory()) {
    violations.push({
      rule: 'C1.4-bindings-integrity',
      file: 'src/bindings',
      line: 1,
      message: 'src/bindings/ directory is missing',
    });
    return;
  }

  const entries = fs.readdirSync(bindingsDir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name !== 'types.ts' || !entry.isFile()) {
      violations.push({
        rule: 'C1.4-bindings-integrity',
        file: `src/bindings/${entry.name}`,
        line: 1,
        message: 'only src/bindings/types.ts is permitted inside src/bindings/',
      });
    }
  }

  const typesFile = path.join(bindingsDir, 'types.ts');
  if (!fs.existsSync(typesFile) || !fs.statSync(typesFile).isFile()) {
    violations.push({
      rule: 'C1.4-bindings-integrity',
      file: 'src/bindings/types.ts',
      line: 1,
      message: 'src/bindings/types.ts is missing',
    });
    return;
  }

  const content = fs.readFileSync(typesFile, 'utf8');
  const firstLine = (content.split(/\r?\n/, 1)[0] ?? '').trimEnd();
  if (!firstLine.startsWith('/* GENERATED — DO NOT EDIT.')) {
    violations.push({
      rule: 'C1.4-bindings-header',
      file: 'src/bindings/types.ts',
      line: 1,
      message: 'line 1 of src/bindings/types.ts must start with "/* GENERATED — DO NOT EDIT."',
    });
  }
}

function scanStyleDeclarations(text, baseOffset, fullRaw, rel, violations) {
  const cleaned = stripCssComments(text);

  // Check CSS property declarations (e.g. `color: #fff;` or inline style `color: #fff`)
  const declRegex = /(?:^|[;{]\s*)([a-zA-Z_-][a-zA-Z0-9_-]*)\s*:\s*([^;{}]+)/g;
  let match;
  while ((match = declRegex.exec(cleaned)) !== null) {
    const prop = (match[1] ?? '').toLowerCase();
    const value = match[2] ?? '';
    const valueStart = match.index + match[0].indexOf(value);

    const hexRegex = /(?:^|[^\w-])(#[0-9a-fA-F]{3,8})(?![0-9a-fA-F_-])/g;
    let hexMatch;
    while ((hexMatch = hexRegex.exec(value)) !== null) {
      const tokenOffset = valueStart + hexMatch.index + (hexMatch[0].length - hexMatch[1].length);
      violations.push({
        rule: 'C7.1-no-raw-hex-color',
        file: rel,
        line: lineNumberAt(fullRaw, baseOffset + tokenOffset),
        message: `raw hex color "${hexMatch[1]}" is forbidden; use --qn-* CSS tokens`,
      });
    }

    if (prop === 'z-index') {
      const zMatch = /^\s*(-?\d+)\b/.exec(value);
      if (zMatch) {
        violations.push({
          rule: 'C7.1-no-magic-z-index',
          file: rel,
          line: lineNumberAt(fullRaw, baseOffset + valueStart),
          message: `hardcoded z-index "${zMatch[1]}" is forbidden; use --qn-z-* tokens`,
        });
      }
    }
  }
}

function checkStyles(rootDir, violations) {
  const srcDir = path.join(rootDir, 'src');
  const cssFiles = walkFiles(srcDir, (f) => f.endsWith('.css'));
  for (const file of cssFiles) {
    const rel = normalizeRel(path.relative(rootDir, file));
    if (rel === 'src/styles/tokens.css') {
      continue;
    }
    const raw = fs.readFileSync(file, 'utf8');
    scanStyleDeclarations(raw, 0, raw, rel, violations);
  }

  const vueFiles = walkFiles(srcDir, (f) => f.endsWith('.vue'));
  for (const file of vueFiles) {
    const rel = normalizeRel(path.relative(rootDir, file));
    const raw = fs.readFileSync(file, 'utf8');

    for (const block of extractVueStyles(raw)) {
      scanStyleDeclarations(block.content, block.offset, raw, rel, violations);
    }

    const templateMatch = /<template\b[^>]*>([\s\S]*?)<\/template>/i.exec(raw);
    if (templateMatch) {
      const tpl = templateMatch[1] ?? '';
      const tplOffset = templateMatch.index + templateMatch[0].indexOf('>') + 1;
      const inlineStyleRegex = /\bstyle\s*=\s*(?:"([^"]*)"|'([^']*)')/gi;
      let attrMatch;
      while ((attrMatch = inlineStyleRegex.exec(tpl)) !== null) {
        const styleVal = attrMatch[1] ?? attrMatch[2] ?? '';
        const valOffset = tplOffset + attrMatch.index + attrMatch[0].indexOf(styleVal);
        scanStyleDeclarations(styleVal, valOffset, raw, rel, violations);
      }
    }
  }
}

export function checkArchitecture(rootDir) {
  const resolvedRoot = path.resolve(rootDir);
  const violations = [];

  checkRustDomain(resolvedRoot, violations);
  checkRustNoPanic(resolvedRoot, violations);
  checkFrontendBoundaries(resolvedRoot, violations);
  checkBindingsDir(resolvedRoot, violations);
  checkStyles(resolvedRoot, violations);

  return violations;
}

function parseRootArg(argv) {
  const idx = argv.indexOf('--root');
  if (idx !== -1 && argv[idx + 1]) {
    return path.resolve(argv[idx + 1]);
  }
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  return path.resolve(scriptDir, '..');
}

const isEntrypoint =
  process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isEntrypoint) {
  const rootDir = parseRootArg(process.argv.slice(2));
  const violations = checkArchitecture(rootDir);

  if (violations.length > 0) {
    console.error(`Architecture check failed with ${violations.length} violation(s):`);
    for (const v of violations) {
      console.error(`  [${v.rule}] ${v.file}:${v.line} — ${v.message}`);
    }
    process.exit(1);
  }

  console.log('Architecture check passed (0 violations).');
}
