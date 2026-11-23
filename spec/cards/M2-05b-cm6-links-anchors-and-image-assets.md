# M2-05b：CM6 Markdown 增强（下）——语法树链接与中文 Slug 锚点跳转、图片粘贴落盘与排版设置热响应

- 所属里程碑：M2
- 关联方案与契约：`v4` §5.6.3、§5.6.4、§6 M2；`C1.2`、`C2.2`、`C2.3`、`C4.2`、`C7.1`、`C8.4`、`C9.1`
- 前置依赖：`M2-01`、`M2-03`、`M2-05a`
- 会话策略：独立新会话执行（零大文档读取，所有语法树节点规则、链接目标解析、二次确认与图片落盘契约已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 语法树链接装饰与目标分流引擎（`src/features/editor/extensions/markdownLinks.ts`，`v4` §5.6.4、`C2.3`、`C8.4`）**：
    - **语法树链接提取（基于 `@codemirror/language` `syntaxTree(state)`）**：
      - 扫描 `Link`、`URL`、`Autolink` 节点，同时扫描文档内的引用式链接定义（`^[ ]{0,3}\[([^\]]+)\]:\s*<?([^\s>]+)>?`）解析 `[text][ref]` 与 `[ref][]`；
      - 导出纯函数 `extractLinksFromState(state: EditorState): ExtractedLink[]`（`{ from: number; to: number; url: string; label: string }`）与 `findLinkAtPos(state: EditorState, pos: number): ExtractedLink | null`；
      - 导出 CM6 `ViewPlugin` `markdownLinkPlugin`：为可点击链接范围应用 `Decoration.mark({ class: 'qn-cm-link', attributes: { 'data-qn-link': url } })`（样式使用 `--qn-accent` 下划线，`⌘` 按下或开启 `singleClickLink` 时显示指针光标）。
    - **纯链接目标解析器 `parseLinkTarget(rawUrl: string, currentDoc: { absPath: string; relPath: string | null; isLibrary: boolean }, libraryRoot: string | null): ParsedLinkTarget`**：
      - `type ParsedLinkTarget =`
        - `{ kind: 'external'; url: string }`：`https://`、`http://`、`mailto:`（忽略大小写）；
        - `{ kind: 'anchor'; anchor: string }`：以 `#` 开头（对后续部分安全 `decodeURIComponent`，去除前导 `#`）；
        - `{ kind: 'relativeNote'; targetAbsPath: string; targetRelPath: string | null; anchor: string | null }`：相对路径（如 `./x.md`、`../docs/guide.md#中文标题`、`sub/a.md`），先按 `#` 拆出可选 `anchor`，再基于 `currentDoc.absPath` 所在目录纯 POSIX 规范化解析 `targetAbsPath`（与 `libraryRoot` 比对得出 `targetRelPath`）；
        - `{ kind: 'localAbsolute'; path: string; requiresConfirmation: true }`：以 `/` 开头或 `file://` 开头（`decodeURIComponent` 剥离 `file://` 前缀），**强制 `requiresConfirmation: true`（`C2.3` 安全红线，绝不静默打开）**；
        - `{ kind: 'invalid'; raw: string; reason: string }`：空串、不支持协议（如 `javascript:`）等。
    - **中文 Slug 锚点匹配 `resolveHeadingAnchor(body: string, rawAnchor: string): OutlineItem | null`**：
      - 复用 `@/features/panels` 导出的 `extractMarkdownOutline(body)` 与 `headingToSlug`（完全遵循 `C8.4`：全小写、空格转 `-`、去标点、保留汉字/假名/韩文、重复标题 `-1`/`-2`；不支持 `{#custom-id}`）；
      - 同时支持直接匹配已生成的 `item.slug`（例如 `中文标题`、`中文标题-1`）或对用户手写锚点做规范化后匹配，保证精确命中。
    - **交互触发规则**：
      - 默认仅在 **`⌘` + 单击**（`event.metaKey && !event.ctrlKey && !event.altKey`）时触发打开链接，防止日常编辑误触；
      - 当 `settings.editor.singleClickLink === true` 时，普通鼠标左键单击链接即可触发；
      - 右键点击处于链接上的位置时，`MarkdownEditor.vue` 的 `QnContextMenu` 顶部动态追加两项同源链接动作：“打开链接”（调用同一跳转处理器）与“复制链接”（写入 `navigator.clipboard.writeText(url)`）；
      - **目标执行策略**：
        - `anchor`：调用 `resolveHeadingAnchor(activeTab.draftBody, target.anchor)` → 命中则调用 `editorTabsStore.requestScrollToLine(item.line, item.from)` 滚动并聚焦；未命中则显示非阻塞轻提示（如 `未找到标题锚点：#${target.anchor}`），不崩溃；
        - `relativeNote`：调用 `editorTabsStore.openNote(target.targetRelPath ?? target.targetAbsPath, { preview: false, absPath: target.targetAbsPath })`；若目标存在且带 `anchor`，打开后在目标文档正文上调用 `resolveHeadingAnchor` 并滚动至该标题行；若目标文件不存在（`NotFound`），显示轻提示“目标文件不存在，可能已被移动或删除”，不崩溃、不破坏当前标签；
        - `localAbsolute`：弹出 `QnModalShell` 路径二次确认对话框（`data-testid="local-path-confirm-modal"`），清晰展示完整绝对路径，用户点击“确认打开”后：若是 `.md`/文本文件调用 `editorTabsStore.openNote(target.path, { absPath: target.path, preview: false })` 以只读/外部标签打开，否则给出提示；取消则什么都不做；
        - `external`：调用安全打开（`window.open(target.url, '_blank', 'noopener,noreferrer')`）。
  - **2. 图片粘贴与拖入自动落盘 `assets/YYYY-MM/`（`src/features/editor/extensions/markdownImages.ts`，`v4` §5.6.3、`C4.2`）**：
    - 在 `src/features/editor/api.ts` 补全封装：`saveNoteImage(args: SaveImageArgs): Promise<SavedImageResult>`（调用 `noteSaveImage`）。
    - 纯辅助函数：
      - `extractImageExtFromFile(file: { name?: string; type?: string }): string | null`：从 MIME（`image/png` → `png`、`image/jpeg` → `jpg`、`image/webp` → `webp`、`image/gif` → `gif`、`image/svg+xml` → `svg`）或文件名后缀提取白名单扩展名；
      - `arrayBufferToBase64(buffer: ArrayBuffer): string`（无 `any`，分块转换防止大数组栈溢出）；
      - `buildMarkdownImageSnippet(relPath: string, alt?: string): string` → `![${alt || 'image'}](${relPath})`。
    - 在 `MarkdownEditor.vue` 注册 CM6 `EditorView.domEventHandlers({ paste, drop })`：
      - `paste`：检查 `event.clipboardData?.items` / `files` 中是否存在 `image/*` 文件；若存在，调用 `event.preventDefault()`，读取字节转 base64；
      - `drop`：检查 `event.dataTransfer?.files` 中的图片文件；若存在，调用 `event.preventDefault()`，按拖放坐标定位插入点；
      - **库内 vs 库外分流（`v4` §5.6.3）**：
        - 库内文档（`activeDoc.isLibrary === true`）：直接调用 `saveNoteImage({ sourcePath: null, bytesBase64, ext, contextRelPath: activeDoc.relPath })`，成功后在目标光标位置插入 `![alt](assets/YYYY-MM/...)`；
        - 库外文档（`activeDoc.isLibrary === false`）：弹出 `QnModalShell` 确认框（`data-testid="external-image-confirm-modal"`）询问“当前为库外文件，是否将图片保存至笔记库 `assets/YYYY-MM/` 并插入引用？”，确认后再落盘插入；
      - 失败（未选库 / 超 20MB / 格式不支持）时在编辑器顶部显示可关闭轻提示（`QnBanner`），绝不清空或破坏当前编辑器内容。
  - **3. 编辑器设置项动态热响应（CM6 `Compartment`）**：
    - 在 `MarkdownEditor.vue` 使用 `@codemirror/state` 的 `Compartment` 动态响应 `settingsStore.snapshot?.settings?.editor`：
      - `fontSize`、`lineHeight`、`maxWidth` 实时作用于编辑器容器样式（通过 CSS 变量或动态主题 `Compartment`，全部无硬编码色值）；
      - `tabSizeProse`、`tabSizeCode`、`singleClickLink` 实时生效，设置窗口修改后主窗口无需重建 `EditorState` 立即生效！
