# M4-audit：M4 独立只读审查问题清零（2 项阻断 + 6 项中等问题）

- 所属里程碑：M4（收官审查修复）
- 关联方案与契约：`v4` §3.1、§5.5、§5.6.7、§5.7、§5.11；`C2.1`、`C4.2`、`C6.2`、`C6.3`、`C7.1`、`C8.5`、`C8.7`、`C9.1`、`D-0003`、`D-0005`、`D-0006`、`D-0008`
- 前置依赖：`M4-01a`–`M4-06` 均已合并至 `dev`（基于 M4 独立只读审查报告分拣）
- 建议分支：`fix/M4-audit`
- 会话策略：独立新会话执行（零大文档读取，所有审查定位、根因与精确修复方案已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做（精准清零 M4 审查 2 项 🔴 + 6 项 🟡 + 1 项 🟢 烟测）

1. **🔴#1 修复 45% 侧栏辅助窗口默认模式、冷启动恢复及无参唤起前后端状态脑裂**：
   - **后端（`domain/window.rs` + `application/window.rs` + `lib.rs`）**：
     - 在 `WindowPolicyState` 新增 `pub fn with_initial(main_visible: bool, main_mode: WindowMode) -> Self`（当 `main_mode == WindowMode::Fullscreen` 时同步 `main_fullscreen = true`）；
     - 在 `WindowManager::new` 支持传入初始 `WindowMode`（或在 `lib.rs` `setup` 中读取 `pm.ui_state_get().windows.main.mode` 与 `pm.settings_get().settings.main_window.mode`：若 `ui_state` 为有效模式取之，首次未配置则回退 `settings.main_window.mode`）；
     - 若启动时 `main` 窗口已可见且初始模式为 `WindowMode::Auxiliary`，在 `lib.rs` `setup` 中立即应用原生辅助窗口几何与 `fullScreenAuxiliary`；
     - 在 `WindowManager::show_main(app, mode, auxiliary_width_percent)` 中：取实际生效模式 `effective_mode = mode.unwrap_or(win_state.mode.unwrap_or(WindowMode::Standard))`；当 `mode.is_some()` **或** `effective_mode == WindowMode::Auxiliary` 时，均驱动 `bridge.configure_main_window_mode(ns_ptr, effective_mode, auxiliary_width_percent)`，消除托盘/单实例/`mode = None` 唤起时前端无左栏但原生窗口仍为标准全宽的脑裂。
   - **前端（`MainWorkbenchShell.vue` + `settings/store.ts`）**：
     - 在 `src/features/settings/store.ts` 修改 `mainWindow.mode`（`updateMainWindowMode`）成功后，除调用 `settings_update` 外，同步更新 `ui_state_set('windows', { ...windows, main: { ...windows.main, mode } })`，防止重启后旧 `ui-state.json` 遮蔽用户在设置页修改的“默认窗口模式”；
     - 在 `src/app/MainWorkbenchShell.vue::restoreMainWindowMode()` 中：若恢复结果为 `'auxiliary'`，除更新前端 `applyMainWindowMode('auxiliary')` 外，立即调用 `windowShowMain({ mode: 'auxiliary' })` 驱动 macOS 原生窗口同步贴边与启用 `fullScreenAuxiliary`（测试/非 Tauri 环境静默容错）。

2. **🔴#2 修复多工作区新建终端会话未将旧活跃会话切为 `hidden_alive`**：
   - 位置：`src/features/terminal/store.ts::spawnOrFocusContext`。
   - 修复：在 `spawnTerminalSession` 前记录 `const prevId = activeSessionId.value;`；新会话 `info` 创建成功并写入 `sessions.value` 后：
     - 若 `prevId && prevId !== info.sessionId && sessions.value[prevId]`，立即异步调用 `setTerminalVisibility({ sessionId: prevId, visible: false })`，并在本地将 `sessions.value[prevId]` 的 `state` 同步更新为 `'hidden_alive'`；
     - 若创建完成瞬间 `!isPanelVisible.value`，也对新会话调用 `setTerminalVisibility({ sessionId: info.sessionId, visible: false })`。
   - 确保旧工作区会话立即进入 1MB 环形缓冲蓄水与 30 分钟空闲回收资格。

3. **🟡#1 修复 `EditorMergeView.vue` 与 `SettingsView.vue` 未定义 `--qn-*` CSS Token（`C7.1`）**：
   - 将 `EditorMergeView.vue` 与 `SettingsView.vue` 中所有未在 `src/styles/tokens.css` 定义的 Token 替换为合法现有 Token：
     - `--qn-bg-surface` → `--qn-bg-sidebar`（工具栏/槽位）或 `--qn-bg-elevated`（输入框/悬浮条）；
     - `--qn-text-tertiary` → `--qn-text-muted`；
     - `--qn-border-default` → `--qn-border-subtle`（常规边框）或 `--qn-border-strong`（输入控件边框）。
   - 排查全部 M4 新增/修改的 `.vue` 文件（`TerminalPanel.vue`、`EditorMergeView.vue`、`ExportPdfModal.vue`、`SettingsView.vue`、`SettingsExternalPanel.vue`），确认所有 `var(--qn-*)` 均精确存在于 `src/styles/tokens.css`。

