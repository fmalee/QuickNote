# M3-05b：非 Markdown 多语言语法高亮与轻量诊断（大文件降级保护）+ 标签重命名与全库批量合并

- 所属里程碑：M3
- 关联方案与契约：`v4` §5.5.1、§5.6.1 第 5 条、§6 M3；`C1.2`、`C2.2`、`C4.2`、`C7.1`、`C9.1`
- 前置依赖：`M3-05a` 已合并至 `dev`
- 建议分支：`feat/M3-05b-non-md-highlight-diagnostics-and-tag-merge`
- 会话策略：独立新会话执行（零大文档读取，所有语言清单、体积阈值、诊断规则与 IPC 签名已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **非 Markdown 15 类语言识别、零依赖词法高亮与缩进自动侦测（`src/features/editor/extensions/codeLanguages.ts`，`v4` §5.5.1 & §5.6.1 第 5 条）**：
   - **零新增 npm 依赖**：直接基于已安装的 `@codemirror/language`（`StreamLanguage`）实现轻量、确定性的词法高亮器，复用于 `MarkdownEditor.vue` 既有的 `--qn-*` `HighlightStyle`！
   - 实现 `detectDocumentLanguage(absPath: string, content: string): DetectedLanguage`：
     - 优先按文件名与扩展名识别，未知扩展名时扫描首行 shebang（`#!/usr/bin/env bash|zsh|sh|python3|node|ts-node|ruby` 等）识别；
     - 完整覆盖全部 16 类语言映射（Markdown + 15 类非 Markdown）：
       1. `markdown`（`.md`, `.markdown`）
       2. `yaml`（`.yml`, `.yaml`）
       3. `json` / `jsonc`（`.json`, `.jsonc`, `.code-workspace`, `tsconfig*.json`）
       4. `toml`（`.toml`, `Cargo.lock`）
       5. `shell`（`.sh`, `.bash`, `.zsh`, `.bashrc`, `.zshrc`, `.env`, shebang `sh`/`bash`/`zsh`）
       6. `dockerfile`（`Dockerfile`, `Dockerfile.*`, `*.dockerfile`）
       7. `javascript` / `typescript`（`.js`, `.mjs`, `.cjs`, `.jsx`, `.ts`, `.mts`, `.tsx`, shebang `node`）
       8. `html`（`.html`, `.htm`, `.vue`）
       9. `css`（`.css`, `.scss`, `.less`）
       10. `xml`（`.xml`, `.svg`, `.plist`）
       11. `python`（`.py`, `.pyi`, shebang `python`）
       12. `rust`（`.rs`）
       13. `go`（`.go`）
       14. `sql`（`.sql`）
       15. `ini`（`.ini`, `.cfg`, `.properties`, `.gitconfig`）
       16. `nginx` / `conf`（`nginx.conf`, `*.conf`, 其余未知配置降级 `plaintext` 或 `conf`）
   - 为每类非 Markdown 语言提供 `StreamLanguage.define(...)` 高亮（精准标注 `comment`、`string`、`keyword`、`number`、`propertyName`、`variableName`、`operator`、`tagName`、`meta` 等标准 token，无缝接入 `HighlightStyle` 双主题配色）。
   - **非 Markdown 缩进自动侦测（`v4` §5.6.1 第 5 条）**：
     - 实现纯函数 `detectFileIndentation(content: string): { useTabs: boolean; unit: string; tabSize: number }`（扫描前 200 行缩进频率，自动识别既有 `\t` 或 2/4 空格宽度，默认 2 空格）；
     - 当打开非 Markdown 文件时，Tab/⇧Tab 走普通代码缩进/反缩进（按侦测出的 `unit`），**不套用 Markdown 列表/引用缩进、不启用 Markdown 待办复选框/链接修饰/格式快捷键**。

