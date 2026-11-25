import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

function normalizeRel(p) {
  return p.split(path.sep).join('/');
}

/**
 * Explicit v5 refactor module & directory allowlist.
 * Keeps C1.2 unidirectional layering intact while permitting upcoming v5 modules:
 * - Rust domain: `src-tauri/src/domain/bookmark.rs`, `src-tauri/src/domain/workspace_context.rs`
 * - Rust application: `src-tauri/src/application/reconciler.rs`
 * - Rust infra: `src-tauri/src/infra/fs_browser.rs`
 * - Frontend editor core: `src/features/editor/core/` (intra-feature only; cross-feature deep imports remain forbidden)
 */
export const V5_MODULE_ALLOWLIST = Object.freeze({
  rustDomain: ['src-tauri/src/domain/bookmark.rs', 'src-tauri/src/domain/workspace_context.rs'],
  rustApplication: ['src-tauri/src/application/reconciler.rs'],
  rustInfra: ['src-tauri/src/infra/fs_browser.rs'],
  frontendFeatureDirs: ['src/features/editor/core/'],
});

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

function matchRustCharLiteralEnd(source, quoteIndex) {
  const len = source.length;
  let j = quoteIndex + 1;
  if (j >= len || source[j] === '\n' || source[j] === "'") {
    return -1;
  }

  if (source[j] === '\\') {
    j += 1;
    if (j >= len || source[j] === '\n') {
      return -1;
    }
    const esc = source[j];
    j += 1;
    if (esc === 'x') {
      if (j + 1 < len && /[0-9a-fA-F]/.test(source[j]) && /[0-9a-fA-F]/.test(source[j + 1])) {
        j += 2;
      } else {
        return -1;
      }
    } else if (esc === 'u') {
      if (j < len && source[j] === '{') {
        j += 1;
        let digits = 0;
        while (j < len && /[0-9a-fA-F_]/.test(source[j])) {
          digits += 1;
          j += 1;
        }
        if (digits === 0 || j >= len || source[j] !== '}') {
          return -1;
        }
        j += 1;
      } else {
        return -1;
      }
    }
    return j < len && source[j] === "'" ? j + 1 : -1;
  }

  const cp = source.codePointAt(j);
  const charLen = cp !== undefined && cp > 0xffff ? 2 : 1;
  j += charLen;
  return j < len && source[j] === "'" ? j + 1 : -1;
}