4. **🟡#2 修复原生全屏态直接切入辅助模式的 AppKit 异步退全屏竞态与标准 Frame 污染**：
   - 位置：`src-tauri/src/infra/macos/bridge.m`（`qn_set_main_window_mode`）与 `src-tauri/src/application/window.rs`。
   - 修复：
     - 在 `bridge.m` 中检测 `BOOL isFullScreen = (win.styleMask & NSWindowStyleMaskFullScreen) != 0;`；
     - **防标准 Frame 污染**：仅当 `!g_is_auxiliary_mode && !isFullScreen` 时保存 `g_saved_standard_frame = win.frame`；
     - **消除退全屏异步竞态**：当 `mode == 2 (Auxiliary)` 且 `isFullScreen == YES` 时，先注册一次性 `NSWindowDidExitFullScreenNotification` 观察者（附 3 秒清理超时保护），调用 `[win toggleFullScreen:nil]`，待系统完成退全屏空间动画回调后，再在主线程应用 `NSWindowCollectionBehaviorFullScreenAuxiliary`、`NSFloatingWindowLevel` 与右侧 `auxiliary_width_percent` 贴边 `setFrame:display:animate:YES`；
     - 若 `mode == 0 (Standard)` 且之前挂起过退全屏切辅助观察者，立即取消该观察者防回跳；
     - 在 `application/window.rs::show_main` 中，当目标模式为 `WindowMode::Auxiliary` 且窗口当前处于全屏（`win.is_fullscreen().unwrap_or(false)`）时，由 `bridge.configure_main_window_mode` 统一接管优雅退全屏再贴边，避免 Rust 侧无等待 `set_fullscreen(false)` 与后续同步 `setFrame` 竞态。

5. **🟡#3 优化终端 10MB 背压统计，拦截中低速持续刷屏**：
   - 位置：`src-tauri/src/application/terminal.rs`。
   - 修复：将当前每 `1000ms` 硬归零的 `unconsumed_bytes_in_window` 改为**静默间隙复位（`BACKPRESSURE_QUIET_RESET_MS = 2_000`）**：
     - 在 `Active` 态收到输出时，仅当距上一次输出的时间间隔 `now.saturating_sub(prev_output_at_ms) >= BACKPRESSURE_QUIET_RESET_MS`（出现 ≥ 2 秒静默停顿）时才重置 `continuous_output_bytes = 0`；
     - 否则持续累加 `continuous_output_bytes += bytes.len()`；一旦 `continuous_output_bytes >= TERMINAL_BACKPRESSURE_LIMIT_BYTES`（10MB），立即将 `output_paused = true`、停止推送并广播 `terminal:session-state`；
     - 用户主动输入（`write_session`）时若未处于 `output_paused` 可清零 `continuous_output_bytes`，避免正常长时间交互式会话误触发暂停；同时保障 5–9MB/s 等中低速持续刷屏在累计达 10MB 时必被拦截。

6. **🟡#4 将 `terminal_kill` 改为异步卸载至阻塞线程池并消除冗余 200ms 等待**：
   - 位置：`src-tauri/src/commands/terminal.rs`、`src-tauri/src/infra/pty.rs`。
   - 修复：
     - 将 `commands/terminal.rs::terminal_kill` 改为 `pub async fn terminal_kill(state: State<'_, Arc<TerminalManager>>, args: TerminalKillArgs) -> Result<(), AppError>`，使用 `tauri::async_runtime::spawn_blocking(move || manager.kill_session(args))` 避免阻塞 UI 主线程；
     - 在 `infra/pty.rs::terminate_process_group` 中：`SIGHUP` 后轮询 `try_wait` 最长 `500ms`；若超时仍存活，立即发 `SIGKILL` 并轮询等待最多 `200ms`（外加兜底 `wait()` 回收僵尸），移除发 `SIGKILL` 前多余的 200ms 空等。

7. **🟡#5 修复 `MergeView` 合并提交成功时未同步外部变更 `diskMeta`**：
   - 位置：`src/features/editor/store.ts::commitConflictMerge`。
   - 修复：保存前先取出 `const diskMeta = tab.conflictState.diskMeta;`；`writeNote` 成功后，若 `res.updatedMeta !== null` 沿用现有逻辑更新 `tab.doc.meta`；否则若 `diskMeta !== null` 则同步 `tab.doc.meta = diskMeta` 并触发 `notifyNotesRefresh()`，保证外部同时修改 frontmatter + 正文合并后 `DocHeaderBar` 展示最新元数据。

