# M2-04：文档头元数据内联编辑、`[属性]` Popover、模板新建（`⌘N` / `⌥⌘↵`）与收件箱提取

- 所属里程碑：M2
- 关联方案与契约：`v4` §3.4、§3.5.1、§4.9–§4.11、§6 M2；`C1.2`、`C2.2`、`C3.4`、`C4.2`、`C6.4`、`C7.1`、`C7.2`、`C9.1`、`D-0006`、`D-0007`
- 前置依赖：`M2-01`、`M2-02`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、Store 状态流、外部无 frontmatter 确认规则与 UI 交互已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 文档头内联元数据编辑与 `[属性]` 轻量 Popover（`src/features/editor/`，`v4` §3.4、`D-0006`）**：
    - 在 `src/features/editor/api.ts` 补全封装：
      - `updateNoteMeta(args: NoteUpdateMetaArgs): Promise<NoteDoc>`（调用 `noteUpdateMeta`）
      - `extractInboxToNote(args: InboxExtractArgs): Promise<NoteDoc>`（调用 `inboxExtractToNote`）
      - `listTemplates(): Promise<NoteSummary[]>`（调用 `notesList({ tab: 'templates', dirRelPath: null, tags: [], workspaceKey: null, filterText: null })`）
    - 在 `src/features/editor/store.ts` 扩展 `useEditorTabsStore`：
      - 选区状态跟踪（供收件箱提取与上下文动作 `when` 判断）：在 `EditorTab` 增加 `selectionFromLine: number`、`selectionToLine: number`、`hasSelection: boolean`（默认 `1`、`1`、`false`），新增 `updateTabSelection(key: string, fromLine: number, toLine: number, hasSelection: boolean)` 并在 `MarkdownEditor.vue` 的 `updateListener` 同步更新；
      - 纯判定辅助函数 `export function needsExternalFrontmatterConfirm(doc: NoteDoc): boolean`：当 `!doc.isLibrary` 且 `doc.meta.id === null && doc.meta.created === null && doc.meta.tags.length === 0 && doc.meta.project === null && doc.meta.openDir === null && doc.meta.codeWorkspace === null && doc.meta.terminalDir === null && !doc.meta.pinned && !doc.meta.archived && doc.meta.docType === 'note' && doc.meta.metaError === null` 时返回 `true`（外部文件默认无 frontmatter，`v4` §3.4）；维护会话级已确认集合 `confirmedExternalMetaPaths` 避免同文件重复弹窗；
      - `updateActiveMeta(patch: NoteMetaPatch): Promise<NoteDoc | null>`：
        - 对当前激活标签调用 `updateNoteMeta({ absPath: tab.absPath, patch })`；
        - 成功后：更新 `tab.doc = updatedDoc`（**绝不覆盖编辑器中尚未保存的 `tab.draftBody`**，`D-0006` `meta + body` 分离！），更新 `tab.doc.meta = updatedDoc.meta`，标记 `confirmedExternalMetaPaths.add(tab.absPath)`，并调用 `notifyNotesRefresh()` 刷新左侧列表与标签树；
      - `extractInboxSelectionToNote(args: { newTitle: string; targetDirRelPath: string; templateName: string; fromLine?: number; toLine?: number }): Promise<NoteDoc | null>`：
        - 仅在当前激活标签为收件箱（`doc.isInbox || doc.relPath === 'inbox.md'`）时允许执行；
        - 先执行 `await flushSave(inboxTab.key)` 确保磁盘与当前编辑内容同步；
        - 调用 `extractInboxToNote({ fromLine, toLine, newTitle, targetDirRelPath, templateName })`；
        - 成功后：重新调用 `readNote({ absPath: inboxTab.absPath })` 静默同步 `inboxTab` 的最新 `draftBody`/`baseBodyHash`/`doc` 并递增 `inboxTab.syncSeq`（通知 CM6 重载无选区残留的 `inbox.md`），刷新 `notesStore`，并在新固定标签（`preview: false`）中打开返回的新 `NoteDoc`！
    - 新增组件 `src/features/editor/DocHeaderBar.vue` 与 `src/features/editor/DocPropertiesPopover.vue`：
      - **`DocHeaderBar.vue`**（替换 `MainWorkbenchShell.vue` 中的只读 `<header class="qn-main-shell__doc-header">`）：
        - **标题内联编辑**（`data-testid="doc-header-title-input"`）：点击/聚焦编辑 `meta.title`，回车或失焦且值变化时调用 `updateActiveMeta({ title })`（空白不提交并恢复原值）；
        - **类型下拉**（`data-testid="doc-header-type-select"`）：切换 `note` / `doc` / `command-book` / `template`，立即调用 `updateActiveMeta({ docType })`；
        - **标签 Chip 增删**（`data-testid="doc-header-tags"`）：渲染 `#tag` chip 带删除按钮 `×`；内置 `+ 标签` 输入框，回车自动去除前导 `#`、去重后调用 `updateActiveMeta({ tags: nextTags })`；
        - **`[属性]` 按钮**（`data-testid="doc-header-properties-btn"`）：点击展开 `DocPropertiesPopover`（若 `needsExternalFrontmatterConfirm(doc)` 为真且尚未确认，先弹出 `QnModalShell` 确认框“该外部文件当前不包含 YAML frontmatter，保存属性将在文件顶部写入最小 frontmatter，是否继续？”，用户确认后再打开/提交）；
        - 保留 `metaError` 警告徽章（`data-testid="editor-doc-meta-error"`）及非阻塞错误提示。
      - **`DocPropertiesPopover.vue`**（层级使用 `z-index: var(--qn-z-popover)`）：
        - 编辑单篇文档全部项目与状态字段：
          - `project`（文本输入 + 已有工作区快捷候选 datalist/select）
          - `openDir`（项目打开目录路径）
          - `codeWorkspace`（`.code-workspace` 文件路径）
          - `terminalDir`（终端工作目录路径）
          - `pinned`（置顶复选框）
          - `archived`（归档复选框）
        - 空字符串转为 `null`（符合 `NoteMetaPatch` 语义：`null` 删除该键），点击“保存属性”调用 `updateActiveMeta(patch)` 并关闭 Popover；按 `Esc` 或点击外部关闭。
  - **2. 收件箱“选中内容移到新笔记…”模态框（`src/features/editor/InboxExtractModal.vue`）**：
    - 展示待提取行范围（如 `inbox.md 第 L3–L6 行`）、新笔记标题输入框（默认自动取选中首行去除 `- HH:mm ` 前缀后的前 30 字）、目标目录输入/下拉（从 `notesStore.navTree.dirs` 获取）、模板下拉（加载自 `_templates/`，默认按目标目录上下文推导）；
    - 在 `InboxBloatBanner.vue` 或 `DocHeaderBar.vue`（收件箱有选区时显示“移到新笔记…”按钮 `data-testid="inbox-extract-btn"`）提供直接入口，同时在 `MainWorkbenchShell.vue` 中向 `actionRegistry.bindHandler('editor.moveSelectionToNewNote', ...)` 绑定打开该弹窗（供 `M2-05a` 右键菜单直接复用！）。
  - **3. 模板系统与新建笔记入口（主窗口 `⌘N` + 侧栏“从模板新建” + Quick Bar `⌥⌘↵` 另存新笔记，`v4` §4.10、§3.5.1）**：
    - 纯推导函数（放 `src/features/notes/templateUtils.ts` 并从 `src/features/notes/index.ts` 导出）：
      - `inferDefaultTemplateName(dirRelPath: string | null | undefined): 'note' | 'doc' | 'command-book'`：与 Rust `domain::template::infer_default_template` 100% 一致（`commands` / `commands/*` → `'command-book'`，`projects` / `projects/*` → `'doc'`，其余 → `'note'`）；
      - `deriveTitleFromContent(content: string): string`：提取首行非空文本，剥离前导 `# ` / `- ` / `- [ ] ` / `- [x] ` / `HH:mm `，截断至 30 字符，为空时回退 `'未命名笔记'`。
    - 新增 `src/features/notes/NewNoteTemplateModal.vue`：
      - 由侧栏“新建笔记 / 从模板新建”按钮与主窗口快捷键 `⌘N`（绑定 `actionRegistry.bindHandler('window.newNote', ...)`）打开；
      - 自动带入当前上下文目录（`notesStore.selectedDir` 或当前打开文档所在目录），并根据该目录调用 `inferDefaultTemplateName` 自动预选对应默认模板（用户可自由改选 `_templates/` 下的任意模板）；
      - 提交调用 `notesStore.createNoteAndOpen({ title, dirRelPath, templateName })`（升级 `notesStore.createNoteAndOpen` 支持 `templateName` 参数），建文后刷新树/列表并在新固定标签打开。
    - **Quick Bar `⌥⌘↵` 另存新笔记**（`src/features/quickbar/api.ts`、`store.ts`、`QuickBarView.vue`）：
      - `api.ts` 封装 `createQuickBarNote(args: CreateNoteArgs): Promise<NoteDoc>`（调用 `noteCreate`）与 `listQuickBarTemplates(): Promise<NoteSummary[]>`（调用 `notesList({ tab: 'templates', ... })`）；
      - `store.ts` 新增状态与方法：
        - `saveAsModalOpen: Ref<boolean>`、`saveAsTitle: Ref<string>`、`saveAsDir: Ref<string>`、`saveAsTemplate: Ref<string>`、`availableTemplates: Ref<string[]>`；
        - `openSaveAsNoteDialog()`：内容非空时打开轻量内联另存条，默认 `saveAsTitle = deriveTitleFromContent(draftText)`，默认目录按当前目标所在目录或空（库根），默认模板按目录推导，并异步拉取 `_templates/` 列表；
        - `confirmSaveAsNote(): Promise<NoteDoc | null>`：按当前 `captureMode` 格式化或保留 `draftText`，调用 `createQuickBarNote({ title, dirRelPath, templateName, body })`，成功后清空草稿（含 `localStorage`）、将新文档写入 `uiStateSet('lastActiveDoc', ...)`（方便主窗口切到该文档）、隐藏 Quick Bar（`hideQuickBarWindow({ reason: 'esc' })`）并调用 `showMainWindowForOnboarding()` 激活主窗口！
      - `QuickBarView.vue`：
        - `handleKeydown` 监听 `⌥⌘↵`（`event.metaKey && event.altKey && event.key === 'Enter'`）触发 `store.openSaveAsNoteDialog()`；
        - 底部操作栏新增 `⌥⌘↵ 另存新笔记` 按钮（`data-testid="quickbar-save-as-btn"`）与轻量内联另存表单（标题、目录、模板下拉、确认/取消按钮）。
