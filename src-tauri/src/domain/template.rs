//! Built-in note templates (`v4` Appendix B.3) and pure template renderer (`C1.2`).
//!
//! Pure in-memory functions only: no filesystem, database or runtime access.

use crate::domain::frontmatter::{render_yaml_string, split_frontmatter};

/// Default `note` template (`v4` Appendix B.3).
pub const DEFAULT_TEMPLATE_NOTE: &str = "\
---
id: {{id}}
title: {{title}}
type: note
tags: []
created: \"{{date}}T{{time}}\"
pinned: false
archived: false
---
# {{title}}
";

/// Default `doc` template (`v4` Appendix B.3).
pub const DEFAULT_TEMPLATE_DOC: &str = "\
---
id: {{id}}
title: {{title}}
type: doc
tags: []
project: \"\"
open_dir: \"\"
code_workspace: \"\"
terminal_dir: \"\"
created: \"{{date}}T{{time}}\"
pinned: false
archived: false
---
# {{title}}

## 想法

## 待办
";

/// Default `command-book` template (`v4` Appendix B.3).
pub const DEFAULT_TEMPLATE_COMMAND_BOOK: &str = "\
---
id: {{id}}
title: {{title}}
type: command-book
tags:
  - commands
project: \"\"
open_dir: \"\"
created: \"{{date}}T{{time}}\"
---
# {{title}}

## 常用

### 示例命令

命令用途说明。

```bash
echo \"hello quicknote\"
```
<!-- keywords: 示例 demo -->
";

/// Variables substituted into a template (`{{id}}`, `{{title}}`, `{{date}}`, `{{time}}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateVars<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub date: &'a str,
    pub time: &'a str,
}

/// Returns a built-in template by name (`note`, `doc`, `command-book`; tolerates `.md` suffix).
pub fn builtin_template(name: &str) -> Option<&'static str> {
    let trimmed = name.trim();
    let canonical = trimmed.strip_suffix(".md").unwrap_or(trimmed);
    match canonical {
        "note" => Some(DEFAULT_TEMPLATE_NOTE),
        "doc" => Some(DEFAULT_TEMPLATE_DOC),
        "command-book" => Some(DEFAULT_TEMPLATE_COMMAND_BOOK),
        _ => None,
    }
}

/// Infers the default template name from a library-relative directory path.
pub fn infer_default_template(dir_rel_path: &str) -> &'static str {
    let normalized = dir_rel_path.trim().replace('\\', "/");
    let clean = normalized.trim_matches('/');
    if clean == "commands" || clean.starts_with("commands/") {
        "command-book"
    } else if clean == "projects" || clean.starts_with("projects/") {
        "doc"
    } else {
        "note"
    }
}

fn escape_double_quoted_inner(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn replace_title_in_frontmatter(fm: &str, title: &str) -> String {
    const PLACEHOLDER: &str = "{{title}}";
    let mut out = String::with_capacity(fm.len() + title.len());
    let mut rest = fm;

    while let Some(idx) = rest.find(PLACEHOLDER) {
        out.push_str(&rest[..idx]);
        let before_char = rest[..idx].chars().next_back();
        let after_start = idx + PLACEHOLDER.len();
        let after_char = rest[after_start..].chars().next();

        let is_quoted = matches!(
            (before_char, after_char),
            (Some('"'), Some('"')) | (Some('\''), Some('\''))
        );

        if is_quoted {
            if before_char == Some('"') {
                out.push_str(&escape_double_quoted_inner(title));
            } else {
                out.push_str(&title.replace('\'', "''"));
            }
        } else {
            out.push_str(&render_yaml_string(title));
        }

        rest = &rest[after_start..];
    }

    out.push_str(rest);
    out
}

/// Renders a template string with `TemplateVars`, safely YAML-quoting `{{title}}` inside frontmatter.
pub fn render_template(raw: &str, vars: &TemplateVars<'_>) -> String {
    let split = split_frontmatter(raw);
    match split.block {
        Some(block) => {
            let rendered_block = replace_title_in_frontmatter(block, vars.title)
                .replace("{{id}}", vars.id)
                .replace("{{date}}", vars.date)
                .replace("{{time}}", vars.time);
            let rendered_body = split
                .body
                .replace("{{id}}", vars.id)
                .replace("{{title}}", vars.title)
                .replace("{{date}}", vars.date)
                .replace("{{time}}", vars.time);
            format!("{rendered_block}{rendered_body}")
        }
        None => raw
            .replace("{{id}}", vars.id)
            .replace("{{title}}", vars.title)
            .replace("{{date}}", vars.date)
            .replace("{{time}}", vars.time),
    }
}
