# M4-02：内嵌 xterm.js 终端面板、折叠保活、活跃指示点与快捷键隔离

- 所属里程碑：M4
- 关联方案与契约：`v4` §3.7.2、§5.6.6、§5.6.7、§6 M4；`C1.2`、`C1.3`、`C2.2`、`C2.3`、`C4.2`、`C4.3`、`C7.1`–`C7.4`、`C8.7`、`C9.1`、`C9.3`、`D-0005`
- 前置依赖：`M4-01a`、`M4-01b` 已合并至 `dev`
- 建议分支：`feat/M4-02-embedded-xterm-terminal`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、按键隔离矩阵、保活/状态点与双主题规则已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **安装终端前端依赖（`C1.3` 串行声明）**：
   - 在 `package.json` 新增 `@xterm/xterm` 与 `@xterm/addon-fit`（锁定兼容版本并更新 `pnpm-lock.yaml`）。
2. **新增独立终端特性模块 `src/features/terminal/`（遵循 `C1.2` 单向分层）**：
   - `src/features/terminal/api.ts`：封装 `src/lib/ipc/commands.ts` 与 `src/lib/ipc/events.ts` 中的终端 IPC：
     - `terminalSpawn(args, onData)`、`terminalWrite(args)`、`terminalResize(args)`、`terminalSetVisibility(args)`、`terminalKill(args)`、`terminalListSessions()`；
     - `onTerminalSessionState(handler)`、`onTerminalExit(handler)`。
   - `src/features/terminal/theme.ts`：
     - 严格遵循 `C7.1`（禁止硬编码 hex 色值进组件 CSS！）：通过 `getComputedStyle(document.documentElement)` 读取 `--qn-bg-base`、`--qn-text-primary`、`--qn-accent`、`--qn-bg-active`、`--qn-danger`、`--qn-warning`、`--qn-success`、`--qn-text-muted` 动态构造 xterm `ITheme`，监听根元素 `data-theme` 与 `prefers-color-scheme` 变更实时刷新终端配色。
     - 提供浏览器端 base64 ↔ `Uint8Array` / UTF-8 安全编解码工具（支持多字节中文 UTF-8 分片解码不乱码，使用 `TextDecoder({ fatal: false })` 流式 `stream: true`）。
   - `src/features/terminal/store.ts`（Pinia `useTerminalStore`）：
     - 维护活跃会话表 `sessions: Record<string, TerminalSessionInfo>`、当前选中 `activeSessionId: string | null`、会话提示状态（`errorMessage`、`recycledNotice`）与 `hasSelection: boolean`（用于 `C7.3` `terminal.copySelection` 上下文判断）；
     - `hasAnyActiveForegroundChild` 计算属性：任意存活会话的 `hasActiveForegroundChild === true` 时返回 `true`，供右侧面板选项卡 `终端` 与折叠图标条渲染醒目运行指示点 `●`；
     - 工作区上下文解析函数 `resolveTerminalLaunchContext(activeDoc, workspaces, selectedWorkspaceKey, libraryRoot)`：
       - 优先级对齐 `v4` §5.6.7：文档 frontmatter `terminalDir` → `openDir` → 文档关联工作区或当前选中工作区的 `terminalDir` / `openDir` / `root` → 库外文件父目录 → 若仍无则使用工作区键 `default` 与当前库根（或给出引导提示让用户选择已配置工作区）；
       - 每个工作区至多 1 个会话：若该 `workspaceKey` 已有存活会话，直接切换激活该会话而不重复 `spawn`；
       - 达到全局并发上限（默认 3，后端返回 `TerminalLimitReached`）时展示友好提示横幅，引导关闭一个旧会话后再启动。
     - 生命周期联动：
       - `initListeners()` 订阅 `terminal:session-state` 与 `terminal:exit`：收到 `session-state` 即时更新 `hasActiveForegroundChild`、`outputPaused`、`state`；收到 `exit`（区分 `process_exit` / `manual_close` / `idle_recycled` / `app_exit`）清理活跃态并保留简明退出提示（如“终端因闲置 30 分钟已自动回收，点击重新启动”）；
       - `syncPanelVisibility(visible: boolean)`：当右侧面板折叠或切换到大纲/相关/最近时，对当前存活会话调用 `terminal_set_visibility(sessionId, false)`；当切回终端面板并展开时，调用 `terminal_set_visibility(sessionId, true)` 触发 Rust 吐回 1MB 环形缓冲并恢复流式写入；
       - `resumePausedOutput(sessionId)`：对 `outputPaused === true` 的会话清空前端显示缓冲并调用 `terminal_set_visibility(sessionId, true)` 通知后端解除暂停；
       - `closeSession(sessionId)`：调用 `terminal_kill` 手动关闭并清理对应 xterm 实例。
   - `src/features/terminal/TerminalPanel.vue`（对外经 `src/features/terminal/index.ts` 导出）：
     - **顶部控制栏**：
       - 显示当前会话工作区标签与 `cwd` 路径摘要；若存在多个会话（最多 3 个），提供紧凑会话切换 chip（带运行指示点 `●`）与 `[+ 新建]`（未满上限且当前工作区无会话时显示）；
       - 右上角提供显式 `[× 关闭会话]` 按钮（`data-testid="terminal-close-session-btn"`）；
       - 保留外部终端快捷入口图标按钮（复用 `external.terminal`）。
     - **10MB 背压暂停横幅**：当当前会话 `outputPaused === true` 时显示警告横幅“输出过大，已暂停显示”与“清空并恢复显示”按钮（`data-testid="terminal-resume-output-btn"`）。
     - **空态 / 退出态引导**：尚无会话或会话被空闲回收/退出时，展示启动卡片（显示目标目录与“启动内嵌终端”按钮；首次主动打开终端面板且有有效工作区目录时可自动启动首个会话）。
     - **xterm.js 容器与 `ResizeObserver`**：
       - 为每个存活 `sessionId` 维护独立挂载的 `Terminal` + `FitAddon` 实例（切会话用 `v-show` 保活，不销毁缓冲）；
       - 绑定 `ResizeObserver`（防抖 `50ms`）在容器可见且尺寸有效时调用 `fitAddon.fit()` 并触发 `terminal_resize({ sessionId, cols, rows })`；
       - 用户输入 `term.onData(data)` 编码为 base64 调用 `terminal_write`；
       - 监听 `term.onSelectionChange` 同步 `terminalStore.setHasSelection(term.hasSelection())`；
       - **中文 IME 组合态支持**：确保 xterm textarea 不被外层键盘监听抢占 `event.isComposing || event.keyCode === 229`。
     - **焦点与快捷键隔离（`C7.3`、`D-0005`）**：
       - 容器标注 `data-action-scope="terminal"`，与 `MainWorkbenchShell.vue` 的 `resolveActiveScopes()` 匹配；
       - 在 `term.attachCustomKeyEventHandler(event)` 与全局 `actionRegistry` 联动实现精确隔离：
         1. **`⌥⌘C`（`alt+cmd+c`）**：显式拦截吞掉（`preventDefault` + 返回 `false`），严禁穿透触发编辑器 `editor.codeBlock`；
         2. **`⌘C`（`cmd+c`）**：若 `term.hasSelection()` 为 `true`，复制选中文本到剪贴板并拦截（不写入 PTY）；若无选区，放行给终端/系统默认行为；
         3. **`⌘V`（`cmd+v`）**：支持从系统剪贴板粘贴文本写入 PTY（注意遵守 `C2.3` 不打印剪贴板内容）；
         4. **`⌃C` / `⌃D` / `⌃Z`**：必须放行直传 PTY，绝不触发任何应用动作；
         5. **面板切换与全局热键（如 `⌃⌘T`、`⌃⌘O`、`⌃⌘0`、`⌘K`）**：在 `attachCustomKeyEventHandler` 返回 `false` 放行给外层 `actionRegistry`，允许用户随时用快捷键折叠/切换面板。