- 不做：
  - 不实现 CM6 编辑器内右键菜单、上下文 Tab 缩进、复选框 Widget 或格式快捷键命令体（归 `M2-05a`）。
  - 不实现链接跳转与图片粘贴（归 `M2-05b`）。
  - 不在 Quick Bar 中引入复杂的独立系统窗口弹窗，另存表单直接内嵌在 Quick Bar 浮层内以保持零焦点丢失。

## 2. 允许修改文件白名单
- 共享串行文件：无
- 前端业务模块：
  - `src/features/editor/**`（新增 `DocHeaderBar.vue`、`DocPropertiesPopover.vue`、`InboxExtractModal.vue`；更新 `api.ts`、`store.ts`、`MarkdownEditor.vue`、`index.ts`）
  - `src/features/notes/**`（新增 `templateUtils.ts`、`NewNoteTemplateModal.vue`；更新 `api.ts`、`store.ts`、`NotesSidebar.vue`、`index.ts`）
  - `src/features/quickbar/**`（更新 `api.ts`、`store.ts`、`QuickBarView.vue`）
  - `src/app/MainWorkbenchShell.vue`
- 前端单元测试：
  - `src/features/editor/__tests__/doc-meta-and-inbox-extract.test.ts`（新增）
  - `src/features/quickbar/__tests__/quickbar-save-as-note.test.ts`（新增）

