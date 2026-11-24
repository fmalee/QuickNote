# M3-04：Quick Bar 命令模式与主窗口大尺寸命令库

- 所属里程碑：M3
- 关联方案与契约：`v4` §3.5.3、§3.5.4、§3.6、§3.7.1、§4.8、§6 M3；`C1.2`、`C2.2`、`C4.2`、`C7.1`–`C7.3`、`C9.1`、`D-0001`
- 前置依赖：`M3-03` 已合并至 `dev`
- 建议分支：`feat/M3-04-command-book-ui`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、键盘交互契约与目录解析规则已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **抽离共享命令库检索与执行核心（`src/features/commands/`，供 Quick Bar 命令模式与主窗口大尺寸命令库共用，独立实例互不共享状态）**：
   - `src/features/commands/api.ts`：
     - `searchCommands(query: string): Promise<CommandEntry[]>`（调用 `commandsSearch({ query })`）
     - `resolveCommandExternalActions(args: { absPath: string | null; workspaceKey: string | null; commandId: string }): Promise<ExternalActionState[]>`（调用 `externalActionsResolve`）
     - `openExternalTerminalAtDir(dirPath: string): Promise<void>`（调用 `externalActionRun({ id: 'external.terminal', absPath: null, dirPath })`）
     - `fetchLibraryRoot(): Promise<string | null>`（调用 `libraryGetInfo()` 以将 `entry.noteRelPath` 拼为 `absPath`）
     - `showMainWindow()` / `hideQuickBarWindow(reason)`
     - `copyTextToClipboard(text: string): Promise<void>`（优先 `navigator.clipboard.writeText`，降级 `execCommand('copy')` 兜底，日志严禁打印复制内容 `C2.3`）
   - `src/features/commands/useCommandPalette.ts`（Composable）：
     - 80ms 防抖检索 `commands_search`，附递增序号防异步乱序（`C2.2`）；
     - 维护 `selectedIndex`、`selectedSection`（大尺寸版支持按 `section` 分组筛选）、`selectedEntry`、`terminalState`（实时对当前选中 `entry.id` 结合上下文 `activeAbsPath` / `activeWorkspaceKey` 调用 `external_actions_resolve` 获取 `external.terminal` 的 `enabled`、`resolvedPath` 与 `disabledReason`）；
     - 严格实现 3 条核心操作（`v4` §3.5.3 / §3.7.1）：
       1. **`copySelected()`（`↵`）**：将 `entry.code` 复制到剪贴板，展示轻反馈“已复制命令”，**保持浮层/弹窗打开**（便于连续复制多条）；
       2. **`copyAndOpenTerminal()`（`⌘↵`）**：
          - 先复制 `entry.code` 到剪贴板（**任何情况下都不自动粘贴或自动回车执行**！）；
          - 若 `terminalState.enabled && terminalState.resolvedPath`：调用 `openExternalTerminalAtDir(resolvedPath)`，若在 Quick Bar 中随后隐藏浮层；
          - 若未解析出可用目录（`!terminalState.enabled`）：**绝不静默使用用户主目录**，打开内联「选择目标目录」输入/工作区快捷列表面板，用户选择工作区或输入有效目录确认后再调用 `openExternalTerminalAtDir(chosenDir)`；
       3. **`openSourceFile()`（`⌘O`）**：将 `entry.noteRelPath` 解析为绝对路径（`<libraryRoot>/<noteRelPath>`），在 Quick Bar 中通过 `M3-03` 约定的 `PENDING_OPEN_FILE_STORAGE_KEY`（携带 `{ absPath, newTab: false, line: entry.line, ts }`）并调用 `windowShowMain()` + `windowHideQuickBar()`；在主窗口中直接回调 `onOpenSource(absPath, entry.line)` 定位到该 H3 对应行号！
   - `src/features/commands/CommandBookPalette.vue`（共享视图，支持 `variant: 'compact' | 'large'`）：
     - **`compact`（Quick Bar 命令模式）**：
       - 顶部搜索框 + 按分区聚合的紧凑命令列表；
       - 每条显示：标题（H3 / 兼容段落）、描述摘要、等宽代码块（含语言标签）与操作按钮 `[↵ 复制]` `[⌘↵ 开终端]`；
       - **危险命令高亮**：`entry.isDanger === true` 时渲染醒目 `[⚠ 危险]` 徽标（`QnBadge tone="danger"`）与 `--qn-danger` 警示边框；
       - 底部状态栏明确显示即将打开的外部终端目录（如 `将在 ~/Sites/Projects/quicknote 打开外部终端` 或 `未关联项目目录 · 按 ⌘↵ 手动选择`）与 `[⌘O 源文件]` 提示。
     - **`large`（主窗口大尺寸命令库，`v4` §3.6）**：
       - 左侧分区目录栏（全部 + 各 `section` 计数）+ 中间命令条目列表 + 右侧完整详情预览区（完整描述、多行代码块、`keywords` 标签、`isDanger` 警告横幅、目标目录预览、一键复制 / 复制并开终端 / 跳回源文件编辑按钮）。

