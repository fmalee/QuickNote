# M4-05：45% 侧栏辅助窗口模式（Auxiliary）与全屏工作台完善

- 所属里程碑：M4
- 关联方案与契约：`v4` §3.1、§3.2、§5.8、§5.9、§6 M4；`C1.2`、`C2.2`、`C4.2`、`C4.3`、`C6.2`、`C6.3`、`C7.1`–`C7.4`、`C9.1`、`C9.3`、`D-0003`
- 前置依赖：`M4-04` 已合并至 `dev`（且 `M4-01b` 已实现 `WindowMode::Auxiliary` 原生桥接）
- 建议分支：`feat/M4-05-auxiliary-window-and-fullscreen`
- 会话策略：独立新会话执行（零大文档读取，所有窗口模式状态流转、布局规则、菜单入口与设置绑定已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **主窗口模式状态同步与布局适配（`src/app/MainWorkbenchShell.vue` + `src/features/panels/store.ts`）**：
   - 在 `MainWorkbenchShell.vue` 维护当前主窗口模式 `mainWindowMode: Ref<WindowMode>`（初始从 `ui_state_get().windows.main.mode` 与 `settingsStore.settings.mainWindow.mode` 恢复，默认 `'standard'`）；
   - 在既有 `onWindowState` 监听器中，当收到 `state.windowId === 'main'` 且 `state.mode !== null` 时，实时更新 `mainWindowMode.value = state.mode`，并防抖同步 `ui_state_set('windows', ...)` 持久化最新模式；
   - **45% 侧栏辅助模式（`mainWindowMode === 'auxiliary'`）布局规则（严格对齐 `v4` §3.1、§3.2 与 `C7.4`）**：
     - 默认隐藏左侧导航+列表栏（`NotesSidebar`），仅保留顶部精简标题栏、标签栏、文档头、编辑器与右侧面板组，最大化同屏对照编辑面积；
     - 顶部标题栏显示醒目“侧栏辅助”徽标及一键“退出侧栏辅助（返回标准窗口）”按钮（`data-testid="exit-auxiliary-mode-btn"`），同时提供临时抽屉按钮允许用户在辅助模式下快速按需拉出导航栏选择文档；
     - 在 `src/features/panels/store.ts` 新增 `forceOverlayDrawer: Ref<boolean>`（由 `mainWindowMode === 'auxiliary'` 驱动）：当 `isOverlayDrawer` 计算属性为 `viewportWidth < 900 || forceOverlayDrawer` 时，右侧面板组一律以覆盖式抽屉浮于编辑区右侧，**绝不挤压编辑区至 `< 480px`**。
   - **标准模式 ↔ 45% 侧栏辅助模式 ↔ 全屏工作台互切**：
     - 提供 `toggleAuxiliaryMode()`：若当前为 `'auxiliary'` 则调用 `windowShowMain({ mode: 'standard' })` 恢复原标准窗口；否则调用 `windowShowMain({ mode: 'auxiliary' })` 贴靠当前屏幕右侧 45%（按设置中的 `auxiliaryWidthPercent`）；
     - 标题栏提供紧凑模式切换按钮（`data-testid="toggle-auxiliary-mode-btn"`，提示“切换 45% 侧栏辅助模式”）。
2. **统一动作注册表与原生“视图”菜单集成**：
   - 在 `src/lib/actions/catalog.ts` 注册动作：
     - `window.toggleAuxiliary`：标题“切换 45% 侧栏辅助模式”，`category: 'window'`，`defaultKeys: null`（避免侵占常用组合键，用户可在设置页自由录制），`scope: 'main-window'`，`menuGroup: 'view'`；
   - 在 `src-tauri/src/lib.rs` 系统“视图”子菜单新增 `window.toggleAuxiliary`（“切换 45% 侧栏辅助模式”）菜单项并通过既有 `menu:action` 通道触发；
   - 在 `src/app/MainWorkbenchShell.vue` 绑定 `window.toggleAuxiliary` 动作处理器（使其同时可通过 `⌘K` 快速打开的 `>` 动作模式与系统“视图”菜单调用）。
