# M4-04：导出能力（复制为富文本 + 导出 PDF）与设置窗口配置导入导出 / 终端配置

- 所属里程碑：M4
- 关联方案与契约：`v4` §5.5、§5.6.7、§5.11、§6 M4；`C1.2`、`C2.2`、`C2.3`、`C4.2`、`C6.2`、`C7.1`、`C7.2`、`C9.1`、`D-0008`
- 前置依赖：`M4-03` 已合并至 `dev`（且 `M4-01b` 已提供 `note_export_pdf`、`settings_export`、`settings_import`）
- 建议分支：`feat/M4-04-export-and-settings-backup`
- 会话策略：独立新会话执行（零大文档读取，所有渲染规则、剪贴板双 MIME 构造、PDF 排版与设置表单契约已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **前端轻量 Markdown → 富文本 HTML 与打印级 HTML 渲染器（零新增 npm 依赖，`src/features/editor/export/markdownExport.ts`）**：
   - `export function escapeHtml(raw: string): string`：转义 `&`、`<`、`>`、`"`、`'`。
   - `export function sanitizeHref(raw: string): string | null`：仅放行 `http://`、`https://`、`mailto:`、`#` 与相对路径；拒绝 `javascript:`、`vbscript:`、`data:text/html` 防 XSS。
   - `export function renderMarkdownToRichHtml(markdown: string): string`：
     - 支持完整常用 Markdown 语法：ATX 标题（H1–H6）、段落、软换行、加粗（`**`/`__`）、斜体（`*`/`_`）、删除线（`~~`）、行内代码（`` ` ``）、围栏代码块（```` ```lang ````）、引用块（`>`）、无序列表（`-`/`*`/`+`）、有序列表（`1.`）、待办列表（`- [ ]` / `- [x]` 转为禁用态勾选框或 `☐`/`☑`）、链接（`[text](url)`）、图片（`![alt](src)`）、GFM 表格（表头+对齐分隔线+数据行）与水平分割线（`---`）。
     - 生成带简洁内联样式（`style="..."`）的 HTML 片段，确保粘贴进 Apple Mail、飞书、Notion、Word 等富文本目标时排版整洁（标题字号、代码块浅灰底与等宽字体、表格细边框、引用左竖条）。
   - `export function buildPlainTextFallback(markdown: string): string`：提供清晰纯文本载体，与 `text/html` 组成双 MIME 剪贴板载荷。
   - `export async function copyMarkdownAsRichText(markdown: string): Promise<'rich' | 'plain'>`：
     - 优先通过 `navigator.clipboard.write([new ClipboardItem({ 'text/html': new Blob([html], { type: 'text/html' }), 'text/plain': new Blob([plain], { type: 'text/plain' }) })])` 写入双 MIME；
     - 在不支持 `ClipboardItem` 的环境自动降级至 `navigator.clipboard.writeText(plain)`。
   - `export function buildPrintHtmlDocument(doc: NoteDoc, body: string): string`：
     - 构造完整自包含 `<!doctype html>` 打印文档：
       - `@page { size: A4; margin: 18mm 16mm; }`，浅色高对比度打印主题（白底深灰字，与当前界面是否暗色模式解耦，保证纸面/PDF 阅读清晰）；
       - 顶部文档标题（`<h1>`）与元数据摘要行（类型徽标、标签、关联项目、创建时间，均做 `escapeHtml`）；
       - 正文区：若为 Markdown 文档使用 `renderMarkdownToRichHtml(body)`；若为非 Markdown 纯文本/配置文件则包裹于 `<pre><code>` 等宽排版块中；
       - 分页保护 CSS：`h1, h2, h3 { break-after: avoid; }`、`pre, blockquote, table { break-inside: avoid; }`。
   - `export function buildDefaultPdfExportPath(doc: NoteDoc): string`：
     - 将标题（或文件名）清理非法路径字符（`/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`），生成默认路径 `~/Desktop/<safe-title>.pdf`。
