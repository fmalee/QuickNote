# M1-04b：CodeMirror 6 单实例多状态编辑器、600ms/2s 自动保存、三态外部变更横幅与右侧大纲面板

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.2、§3.4、§3.9（G/H）、§4.4、§5.5、§5.6.2、§5.7、§6 M1；`C1.2`、`C1.3`、`C2.2`、`C4.2`、`C4.3`、`C6.1`–`C6.3`、`C7.1`、`C7.4`、`C8.4`、`C8.5`、`C9.1`、`D-0006`、`D-0007`
- 前置依赖：`M1-04a`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、Store 状态机、大纲算法与冲突三态规格已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 安装 CodeMirror 6 核心依赖与已有 IPC 接口**：
    - 执行 `pnpm add @codemirror/state @codemirror/view @codemirror/commands @codemirror/language @codemirror/lang-markdown @lezer/highlight`。
    - 已在 `src/lib/ipc/` 就绪并在 `src/features/editor/api.ts` 封装（补充 `writeNote`）：
      - `noteWrite(args: WriteNoteArgs): Promise<WriteResult>` → `WriteNoteArgs { absPath: string, body: string, baseBodyHash: string }`，`WriteResult { bodyHash: string, mtimeMs: number, updatedMeta: NoteMeta | null }`
      - `onFsExternalChange(handler: (event: ExternalChangeEvent) => void): Promise<UnlistenFn>` → `ExternalChangeEvent { absPath: string, kind: 'modified' | 'deleted', newBody: string | null, newBodyHash: string | null, newMeta: NoteMeta | null }`
      - `uiStateGet(): Promise<UiState>` & `uiStateSet(args: { key: 'panelGroup' | 'tabs' | ..., value: unknown }): Promise<UiState>` → `PanelGroupUiState { visible: boolean, activeId: 'outline' | 'terminal' | 'related' | 'recent', width: number, collapsed: boolean }`
  - **2. 在 `src/features/editor/` 扩展 `useEditorTabsStore` 并落地 CM6 单一 `EditorView` + 每标签独立 `EditorState`（`MarkdownEditor.vue` / `EditorConflictBanner.vue` / `store.ts`）**：
    - **扩展 `EditorTab` 运行时状态**：
      - `draftBody: string`（当前编辑器内存中的正文，初始等于 `doc.body`，**绝不包含 YAML frontmatter** `D-0006`）
      - `baseBodyHash: string`（当前乐观并发基准哈希，初始等于 `doc.bodyHash`）
      - `conflictState: { kind: 'modified'; diskBody: string; diskBodyHash: string; diskMeta: NoteMeta | null } | { kind: 'deleted' } | null`
      - `saveStatus: 'saved' | 'dirty' | 'saving' | 'conflict' | 'deleted' | 'external-synced' | 'error'`
      - `cursorLine: number`（1-based，默认 `1`）、`cursorCol: number`（1-based，默认 `1`）
    - **单一 `EditorView` + 每标签 `EditorState` Map**：
      - `MarkdownEditor.vue` 维护单一 `EditorView` 实例与 `Map<string, EditorState>`（以 `tab.key` 为键）；
      - 切换 `activeTab` 时：保存旧标签的 `scrollTop` 与 `cursorPos`，然后通过 `view.setState(targetState)` 瞬切到新标签状态并恢复 `scrollTop`；
      - CM6 主题样式（`EditorView.theme(...)`）**必须 100% 使用 `var(--qn-*)` CSS 变量**（**严禁在 TS/Vue/CSS 中写任何 `#xxx` 十六进制裸色值**，否则 `scripts/check-architecture.mjs` 会报错阻断门禁！）；编辑器阅读宽度绑定 `--qn-editor-max-width`（可根据 `settingsStore.snapshot?.settings?.editor.maxWidth` 动态设置，默认 `900px` 居中，`0` 表示 `100%`）。
      - 当用户在编辑器输入产生 `docChanged` 时：调用 `editorTabsStore.updateTabBody(tab.key, newBody, cursorOffset, line, col)` → 若该标签为 `isPreview: true`，自动晋升为固定标签（`isPreview = false, isPinned = true`），标记 `isDirty = true`、`saveStatus = 'dirty'`，并启动自动保存调度器。
  - **3. 落地 600ms 防抖 / 2000ms 最大等待自动保存与 ULID 补写无感回填**：
    - 自动保存参数取自 `settingsStore.snapshot?.settings?.editor`：`autoSaveDebounceMs`（默认 `600`）与 `autoSaveMaxWaitMs`（默认 `2000`）。
    - 当标签 `updateTabBody` 被调用且 `conflictState === null` 且 `!isMissing` 时：
      - 重置 `600ms` 防抖定时器；若首次变脏后尚未设置 `maxWait` 定时器，同时启动 `2000ms` 最大等待定时器；任一定时器到期即触发 `saveTab(key)`。
      - **立即冲刷场景**：切换标签（`activateTab`）、关闭标签前、按下 `⌘S`、窗口 `blur` 失焦时，立即对当前脏标签执行 `flushSave(key)`。
    - `saveTab(key)` 逻辑：
      - 调用 `writeNote({ absPath: tab.absPath, body: tab.draftBody, baseBodyHash: tab.baseBodyHash })`；
      - 成功后：更新 `tab.baseBodyHash = res.bodyHash`、`tab.doc.body = tab.draftBody`、`tab.doc.bodyHash = res.bodyHash`、`tab.doc.mtimeMs = res.mtimeMs`、`tab.isDirty = false`、`tab.saveStatus = 'saved'`；
      - **ULID 补写唯一例外回填（`D-0006`）**：若 `res.updatedMeta !== null`（旧库无 `id` 笔记首次保存由后端自动补写了新 ULID），更新 `tab.doc.meta = res.updatedMeta`；若原 `tab.key` 是 `path:<relPath>` 而 `res.updatedMeta.id` 非空，同步将 `tab.key`（及 `activeKey`）平滑迁移为新 ULID，并通知 `notesStore.refreshAll()` 刷新左侧列表摘要。
      - 若 `writeNote` 抛出 `ErrorCode::Conflict`（`detail` 含 `ConflictInfo` JSON）：解析 `ConflictInfo` 将该标签转入 `conflictState = { kind: 'modified', diskBody: info.currentBody, diskBodyHash: info.currentBodyHash, diskMeta: null }`，`saveStatus = 'conflict'`，并暂停自动保存。
  - **4. 落地外部变更三态处理（`C8.5`）**：
    - 在 `editorTabsStore` 初始化时订阅 `onFsExternalChange((event) => handleExternalChange(event))`（按 `tab.absPath === event.absPath` 匹配已打开标签）：
      1. **干净态收到外部修改（`!tab.isDirty && event.kind === 'modified'`）**：
         - 静默更新 `tab.draftBody = event.newBody ?? ''`、`tab.baseBodyHash = event.newBodyHash ?? tab.baseBodyHash`（以及 `tab.doc.meta = event.newMeta ?? tab.doc.meta`），重置该标签 `EditorState` 并保持原 `scrollTop` 与夹取后的 `cursorPos`，设置 `tab.saveStatus = 'external-synced'`（状态栏显示“已同步外部修改”），同时刷新左侧笔记列表。
      2. **脏态收到外部修改（`tab.isDirty && event.kind === 'modified'`）**：
         - **立即清除并暂停该标签的自动保存定时器**；
         - 设置 `tab.conflictState = { kind: 'modified', diskBody: event.newBody ?? '', diskBodyHash: event.newBodyHash ?? '', diskMeta: event.newMeta }`、`tab.saveStatus = 'conflict'`；
         - 在编辑器上方渲染 `EditorConflictBanner.vue`（`data-testid="editor-conflict-banner"`），提供三个操作按钮：
           - **重新加载磁盘版本**（`resolveConflictReload(key)`）：放弃本地未保存修改，将 `draftBody` 与 `baseBodyHash` 替换为 `diskBody` / `diskBodyHash`，清除 `isDirty` 与 `conflictState`，`saveStatus = 'saved'`。
           - **备份并覆盖磁盘**（`resolveConflictOverwrite(key)`）：以 `diskBodyHash` 作为新的 `baseBodyHash` 调用 `writeNote({ absPath, body: tab.draftBody, baseBodyHash: diskBodyHash })` 写回本地内容，成功后清除 `conflictState` 与 `isDirty`。
           - **暂缓处理**（`dismissConflictBanner(key)`）：暂时隐藏横幅或保持手动编辑（用户可随时手动 `⌘S` 覆盖或重载）。
      3. **收到外部删除（`event.kind === 'deleted'`）**：
         - **绝不关闭标签，绝不清空当前 `tab.draftBody`**！
         - 清除自动保存定时器，设置 `tab.conflictState = { kind: 'deleted' }`、`tab.saveStatus = 'deleted'`；
         - 标签头与顶部横幅（`data-testid="editor-deleted-banner"`）标红警告“磁盘文件已丢失”，提供“写回原路径”（`restoreDeletedTabToDisk(key)`：调用 `writeNote` 重新落盘并恢复 `saved` 状态）与“关闭标签”（`closeTab(key)`）。
  - **5. 在 `src/features/panels/` 落地右侧默认大纲面板（`outlineExtractor.ts` / `store.ts` / `OutlinePanel.vue` / `index.ts`，`v4` §5.6.2、`C7.4`、`C8.4`）**：
    - **纯函数大纲提取与中文 Slug（`outlineExtractor.ts`）**：
      - `OutlineItem { level: number; text: string; slug: string; line: number; from: number }`
      - `headingToSlug(rawHeading: string, seenCounts: Map<string, number>): string`（与 Rust `C8.4` 完全一致：剥离 `**`/`__`/`*`/`_`/`` ` ``/`[text](url)` → `toLowerCase()` → 空白折叠为单 `-` → 去中英文标点，保留汉字/假名/韩文/字母数字/`-`/`_` → 去首尾 `-` → 重复标题依次追加 `-1`、`-2`）。
      - `extractMarkdownOutline(body: string): OutlineItem[]`：按行扫描 `body`，追踪 `` ``` `` / `~~~` 围栏代码块，**跳过代码块内部行**，匹配 ATX 标题 `^(#{1,6})\s+(.+)$`，生成 `{ level, text, slug, line, from }`。
      - `findActiveHeadingIndex(items: OutlineItem[], currentLine: number): number`：返回 `line <= currentLine` 的最后一个标题索引（若在首个标题之前且存在标题则返回 `0`，无标题返回 `-1`）。
    - **大纲 Store 与 `150ms` 防抖（`usePanelsStore`）**：
      - 监听当前激活标签 `draftBody` 变化，以 **`150ms` 防抖**重新计算 `outlineItems`（切换标签时立即计算）；
      - 随编辑器当前光标/视口顶行 `activeTopLine` 自动高亮当前章节；点击大纲项时通知编辑器滚动并聚焦到对应标题行（`scrollToLine(item.line, item.from)`）。
      - 面板组折叠与快捷键（`C7.2`、`C7.4`）：
        - 默认展开 `outline` 面板，宽度 `260px`（最小 `220px`）；
        - 支持 `⌃⌘O`（`Ctrl+Meta+KeyO`）切换大纲面板显隐（若已打开 `outline` 再按则折叠面板组，若已折叠则展开 `outline`）；支持 `⌃⌘0`（`Ctrl+Meta+Digit0`）折叠/展开整个右侧面板组；
        - 每次切换后调用 `uiStateSet({ key: 'panelGroup', value: panelGroupState })` 持久化，并在启动时从 `uiStateGet()` 恢复 `panelGroup` 状态。
  - **6. 装配 `MainWorkbenchShell.vue` 与底部状态栏**：
    - 将 `MainWorkbenchShell.vue` 中间的 `<pre>` 静态预览替换为 `EditorConflictBanner` + `MarkdownEditor`，右侧替换为 `OutlinePanel`；
    - 底部状态栏（`data-testid="workbench-statusbar"`）展示：保存状态文案（已保存 / 未保存 / 保存中… / 存在冲突 / 磁盘文件已丢失 / 已同步外部修改）、当前文档字符/词数统计、光标 `Ln X, Col Y`、热键状态。