3. **设置窗口主窗口形态、侧栏宽度比例、全屏引导与 Quick Bar 回收周期绑定**：
   - 在 `src/features/settings/SettingsView.vue`（“通用与热键”分区）新增“主窗口形态与内存回收”卡片（`data-testid="settings-window-mode-card"`）：
     - **默认窗口模式**（绑定 `settings.mainWindow.mode`）：支持在 `standard`（标准窗口，默认）与 `auxiliary`（45% 侧栏辅助，标注“默认关闭，适合全屏阅读对照记录”）之间切换；并提供“立即以侧栏辅助模式显示主窗口”快捷预览按钮（调用 `windowShowMain({ mode: 'auxiliary' })`）；
     - **侧栏辅助宽度比例**（绑定 `settings.mainWindow.auxiliaryWidthPercent`）：数值范围 `30%–60%`（默认 `45%`），修改并保存后若主窗口当前处于辅助模式，调用 `windowShowMain({ mode: 'auxiliary' })` 实时生效；
     - **Quick Bar 隐藏后内存释放延时**（绑定 `settings.quickBar.destroyAfterHiddenMinutes`，默认 `10` 分钟，`0` 为不自动销毁，对齐 `v4` §5.8 内存预算设置项完整性）；
     - **全屏工作台 Mission Control 排序提示**（`v4` §3.1）：简洁提示框说明“按 `⌃⌘F` 进入全屏工作台后，可在 Mission Control（`⌃↑`）中一次性将 QuickNote 空间拖至主工作应用左侧，macOS 将自动持久化排序”。
4. **Rust 原生全屏状态回传微调（如有需要）**：
   - 核查 `src-tauri/src/lib.rs` 主窗口 `WindowEvent::Resized` / 全屏切换时是否同步更新 `WindowPolicyState::set_main_fullscreen`；若主窗口通过系统绿灯/`⌃⌘F` 进出原生全屏，确保 `WindowManager` 与前端 `window:state` 同步 `fullscreen` 与 `mode`（避免全屏态下切 `auxiliary` 误判状态）。

### 明确不做
- 不修改 Quick Bar 与 Toast 的 `NSPanel` 行为（严格保持 `D-0001` / `D-0002` 不动）。
- 不实现自动介入其他 App 窗口布局或 Split View（`v4` §3.1 明确声明不改变其他 App 尺寸）。

## 2. 允许修改文件白名单

### 业务与测试文件
- `src-tauri/src/lib.rs`（仅补充“视图”菜单 `window.toggleAuxiliary` 与主窗全屏状态同步）
- `src-tauri/src/application/window.rs`（仅按需补充 `sync_main_fullscreen_state` 方法）
- `src/lib/actions/catalog.ts`
- `src/features/panels/store.ts`
- `src/app/MainWorkbenchShell.vue`
- `src/features/settings/SettingsView.vue`
- `src/features/settings/store.ts`
- `src/app/__tests__/auxiliary-window-mode.test.ts`（新增）
- `src/features/panels/__tests__/right-panel-group.test.ts`

## 3. 具体实现要求

1. **默认形态保守原则（`v4` §3.1）**：
   - 首次安装与默认配置下 `mainWindow.mode` 始终保持 `standard`（45% 侧栏辅助默认关闭，仅由用户主动切换或设置启用）。
2. **响应式与覆盖抽屉零挤压（`C7.4`）**：
   - 辅助模式下展开右侧面板（如大纲或内嵌终端）必须浮于编辑区上方并附带半透明遮罩点击收起，编辑区宽度不受挤压。
3. **样式 Token 红线（`C7.1`）**：
   - 所有新增徽标、工具按钮与提示卡片均仅使用 `--qn-*` CSS Token。

## 4. 自动化验收标准（必须全绿）

- 新增测试 `src/app/__tests__/auxiliary-window-mode.test.ts`，至少覆盖：
  1. 收到 `window:state(windowId = 'main', mode = 'auxiliary')` 时主工作台隐藏默认导航栏、展示“退出侧栏辅助”按钮，且 `panelsStore.isOverlayDrawer` 强制为 `true`；
  2. 点击“退出侧栏辅助”或触发 `window.toggleAuxiliary` 动作正确调用 `window_show_main({ mode: 'standard' })`；在标准模式下触发则调用 `window_show_main({ mode: 'auxiliary' })`；
  3. 设置窗口修改 `mainWindow.mode`、`mainWindow.auxiliaryWidthPercent`（夹取 `30–60`）与 `quickBar.destroyAfterHiddenMinutes` 正确调用 `settings_update`。
- 必过命令：`pnpm check:all`。

## 5. 人工点验清单（涉及 macOS 原生窗口行为，`C9.3`）

- [ ] 将 Safari 或 VS Code 置于 macOS 全屏 Space，通过菜单栏或快捷动作切换至 **45% 侧栏辅助窗口**：确认 QuickNote 覆盖于全屏 App 右侧约 45% 宽度，不触发 Split View、不改变背景全屏 App 尺寸；
- [ ] 在侧栏辅助模式下打开右侧大纲/终端面板，确认以覆盖式抽屉浮现而不挤压编辑区；点击“返回标准窗口”确认恢复原标准窗口尺寸与位置；
- [ ] 按 `⌃⌘F` 进入全屏工作台，在 Mission Control（`⌃↑`）拖拽排序并验证返回桌面与隐藏/再显示期间 `.accessory ↔ .regular` 激活策略正常。