- 不做：
  - 不实现外部 App 打开（`external_action_run` / Finder / VS Code，归 `M3`）或内嵌图片预览渲染（遵循 `v4` §3.4 不做实时渲染预览）。
  - 不修改 Rust 后端代码（`note_save_image` 已在 `M2-01` 完成）。

## 2. 允许修改文件白名单
- 共享串行文件：无
- 前端业务模块：
  - `src/features/editor/extensions/markdownLinks.ts`（新增）
  - `src/features/editor/extensions/markdownImages.ts`（新增）
  - `src/features/editor/api.ts`
  - `src/features/editor/MarkdownEditor.vue`
  - `src/features/editor/index.ts`
- 前端单元测试：`src/features/editor/__tests__/cm6-links-and-images.test.ts`（新增）

## 3. 具体实现要求
- **严格分层与安全红线（`C1.2`、`C2.3`）**：
  - 仅通过 `@/features/panels` 顶层导出引用 `extractMarkdownOutline` / `headingToSlug`，禁止跨 feature 深层导入；
  - 本地绝对路径与 `file://` 链接**强制二次确认**，拦截 `javascript:` 等危险协议；
  - 零硬编码色值与魔法 `z-index`（`C7.1`）。
- **日志脱敏（`C2.3`）**：错误处理与日志严禁打印 base64 图片字节流或文档正文片段。