2. **轻量语法诊断与大文件体积分级守卫（`src/features/editor/extensions/codeDiagnostics.ts`，`v4` §5.5.1）**：
   - 定义大文件硬阈值常量：
     - `DIAGNOSTICS_MAX_BYTES = 2 * 1024 * 1024`（2MB：超过此体积**关闭语法诊断**）
     - `PLAIN_TEXT_FALLBACK_BYTES = 5 * 1024 * 1024`（5MB：超过此体积**降级纯文本，关闭语法高亮与语法诊断防卡顿**）
   - 定义诊断模型：
     ```ts
     export interface EditorDiagnostic {
       line: number; // 1-based
       column: number; // 1-based
       from: number; // 字符偏移
       to: number;
       severity: 'error' | 'warning';
       message: string;
     }
     ```
   - 实现纯函数 `runLightweightDiagnostics(language: DetectedLanguageId, content: string, metaError: string | null, sizeBytes: number): { diagnostics: EditorDiagnostic[]; disabledReason: string | null }`：
     - 若 `sizeBytes > PLAIN_TEXT_FALLBACK_BYTES`：返回空诊断 + `disabledReason: '文件超过 5MB，已降级为纯文本模式以保持流畅'`；
     - 若 `sizeBytes > DIAGNOSTICS_MAX_BYTES`：返回空诊断 + `disabledReason: '文件超过 2MB，已自动关闭语法诊断'`；
     - **Markdown**：仅报告 `metaError`（若存在，行 1 警告/错误，不做正文 LSP）；
     - **JSON / JSONC**：JSON 直接 `JSON.parse` 捕获语法错误并从 `position` / `line X column Y` 精确定位行列；JSONC 先保留换行符剥离 `//` 与 `/* */` 注释及尾逗号后 `JSON.parse` 报错；
     - **YAML**：静态校验致命常见错误（缩进中使用 Tab 字符、未闭合单/双引号字符串、未闭合 `[`/`{` 括号、非法顶层冒号结构）；
     - **TOML**：静态校验节标题 `[section]` 闭合、`key = value` 合法性、未闭合引号与数组；
     - **Shell**：静态校验未闭合单/双引号及常见块配对（`if`/`fi`、`do`/`done`、`case`/`esac`）；
     - **Nginx / conf**：静态校验花括号 `{}` 配对及指令末尾分号遗漏提醒。
   - 在 CM6 中提供轻量诊断装饰扩展（基于 `StateEffect` + `StateField` + `Decoration.mark({ class: 'qn-cm-diagnostic-error' })` 波浪下划线），并在 `MarkdownEditor.vue` 底部集成紧凑诊断状态栏（展示当前识别语言徽标、缩进信息如 `2 空格` / `Tab`、诊断条数或大文件降级提示，点击诊断条可跳转到对应行）。

3. **编辑器 `MarkdownEditor.vue` 动态语言/体积 Compartment 接入**：
   - 新增 `languageCompartment = new Compartment()` 与 `diagnosticsCompartment = new Compartment()`：
     - 根据当前文档的 `absPath`、`sizeBytes`（取 `Math.max(doc.sizeBytes, new TextEncoder().encode(draftBody).length)`）决定加载：
       - `> 5MB`：纯文本（零语言解析、零诊断、零 Markdown widget）；
       - `<= 5MB` 且为 Markdown：完整 Markdown 语言 + Todo/Link/Indent 扩展；
       - `<= 5MB` 且为非 Markdown：对应 `StreamLanguage` + 自动侦测缩进 Tab/⇧Tab；
     - 诊断防抖 `300ms` 更新，超过 `2MB` 自动清空并关闭诊断。

4. **标签重命名与全库标签批量合并（`src/features/notes/`，`C4.2` `tags_batch_rename`）**：
   - 在 `src/features/notes/api.ts` 封装 `batchRenameTag(oldTag: string, newTag: string): Promise<TagRenameResult>`（调用 `tagsBatchRename({ oldTag, newTag })`）。
   - 在 `src/features/notes/store.ts` 新增：
     - `renameOrMergeTag(oldTag: string, newTag: string): Promise<number>`：
       - 调用 `batchRenameTag(oldTag, newTag)`；
       - 若当前 `selectedTags` 含 `oldTag` 或其子标签，同步替换为对应新标签并去重；
       - 刷新 `refreshNavTree()` 与 `refreshNotesList()`；
       - 返回更新文件数 `updatedFiles`；
     - 通知已打开的编辑器标签页刷新磁盘元数据（调用 `editorTabsStore.refreshOpenTabsMetaFromDisk()` 或逐一同步干净态标签，确保打开的笔记文档头标签芯片即时变成新标签）。
   - 在 `src/features/notes/NotesSidebar.vue`：
     - 为标签树每个节点提供右键菜单（`QnContextMenu`）与悬停操作按钮“重命名 / 合并…”；
     - 点击打开轻量模态框（`QnModalShell`，`data-testid="tag-rename-modal"`）：
       - 展示原标签名、输入新标签名；
       - 实时检查目标标签是否已存在于当前 `navTree.tags`：若已存在，展示醒目提示（`QnBanner tone="warning"`）“目标标签 `#<newTag>` 已存在，确认后将把原标签下的全部笔记**合并**到该标签下（自动去重，不覆盖注释与未知字段）”；
       - 确认提交后显示成功轻提示（如“已更新 N 篇笔记的标签”）。