- 不做：
  - 不实现 Markdown 格式快捷键（`⌘B`/`⌘I`/`⌥⌘C`）、上下文 Tab 缩进、行首复选框 widget、图片粘贴落盘或文档头 `[属性]` Popover 完整表单（全部归 `M2`）。
  - 不实现右侧 `related`、`recent`、`terminal` 面板（归 `M3`/`M4`）或 CM6 `MergeView` 并排对比（归 `M4`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`package.json`、`pnpm-lock.yaml`
- 前端业务模块：`src/features/editor/**`、`src/features/panels/**`、`src/features/notes/**`（仅限与编辑器保存/外部变更刷新联动的最小调整）、`src/app/MainWorkbenchShell.vue`
- 前端单元测试：`src/features/editor/__tests__/**`、`src/features/panels/__tests__/**`

## 3. 具体实现要求
- **严格遵守前端分层与 Token 规范（`C1.2`、`C7.1`）**：
  - 零 `@tauri-apps/api` 直连；跨 feature 只经顶层 `@/features/<name>`（`index.ts`）导入；
  - **特别注意 CM6 主题**：`EditorView.theme(...)` 内严禁出现任何 `#xxx` 裸十六进制颜色或硬编码数字 `z-index`，必须全部使用 `'var(--qn-text-primary)'`、`'var(--qn-bg-base)'`、`'var(--qn-accent)'` 等 CSS Token！
