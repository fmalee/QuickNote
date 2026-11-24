# M4-qa：M4 实机点验体验修复（终端滚动条、Cmd+V 原生直贴、外部终端与 Banner 样式）

- 所属里程碑：M4（实机点验修复）
- 关联方案与契约：`v4` §3.7.1、§5.6.7、§5.10；`C1.2`、`C2.2`、`C2.3`、`C7.1`、`C7.3`、`C9.1`、`D-0005`
- 前置依赖：`fix/M4-audit`（`6cf8114`）已合并至 `dev`
- 建议分支：`fix/M4-qa`
- 会话策略：独立新会话执行（零大文档读取，所有定位与修复要求已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **隐藏内嵌终端滚动条，防止遮挡右侧命令行内容（`src/features/terminal/TerminalPanel.vue`）**：
   - 参考 `EditorTabBar.vue`，在 `TerminalPanel.vue` 样式中为 xterm 视口及多会话标签栏隐藏视觉滚动条（保留触控板/鼠标滚轮与内容滚动能力）：
     ```css
     .qn-terminal-panel__xterm-host :deep(.xterm-viewport) {
       scrollbar-width: none;
     }

     .qn-terminal-panel__xterm-host :deep(.xterm-viewport::-webkit-scrollbar) {
       display: none;
     }

     /* @xterm/xterm v6 使用 VS Code DomScrollableElement 自绘 DOM 滚动条 */
     .qn-terminal-panel__xterm-host :deep(.xterm-scrollable-element > .scrollbar),
     .qn-terminal-panel__xterm-host :deep(.xterm-scrollable-element > .shadow) {
       display: none !important;
       opacity: 0 !important;
       pointer-events: none !important;
     }

     .qn-terminal-panel__session-tabs {
       scrollbar-width: none;
     }

     .qn-terminal-panel__session-tabs::-webkit-scrollbar {
       display: none;
     }
     ```
2. **修复 macOS `Cmd + V` 触发系统 `Paste` 确认气泡，实现静默直贴命令行（`src/features/terminal/TerminalPanel.vue` & `src/features/terminal/store.ts`）**：
   - **根因**：macOS 13+ `WKWebView` 下，JS 主动调用 `navigator.clipboard.readText()` 会触发系统级隐私确认，在鼠标光标位置弹出原生 `Paste` 气泡；而放行浏览器原生 `Cmd + V`（系统「编辑 > 粘贴」）会直接向 `xterm.js` 内聚焦的 `.xterm-helper-textarea` 派发原生 `paste` DOM 事件，`xterm.js` 原生消费 `event.clipboardData` 并经 `term.onData` 写入 PTY，**全程无系统气泡、一键直贴命令行**。
   - **修复**：
     - 在 `TerminalPanel.vue` 的 `attachCustomKeyEventHandler` 中，遇到 `decision === 'paste_clipboard'` 时**不再调用 `event.preventDefault()`、不再调用 `navigator.clipboard.readText()`**，直接返回 `false`（将 `Cmd + V` 交回浏览器/WKWebView 原生粘贴链路给 xterm.js 内部 textarea 处理）；
     - 同时确保外层 `MainWorkbenchShell.vue` 或 `actionRegistry` 不会拦截终端聚焦时的 `Cmd + V`；
     - 保留 `terminalStore.pasteFromClipboardToSession` 供显式按钮或非原生场景（如有需要）调用，但按键直贴完全走原生 `paste` 事件；
     - 同步更新 `terminal-panel.test.ts` 中对 `Cmd + V` 决策及行为的断言（确保按键处理返回 `false`、不阻止默认原生粘贴事件、不主动触发 `navigator.clipboard.readText()`）。