/**
 * Strips comments (and optionally string/char literals) while preserving newlines so line numbers stay accurate.
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

      // Byte character literal: b'...'
      if (ch === 'b' && next === "'" && (i === 0 || !/[a-zA-Z0-9_]/.test(source[i - 1]))) {
        const end = matchRustCharLiteralEnd(source, i + 1);
        if (end !== -1) {
          out += ' '.repeat(end - i);
          i = end;
          continue;
        }
      }

      // Character literal: '...' (distinguished from lifetimes like 'a or 'static)
      if (ch === "'") {
        const end = matchRustCharLiteralEnd(source, i);
        if (end !== -1) {
          out += ' '.repeat(end - i);
          i = end;
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
    { name: 'crate::application', regex: /\bcrate\s*::\s*application\b/g },
    { name: 'crate::infra', regex: /\bcrate\s*::\s*infra\b/g },
    { name: 'crate::commands', regex: /\bcrate\s*::\s*commands\b/g },
    {
      name: 'crate::{...application/infra/commands...}',
      regex: /\buse\s+crate\s*::\s*\{[^}]*\b(?:application|infra|commands)\b[^}]*\}/g,
    },
    {
      name: 'super::application/infra/commands',
      regex: /\bsuper\s*::\s*(?:application|infra|commands)\b/g,
    },
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

function checkRustInfra(rootDir, violations) {
  const infraDir = path.join(rootDir, 'src-tauri', 'src', 'infra');
  const files = walkFiles(infraDir, (f) => f.endsWith('.rs'));
  const forbiddenPatterns = [
    { name: 'crate::application', regex: /\bcrate\s*::\s*application\b/g },
    { name: 'crate::commands', regex: /\bcrate\s*::\s*commands\b/g },
    {
      name: 'crate::{...application/commands...}',
      regex: /\buse\s+crate\s*::\s*\{[^}]*\b(?:application|commands)\b[^}]*\}/g,
    },
    {
      name: 'super::application/commands',
      regex: /\bsuper\s*::\s*(?:application|commands)\b/g,
    },
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
          rule: 'C1.2-rust-infra-isolation',
          file: rel,
          line: lineNumberAt(cleaned, match.index),
          message: `infra module must not reference forbidden dependency "${rule.name}"`,
        });
      }
    }
  }
}

function checkRustNoPanic(rootDir, violations) {
  const srcDir = path.join(rootDir, 'src-tauri', 'src');
  const files = walkFiles(srcDir, (f) => f.endsWith('.rs'));
  const panicPatterns = [
    {
      regex: /\.\s*(?:unwrap|expect|unwrap_err|expect_err)\s*\(/g,
      message:
        'production Rust code under src-tauri/src/** must not call .unwrap(), .expect(), .unwrap_err(), or .expect_err()',
    },
    {
      regex:
        /\b(?:Option|Result)\s*::\s*(?:<[^>]*>\s*::\s*)?(?:unwrap|expect|unwrap_err|expect_err)\b/g,
      message:
        'production Rust code under src-tauri/src/** must not reference Option/Result::unwrap*, expect*',
    },
    {
      regex: /\bpanic\s*!\s*[([{]/g,
      message: 'production Rust code under src-tauri/src/** must not invoke panic!()',
    },
  ];

  for (const file of files) {
    const raw = fs.readFileSync(file, 'utf8');
    const cleaned = stripRust(raw, { stripStrings: true });
    const rel = normalizeRel(path.relative(rootDir, file));

    for (const pattern of panicPatterns) {
      pattern.regex.lastIndex = 0;
      let match;
      while ((match = pattern.regex.exec(cleaned)) !== null) {
        violations.push({
          rule: 'C2.1-rust-no-unwrap-expect',
          file: rel,
          line: lineNumberAt(cleaned, match.index),
          message: pattern.message,
        });
      }
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

function maskPreservingNewlines(source, regex) {
  return source.replace(regex, (m) => m.replace(/[^\n]/g, ' '));
}

function extractVueOutermostTemplate(vueSource) {
  let masked = maskPreservingNewlines(vueSource, /<script\b[^>]*>[\s\S]*?<\/script>/gi);
  masked = maskPreservingNewlines(masked, /<style\b[^>]*>[\s\S]*?<\/style>/gi);
  masked = maskPreservingNewlines(masked, /<!--[\s\S]*?-->/g);

  const tagRegex = /<\/?template\b[^>]*>/gi;
  let depth = 0;
  let contentStart = -1;
  let match;

  while ((match = tagRegex.exec(masked)) !== null) {
    const tag = match[0];
    if (/^<\/template/i.test(tag)) {
      if (depth > 0) {
        depth -= 1;
        if (depth === 0 && contentStart !== -1) {
          return {
            content: vueSource.slice(contentStart, match.index),
            offset: contentStart,
          };
        }
      }
    } else if (!/\/>\s*$/.test(tag)) {
      if (depth === 0) {
        contentStart = match.index + tag.length;
      }
      depth += 1;
    }
  }

  return null;
}

function scanDynamicStyleBinding(expr, baseOffset, fullRaw, rel, violations) {
  const hexRegex = /(?:^|[^\w-])(#[0-9a-fA-F]{3,8})(?![0-9a-fA-F_-])/g;
  let hexMatch;
  while ((hexMatch = hexRegex.exec(expr)) !== null) {
    const tokenOffset = hexMatch.index + (hexMatch[0].length - hexMatch[1].length);
    violations.push({
      rule: 'C7.1-no-raw-hex-color',
      file: rel,
      line: lineNumberAt(fullRaw, baseOffset + tokenOffset),
      message: `raw hex color "${hexMatch[1]}" is forbidden; use --qn-* CSS tokens`,
    });
  }

  const zIndexRegex = /(?:['"]z-index['"]|\bzIndex)\s*:\s*['"]?\s*(-?\d+)\b/g;
  let zMatch;
  while ((zMatch = zIndexRegex.exec(expr)) !== null) {
    violations.push({
      rule: 'C7.1-no-magic-z-index',
      file: rel,
      line: lineNumberAt(fullRaw, baseOffset + zMatch.index),
      message: `hardcoded z-index "${zMatch[1]}" is forbidden; use --qn-z-* tokens`,
    });
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

    const templateBlock = extractVueOutermostTemplate(raw);
    if (templateBlock) {
      const tpl = templateBlock.content;
      const tplOffset = templateBlock.offset;

      const staticStyleRegex = /(?:^|\s)style\s*=\s*(?:"([^"]*)"|'([^']*)')/gi;
      let attrMatch;
      while ((attrMatch = staticStyleRegex.exec(tpl)) !== null) {
        const styleVal = attrMatch[1] ?? attrMatch[2] ?? '';
        const valOffset = tplOffset + attrMatch.index + attrMatch[0].indexOf(styleVal);
        scanStyleDeclarations(styleVal, valOffset, raw, rel, violations);
      }

      const dynamicStyleRegex = /(?:^|\s)(?::|v-bind:)style\s*=\s*(?:"([^"]*)"|'([^']*)')/gi;
      let dynMatch;
      while ((dynMatch = dynamicStyleRegex.exec(tpl)) !== null) {
        const styleExpr = dynMatch[1] ?? dynMatch[2] ?? '';
        const exprOffset = tplOffset + dynMatch.index + dynMatch[0].indexOf(styleExpr);
        scanDynamicStyleBinding(styleExpr, exprOffset, raw, rel, violations);
      }
    }
  }
}

export function checkArchitecture(rootDir) {
  const resolvedRoot = path.resolve(rootDir);
  const violations = [];

  checkRustDomain(resolvedRoot, violations);
  checkRustInfra(resolvedRoot, violations);
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