3. **接入右侧面板组与主工作台**：
   - 更新 `src/features/panels/RightPanelGroup.vue`：
     - 用 `TerminalPanel`（从 `@/features/terminal` 公开入口导入）替换 `TerminalPlaceholderPanel.vue`（并清理不再使用的 `TerminalPlaceholderPanel.vue`）；
     - **折叠保活关键点**：即使面板组折叠或当前切换到 `outline`/`related`/`recent`，只要 `terminalStore.hasAnySession` 为真，终端容器以 `v-show` 保持隐式挂载（或由外层保活槽位承接），避免 Vue `v-if` 卸载销毁 `xterm.js` 画布与历史滚动内容；
     - 在右侧面板 `终端` 选项卡（展开态）和折叠态窄图标条 `终端` 按钮旁，当 `terminalStore.hasAnyActiveForegroundChild` 为 `true` 时渲染运行状态指示点 `●`（`data-testid="terminal-active-dot"`，使用 `--qn-success` Token）。
   - 更新 `src/app/MainWorkbenchShell.vue`：
     - 在 `buildActionContext()` 注入 `hasTerminalSelection: terminalStore.hasSelection`；
     - 绑定 `terminal.copySelection` 动作至 `terminalStore.copyActiveSelection()`；
     - 监听右侧面板展开/折叠及 `activeId` 变化，驱动 `terminalStore.syncPanelVisibility(panelsStore.isPanelGroupExpanded && panelsStore.panelGroup.activeId === 'terminal')`。

