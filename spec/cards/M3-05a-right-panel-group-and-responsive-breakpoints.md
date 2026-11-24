# M3-05a：完整右侧面板组（大纲 / 终端占位 / 相关文档 / 最近编辑）、宽度持久化与响应式断点

- 所属里程碑：M3
- 关联方案与契约：`v4` §3.2、§3.4、§5.6.6、§6 M3；`C1.2`、`C2.2`、`C4.2`、`C6.3`、`C7.1`–`C7.4`、`C8.6`、`C9.1`
- 前置依赖：`M3-04` 已合并至 `dev`
- 建议分支：`feat/M3-05a-right-panel-group-and-responsive`
- 会话策略：独立新会话执行（零大文档读取，所有面板注册表规范、断点数值与 IPC 签名已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **右侧面板统一注册表（`src/features/panels/registry.ts`，严格执行 `C7.4` 与 `v4` §5.6.6）**：
   - 定义注册项类型 `RightPanelDef`：
     ```ts
     export interface RightPanelDef {
       id: PanelId; // 'outline' | 'terminal' | 'related' | 'recent'
       title: string;
       shortLabel: string;
       order: number;
       defaultVisible: boolean;
       defaultWidth: number;
       minWidth: number;
       shortcut: string;
       actionId: string;
     }
     ```
   - 严格按 `C7.4` 顺序与默认值注册全部 4 个内置面板：
     1. `outline`：标题 `大纲`，`order: 1`，`defaultVisible: true`，`defaultWidth: 260`，`minWidth: 220`，`shortcut: 'ctrl+cmd+o'`，`actionId: 'panel.toggle.outline'`
     2. `terminal`：标题 `终端`，`order: 2`，`defaultVisible: false`，`defaultWidth: 360`，`minWidth: 220`，`shortcut: 'ctrl+cmd+t'`，`actionId: 'panel.toggle.terminal'`
     3. `related`：标题 `相关文档`，`order: 3`，`defaultVisible: false`，`defaultWidth: 260`，`minWidth: 220`，`shortcut: 'ctrl+cmd+r'`，`actionId: 'panel.toggle.related'`
     4. `recent`：标题 `最近编辑`，`order: 4`，`defaultVisible: false`，`defaultWidth: 240`，`minWidth: 220`，`shortcut: 'ctrl+cmd+e'`，`actionId: 'panel.toggle.recent'`
   - 导出常量：`MIN_PANEL_WIDTH = 220`、`MIN_EDITOR_WIDTH = 480`、`BP_COLLAPSE_RIGHT_PANEL = 1200`、`BP_COLLAPSE_LEFT_SIDEBAR = 1000`、`BP_OVERLAY_DRAWER = 900`。

2. **扩展 `src/features/panels/store.ts` 与 `api.ts`**：
   - 新增 `src/features/panels/api.ts` 封装：
     - `fetchRelatedNotes(relPath: string): Promise<RelatedNoteItem[]>`（调用 `notesGetRelated({ relPath })`）
     - `fetchRecentNotes(): Promise<NoteSummary[]>`（调用 `notesList({ tab: 'recent', dirRelPath: null, tags: [], workspaceKey: null, filterText: null })` 并截取前 **20** 条）
     - `fetchUiState()` / `patchPanelGroupUiState(value: PanelGroupUiState)`
   - 在 `usePanelsStore` 中扩展：
     - 通用切换方法 `togglePanel(id: PanelId)` 与 `selectPanel(id: PanelId)`：
       - 若当前面板组隐藏/折叠或激活项不是 `id`：展开并切换 `activeId = id`；若已展开且当前即 `id`：折叠/隐藏面板组；
       - 切到 `related` 或当前活跃文档切换（且 `related` 可见）时自动拉取 `notes_get_related`（带递增序号防异步乱序，`C2.2`；库外文件或无活跃文档时显示明确空状态“库外文件暂不计算库内关联”）；
       - 切到 `recent` 或文档保存/切换（且 `recent` 可见）时刷新最近编辑前 **20** 篇；
     - 宽度调节方法 `setPanelWidth(nextWidth: number, viewportWidth: number)`：
       - 强制约束 `>= MIN_PANEL_WIDTH (220)` 且在非抽屉模式下确保留给编辑区的宽度 `>= MIN_EDITOR_WIDTH (480)`；
       - 防抖持久化到 `ui-state.json` 的 `panelGroup: { visible, activeId, width, collapsed }`（`C6.3`）；
     - 响应式状态管理 `applyViewportWidth(width: number)`：
       - `width < 900`：`isOverlayDrawer = true`（右侧面板展开时以覆盖式抽屉浮于编辑区右侧，不挤占编辑区），默认自动收起；
       - `900 <= width < 1200`：`isOverlayDrawer = false`，首次跨入 `< 1200` 时自动收起右侧面板为窄图标条（用户仍可手动点开）；
       - `width < 1000`：`isNarrowLeftBreakpoint = true`，左侧导航+列表自动收起为窄图标条（提供一键展开/收起切换按钮，在 `< 900` 模式下左侧展开也可不把编辑区挤破 `480px`）；
       - `width >= 1200`：恢复用户持久化偏好态。

