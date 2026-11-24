# M4-03：CM6 MergeView 冲突并排对比与逐块合并写回

- 所属里程碑：M4
- 关联方案与契约：`v4` §5.5、§5.7、§5.14、§6 M4；`C1.2`、`C1.3`、`C2.2`、`C3.4`、`C7.1`、`C8.5`、`C9.1`、`D-0006`
- 前置依赖：`M4-02` 已合并至 `dev`
- 建议分支：`feat/M4-03-cm6-merge-view`
- 会话策略：独立新会话执行（零大文档读取，所有状态流转、冲突基线刷新、写回契约与白名单已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **安装 CM6 合并依赖（`C1.3` 串行声明）**：
   - 在 `package.json` 新增 `@codemirror/merge`（选择与现有 `@codemirror/state ^6.7.6` / `@codemirror/view ^6.43.15` 兼容的 `^6.x` 版本，更新 `pnpm-lock.yaml`）。
2. **扩展编辑器标签冲突状态与合并动作（`src/features/editor/store.ts`）**：
   - 在 `EditorConflictState` 的 `kind: 'modified'` 分支增加 `mergeMode?: boolean`（与 `mergeDraftBody?: string`、`mergeSaving?: boolean`、`mergeError?: string | null`）：
     ```typescript
     export type EditorConflictState =
       | {
           kind: 'modified';
           diskBody: string;
           diskBodyHash: string;
           diskMeta: NoteMeta | null;
           bannerDismissed?: boolean;
           mergeMode?: boolean;
         }
       | {
           kind: 'deleted';
         };
     ```
   - 新增四个明确动作方法：
     - `enterConflictMerge(key: string): void`：要求该标签 `conflictState?.kind === 'modified'`；设置 `mergeMode = true`、`bannerDismissed = false`，保持自动保存暂停状态。
     - `cancelConflictMerge(key: string): void`：退出 `mergeMode = false`，保留当前 `draftBody` 与冲突横幅，用户可继续编辑或重新选择解决方式。
     - `updateMergeDraftBody(key: string, nextBody: string): void`：同步用户在右栏合并编辑器中的实时修改至 `tab.draftBody`。
     - `commitConflictMerge(key: string, mergedBody: string): Promise<boolean>`：
       - 以当前 `conflictState.diskBodyHash` 作为 `baseBodyHash` 调用 `note_write({ absPath, body: mergedBody, baseBodyHash: conflictState.diskBodyHash })`（严格遵守 `D-0006` 乐观并发与 `meta + body` 分离，不接触 frontmatter）；
       - **成功时**：更新 `tab.doc.body = mergedBody`、`tab.draftBody = mergedBody`、`tab.baseBodyHash = result.bodyHash`、`tab.doc.bodyHash = result.bodyHash`、`tab.doc.mtimeMs = result.mtimeMs`（若返回 `updatedMeta` 同步更新 `tab.doc.meta`）、清空 `conflictState = null`、`isDirty = false`、`saveStatus = 'saved'`、递增 `syncSeq` 使切回 `MarkdownEditor` 时立即同步最新文档并恢复自动保存；
       - **若写回期间磁盘再次被外部修改（返回 `ErrorCode::Conflict`）**：解析 `ConflictInfo`，将最新 `currentBody` / `currentBodyHash` 刷新到 `conflictState.diskBody` / `diskBodyHash`，保留用户右栏 `mergedBody` 不丢弃，并提示“合并保存期间磁盘又有新修改，已刷新左栏基线，请复核后再次完成合并”。
   - 当处于 `mergeMode === true` 且收到新的 `fs:external-change(kind = 'modified')` 时：自动更新 `conflictState.diskBody`、`diskBodyHash`（与 `diskMeta`），不退出 `mergeMode`，也不覆盖右栏用户未提交合并内容。
3. **升级冲突横幅为完整四选一（`src/features/editor/EditorConflictBanner.vue`）**：
   - 在 `kind === 'modified'` 横幅的 `#actions` 槽位新增“并排对比”按钮（`data-testid="conflict-merge-btn"`，位于“备份并覆盖磁盘”与“暂缓处理”之间），点击调用 `editorTabsStore.enterConflictMerge(activeTab.key)`；
   - 当当前标签已进入 `mergeMode === true` 时隐藏常规冲突横幅（避免与 `EditorMergeView` 顶部工具栏重复占位）。