2. **主窗口导出入口与导出 PDF 弹窗**：
   - 在 `src/lib/actions/catalog.ts` 新增两条编辑器动作：
     - `editor.copyRichText`（标题“复制为富文本”，`category: 'editor'`, `defaultKeys: null`, `scope: 'editor'`, `when: (ctx) => Boolean(ctx.hasActiveDoc)`）；
     - `editor.exportPdf`（标题“导出为 PDF…”，`category: 'editor'`, `defaultKeys: null`, `scope: 'editor'`, `when: (ctx) => Boolean(ctx.hasActiveDoc)`）。
   - 在 `src/features/editor/api.ts` 封装 `exportNotePdf(args: ExportPdfArgs)`（调用 `noteExportPdf`）。
   - 新增轻量保存弹窗 `src/features/editor/ExportPdfModal.vue`（复用 `QnModalShell`、`QnInput`、`QnButton`、`QnBanner`，遵循项目暂不引入 `tauri-plugin-dialog` 的路径输入约定）：
     - 预填默认路径 `~/Desktop/<safe-title>.pdf`，允许用户修改；
     - 提交时构造 `buildPrintHtmlDocument(doc, currentDraftBody)` 并调用 `exportNotePdf`；
     - 成功后展示成功提示并提供“在访达中显示”（调用 `externalActionRun({ id: 'external.reveal', absPath: savedPath })`）与“关闭”按钮；失败展示清晰错误提示。
   - 在 `src/features/editor/DocHeaderBar.vue` 的 `[⋯]` 菜单中新增分隔组：“复制为富文本”（复制成功后给出轻状态提示“已复制为富文本”）与“导出为 PDF…”；
   - 在 `src/app/MainWorkbenchShell.vue` 绑定 `editor.copyRichText` 与 `editor.exportPdf` 动作处理器（使其同时可通过 `⌘K` 命令/快速打开 `>` 触发），并在 `src-tauri/src/lib.rs` 系统“文件”菜单补充 `editor.exportPdf`（“导出为 PDF…”）与“编辑”菜单补充 `editor.copyRichText`（“复制为富文本”），通过既有 `menu:action` 事件转发。
3. **设置窗口配置导入导出与内嵌终端参数配置**：
   - 在 `src/features/settings/api.ts` 封装 `exportSettingsBackup(destPath: string)`（调用 `settingsExport`）与 `importSettingsBackup(srcPath: string)`（调用 `settingsImport`）。
   - 在 `src/features/settings/store.ts` 新增：
     - `exportSettings(destPath: string): Promise<boolean>`
     - `importSettings(srcPath: string): Promise<boolean>`（导入成功后立即将返回的 `SettingsSnapshot` 同步至 `settings` 与 `loadStatus`，清零旧错误，失败则保留精确行列号/未知字段错误提示到 `backupError`）。
   - 在 `src/features/settings/SettingsView.vue`（“通用与热键”分区）新增两张标准 `QnPanelCard`：
     1. **配置备份与恢复卡片**（`data-testid="settings-backup-card"`）：
        - 导出路径输入（默认 `~/Desktop/quicknote-settings-backup.yaml`）+ “导出配置备份”按钮（`data-testid="settings-export-btn"`）；
        - 导入路径输入 + “导入并恢复配置”按钮（`data-testid="settings-import-btn"`），附风险提示“导入前将自动备份当前配置至 backups/，格式错误不会覆盖现有文件”；
        - 成功/失败反馈横幅。
     2. **内嵌终端面板配置卡片**（`data-testid="settings-terminal-panel-card"`，也可按布局整洁性放在“外部与搜索”或“通用与热键”下，建议在“外部与搜索” `SettingsExternalPanel.vue` 中与外部终端并列便于查找，本卡白名单两者均放行）：
        - 登录 Shell 路径（`panels.terminal.shell`，默认 `/bin/zsh`）；
        - 隐藏闲置自动回收分钟数（`panels.terminal.idleRecycleMinutes`，默认 `30`，标明 `0` 表示禁用自动回收）；
        - 全局最大并发会话数（`panels.terminal.maxConcurrentSessions`，范围 `1–6`，默认 `3`）；
        - 全部双向绑定 `settings_update` 并即时保存。

