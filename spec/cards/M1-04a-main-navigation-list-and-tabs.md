# M1-04a：主界面第一层（导航树 / 层级标签树 / 笔记列表 / 重命名 / 收件箱 500 行膨胀提醒）与多标签状态栏

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.2–§3.4、§3.9（F/H）、§4.6、§4.11、§5.9、§6 M1；`C1.2`、`C2.2`、`C4.2`、`C4.3`、`C6.2`、`C6.3`、`C7.1`、`C9.1`、`D-0006`、`D-0007`、`D-0009`
- 前置依赖：`M1-03`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、Store 状态机与 Token 约束已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 已就绪的 `src/lib/ipc/` 接口与 `@/bindings/types` 类型（直接使用，严禁手改 `src/bindings/types.ts` 或在组件直连 `@tauri-apps/api`）**：
    - `notesGetTreeAndTags(): Promise<NavTreeSnapshot>` → `{ inboxCount: number, dirs: DirNode[], tags: TagNode[], workspaces: WorkspaceSummary[] }`（`DirNode { relPath, name, count, children }`、`TagNode { tag, count, children }`、`WorkspaceSummary { key, name, root, openDir }`）
    - `notesList(args: NotesFilterArgs): Promise<NoteSummary[]>` → `NotesFilterArgs { tab: string, dirRelPath: string | null, tags: string[], workspaceKey: string | null, filterText: string | null }`；`NoteSummary { key, relPath, absPath, title, docType, tags, project, excerpt, pinned, archived, isInbox, mtimeMs, metaError }`
    - `noteRead(args: ReadNoteArgs): Promise<NoteDoc>` → `NoteDoc { key, relPath, absPath, isLibrary, isInbox, meta: NoteMeta, body, bodyHash, mtimeMs, sizeBytes }`
    - `noteCreate(args: CreateNoteArgs): Promise<NoteDoc>` → `CreateNoteArgs { title, dirRelPath, templateName: string | null, body: string | null }`
    - `noteRename(args: RenameNoteArgs): Promise<NoteDoc>` → `RenameNoteArgs { relPath, newRelPath }`
    - `noteDelete(args: RelPathArgs): Promise<void>` → `RelPathArgs { relPath }`
    - `uiStateGet(): Promise<UiState>` & `uiStateSet(args: { key: UiStatePatchKey, value: unknown }): Promise<UiState>`（`UiStatePatchKey` 含 `'tabs' | 'filters' | 'selectedWorkspace' | 'lastActiveDoc'` 等；`TabsUiState { items: TabItemState[], activeKey: string | null, closedStack: string[] }`，其中 `TabItemState { key, relPath, absPath, isPinned, isPreview, scrollTop, cursorPos }`；`FiltersUiState { activeTab: 'inbox' | 'projects' | 'tags' | 'recent' | 'workspaces', selectedDir: string | null, selectedTags: string[], listScrollTop: number }`）
    - 事件监听：`onIndexStatus(handler)`、`onFsExternalChange(handler)`、`onToastAppendUndo(handler)`。
  - **2. 在 `src/features/notes/` 落地主界面第一层（导航 + 列表，`api.ts` / `store.ts` / `NotesSidebar.vue` / `index.ts`）**：
    - 顶部 5 个导航分段切换（`activeTab: 'inbox' | 'projects' | 'tags' | 'recent' | 'workspaces'`）：
      - `inbox`：展示 `inboxCount` 徽标，固定筛选收件箱，或点击直接打开 `inbox.md`。
      - `projects`：渲染可展开/折叠的目录树 `DirNode`（支持选定 `selectedDir` 过滤子目录，再次点击或点“全部项目”清除目录过滤）。
      - `tags`：渲染层级标签树 `TagNode`（如 `dev` 下含 `dev/quicknote`），支持**点击多选/取消标签（AND 组合筛选）**，顶部展示已选标签筛选 chips 并支持一键清除。
      - `recent`：按 `mtimeMs` 降序展示最近笔记列表。
      - `workspaces`：展示 `WorkspaceSummary` 列表，选中某个工作区时筛选 `project == workspaceKey` 的文档，并调用 `uiStateSet({ key: 'selectedWorkspace', value: workspaceKey })` 同步锁定工作区（支持清除选择为 `null`）。
    - 搜索过滤与防竞态（`C2.2`）：
      - 列表上方提供过滤输入框（当焦点位于左侧导航/列表区域时，按 `⌘F` 聚焦该过滤框）；
      - 输入变化时以 **`80ms` 防抖 + 递增 `requestSeq` 版本号**调用 `notesList`，**过期响应必须丢弃**，防止乱序覆盖最新列表。
    - 列表卡片与 CRUD 操作：
      - 每条 `NoteSummary` 展示：`title`、`excerpt`、`tags`、置顶标识 `pinned`、格式化时间、`metaError` 异常徽标（当 YAML 损坏或重复 `id` 时醒目提示）。
      - `[+ 新建]`：在当前选定目录（默认根目录 `""` 或当前 `selectedDir`）调用 `noteCreate` 创建新笔记并在右侧以固定标签打开。
      - **内联重命名（`note_rename`）**：支持在列表项触发重命名（保持所在目录前缀不变，修改文件名 `<newName>.md`，`inbox.md` 禁止重命名）；成功后刷新列表与导航树，并调用 `editorTabsStore.handleNoteRenamed(oldRelPath, updatedDoc)` 同步更新已打开标签页的 `relPath` / `absPath` / `title`。
      - **删除笔记（`note_delete`）**：支持删除非 `inbox.md` 笔记，删除后同步关闭对应标签页并刷新列表。
  - **3. 在 `src/features/editor/` 落地多标签页状态机（`api.ts` / `store.ts` / `EditorTabBar.vue` / `index.ts`）與 `ui-state.json` 恢复**：
    - 标签项运行时结构 `EditorTab`：包含 `TabItemState` 字段（`key, relPath, absPath, isPinned, isPreview, scrollTop, cursorPos`）及已加载的 `doc: NoteDoc | null`、`isDirty: boolean`、`isMissing: boolean`（搬库或磁盘失联占位标志）。
    - **预览标签（斜体）vs 固定标签状态机**：
      - 全局最多只允许存在 **1 个** `isPreview === true` 的未固定预览标签；
      - **单击**左侧笔记列表项：若该笔记已打开则直接激活该标签；若未打开，则复用/替换当前唯一的预览标签（`isPreview: true, isPinned: false`）加载新 `NoteDoc`；
      - **双击**左侧笔记列表项、双击标签栏上的预览标签头、或调用 `promoteTabToPinned(key)`（供编辑变更时调用）：立即将该标签转为固定标签（`isPreview: false, isPinned: true`）。
    - **标签操作与快捷键**：
      - 点击标签 `×` 或鼠标中键关闭标签：将该标签的 `absPath` 推入 `closedStack`（最多保留 20 条，去重）；若关闭的是当前激活标签，自动激活相邻标签。
      - 支持“关闭其他标签”；支持 `⌘1…9` 切换到第 1~9 个打开的标签页；支持 `⌘⇧T` 从 `closedStack` 栈顶弹出最近关闭的 `absPath`，调用 `noteRead` 重新作为固定标签打开。
      - 每次激活或打开库内文档时，同步调用 `uiStateSet({ key: 'lastActiveDoc', value: { relPath, workspaceKey: doc.meta.project ?? null, touchedAtMs: Date.now() } })`，并防抖保存 `tabs` 与 `filters` 到 `ui-state.json`。
    - **启动与搬库状态恢复（`v4` §5.9、`C6.3`）**：
      - 初始化（或 `librarySetRoot` 搬库完成）时读取 `uiStateGet()` 与全库 `notesList`：
      - 遍历 `uiState.tabs.items`：优先按 `relPath` 在当前库中查找匹配笔记；若 `relPath` 找不到（例如文件被重命名或移动），再按 `tab.key`（ULID）在当前库中兜底匹配并自动修正为最新 `relPath` 与 `absPath`；若彻底找不到且磁盘 `noteRead` 返回 `notFound`，保留该标签并标记 `isMissing = true`（界面显示“文件已移走或失联”占位提示与关闭按钮，不直接崩溃或静默丢弃）。
      - 若恢复后 `items` 为空，默认打开 `inbox.md`。
  - **4. 收件箱 500 行被动膨胀提醒与 `MainWorkbenchShell.vue` 装配**：
    - 当当前激活标签文档满足 `doc.isInbox === true`（或 `relPath === 'inbox.md'`）且其正文行数 `doc.body.split('\n').length > (settingsStore.settings?.inbox.bloatThresholdLines ?? 500)` 时，在编辑区顶部展示温和非阻塞 `QnBanner`（`tone="warning"`，`data-testid="inbox-bloat-banner"`）：“收件箱已超过 500 行，建议整理”。
    - 更新 `src/app/MainWorkbenchShell.vue`（及 `src/app/App.vue` 初始化钩子）：将左侧骨架替换为真实 `NotesSidebar`，右侧顶部替换为真实 `EditorTabBar`、文档头（展示 `doc.meta.title`、`doc.meta.docType`、`doc.meta.tags`、`doc.meta.metaError`）与当前 `doc.body` 内容舞台（为 `M1-04b` 挂载 CM6 实例留好干净容器）。