## 4. 自动化验收标准（必须全绿）
- 新增测试文件：`src/features/editor/__tests__/cm6-links-and-images.test.ts`，覆盖：
  1. **语法树链接提取**：验证从含行内 `[文档](./a.md#配置)`、裸 URL `https://example.com`、引用式 `[规范][spec]\n\n[spec]: ./spec.md` 的 CM6 `EditorState` 准确提取全部链接位置与目标 URL。
  2. **中文 Slug 锚点匹配（`C8.4`）**：验证 `# 实现方案`、`# 实现方案`（第二次重复 → `实现方案-1`）、`## CJK + **混合**分词` 锚点均可精确解析到对应行号，失效锚点返回 `null` 不抛异常。
  3. **链接目标分流与安全确认（`C2.3`）**：验证 `https://` / `mailto:` 归类 `external`、`#中文` 归类 `anchor`、`./sub/note.md#章节` 正确解析相对路径与锚点、`/etc/hosts` 与 `file:///Users/a/b.md` 归类 `localAbsolute` 且 `requiresConfirmation === true`、`javascript:alert(1)` 归类 `invalid`。
  4. **触发模式判定**：验证默认 `singleClickLink = false` 时仅 `metaKey` 单击触发；`singleClickLink = true` 时普通单击触发。
  5. **图片粘贴/拖入落盘**：验证 `extractImageExtFromFile`、`arrayBufferToBase64` 与图片落盘处理器对库内文档直接调用 `noteSaveImage` 并在光标插入 `![...](assets/YYYY-MM/...)`，对库外文档要求先经过二次确认，对非法格式或 IPC 失败优雅提示不破坏正文。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] 在 Markdown 笔记中写入 `[跳到重复标题](#想法-1)`、`[同目录笔记](./另一篇.md#待办)`、`[本机路径](/tmp/test.md)` 与 `https://example.com`：默认普通单击进入编辑不误触，`⌘` + 单击分别正确跳转标题行、打开相对笔记并定位锚点、弹出绝对路径二次确认框、调起系统浏览器；右键链接可“打开链接 / 复制链接”。
- [ ] 截图复制到剪贴板后在库内笔记按 `⌘V` 粘贴（或从 Finder 拖入 PNG 图片）：图片自动保存至 `<library_root>/assets/YYYY-MM/`，光标处自动插入 `![...](assets/YYYY-MM/...)`。
- [ ] 打开设置窗口调整字号/行高/最大阅读宽度/单击打开链接开关，观察主窗口编辑器实时响应生效。
