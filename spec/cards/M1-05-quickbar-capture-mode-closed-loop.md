# M1-05：Quick Bar 记录模式完整闭环（`S1–S8` 顶部 Chip、`⌘T` 目标选择器、相关笔记改投、`⌘↵`/`⇧⌘↵` 与草稿恢复）

- 所属里程碑：M1
- 关联方案与契约：`v4` §1.2–§1.5、§3.5.1、§3.9（A/B/C）、§5.4、§6 M1；`C1.2`、`C2.2`、`C4.2`、`C4.3`、`C6.1`、`C6.3`、`C7.1`、`C7.2`、`C8.1`–`C8.3`、`C9.1`、`C9.3`、`D-0001`–`D-0003`、`D-0006`
- 前置依赖：`M1-04b`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、Store 状态机与跨窗口协同规则已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 已就绪的 `src/lib/ipc/` 接口与 `@/bindings/types` 类型（在 `src/features/quickbar/api.ts` 封装调用，严禁直连 `@tauri-apps/api`）**：
    - `captureResolveTarget(args: TargetResolveInput): Promise<ResolvedTarget>`
      - `TargetResolveInput { mainVisible: boolean, activeDocKey: string | null, selectedWorkspaceKey: string | null, isInboxView: boolean }`
      - `ResolvedTarget { scenario: string, kind: 'unconfigured' | 'note' | 'inbox', targetRelPath: string | null, targetTitle: string, sectionHint: string | null, workspaceKey: string | null, workspaceName: string | null, isCrossWorkspaceFallback: boolean, recommendedMode: CaptureMode, chipLabel: string }`（`CaptureMode = 'list' | 'todo' | 'prose'`）
    - `noteAppend(args: AppendNoteArgs): Promise<AppendResult>`
      - `AppendNoteArgs { targetRelPath: string, content: string, mode: CaptureMode, workspaceKey: string | null }`
      - `AppendResult { relPath: string, title: string, addedLines: number, sectionHeading: string | null, bodyHash: string, mtimeMs: number, undoAvailable: boolean }`
    - `searchNotesAndFiles(args: SearchQueryArgs): Promise<SearchResponse>`
      - `SearchQueryArgs { query: string, limit: number | null, requestId: number }`
      - `SearchResponse { requestId: number, items: SearchResultItem[], truncated: boolean }`（`SearchResultItem { kind, key, title, subtitle, relPath, absPath, score, source }`）
    - `notesGetTreeAndTags(): Promise<NavTreeSnapshot>` & `notesList(args: NotesFilterArgs): Promise<NoteSummary[]>`（供 `⌘T` 目标选择器加载最近文档、标签与工作区列表）
    - `uiStateGet(): Promise<UiState>` & `uiStateSet(args: { key: 'selectedWorkspace' | 'workspaceModes' | 'windows' | ..., value: unknown }): Promise<UiState>`
    - `windowShowMain(args?: MainWindowArgs): Promise<WindowState>` & `windowHideQuickBar(args: { reason: HideReason }): Promise<WindowState>`
    - 事件：`onWindowState(handler)`、`onToastAppendUndo(handler)`。
  - **2. 在 `src/features/quickbar/` 升级 `useQuickBarStore`、`QuickBarView.vue` 并新增 `QuickBarTargetPicker.vue`（`⌘T` 目标选择器）**：
    - **`S1–S8` 实时目标解析与顶部 Chip（`C8.1`）**：
      - `refreshState()` 时并发读取 `libraryGetInfo()`、`settingsGet()`、`uiStateGet()`（注意兼容既有测试：`uiStateGet` 若在旧测试 mock 中未返回或返回 `undefined`，安全回退默认空 `UiState`）；
      - 当 `libraryInfo.root !== null` 时，从 `uiState` 提取 `mainVisible = uiState.windows.main.isOpen`、`activeDocKey = uiState.tabs.activeKey`、`selectedWorkspaceKey = uiState.selectedWorkspaceKey`、`isInboxView = uiState.filters.activeTab === 'inbox'`，调用 `captureResolveTarget` 获取 `resolvedTarget: ResolvedTarget | null`（若 `captureResolveTarget` 在旧测试未 mock，安全回退默认 `{ scenario: 'S8', kind: 'inbox', targetRelPath: 'inbox.md', targetTitle: '收件箱', sectionHint: null, workspaceKey: null, workspaceName: null, isCrossWorkspaceFallback: false, recommendedMode: 'list', chipLabel: '收件箱 · inbox.md' }`）。
      - **有效目标优先级**：`explicitTarget`（用户通过 `⌘T` 目标选择器或 `⌘↑`/`⌘↓` 相关笔记显式选中的笔记 `{ relPath, title, workspaceKey }`） > `resolvedTarget`（`S1–S8` 默认目标）。
      - **顶部目标 Chip 渲染**：
        - 当 `isUnconfigured`（`S1`）：展示 `未选库 · S1` 警告徽标与 `data-testid="quickbar-s1-onboarding"` 引导卡（保持与 `M0-04` `onboarding-state.test.ts` 断言 100% 兼容！）。
        - 当存在 `explicitTarget`：展示 `{explicitTarget.title} › {modeSectionLabel}`（并带 `[× 恢复默认]` 或提示）。
        - 当使用 `resolvedTarget`：若 `resolvedTarget.isCrossWorkspaceFallback === true`，以醒目 warning 样式展示 `收件箱 · 跨工作区`（`data-testid="quickbar-target-chip"`）；否则展示 `resolvedTarget.chipLabel`；若 `resolvedTarget.sectionHint === '将追加到文末'`，显示“将追加到文末”辅助提示。
    - **内容模式切换与按工作区记忆（`⌘1` 列表 / `⌘2` 待办 / `⌘3` 正文，`v4` §1.3、`C6.3`）**：
      - 当前捕捉模式 `captureMode: Ref<CaptureMode>`：当目标解析完成且用户尚未在当前草稿会话手动切过模式时，同步为 `resolvedTarget.recommendedMode`（若目标为 `inbox.md` 固定默认 `'list'`）。
      - 用户按 `⌘1` / `⌘2` / `⌘3` 或点击底部模式按钮切换 `setCaptureMode(nextMode)` 时：立即更新 `captureMode = nextMode`；若当前有效工作区 `activeWorkspaceKey`（`explicitTarget?.workspaceKey ?? resolvedTarget?.workspaceKey ?? uiState.selectedWorkspaceKey`）非空且当前目标不是 `inbox.md`，同步更新 `uiState.workspaceModes[activeWorkspaceKey] = nextMode` 并调用 `uiStateSet({ key: 'workspaceModes', value: nextWorkspaceModes })` 持久化。
    - **`⌘T` 目标选择器弹层（`QuickBarTargetPicker.vue`）**：
      - 按 `⌘T` 或点击顶部目标 Chip 切换 `targetPickerOpen`；
      - 选择器内提供：
        1. **锁定工作区切换栏**：列出配置的工作区（来自 `settings.workspaces` 或 `NavTreeSnapshot.workspaces`）与 `[不锁定工作区]` 按钮；点击某个工作区或清除锁定时，调用 `selectWorkspaceLock(workspaceKey: string | null)` → `uiStateSet({ key: 'selectedWorkspace', value: workspaceKey })`，清除 `explicitTarget` 并重新调用 `captureResolveTarget` 刷新默认目标！
        2. **目标文档筛选与选择**：提供一键选择 `inbox.md（收件箱）`、按标签过滤 chips、以及搜索过滤输入框（`80ms` 防抖 + 递增 `requestId`），点击或回车选中某篇文档时设置 `explicitTarget = { relPath, title, workspaceKey }` 并关闭选择器，焦点交还输入框。
    - **相关笔记实时检索与 `⌘↑` / `⌘↓` 快速改投（`v4` §1.2、§3.5.1、`C2.2`）**：
      - 当 `draft.trim().length >= 2` 时，以 **`80ms` 防抖 + 递增 `relatedRequestId`** 调用 `searchNotesAndFiles({ query: draft.trim(), limit: 3, requestId })`；
      - 收到响应时**严格校验 `res.requestId === relatedRequestId`**，过期响应直接丢弃；过滤出前 3 条库内笔记存入 `relatedNotes: SearchResultItem[]`（当 `draft.trim().length < 2` 时清空 `relatedNotes` 与 `selectedRelatedIndex = -1`）。
      - 按 `⌘↓` / `⌘↑`（`Meta+ArrowDown` / `Meta+ArrowUp`）或点击底部相关笔记项：在 `-1`（不选，使用默认目标）与 `0..relatedNotes.length - 1` 之间循环切换 `selectedRelatedIndex`；当 `selectedRelatedIndex >= 0` 时，自动将该相关笔记设为当前改投目标 `explicitTarget`（显式改投不受工作区过滤限制）；切回 `-1` 时清除相关笔记改投恢复 `resolvedTarget`。
    - **草稿实时保存与恢复（`v4` §1.5、`C6.1`）**：
      - 输入草稿、切换 `captureMode` 或 `explicitTarget` 时，同步保存到本地持久化存储（`localStorage` key `'quicknote:quickbar-draft-v1'` 结构 `{ mode: CaptureMode, targetRelPath: string | null, selectedWorkspaceKey: string | null, text: string, updatedAt: number }`）；
      - 当按 `Esc`、`⌃⌥Space` 或切 Space（收到 `window:state` `windowId === 'quickbar' && !visible && reason === 'space_changed'`）隐藏浮层时，**完整保留草稿**；下次呼出 `initialize()` / `refreshState()` 时自动无损恢复未提交的草稿文本与模式；
      - 仅当提交成功（`⌘↵` 或 `⇧⌘↵`）或用户显式点击 `[清空]` 时，才清除草稿（后端 `note_append` 也会同步清理 `$APP_DATA_DIR/quickbar-draft.json`）。
    - **分层 `Esc` 回退纪律**：
      - 若 `targetPickerOpen === true`：按 `Esc` 仅关闭目标选择器（`targetPickerOpen = false`）并聚焦输入框，**不隐藏 Quick Bar 窗口**；
      - 若 `mode !== 'capture'`（正处于 `open` 或 `command` 占位视图）：按 `Esc` 先切回 `mode = 'capture'` 并聚焦输入框，**不隐藏 Quick Bar 窗口**；
      - 若 `mode === 'capture'` 且 `targetPickerOpen === false`：按 `Esc` 调用 `hide('esc')` 隐藏 Quick Bar 窗口并保留草稿。
    - **双提交动作与主窗口升级协同（`⌘↵` vs `⇧⌘↵`，`v4` §1.4–§1.5）**：
      - **`⌘↵`（`submitAppend(false)`：追加并关闭）**：
        1. 校验 `canSubmit`（非 `S1` 且 `draft.trim().length > 0`）；
        2. 确定目标 `targetRelPath = explicitTarget?.relPath ?? resolvedTarget?.targetRelPath ?? 'inbox.md'`；
        3. 调用 `noteAppend({ targetRelPath, content: draft, mode: captureMode, workspaceKey })`；
        4. 成功后：清空 `draft`、清除持久化草稿、重置 `explicitTarget = null`、`selectedRelatedIndex = -1`、`relatedNotes = []`；
        5. 调用 `windowHideQuickBar({ reason: 'esc' })` 关闭浮层（后端 `note_append` 自动弹出 5 秒非激活 Toast 并广播 `toast:append-undo`）。
      - **`⇧⌘↵`（`submitAppend(true)`：追加并打开主窗口定位末尾 + 展开大纲）**：
        1. 执行与上面相同的 `noteAppend` 写入并清空草稿；
        2. 写入跨窗口升级定位标记（或通过 `uiStateSet` / 事件）：调用 `uiStateSet({ key: 'panelGroup', value: { visible: true, activeId: 'outline', width: 260, collapsed: false } })` 确保大纲面板展开；
        3. 调用 `windowHideQuickBar({ reason: 'esc' })` 并调用 `windowShowMain({ mode: null })` 唤醒主窗口（触发 `.accessory → .regular`）。
  - **3. 主窗口联动刷新与 `⇧⌘↵` / Toast 撤销协同（`src/features/editor/store.ts`、`src/app/MainWorkbenchShell.vue`）**：
    - 在主窗口 `useEditorTabsStore`（或 `MainWorkbenchShell.vue`）中：
      - 监听 `onToastAppendUndo(async (appendResult) => ...)`：当 Quick Bar 追加成功时，刷新左侧笔记列表与导航树计数（`notesStore.refreshAll()`）；若 `appendResult.relPath` 已经在主窗口标签页中打开（且非脏态），自动调用 `readNote` 重新加载最新正文与 `bodyHash`，并将光标定位到最新正文末尾（`cursorPos = doc.body.length`，更新大纲）；
      - 监听 `onWindowState(async (winState) => ...)`：当 `winState.windowId === 'main' && winState.visible` 时，读取 `uiStateGet()` 检查 `lastActiveDoc` 与 `panelGroup`：同步 `panelsStore` 展开状态，若 `lastActiveDoc.relPath` 刚被 Quick Bar 更新（尚未在当前激活标签打开或内容已更新），自动打开/激活该文档标签页并将编辑器光标定位至文档末尾（`doc.body.length`）。