### 明确不做
- 不在主编辑区提供常驻 Markdown 分屏预览（`v4` §5.5）。
- 不引入 `marked` / `markdown-it` / `dompurify` 等重型第三方渲染库，保持轻量零依赖与完整单测覆盖。

## 2. 允许修改文件白名单

### 业务与测试文件
- `src-tauri/src/lib.rs`（仅在原生菜单栏补充“导出为 PDF…”与“复制为富文本”菜单项并沿用 `menu:action` 转发）
- `src/lib/actions/catalog.ts`
- `src/features/editor/export/markdownExport.ts`（新增）
- `src/features/editor/ExportPdfModal.vue`（新增）
- `src/features/editor/api.ts`
- `src/features/editor/DocHeaderBar.vue`
- `src/features/editor/index.ts`
- `src/app/MainWorkbenchShell.vue`
- `src/features/settings/api.ts`
- `src/features/settings/store.ts`
- `src/features/settings/SettingsView.vue`
- `src/features/settings/SettingsExternalPanel.vue`
- `src/features/editor/__tests__/export-rich-text-and-pdf.test.ts`（新增）
- `src/features/settings/__tests__/settings-backup-and-terminal.test.ts`（新增）

## 3. 具体实现要求

1. **防 XSS 与安全排版**：
   - `renderMarkdownToRichHtml` 与 `buildPrintHtmlDocument` 对用户输入文本（含标题、标签、代码块、表格单元格）一律做 HTML 转义；危险协议链接降级为纯文本或安全锚点；严禁拼接未转义 `<script>`。
2. **日志脱敏（`C2.3`）**：
   - 严禁在 `console.*` 输出富文本 HTML、剪贴板内容或笔记正文。
3. **样式 Token 守则（`C7.1`）**：
   - 弹窗与设置卡片 UI 全部使用 `--qn-*` CSS Token；注意 `markdownExport.ts` 生成的离屏打印 HTML 字符串位于 TS 字符串模板中，确保 `scripts/check-architecture.mjs` 扫描规则完全通过（`check-architecture.mjs` 仅扫描 CSS/Vue style 的硬编码色值，若扫描 TS 需验证 `pnpm check:arch` 全绿）。

## 4. 自动化验收标准（必须全绿）

- 新增测试 `src/features/editor/__tests__/export-rich-text-and-pdf.test.ts`，覆盖：
  1. 标题、加粗/斜体/删除线、代码块、引用、有序/无序/待办列表、链接、表格渲染正确性；
  2. `<script>` 标签转义与 `javascript:` 链接过滤（XSS 防护）；
  3. `copyMarkdownAsRichText` 正确构造 `text/html` + `text/plain` 双 MIME `ClipboardItem`，以及无 `ClipboardItem` 时的纯文本降级；
  4. `buildPrintHtmlDocument` 输出完整打印级 HTML（含 `@page`、标题、元数据摘要与正文）；
  5. `ExportPdfModal` 提交流程调用 `note_export_pdf` 并展示“在访达中显示”快捷操作。
- 新增测试 `src/features/settings/__tests__/settings-backup-and-terminal.test.ts`，覆盖：
  1. 设置导出调用 `settings_export` 与成功提示；
  2. 设置导入成功热更新 store 配置；导入非法配置返回错误时清晰展示错误信息且不破坏现有配置；
  3. 内嵌终端配置（`shell`、`idleRecycleMinutes`、`maxConcurrentSessions`）修改触发 `settings_update`。
- 必过命令：`pnpm check:all`。

## 5. 人工点验清单

- [ ] 对含中文标题、表格、代码块与待办项的笔记点击 `[⋯] → 复制为富文本`，粘贴至备忘录/邮件/飞书确认格式保真；
- [ ] 点击 `[⋯] → 导出为 PDF…` 导出至桌面，使用预览 App 打开确认分页、页边距、中文渲染与代码块排版清晰；
- [ ] 在设置窗口执行配置导出，手改一项设置后再导入该备份文件，确认配置无损恢复；尝试导入语法损坏 YAML 确认精确报错且现网配置不受损。