### 明确不做
- 不修改后端 PTY 代码（已在 `M4-01a` 完成）。
- 不在本卡新增设置窗口终端配置表单（归 `M4-04` 统一收口）。

## 2. 允许修改文件白名单

### 共享串行文件（`C1.3` 显式声明）
- `package.json`、`pnpm-lock.yaml`

### 业务与测试文件
- `src/features/terminal/api.ts`（新增）
- `src/features/terminal/theme.ts`（新增）
- `src/features/terminal/store.ts`（新增）
- `src/features/terminal/TerminalPanel.vue`（新增）
- `src/features/terminal/index.ts`（新增）
- `src/features/terminal/__tests__/terminal-panel.test.ts`（新增）
- `src/features/panels/RightPanelGroup.vue`
- `src/features/panels/TerminalPlaceholderPanel.vue`（删除或替换为导出重定向，推荐删除并确保无残留引用）
- `src/features/panels/__tests__/right-panel-group.test.ts`
- `src/app/MainWorkbenchShell.vue`

## 3. 具体实现要求

1. **样式与 Token 红线（`C7.1`）**：
   - 导入 `@xterm/xterm/css/xterm.css`；所有自定义容器边框、顶栏、提示横幅与运行指示点 `●` 一律使用 `--qn-*` CSS Token，禁止任何硬编码色值与魔法 `z-index`。
2. **隐私红线（`C2.3`）**：
   - 严禁在 `console.*` 输出终端输入、输出或剪贴板文本。
3. **无头测试兼容性**：
   - `TerminalPanel.vue` 与 `store.ts` 对 `ResizeObserver` 和 `xterm.js` canvas/DOM 保持优雅容错，确保在 Vitest `jsdom` 环境下无需真实 Canvas 也能完整测试会话管理、折叠保活、指示点 `●`、10MB 暂停横幅、`⌥⌘C` 吞键与 `⌘C` 有无选区分流。

## 4. 自动化验收标准（必须全绿）

- 新增测试 `src/features/terminal/__tests__/terminal-panel.test.ts` 并更新 `right-panel-group.test.ts`，至少覆盖：
  1. 工作区 `cwd` 解析优先级（frontmatter `terminalDir` > `openDir` > 工作区配置 > 库外父目录）；
  2. 折叠保活：切到 `outline` 或折叠面板组时触发 `terminal_set_visibility(id, false)` 且终端会话实例不被销毁；切回时触发 `terminal_set_visibility(id, true)`；
  3. 收到 `terminal:session-state(hasActiveForegroundChild = true)` 时，选项卡与折叠图标条显示指示点 `●`；
  4. 收到 `outputPaused = true` 时展示“输出过大，已暂停显示”横幅，点击“清空并恢复显示”触发恢复调用；
  5. 收到 `terminal:exit(reason = "idle_recycled")` 时重置会话态并显示闲置回收提示；
  6. 按键隔离（`C7.3`）：终端聚焦下 `⌥⌘C` 被显式吞掉不触发 `editor.codeBlock`；`⌘C` 有选区时复制且不向 PTY 写入、无选区时放行；`⌃C`/`⌃D`/`⌃Z` 放行。
- 必过命令：`pnpm check:all`。

## 5. 人工点验清单（涉及 macOS 原生与输入法交互，`C9.3`）

- [ ] `⌃⌘T` 打开内嵌终端面板，执行 `ls` / `pwd` 验证工作目录与登录 shell 环境变量正常；
- [ ] 在终端内运行 `sleep 30` 或 `npm run dev`，确认选项卡旁显示运行状态指示点 `●`，切换到大纲或折叠面板后进程不中断，切回毫秒级恢复画面；
- [ ] 在终端聚焦时按 `⌥⌘C` 确认不会误向编辑器插入代码块；选中终端文本按 `⌘C` 确认仅复制不发中断；未选区按 `⌃C` 正常中断前台进程；
- [ ] 中文拼音输入法在 xterm.js 终端内组合态选词与上屏正常；
- [ ] 点击 `[× 关闭会话]` 或 `⌘Q` 退出应用后，`ps aux` 确认无残留僵尸 shell 子进程。