2. **Quick Bar 接入命令模式（`v4` §3.5.3、§3.5.4）**：
   - 更新 `src/features/quickbar/store.ts` 与 `src/features/quickbar/QuickBarView.vue`：
     - 补全 `'command'` 模式渲染（挂载 `<CommandBookPalette variant="compact" />`）；
     - 浮层内按 **`⌘⇧K`** 切换至命令模式；`⌘K` 切换至打开模式；在命令模式按 **`Esc`** 先退回记录模式（若目录选择子面板已打开则先关闭子面板）；
     - 将 Quick Bar 当前活跃上下文（`resolvedTarget?.workspaceKey`、`uiState.lastActiveDoc?.relPath` 对应的绝对路径）传入命令面板作为终端目录解析上下文。

3. **主窗口大尺寸命令库与 `⌘O` 精准行号跳转（`v4` §3.6）**：
   - 更新 `src/app/MainWorkbenchShell.vue`：
     - 绑定 `window.commandBook`（`⌘⇧K`）及系统菜单 `menu_command_book` / `menu_tools_commands` 唤起大尺寸 `CommandBookPalette`（包裹于 `QnModalShell`，`variant="large"`）；
     - 上下文取主窗口当前激活标签的 `absPath` 与 `meta.project`（工作区）；
     - 点击/按 `⌘O` 打开源文件后关闭弹窗，打开对应命令书文档并调用 `editorTabsStore.requestScrollToLine(tab.key, entry.line)` 滚动定位至目标 H3 所在行（同时确保 `PENDING_OPEN_FILE_STORAGE_KEY` 跨窗口打开时若携带 `line` 也自动滚动定位）。

### 明确不做什么
- 本卡**不做**内嵌 `xterm.js` 终端（归 M4；本卡 `⌘↵` 一律调用 `external_action_run(external.terminal)` 打开外部终端且绝不自动粘贴）。
- 本卡**不做**右侧面板组与非 Markdown 高亮（归 `M3-05a` / `M3-05b`）。
- 本卡**不修改**任何 Rust 代码或 `C1.3` 共享文件。

## 2. 允许修改文件白名单

- 新增命令库 feature（`src/features/commands/`）：
  - `src/features/commands/api.ts`（新增）
  - `src/features/commands/useCommandPalette.ts`（新增）
  - `src/features/commands/CommandBookPalette.vue`（新增）
  - `src/features/commands/index.ts`（新增）
- Quick Bar 与主窗口集成：
  - `src/features/quickbar/store.ts`
  - `src/features/quickbar/QuickBarView.vue`
  - `src/features/editor/store.ts`（仅若需补齐 `requestScrollToLine` 辅助方法供行号跳转）
  - `src/app/MainWorkbenchShell.vue`