- 不做：
  - 不实现 `⌥⌘↵` 另存为独立新笔记的模板弹窗（归 `M2`）。
  - 不实现 Quick Bar 打开模式（`⌘K`）与命令模式（`⌘⇧K`）的具体搜索业务（归 `M3`；切到 `open`/`command` 时展示清晰的功能占位提示，按 `Esc` 退回 `capture` 模式）。

## 2. 允许修改文件白名单
- 前端 Quick Bar 模块：`src/features/quickbar/**`
- 前端主窗口协同：`src/features/editor/**`、`src/features/notes/**`、`src/features/panels/**`（仅限同步 `panelGroup` 展开状态的最小调整）、`src/app/MainWorkbenchShell.vue`
- 前端单元测试：`src/features/quickbar/__tests__/**`、`src/features/onboarding/__tests__/onboarding-state.test.ts`（仅当需补充新命令 mock 时微调，必须保持原有 S1 测试 100% 通过）

## 3. 具体实现要求
- **严格遵守前端分层与 Token 规范（`C1.2`、`C7.1`）**：
  - 零 `@tauri-apps/api` 直连；跨 feature 只经顶层 `@/features/<name>`（`index.ts`）导入；
  - 所有新增组件与样式 100% 使用 `var(--qn-*)` CSS Token，严禁出现任何 `#xxx` 裸十六进制色值或硬编码数字 `z-index`；
  - TypeScript 严格遵守 `"strict": true` 与 `"noUncheckedIndexedAccess": true`，禁止使用 `any`。