- 不做：
  - 不在本卡安装或挂载 CodeMirror 6 编辑器实例、600ms 自动保存、三态冲突横幅或右侧大纲提取（全部归 `M1-04b`）。

## 2. 允许修改文件白名单
- 前端工具与组件：`src/lib/utils/**`、`src/components/**`
- 前端业务模块：`src/features/notes/**`、`src/features/editor/**`、`src/features/onboarding/store.ts`（如需在完成选库后触发笔记与标签初始化）、`src/app/MainWorkbenchShell.vue`、`src/app/App.vue`、`src/app/bootstrapStore.ts`
- 前端单元测试：`src/features/notes/__tests__/**`

## 3. 具体实现要求
- **严格遵守前端单向分层与 Token 规范（`C1.2`、`C7.1`）**：
  - 只有 `src/lib/ipc/**` 可以导入 `@tauri-apps/api`；`src/features/notes/` 与 `src/features/editor/` 只通过各自的 `api.ts` 调用 `@/lib/ipc/commands` 与 `@/lib/ipc/events`。
  - 跨 feature 导入只允许通过顶层 `@/features/<name>`（即 `index.ts`），严禁 `@/features/editor/store` 这类深层跨模块导入（`scripts/check-architecture.mjs` 会静态拦截）。
  - 所有 `.vue` 与 `.css` 严禁出现任何裸十六进制色值（`#xxx`）与硬编码数字 `z-index`，100% 使用 `var(--qn-*)` Token。