- 测试文件：
  - `src/features/commands/__tests__/command-book-palette.test.ts`（新增）
  - `src/features/quickbar/__tests__/quickbar-command-mode.test.ts`（新增）

## 3. 具体实现要求

1. **已内联的 IPC 签名（全部位于 `src/lib/ipc/commands.ts` 与 `src/bindings/types.ts`）**：
   - `commandsSearch(args: { query: string }): Promise<CommandEntry[]>`
     - `CommandEntry = { id: string; noteRelPath: string; section: string; title: string; description: string; code: string; language: string | null; keywords: string[]; isDanger: boolean; line: number }`
   - `externalActionsResolve(args: { absPath: string | null; workspaceKey: string | null; commandId: string | null }): Promise<ExternalActionState[]>`
   - `externalActionRun(args: { id: ExternalActionId; absPath: string | null; dirPath: string | null }): Promise<void>`
   - `libraryGetInfo(): Promise<LibraryInfo>`
2. **命令执行安全红线（`v4` §3.5.3、§3.7.1）**：
   - `↵` 只复制，保持浮层打开；
   - `⌘↵` 只做两件事：① 复制命令文本到剪贴板；② 在解析出的目录打开外部终端（仅传递 `dirPath`，**绝不传递命令字符串给终端、绝不自动粘贴或自动回车**）；
   - 目标目录解析失败时必须由用户显式选择工作区或输入路径，**绝不把 `~` 或 `$HOME` 当默认目录静默填入提交**。
3. **分层、样式与输入法保护（`C1.2`、`C2.2`、`C7.1`）**：
   - `src/features/commands/` 禁止直接 import `@tauri-apps/api/*`，禁止跨 feature 深层导入；
   - 全程仅用 `--qn-*` CSS Token；
   - 键盘处理统一拦截 IME 组合态（`event.isComposing || event.keyCode === 229`）。

## 4. 自动化验收标准（必须全绿）

1. `src/features/commands/__tests__/command-book-palette.test.ts` 覆盖：
   - 80ms 防抖查询与乱序响应丢弃；
   - `isDanger: true` 危险标识渲染与 `section` 分区筛选；
   - `↵` 仅复制命令文本且不触发关闭浮层、不调用 `externalActionRun`；
   - `⌘↵` 当存在解析目录时复制命令并调用 `externalActionRun({ id: 'external.terminal', absPath: null, dirPath })`（断言不含任何自动粘贴参数）并通知关闭浮层；
   - `⌘↵` 当 `external.terminal` 被禁用（无目录）时不静默调用 `externalActionRun`，而是弹出手动目录选择器，用户选择有效目录后才调用；
   - `⌘O` 触发打开源文件并传入精确 `absPath` 与 `entry.line`。
2. `src/features/quickbar/__tests__/quickbar-command-mode.test.ts` 覆盖：
   - Quick Bar 内 `⌘⇧K` 切命令模式、`⌘K` 切打开模式、`Esc` 先退回记录模式再按隐藏；
   - Quick Bar 命令模式 `⌘O` 写入 `PENDING_OPEN_FILE_STORAGE_KEY`（含 `line`）、唤起主窗口并隐藏 Quick Bar。
3. 运行 `pnpm check:all` 全部 9 项门禁 100% 通过。

## 5. 人工点验清单

- 在库内 `commands/常用命令.md` 验证 Quick Bar `⌃⌥Space` → `⌘⇧K`：搜索“撤销”命中并按 `↵` 复制且浮层不关；搜索“强制清理”看到红色危险标识；按 `⌘↵` 打开外部终端自动 `cd` 到项目目录且不自动粘贴执行；按 `⌘O` 直接在主窗口打开命令书并滚动到该 H3。
- 主窗口按 `⌘⇧K` 或“工具 > 打开命令库”验证大尺寸分栏命令库正常。