8. **🟡#6 修复 `settings_import` 后热键状态与 `backupCombo` 不同步**：
   - 位置：`src-tauri/src/commands/settings.rs`、`src/features/settings/store.ts`。
   - 修复：
     - 后端 `settings_import` 成功后，无论修改的是 `quick_bar` 还是 `quick_bar_backup`：若 `quick_bar` 变更则热重注册热键；同时确保将最新的 `combo` 与 `backup_combo` 同步更新至 `ui-state.json` 的 `hotkey` 字段；
     - 前端 `useSettingsStore.importSettings` 成功返回后，主动调用 `refreshUiState()`（或重新拉取 `ui_state_get` 更新 `hotkeyStatus`），确保“通用与热键”卡片立即展示导入后的最新主/备热键与自检状态。

9. **🟢#2 补充 `MergeView` 真实 `@codemirror/merge` 挂载烟测**：
   - 在 `src/features/editor/__tests__/merge-view-conflict.test.ts` 新增一条不 mock `@codemirror/merge`（或单独文件 `merge-view-real-smoke.test.ts`）的轻量组件挂载测试，验证真实 `MergeView` 在 jsdom 下正常创建 `.cm-mergeView` 双栏 DOM 并安全卸载不报错。

### 明确不做
- 不在本卡实现 PDF 导出本地相对图片解析（🟢#1，留待 M5 沙箱评估）与 `check-architecture.mjs` 历史 Token 全仓扫描改造（🟢#3，留待 M5）。
- 不修改版本号 `0.1.0` 及任何 M0–M3 历史契约。

## 2. 允许修改文件白名单

### 业务与测试文件
- `src-tauri/src/domain/window.rs`
- `src-tauri/src/application/window.rs`
- `src-tauri/src/application/terminal.rs`
- `src-tauri/src/infra/macos/bridge.m`
- `src-tauri/src/infra/pty.rs`
- `src-tauri/src/commands/terminal.rs`
- `src-tauri/src/commands/settings.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tests/m4_terminal_lifecycle.rs`
- `src-tauri/tests/m4_export_and_auxiliary.rs`
- `src/app/MainWorkbenchShell.vue`
- `src/app/__tests__/auxiliary-window-mode.test.ts`
- `src/features/terminal/store.ts`
- `src/features/terminal/__tests__/terminal-panel.test.ts`
- `src/features/editor/EditorMergeView.vue`
- `src/features/editor/store.ts`
- `src/features/editor/__tests__/merge-view-conflict.test.ts`
- `src/features/editor/__tests__/merge-view-real-smoke.test.ts`（新增，可选）
- `src/features/settings/SettingsView.vue`
- `src/features/settings/store.ts`
- `src/features/settings/__tests__/settings-backup-and-terminal.test.ts`

## 3. 具体实现要求

1. 严格遵守 `C1.2` 单向分层、`C2.1` Rust 生产代码零 `unwrap()`/`expect()`、`C2.3` 零敏感日志、`C7.1` 所有 CSS Token 均存在于 `src/styles/tokens.css`。
2. 严禁修改 `src/bindings/types.ts`（本卡不新增/修改任何 IPC 结构体字段）。

## 4. 自动化验收标准（必须全绿）

- 补充/更新回归测试覆盖上述全部 8 项修复 + 1 项真实 `MergeView` 烟测：
  1. `m4_terminal_lifecycle.rs`：跨 1 秒持续多批输出累计达 10MB 必触发 `output_paused = true`；静默 ≥ 2 秒或用户输入后计数复位不误暂停；
  2. `m4_export_and_auxiliary.rs`：`WindowPolicyState::with_initial(true, WindowMode::Auxiliary)` 初始化与无参 `show_main(None)` 保持 `Auxiliary` 模式；`settings_import` 同步更新 `ui_state.hotkey.backup_combo`；
  3. `terminal-panel.test.ts`：多工作区新建终端会话时旧活跃会话自动触发 `terminal_set_visibility(prevId, false)` 并转入 `hidden_alive`；
  4. `auxiliary-window-mode.test.ts`：冷启动恢复 `auxiliary` 模式会主动调用 `windowShowMain({ mode: 'auxiliary' })`；设置页修改默认模式同步持久化至 `ui_state`；
  5. `merge-view-conflict.test.ts` + 真实挂载烟测：合并写回同步 `diskMeta`；真实 `@codemirror/merge` 双栏挂载与卸载无报错；
  6. `settings-backup-and-terminal.test.ts`：`importSettings` 成功后主动刷新 `hotkeyStatus`。
- 必过命令：`pnpm check:all`（含全量 Rust 与前端测试、clippy、eslint、vue-tsc、check:arch、gen:bindings 零差异）。

## 5. 人工点验清单

- 合入 `dev` 后将连同 M4 全量功能执行 `C9.3` 全部 5 项 + M4 专项 macOS 实机桌面点验。