- **TypeScript 严格模式**：遵守 `"strict": true` 与 `"noUncheckedIndexedAccess": true`，禁止使用 `any`。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src/features/notes/__tests__/notes-tree-list-and-tabs.test.ts`
  1. **导航、多标签 AND 与 80ms 防抖防竞态**：验证切换 `inbox/projects/tags/recent/workspaces`、多选标签 AND 过滤、搜索框 `80ms` 防抖且当先发请求比后发请求晚返回时自动丢弃过期响应。
  2. **预览标签 vs 固定标签与 `⌘⇧T`**：验证单击列表项复用同一斜体预览标签（`isPreview: true`），双击晋升为固定标签（`isPreview: false, isPinned: true`），关闭标签推入 `closedStack` 并可通过 `reopenLastClosedTab()`（`⌘⇧T`）恢复。
  3. **重命名联动与搬库/启动恢复**：验证 `renameNote` 成功后已打开标签页的 `relPath` / `absPath` / `title` 同步更新；验证 `restoreTabsFromUiState` 在 `relPath` 变更时能用 `key`（ULID）兜底重定位，彻底不存在时标记 `isMissing: true`。
  4. **收件箱 500 行膨胀提醒**：验证打开 `inbox.md` 且正文超过 `bloatThresholdLines`（500 行）时 `isInboxBloated` 为 `true` 并渲染提醒横幅，`<= 500` 行时不显示。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡完成后可在主窗口查看左侧导航树、标签树与多标签切换；完整 CM6 编辑器与大纲实机点验在 `M1-04b` 一并执行。