4. **实现 `src/features/editor/EditorMergeView.vue`**：
   - 使用 `@codemirror/merge` 的 `MergeView`：
     - **左栏（`a`）**：只读展示磁盘最新正文（`doc: diskBody`，挂载 `EditorState.readOnly.of(true)`、`EditorView.editable.of(false)` 与对应语言高亮扩展）；
     - **右栏（`b`）**：可编辑当前未保存本地正文（`doc: draftBody`，挂载标准编辑器按键、换行、语言高亮与 `EditorView.updateListener` 同步 `mergedBody`）；
     - 开启逐块合并控件：`revertControls: 'a-to-b'`（允许点击每处差异块上的箭头一键将磁盘变更采纳到右侧，亦支持右侧自由手工编辑与 `⌘Z` 撤销）；
     - 启用 `highlightChanges: true`、`gutter: true`、`collapseUnchanged: { margin: 3, minSize: 4 }`（大文件 diff 更清晰）。
   - **顶部合并工具栏**：
     - 明确双栏标识：左侧徽标“磁盘最新版本（只读）” / 右侧徽标“本地未保存版本（可编辑）”；
     - 操作按钮：
       - `全部采用磁盘`（`data-testid="merge-accept-all-disk-btn"`）：将右栏全文替换为左栏磁盘正文（可 `⌘Z` 撤销）；
       - `取消返回`（`data-testid="merge-cancel-btn"`）：调用 `cancelConflictMerge` 返回常规编辑器；
       - `完成合并并保存`（`data-testid="merge-commit-btn"`，primary）：调用 `commitConflictMerge(tab.key, currentMergedBody)`。
   - 当左栏 `diskBody` Prop 因外部再次写入而更新时，平滑同步 `mergeView.a` 的文档内容而不重置 `mergeView.b` 光标与用户合并进度；组件卸载时务必调用 `mergeView.destroy()` 防内存泄漏。
   - **样式与双主题适配（`C7.1`）**：全部背景、差异高亮（插入 `--qn-success-soft`、删除 `--qn-danger-soft`、边框 `--qn-border-subtle`）、工具栏和文字色均使用 `--qn-*` CSS Token，严禁硬编码色值。
5. **接入主编辑工作区（`src/app/MainWorkbenchShell.vue` 与 `src/features/editor/index.ts`）**：
   - 在 `src/features/editor/index.ts` 导出 `EditorMergeView`；
   - 在 `MainWorkbenchShell.vue` 编辑舞台区域：当 `activeTab?.conflictState?.kind === 'modified' && activeTab.conflictState.mergeMode` 时渲染 `<EditorMergeView />`，否则渲染 `<MarkdownEditor />`。

### 明确不做
- 不在主编辑区提供常驻分屏预览（遵循 `v4` §5.5）。
- 不修改后端 `note_write` 或 `fs:external-change` 逻辑。

## 2. 允许修改文件白名单

### 共享串行文件（`C1.3` 显式声明）
- `package.json`、`pnpm-lock.yaml`

### 业务与测试文件
- `src/features/editor/EditorMergeView.vue`（新增）
- `src/features/editor/EditorConflictBanner.vue`
- `src/features/editor/store.ts`
- `src/features/editor/index.ts`
- `src/app/MainWorkbenchShell.vue`
- `src/features/editor/__tests__/merge-view-conflict.test.ts`（新增）
- `src/features/editor/__tests__/editor-autosave-and-conflict.test.ts`

## 3. 具体实现要求

1. **多标签切换安全**：
   - 仅当前处于冲突并排态的标签展示 `EditorMergeView`；切到另一个正常标签时立刻展示常规 `MarkdownEditor`，切回冲突标签时保留其未完成的合并草稿与 `mergeMode`。
2. **语言与排版一致性**：
   - `EditorMergeView` 复用法定 `resolveLanguageExtension`（来自 `extensions/codeLanguages.ts`）和设置中的 `fontSize` / `lineHeight`，Markdown 与非 Markdown 文件冲突均可清晰对比。
3. **代码质量**：严格 TypeScript 零 `any`，严禁在 `console.*` 打印冲突正文内容（`C2.3`）。

## 4. 自动化验收标准（必须全绿）

- 新增测试 `src/features/editor/__tests__/merge-view-conflict.test.ts`，至少覆盖：
  1. 脏态外部修改冲突横幅呈现完整四按钮（重新加载 / 备份并覆盖 / 并排对比 / 暂缓处理）；
  2. 点击“并排对比”进入 `mergeMode`，`EditorMergeView` 成功初始化 `@codemirror/merge` 双栏（左只读磁盘正文、右可编辑本地正文）；
  3. 在右栏应用磁盘块或编辑合并文本后点击“完成合并并保存”，以最新 `diskBodyHash` 调用 `note_write`，写回成功后自动退出 `mergeMode`、清空冲突并恢复 `saved` 态；
  4. 合并写回期间遭遇二次并发冲突（`Conflict` 错误）时，自动刷新左栏 `diskBody`/`diskBodyHash`、保留右栏用户编辑内容并展示二次冲突提示；
  5. 点击“取消返回”退出 `mergeMode` 且不丢失本地草稿。
- 必过命令：`pnpm check:all`。

## 5. 人工点验清单

- [ ] 在主窗口编辑某笔记未保存，外部编辑器修改同文件触发冲突横幅，点击“并排对比”验证双栏高亮、逐块采纳按钮与深浅主题显示清晰，点击“完成合并并保存”后切回编辑器且磁盘内容准确落盘。