3. **实现 3 个新面板视图与统一宿主 `RightPanelGroup.vue`**：
   - `src/features/panels/RelatedNotesPanel.vue`：
     - 展示 `relatedItems`（最多 20 条）；
     - 每项显示文档标题、相对路径与关联理由徽标（`backlink` → “反向链接”、`shared_tags` → “共有标签”、`same_project` → “同项目”）；
     - 点击条目调用 `editorTabsStore.openNoteByRelPath` / `openNoteByAbsPath` 打开跳转。
   - `src/features/panels/RecentNotesPanel.vue`：
     - 展示最近打开/编辑前 **20** 篇文档（标题、目录、相对时间），高亮当前正在编辑的文档，点击直接打开跳转。
   - `src/features/panels/TerminalPlaceholderPanel.vue`：
     - M3 阶段轻量占位视图（清晰说明“内嵌 PTY 终端将在 M4 阶段启用，当前可按 ⌥⌘T 在外部终端打开项目目录”，并提供快捷按钮触发 `external.terminal`）。
   - `src/features/panels/RightPanelGroup.vue`：
     - 顶部/侧边统一图标选项卡切换条（严格按注册表 4 项顺序：`outline`、`terminal`、`related`、`recent` + 折叠/展开按钮）；
     - 当面板收起（`collapsed || !visible`）时呈现紧凑的 **窄图标条**（36px 宽），点击任意板块图标直接展开到该板块；
     - 展开态左侧提供可拖拽手柄（`pointerdown`/`pointermove`/`pointerup`）实时调整面板宽度，并尊重 `>= 220px` 与编辑区 `>= 480px` 约束；
     - 在 `< 900px` 覆盖式抽屉模式下，通过 `position: absolute; right: 0; top: 0; bottom: 0` + `--qn-z-overlay`（使用已有语义层级 Token，严禁魔法 `z-index`）浮于编辑区右侧，并附半透明背景遮罩点击可收起。

4. **主工作台响应式布局与动作/菜单全打通（`src/app/MainWorkbenchShell.vue`）**：
   - 用 `RightPanelGroup` 替换原单独挂载的 `OutlinePanel`；
   - 监听容器 `ResizeObserver`（或 `window.resize` 兜底）实时驱动 `panelsStore.applyViewportWidth(width)`；
   - 左侧 `NotesSidebar` 在 `< 1000px` 断点支持收起为窄图标条（带展开/收起切换按钮），保证编辑区永远不小于 `480px`（`min-width` 保护）；
   - 在 `MainWorkbenchShell.vue` 的 `bindMainWindowActions()` 完整绑定：
     - `panel.toggle.outline`（`⌃⌘O`）
     - `panel.toggle.terminal`（`⌃⌘T`）
     - `panel.toggle.related`（`⌃⌘R`）
     - `panel.toggle.recent`（`⌃⌘E`）
     - `panel.collapseGroup`（`⌃⌘0`）
   - 确保系统菜单 `menu:action` 转发的上述动作全部与 `actionRegistry` 同源执行。