- **保持既有 `onboarding-state.test.ts` 兼容**：`useQuickBarStore.initialize()` 在 `libraryInfo.root === null` 时无需调用 `captureResolveTarget`，确保 `onboarding-state.test.ts` 原有 S1 测试零报错通过。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src/features/quickbar/__tests__/quickbar-capture-flow.test.ts`
  1. **`S1–S8` 目标解析与顶部 Chip 渲染**：验证 `captureResolveTarget` 返回常规文档、`isCrossWorkspaceFallback: true`（渲染 `收件箱 · 跨工作区`）及 `sectionHint: '将追加到文末'` 时顶部 Chip 与提示准确渲染。
  2. **`⌘1` / `⌘2` / `⌘3` 模式切换与按工作区记忆**：验证切换 `list` / `todo` / `prose` 时更新当前模式，并在有 `workspaceKey`（且非 `inbox.md`）时调用 `uiStateSet({ key: 'workspaceModes', ... })` 持久化。
  3. **相关笔记 `>= 2` 字符 `80ms` 防抖、防竞态与 `⌘↑`/`⌘↓` 改投**：使用 `vi.useFakeTimers()` 验证输入 `< 2` 字符不触发搜索、`>= 2` 字符 `80ms` 防抖触发 `searchNotesAndFiles(limit=3)`、旧 `requestId` 晚返回时被丢弃，以及 `⌘↓`/`⌘↑` 选中相关笔记后提交目标自动改投到该相关笔记。
  4. **`⌘T` 目标选择器、工作区锁定与 `Esc` 分层回退**：验证打开选择器、切换/清除锁定工作区（调用 `uiStateSet({ key: 'selectedWorkspace', ... })` 并重跑 `captureResolveTarget`）、显式选文档；验证 `Esc` 在选择器打开时先关选择器、在 `open`/`command` 模式先退回 `capture` 模式、在 `capture` 主视图才调用 `windowHideQuickBar` 且保留草稿。
  5. **草稿恢复与 `⌘↵` / `⇧⌘↵` 提交闭环**：验证 `Esc` 或 `space_changed` 隐藏后重新初始化可恢复草稿；验证 `⌘↵` 调用 `noteAppend` → 清空草稿 → 调用 `windowHideQuickBar`；验证 `⇧⌘↵` 调用 `noteAppend` → 展开大纲 `panelGroup` → 调用 `windowHideQuickBar` + `windowShowMain`。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（M1 里程碑端到端实机点验 `C9.3`）
- [ ] **全屏速记与 5 秒非激活撤销**：在其他全屏 App 上按 `⌃⌥Space` 呼出 Quick Bar，输入两行想法按 `⌘↵`：浮层立即关闭、不切 Space、原全屏 App 焦点不丢失；屏幕底部弹出 5 秒非激活 Toast 显示 `已追加至「...」 · [撤销]`，点击 `[撤销]` 后文件精确回滚至追加前状态且原全屏 App 焦点依旧保持。
- [ ] **跨工作区保护与 `⌘T` 锁区**：在 `⌘T` 中锁定工作区 B（而最近活跃文档属于工作区 A），确认顶部 Chip 立即切换为 `收件箱 · 跨工作区`，不会误追加到 A 的文档。
- [ ] **切 Space 草稿保留与 `⇧⌘↵` 升级主工作台**：在 Quick Bar 输入文字后切换 macOS Space，确认浮层自动复位隐藏；按 `⌃⌥Space` 重新呼出确认草稿完好无损；按 `⇧⌘↵` 提交，确认主窗口被激活（Dock 图标与系统菜单栏切为 `.regular`）、目标文档自动打开、光标定位在刚追加内容末尾且右侧大纲面板已展开。