### 明确不做什么
- 本卡**不做**任何 LSP / 外部语言服务器集成（重度工程编辑已由 `external.vscode` 解决）。
- 本卡**不新增**任何 `package.json` 外部依赖（全程使用已安装的 `@codemirror/language` 与原生 TS），不修改 Rust 代码或 `C1.3` 共享文件。

## 2. 允许修改文件白名单

- 编辑器高亮与诊断扩展（`src/features/editor/`）：
  - `src/features/editor/extensions/codeLanguages.ts`（新增）
  - `src/features/editor/extensions/codeDiagnostics.ts`（新增）
  - `src/features/editor/MarkdownEditor.vue`
  - `src/features/editor/store.ts`（仅添加打开标签元数据同步方法，如需）
  - `src/features/editor/index.ts`
- 标签重命名与批量合并（`src/features/notes/`）：
  - `src/features/notes/api.ts`
  - `src/features/notes/store.ts`
  - `src/features/notes/NotesSidebar.vue`
- 测试文件：
  - `src/features/editor/__tests__/code-languages-and-diagnostics.test.ts`（新增）
  - `src/features/notes/__tests__/tag-batch-rename.test.ts`（新增）

## 3. 具体实现要求

1. **已内联的 IPC 签名（位于 `src/lib/ipc/commands.ts` 与 `src/bindings/types.ts`）**：
   - `tagsBatchRename(args: { oldTag: string; newTag: string }): Promise<TagRenameResult>`
     - `TagRenameResult = { updatedFiles: number }`
2. **大文件性能防线（`v4` §5.5.1）**：
   - 切勿在 `> 5MB` 文件上同步运行完整正则或 AST 扫描；判断 `sizeBytes > PLAIN_TEXT_FALLBACK_BYTES` 必须最先短路直接返回纯文本配置与空诊断！
   - `> 2MB` 且 `<= 5MB` 时保持轻量流式高亮但完全跳过诊断解析器。
3. **样式与分层红线（`C1.2`、`C7.1`）**：
   - 波浪诊断装饰、语言状态栏与标签重命名弹窗全部使用 `--qn-*` CSS Token（例如 `--qn-danger`、`--qn-warning`、`--qn-border`、`--qn-bg-secondary`），严禁硬编码颜色值。
   - Vue 组件严禁直连 `@tauri-apps/api/*`。

## 4. 自动化验收标准（必须全绿）

1. `src/features/editor/__tests__/code-languages-and-diagnostics.test.ts` 覆盖：
   - 全部 16 类语言（含扩展名与 `#!/usr/bin/env python3` / `#!/bin/bash` shebang）准确识别，并验证 `StreamLanguage` 能正常创建 CM6 `EditorState` 无异常；
   - `detectFileIndentation` 准确识别 Tabs、2 空格、4 空格文件；
   - 轻量诊断精准报错与通过用例：合法/非法 JSON、带注释与尾逗号的 JSONC、Tab 缩进坏 YAML、未闭合 TOML、未闭合 Shell/Nginx、Markdown `metaError`；
   - 体积边界精准断言：`2MB + 1` 字节自动关闭诊断并保留高亮；`5MB + 1` 字节降级纯文本同时关闭高亮与诊断。
2. `src/features/notes/__tests__/tag-batch-rename.test.ts` 覆盖：
   - 侧栏标签重命名弹窗交互、非空与同名拦截校验；
   - 当目标标签已存在时准确显示“合并”提示文案；
   - 确认提交调用 `tagsBatchRename({ oldTag, newTag })`、同步更新 `selectedTags`、刷新导航树与列表并显示更新文件数。
3. 运行 `pnpm check:all` 全部 9 项门禁 100% 通过。

## 5. 人工点验清单

- 在主窗口通过 `⌘K` 打开外部 `nginx.conf`、`settings.yaml`、`package.json` 与 `.sh` 文件，验证语法高亮、自动缩进与改坏 JSON/YAML 时实时波浪诊断提示；打开一个 >2MB 和 >5MB 测试文本文件确认降级提示与滚动流畅不卡顿。
- 在左侧标签树右键一个标签重命名或合并到已有标签，验证全库笔记标签即时更新且文件注释与未知 YAML 字段完整保留。