- **Vitest 100% 可测性**：自动保存定时器（`600ms` / `2000ms`）、三态外部变更（干净态静默重载 / 脏态冲突横幅 / 删除态标红保留正文）以及大纲 `150ms` 防抖提取必须全部在 Store / 纯函数中可直接通过 `vi.useFakeTimers()` 测试。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：
  - `src/features/editor/__tests__/editor-autosave-and-conflict.test.ts`：
    1. 验证编辑预览标签正文时自动晋升为固定标签（`isPreview: false, isPinned: true`）且 `isDirty: true`。
    2. 使用 `vi.useFakeTimers()` 验证 `600ms` 防抖自动保存与持续高频输入下 `2000ms` 最大等待强制保存，以及 `updatedMeta`（补写 ULID）返回后自动更新标签 `meta` 与 `key`。
    3. 验证收到 `fs:external-change` 时：**干净态**静默更新 `draftBody`/`baseBodyHash` 且 `saveStatus === 'external-synced'`；**脏态**立即暂停自动保存定时器、进入 `conflictState.kind === 'modified'` 并支持“重新加载磁盘版本”与“备份并覆盖磁盘”；**删除态（`deleted`）**不关闭标签、完整保留 `draftBody` 并支持“写回原路径”。
  - `src/features/panels/__tests__/outline-panel.test.ts`：
    1. 验证 `extractMarkdownOutline` 跳过 `` ``` `` / `~~~` 代码块内部 `#` 注释、剥离行内 `**`/`*`/`` ` ``/`[text](url)` 修饰符、生成精确 `C8.4` 中文 Slug 及重复标题 `-1`、`-2` 后缀。
    2. 验证 `150ms` 防抖更新大纲、`findActiveHeadingIndex` 随滚动行号高亮当前章节，以及 `⌃⌘O` / `⌃⌘0` 面板折叠切换与 `panelGroup` 持久化。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] 启动主窗口，单击笔记打开预览标签，在 CM6 编辑器中输入文字：标签自动由斜体变为固定标签，600ms 后底部状态栏变为“已保存”，右侧大纲面板在 150ms 内同步刷新标题层级，点击大纲项可跳转对应行。
- [ ] 在外部修改当前干净态笔记：主窗口无感刷新最新正文；在编辑器输入未保存文字时再从外部修改该文件：顶部立即弹出冲突横幅且暂停自动保存。