3. **修复内嵌终端顶部「外部终端」按钮点击无效/静默失败（`src/features/terminal/TerminalPanel.vue` & `src/features/terminal/api.ts` & `src/features/terminal/store.ts`）**：
   - **根因**：
     1. 当前按钮仅调用文档级 `actionRegistry.executeById('external.terminal', ...)`：当打开的库内文档未配置 `open_dir`/`terminal_dir`/工作区时该动作按规则被禁用，但终端面板中已经有一个活跃会话与明确的 `activeSession.cwd`，且点击后失败原因未显示在终端面板内，导致用户看到“点击完全无效”。
     2. 若用户本机未安装默认配置的外部终端（如默认 `iTerm`，用户实际使用 `Terminal.app` / `Warp` / `Ghostty`），报错信息仅落到状态栏外部反馈中，终端面板无明确提示。
   - **修复**：
     - 在 `src/features/terminal/api.ts` 新增 `openExternalTerminalAtDir(dirPath: string): Promise<void>`，封装调用 `externalActionRun({ id: 'external.terminal', absPath: null, dirPath })`（遵循 `C1.2`，通过 `src/lib/ipc/commands.ts` 调用，不直连 Tauri API）；
     - 在 `src/features/terminal/store.ts` 新增 `openExternalTerminalForActiveSession(): Promise<boolean>`：
       - 若存在活跃会话 `activeSession.cwd`（或 `launchContext.cwd`）：直接调用 `openExternalTerminalAtDir(targetDir)`；
       - 若后端抛出错误（如 `ExternalAppMissing` 未安装 iTerm、或目录不存在），捕获并提取清晰可读提示写入 `terminalStore.errorMessage`（例如提示检查“设置 > 外部与搜索 > 外部终端”配置），展示在终端面板顶部横幅中；
     - 在 `TerminalPanel.vue` 的 `openExternalTerminal()` 中：优先走 `terminalStore.openExternalTerminalForActiveSession()`（当有活跃会话或有效工作区目录时直接打开外部终端）；若既无活跃会话也无可用工作区目录，则展示清晰错误提示在终端面板顶部（“未解析到可用目录，请先在设置中配置工作区或为文档配置 open_dir”），不再静默无反应。
4. **修复 `TerminalPanel.vue` 中 `QnBanner` 错误 prop 名称（`variant` → `tone`）**：
   - `QnBanner` 仅接受 `tone?: 'info' | 'warning' | 'danger' | 'success'`，而 `TerminalPanel.vue` 当前误写为 `variant="error"` 与 `variant="warning"`；
   - 修正为 `tone="danger"`（错误横幅）与 `tone="warning"`（10MB 背压暂停横幅），让错误与背压警告呈现正确的红/黄视觉样式。

### 明确不做
- 不修改 Rust 后端代码与 IPC 结构体（现有 `external_action_run` 已支持直接传入 `dirPath` 打开外部终端）。
- 不改动 `C7.3` 的 `⌥⌘C` 吞键、`⌘C` 有无选区分流与 `⌃C`/`⌃D`/`⌃Z` 直传 PTY 逻辑。

## 2. 允许修改文件白名单

### 业务与测试文件
- `src/features/terminal/api.ts`
- `src/features/terminal/store.ts`
- `src/features/terminal/TerminalPanel.vue`
- `src/features/terminal/__tests__/terminal-panel.test.ts`

## 3. 具体实现要求

1. 严守 `C1.2` 单向分层、`C2.2` 零 `any`、`C2.3` 严禁打印剪贴板或终端输出内容、`C7.1` 所有样式仅用 `src/styles/tokens.css` 已定义 Token。
2. 保留终端内 `⌥⌘C` 显式吞键、`⌘C` 有选区复制 / 无选区放行、`⌃C`/`⌃D`/`⌃Z` 放行与中文 IME 组合态透传不受影响。

## 4. 自动化验收标准（必须全绿）

- 更新 `src/features/terminal/__tests__/terminal-panel.test.ts`，覆盖：
  1. `Cmd + V` 按键下不阻止默认事件且不主动调用 `navigator.clipboard.readText()`（交由原生 `paste` 通道直贴 xterm）；
  2. 活跃终端会话下点击「外部终端」直接携带 `activeSession.cwd` 调用 `external_action_run({ id: 'external.terminal', absPath: null, dirPath })`；
  3. 外部终端启动失败（如未安装 iTerm）时，错误消息正确展示在终端顶部 `tone="danger"` 横幅中而非静默吞掉；
  4. `QnBanner` 正确绑定 `tone="danger"` 与 `tone="warning"`。
- 必过命令：`pnpm check:all`。

## 5. 人工点验清单

- [ ] 终端大量输出滚动时，右侧不再出现遮挡文字的滚动条，触控板/滚轮仍可顺畅滚动；
- [ ] 在终端命令行按 `⌘V`，直接静默粘贴至命令行，不再于鼠标位置弹出系统 `Paste` 气泡；
- [ ] 点击终端顶部「外部终端」按钮，直接在当前会话目录打开外部终端（若配置为未安装的 iTerm2，则终端顶部弹出明确红色提示引导到「设置 > 外部与搜索」切换为系统 Terminal / Warp / Ghostty）。