### 明确不做什么
- 本卡**不做**真实 PTY / `xterm.js` 终端实现（归 M4）。
- 本卡**不做**非 Markdown 语法高亮、轻量诊断与标签批量重命名（归 `M3-05b`）。
- 本卡**不修改**任何 Rust 代码或 `C1.3` 共享文件。

## 2. 允许修改文件白名单

- 面板模块（`src/features/panels/`）：
  - `src/features/panels/registry.ts`（新增）
  - `src/features/panels/api.ts`（新增）
  - `src/features/panels/store.ts`
  - `src/features/panels/RelatedNotesPanel.vue`（新增）
  - `src/features/panels/RecentNotesPanel.vue`（新增）
  - `src/features/panels/TerminalPlaceholderPanel.vue`（新增）
  - `src/features/panels/RightPanelGroup.vue`（新增）
  - `src/features/panels/index.ts`
- 主工作台与左侧收起配合：
  - `src/app/MainWorkbenchShell.vue`
  - `src/features/notes/NotesSidebar.vue`（仅添加紧凑窄图标条收起模式 prop/回调，不改标签重命名逻辑）
- 测试文件：
  - `src/features/panels/__tests__/right-panel-group.test.ts`（新增）
  - `src/features/panels/__tests__/outline-panel.test.ts`（保持全绿，必要时同步接口）

## 3. 具体实现要求

1. **已内联的 IPC 签名（全部位于 `src/lib/ipc/commands.ts` 与 `src/bindings/types.ts`）**：
   - `notesGetRelated(args: { relPath: string }): Promise<RelatedNoteItem[]>`
     - `RelatedNoteItem = { key: string; relPath: string; title: string; reason: 'backlink' | 'shared_tags' | 'same_project'; score: number }`
   - `notesList(args: NotesFilterArgs): Promise<NoteSummary[]>`
   - `uiStateGet(): Promise<UiState>` / `uiStateSet(args: { key: 'panelGroup'; value: PanelGroupUiState }): Promise<UiState>`
2. **样式与分层红线（`C1.2`、`C7.1`）**：
   - 所有宽度、边框、阴影、颜色、圆角与层级仅使用 `--qn-*` CSS Token（检查 `src/styles/tokens.css` 已有 `--qn-z-*` 语义层级，禁止魔法数字 `z-index`）；
   - Vue 组件严禁直连 `@tauri-apps/api/*`，禁止跨 feature 深层导入。
3. **竞态防线（`C2.2`）**：
   - 快速切换文档时，慢返回的 `notes_get_related` 旧请求必须因序号过期被丢弃，不污染新文档的相关面板。

## 4. 自动化验收标准（必须全绿）

1. `src/features/panels/__tests__/right-panel-group.test.ts` 覆盖：
   - 面板注册表顺序、默认宽度（`260/360/260/240`）、最小宽度 `220` 与快捷键/动作 ID 同源断言；
   - `related` 面板消费 `notes_get_related`、展示反向链接/共有标签/同项目徽标、点击打开笔记及切换文档防异步乱序；
   - `recent` 面板最多展示前 20 篇并支持点击跳转；
   - 宽度调节严格限制 `>= 220px` 与编辑区 `>= 480px`，并持久化 `uiStateSet({ key: 'panelGroup', ... })`；
   - 三级响应式断点精准断言：
     - `1280px`：常规展开；
     - `1150px`（`< 1200`）：右侧自动收起为窄图标条；
     - `960px`（`< 1000`）：左侧进入可收起窄图标条态；
     - `820px`（`< 900`）：右侧展开为覆盖式抽屉，不压缩编辑区宽度。
2. 运行 `pnpm check:all` 全部 9 项门禁 100% 通过。

## 5. 人工点验清单

- 在主窗口分别测试 `⌃⌘O`、`⌃⌘T`、`⌃⌘R`、`⌃⌘E`、`⌃⌘0` 与“视图”菜单项切换全部 4 个右侧面板；拖拽面板左边缘调宽并重启验证宽度持久化；将主窗口逐步拖窄跨越 `1200px`、`1000px`、`900px`（以及 45% 侧栏辅助窗口宽度）确认响应式收起与覆盖式抽屉绝不挤压编辑区。