## 3. 具体实现要求
- **严格遵守 `D-0006`（`meta + body` 分离）**：调用 `updateActiveMeta` 返回新 `NoteDoc` 后，只更新 `tab.doc.meta` 与 `tab.doc.mtimeMs`，**严禁覆写编辑器内尚未保存的 `tab.draftBody`**。
- **跨 feature 依赖红线（`C1.2`）**：`editor` / `notes` / `quickbar` 之间仅通过顶层 `@/features/<name>`（`index.ts`）或纯工具导入，禁止深层路径导入；零 `@tauri-apps/api` 直连；零硬编码色值与魔法 `z-index`。
- **兼容既有测试**：确保 `notes-tree-list-and-tabs.test.ts`、`editor-autosave-and-conflict.test.ts`、`quickbar-capture-flow.test.ts` 全部零回归通过。

## 4. 自动化验收标准（必须全绿）
- 新增 `src/features/editor/__tests__/doc-meta-and-inbox-extract.test.ts`：
  1. 验证 `updateActiveMeta` 更新 `title`、`docType`、`tags` 及 `[属性]` 字段（`project`/`openDir`/`codeWorkspace`/`terminalDir`/`pinned`/`archived`）时调用 `noteUpdateMeta`，并且保留当前编辑器脏态 `draftBody` 不被覆盖。
  2. 验证 `needsExternalFrontmatterConfirm` 对外部无 frontmatter 文件返回 `true`、对库内文件或已有 frontmatter 外部文件返回 `false`。
  3. 验证 `extractInboxSelectionToNote` 先 `flushSave` 脏收件箱，再调用 `inboxExtractToNote`，随后刷新 `inbox.md` 正文（递增 `syncSeq`）并在新固定标签中打开新建的 `NoteDoc`。
- 新增 `src/features/quickbar/__tests__/quickbar-save-as-note.test.ts`：
  1. 验证 `inferDefaultTemplateName`（`commands/git` → `command-book`，`projects/quicknote` → `doc`，根目录 → `note`）与 `deriveTitleFromContent`。
  2. 验证 Quick Bar `openSaveAsNoteDialog` + `confirmSaveAsNote` 调用 `noteCreate`（携带所选模板与正文）、清空草稿、隐藏 Quick Bar 并激活主窗口。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] 在主窗口打开一篇笔记，顶栏直接修改标题、切换类型、添加 `#dev/m2` 标签并点击 `[属性]` 设置 `project`/`open_dir`/`pinned`：刷新后元数据保持，且原始 Markdown 正文及 YAML 中其他未知字段完整保留。
- [ ] 在 `inbox.md` 选中 3 行条目点击“移到新笔记…”，选择 `projects/` 目录与 `doc` 模板提交：新笔记自动打开并包含提取内容，`inbox.md` 中对应行已被剪切。
- [ ] 呼出 Quick Bar 输入内容后按 `⌥⌘↵` 选模板另存新笔记：草稿清空、浮层隐藏、主窗口自动打开该新笔记。
