# M0-03：macOS 原生窗口桥接（`NSPanel`、非激活 Toast、动态激活与热键）

- 所属里程碑：M0
- 关联方案与契约：[quicknote-design-v4.md](../quicknote-design-v4.md) §1.4、§3.1、§5.8、§6 M0；`C2.4`、`C3.4`、`C4.2`、`C4.3`、`C6.3`、`C9.3`、`D-0001`–`D-0004`
- 前置依赖：`M0-02`
- 会话策略：独立新会话执行（从 `dev` 拉出 `feat/M0-03-macos-native-bridge`）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

- **要做**：
  - **选型前置登记（`C11.1`）**：动手写原生桥接代码前，确定 macOS AppKit / Carbon 桥接技术栈（例如 `objc2` 生态 vs 原生 Swift/ObjC 静态桥接库），并在 `DECISIONS.md` 末尾按 `C11.3` 追加记录后方可编码。
  - **Quick Bar 原生 `NSPanel` 桥接（`D-0001`、`D-0002`）**：创建独立非激活面板（`nonactivatingPanel`、`becomesKeyOnlyIfNeeded = false`、`hidesOnDeactivate = false`、`collectionBehavior = 256` 仅 `fullScreenAuxiliary`、默认 `640×420`）；点击外部区域不关闭；监听 `activeSpaceDidChangeNotification` 切 Space 自动复位隐藏（不激活任何应用）并上报 `HideReason = "space_changed"`。
  - **全局 Toast 非激活 `NSPanel`**：创建屏幕底部独立轻量非激活 Toast 面板（`collectionBehavior = 256`、`nonactivatingPanel`），不抢焦点、不切 Space，默认 5 秒自动隐藏；预留事件 `toast:append-undo` 触发通道供实机验证。
  - **动态激活策略与后台常驻（`D-0003`）**：实现 `domain` 层窗口可见计数器与策略状态机；主窗口与设置窗口均隐藏时为 `.accessory`（托盘常驻、无 Dock）；打开任意主窗口或设置窗口时切为 `.regular`（显示 Dock 与原生顶部菜单栏骨架：应用/文件/编辑/格式/视图/工具/窗口/帮助），支持标准窗口与全屏 Space；关闭主窗口只隐藏不退出、`⌘Q` 真正退出；启用单实例唤起；更新 `app_get_info` 返回真实 `activationPolicy`。
  - **全局热键注册与首启自检（`D-0004`）**：通过 Carbon `RegisterEventHotKey` 注册默认 `⌃⌥Space`（`ctrl+alt+space`），支持备用 `⌃⌥⌘Space`（`ctrl+alt+cmd+space`）；实现 `hotkey_update` 热重注册，并将自检状态写入 `ui-state.json`。
  - 实现全部 6 条窗口与热键 IPC 命令：`window_show_main`、`window_hide_main`、`window_show_quick_bar`、`window_hide_quick_bar`、`window_open_settings`、`hotkey_update`，及事件 `window:state` 推送与 `C2.4` 结构化诊断日志。
- **明确不做**：
  - 不实现 Quick Bar 三模式搜索、目标解析或记录写入（归 `M1`/`M3`）。
  - 不实现 45% 侧栏辅助完整交互与分屏样式（归 `M4`，本卡仅定义并在 `WindowMode` 类型中保留 `"auxiliary"` 枚举）。
  - 不在前端编写复杂业务表单或主题样式体系（归 `M0-04`）。

## 2. 允许修改文件白名单

- 共享串行文件（卡内显式声明）：`DECISIONS.md`（若记录原生桥接实现选型）、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json`、`src-tauri/capabilities/**`、`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（自动生成）、`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`
- 后端模块与原生代码：`src-tauri/build.rs`、`src-tauri/src/domain/window*`、`src-tauri/src/domain/hotkey*`、`src-tauri/src/application/window*`、`src-tauri/src/application/hotkey*`、`src-tauri/src/infra/macos/**`（及原生 Swift/ObjC 源文件如选用）、`src-tauri/src/commands/window*`、`src-tauri/src/lib.rs`、`src-tauri/tests/**`

## 3. 具体实现要求

1. **绝对禁止私有接口与非法权限**：严禁调用任何 SkyLight/CGS 私有 Space 排序 API，严禁触发 macOS 辅助功能或屏幕录制权限请求弹窗。
2. **`collectionBehavior` 严守 `256`**：无论是 Quick Bar 还是全局 Toast 浮窗，绝对不能附带 `canJoinAllSpaces`；确保跨 Space 切换无闪烁残影、不会把 `Esc` 误穿透给其他全屏应用。
3. **动态激活策略状态机放进 `domain` 层**：维护 `{ mainVisible, settingsVisible }` → `ActivationPolicy` 纯状态机，确保“打开设置或主窗口任一即 `.regular`，两者皆关即回 `.accessory`”在纯内存中可测试，且不会在 Quick Bar 显隐时误触发策略切换。
4. **结构化原生调试日志（`C2.4`）**：原生桥接事件发生时记录 `event`、`activationPolicy`、`collectionBehavior`、`isKeyWindow`、`hideReason`，严格遵守 `C2.3` 不打印任何用户输入或正文。
5. **非 macOS/Headless 测试兜底**：`infra/macos` 提供可条件编译或抽象 trait 接口，保证 `cargo test --all-features` 在无图形界面测试线程下不 panic 崩溃。

## 4. 自动化验收标准（必须全绿）

- **新增测试**：
  - `src-tauri/tests/window_policy_test.rs`：覆盖动态激活策略状态机全路径（初始 `.accessory` → 开主窗 `.regular` → 开设置仍 `.regular` → 关主窗设置仍在仍 `.regular` → 关设置回 `.accessory`；单独呼出/关闭 Quick Bar 始终不改变策略）、`HideReason` 枚举与 `WindowState` JSON 序列化、以及热键组合字符串解析校验（`ctrl+alt+space`、`ctrl+alt+cmd+space`、非法字符串返回 `ValidationError`）。
- **必过命令**：`pnpm check:all`（返回码 `0`）。

## 5. 人工点验清单（必须上机实测 `C9.3` 核心原生手感）

- [ ] 打开全屏 Safari（或全屏终端），按 `⌃⌥Space`：Quick Bar 稳定覆盖在全屏画面上方，可正常键入文字，原应用未退出全屏且未切离当前 Space。
- [ ] 点击 Quick Bar 窗口外区域：Quick Bar 保持打开不自动隐藏；按下 `Esc` 关闭后焦点完整交还原前台应用。
- [ ] 在 Quick Bar 打开状态下切换 macOS Space（手势或 `⌃→`）：新 Space 无任何闪现残影；回到原 Space 后浮层已复位隐藏（无幽灵窗口）；在新 Space 按 `⌃⌥Space` 可立即重新呼出。
- [ ] 触发调试 Toast 通知测试事件：屏幕底部弹出 5 秒非激活浮窗并自动消失，过程中不打断当前聚焦应用的键盘输入。
- [ ] 通过托盘打开主窗口：观察 Dock 图标与系统顶部完整菜单栏（文件/格式/视图/工具/窗口）出现（`.regular`），主窗口支持进入全屏 Space 并于 Mission Control 拖拽排序持久化；关闭主窗口后 Dock 消失退回后台（`.accessory`），托盘菜单与 `⌃⌥Space` 仍正常响应。
