# DECISIONS

> QuickNote 架构决策账本（ADR）。本文件按 `D-0001` 起顺序记录基线架构决策，后续仅追加新条目、不篡改既有历史记录；若需废止或演进既有决策，按 `C11.3` 新增条目并更新旧条目状态标记。

## D-0001 Quick Bar 与全局 Toast 采用原生非激活 `NSPanel`，而非 Tauri `WebviewWindow`

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C3.4`、`C4.2`、`C4.3`、`C8.3`、`C9.3`
- 背景：Quick Bar 需在任意其他应用全屏 Space 上用热键呼出输入，且 `⌘↵` 提交关闭后需在不打断当前应用的前提下弹出 5 秒撤销 Toast。标准 `WebviewWindow` 在 macOS 全屏 Space 下会被系统遮挡或强制拉离全屏空间，甚至出现不可见却抢占键盘焦点的异常状态。
- 决策：
  - Quick Bar 宿主窗口由 macOS 原生桥接层创建为 `NSPanel`，启用 `nonactivatingPanel`、`becomesKeyOnlyIfNeeded = false`、`hidesOnDeactivate = false`、`level = floating`。
  - 全局 5 秒撤销 Toast 在主窗口隐藏时通过独立的轻量非激活 `NSPanel` 展示，不抢前台焦点也不切 Space；主窗口前台可见时可复用主窗口内 Toast。
  - 原生面板状态通过自定义 IPC 命令 `window_show_quick_bar` / `window_hide_quick_bar` 与事件 `window:state` 同步前端。
- 后果：
  - Quick Bar 脱离 Tauri 默认窗口生命周期，由原生桥接层接管显示、隐藏与焦点行为；无需申请辅助功能或屏幕录制权限。
  - 面板隐藏后自动交还焦点，无需手动重新激活原前台应用。
- 否决替代：使用 Tauri 默认 `WebviewWindow` 强行置顶（全屏空间不可见或吞键）、使用 45% 常规辅助窗口替代全局捕捉面板（遮挡视野过大且不够轻快）。

## D-0002 Quick Bar 不跨 Space：`collectionBehavior` 仅用 `fullScreenAuxiliary`（`256`），点外不关闭

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C3.4`、`C4.3`、`C6.3`、`C9.3`
- 背景：若同时设置 `canJoinAllSpaces | fullScreenAuxiliary`（`257`），切换 Space 时面板会随动画闪现后再消失，或跟随到新 Space 却无法响应 `Esc` 导致误退前台全屏应用。
- 决策：
  - Quick Bar 与全局 Toast 的 `collectionBehavior` 严格设为 `[.fullScreenAuxiliary]`（`256`），禁止设置 `canJoinAllSpaces`。
  - 监听 `NSWorkspace.activeSpaceDidChangeNotification`：当检测到 Space 切换时执行状态复位（关闭面板、上报 `HideReason = "space_changed"` 并保留草稿），且不激活任何应用。
  - **点击面板外部不关闭**：`esc`、`hotkey`、`space_changed` 是仅有的三种关闭入口，确保用户对照其他窗口看内容或拖拽时草稿面板不会误关。
- 后果：面板仅归属于呼出时所在的 Space，切换 Space 后不会闪现残影；用户切到新 Space 后按热键即可重新在当前 Space 呼出并恢复草稿。
- 否决替代：`257` + 切 Space 关闭（动画闪烁不可消除）、失焦即关闭（对照查错时极易误收起面板）。

## D-0003 macOS 激活策略采用动态 `.accessory ↔ .regular` 切换

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C2.4`、`C4.2`、`C6.3`、`C9.3`
- 背景：纯 `.accessory` 应用没有 Dock 图标，但同时无法在 macOS 顶部展示标准应用菜单栏（文件/格式/视图/工具），也影响主窗口独立全屏 Space 的系统切换；而纯 `.regular` 应用在仅呼出 Quick Bar 捕捉时容易抢占系统顶部菜单栏与焦点的视觉预期。
- 决策：
  - 主窗口与设置窗口均隐藏（仅托盘 + Quick Bar 常驻）时，进程运行于 `NSApplication.ActivationPolicy.accessory`。
  - 当任一主窗口或设置窗口被激活打开（含 `⇧⌘↵`、托盘菜单、快捷键、二次启动唤起）时，动态切换为 `.regular`，展示 Dock 图标与完整顶部菜单栏并支持独立全屏 Space。
  - 当主窗口关闭（隐藏至后台）且设置窗口关闭后，自动切回 `.accessory`。
- 后果：兼顾“后台静默捕捉零打扰”与“打开主工作台拥有完整原生菜单与全屏 Space”；需在 `M0` 实机验证切换瞬间与原生 `NSPanel` 焦点交互的稳定性。
- 否决替代：常驻 `.accessory` 并自绘伪菜单栏（违背原生体验，失去全屏 Space 支持）、常驻 `.regular`（呼出浮层时切换系统顶栏干扰当前工作流）。

## D-0004 全局热键定为 `⌃⌥Space`（备用 `⌃⌥⌘Space`）并强制首启自检与热重注册

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C4.2`、`C6.2`、`C6.3`、`C7.2`
- 背景：Carbon `RegisterEventHotKey` 在组合键已被系统或其他常驻软件（如输入法、启动器）占用时仍可能返回 `status = 0`，应用无法仅凭注册返回值察觉热键冲突；同时单 `Option + Space` 组合在日常 macOS 环境下碰撞概率过高。
- 决策：
  - 默认全局呼出热键定为 `⌃⌥Space`（`ctrl+alt+space`），预设备用组合为 `⌃⌥⌘Space`（`ctrl+alt+cmd+space`）。
  - 首次启动或热键变更时触发自检流程，并将状态 `{ combo, backupCombo, selfCheck }`（`"pending" | "ok" | "conflict"`）持久化至 `ui-state.json`。
  - 提供独立快捷键更新指令 `hotkey_update`，允许用户在设置页一键改键或切换备用热键并同步向 Carbon 重新注册。
- 后果：彻底避开单 `Option + Space` 碰撞；即使新环境发生占用，也能通过首启自检向导清晰引导用户改键，避免“热键静默失灵”。
- 否决替代：采用单 `Option + Space`（高度冲突）、仅依赖 Carbon 返回值判定成功（无法感知系统静默抢键）。

## D-0005 内嵌终端采用 `portable-pty` + `tauri::ipc::Channel` + 折叠保活与 30 分钟空闲防遗忘回收

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C3.4`、`C4.2`、`C4.3`、`C6.2`、`C7.3`、`C8.7`
- 背景：用户需要内嵌终端减少窗口切换，但若折叠面板或切换大纲时即刻销毁 PTY 会话，会导致 `npm run dev` 等长任务被强杀且每次切回需承受 2–3 秒冷启动；反之若无限制后台常驻，遗忘关闭多个会话会持续消耗内存与僵尸子进程。
- 决策：
  - 后端使用 `portable-pty` 启动登录 shell，经 `tauri::ipc::Channel<TerminalData>`（16ms 或 64KB 批处理刷新）向前端 `xterm.js` 流式输出；禁用广播事件传递终端大流量输出。
  - **折叠保活**：面板折叠或切至其他右侧板块时保持 PTY 存活（前端隐藏挂载保活实例，Rust 侧维护 1MB 环形输出缓冲）。
  - **状态可视与手动关闭**：终端标签头提供显式 `[× 关闭会话]` 按钮；存在活跃前台子进程（`tcgetpgrp(master_fd) != shell_pgid`）时在选项卡旁显示运行指示点。
  - **防遗忘空闲自动回收**：当面板处于隐藏态、无活跃前台子进程且连续无输入输出达 `idle_recycle_minutes`（默认 `30` 分钟，`0` 为关闭）时，后端自动安全回收该闲置 PTY 并通知前端。
  - **防爆仓与进程清理**：每工作区最多 1 个活跃会话，全局并发上限 3 个；输出连续积压超 10MB 时暂停向前端推送；应用退出或强杀执行 `kill(-pgid, SIGHUP)` → `try_wait ≤ 500ms` → 200ms 后 `SIGKILL` + `RunEvent::Exit` 兜底。
  - **按键隔离**：终端聚焦时 `⌘C` 有选区仅复制不写 PTY、`⌥⌘C` 显式吞掉防穿透、`⌃C` / `⌃D` / `⌃Z` 直接放行给 PTY。
- 后果：兼顾“不中断构建任务、零冷启动切换”和“不遗忘泄漏资源”；终端状态机需按 `C8.7` 覆盖自动化测试。
- 否决替代：折叠即销毁 PTY（杀掉开发服务器且冷启动体验差）、全天候无上限保活（长期忘关积累多份闲置 shell 进程）。

## D-0006 `NoteDoc` 拆为 `meta + body`，正文写入不接触 YAML（ULID 补写为唯一例外）

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C3.4`、`C4.2`、`C5.2`、`C6.4`、`C8.3`
- 背景：若前端编辑器直接读写含 YAML frontmatter 的全文，极易误删 `id`/`type` 等核心字段，且修改标签会触发正文保存假冲突，还导致全文索引污染。
- 决策：
  - `note_read` 返回强拆分结构 `NoteDoc { meta: NoteMeta, body: String, bodyHash: String, ... }`；编辑器仅渲染与修改 `body`，永不接触原始 YAML 字符串。
  - `note_write` 只接收 `body` 与 `baseBodyHash`；后端读出磁盘当前 `meta` 原样保留并重新拼接到新 `body` 前原子写入。
  - **唯一例外**：当库内旧 Markdown 缺失 `id` 时，由后端在 `note_write` 保存阶段自动生成新 ULID 补入 `meta`，并在 `WriteResult` 中返回更新后的 `NoteMeta`；而 `note_append`（Quick Bar 追加）绝不改动 frontmatter。
  - 乐观并发、FTS5 索引、追加 5 秒撤销快照的哈希校验均严格围绕 `body` 与 `bodyHash` 计算，彻底隔离 frontmatter 变更。
- 后果：根除编辑器误改元数据与假冲突，前后端职责边界极其清晰。
- 否决替代：前端整篇读写全文并自行正则剥离/拼接 YAML（高脆弱性且各模块重复解析）。

## D-0007 元数据编辑走顶栏内联 + `[属性]` Popover（`note_update_meta`）+ 列表重命名（`note_rename`）

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C3.4`、`C4.2`、`C6.4`
- 背景：既然正文不显示 YAML frontmatter，若 1.0 仅允许改 `title`/`tags`/`type`，则开发文档的单篇项目字段（`open_dir`、`project`、`code_workspace`、`terminal_dir`）及状态字段（`pinned`、`archived`）将没有 UI 编辑入口。
- 决策：
  - 文档顶栏提供一行内联编辑：`title`、`type` 下拉与 `tags` chip 增删。
  - 文档头增加轻量 `[属性]` Popover，集中编辑 `project`、`open_dir`、`code_workspace`、`terminal_dir`、`pinned`、`archived`。
  - 顶栏与 `[属性]` Popover 统一提交 `note_update_meta` 命令（执行 YAML 最小补丁，不校验正文 `baseBodyHash`）；文件重命名/移动走独立 `note_rename`（不自动改写其他笔记中的相对链接）。
  - 外部 Markdown 文件默认无 frontmatter，点击 `[属性]` 时先弹确认再插入最小 frontmatter。
- 后果：1.0 即可完整支持开发文档属性配置，无需单独做繁重的第三栏属性常驻面板。
- 否决替代：把元数据塞回编辑器文本（破坏 `D-0006`）、将属性编辑推迟到后续大版本（导致开发文档工作区能力残缺）。

## D-0008 应用配置固定单一 `settings.yaml` 及手改语法错误只读容错回退

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C3.4`、`C4.2`、`C4.3`、`C6.2`
- 背景：支持 YAML 与 JSON 双格式会导致状态解析漂移；仅采用 `settings.yaml` 更便于人类手工编辑，但手改语法出错时若粗暴覆盖或静默失效将导致配置丢失。
- 决策：
  - 应用本地配置唯一格式固定为 `$APP_DATA_DIR/settings.yaml`（设置窗口同时提供导入导出备份）。
  - 后端使用 `deny_unknown_fields` 强校验加载；当检测到手改文件存在 YAML 语法错误、未知字段或非法类型时，**绝不覆盖修改磁盘原文件**，自动回退至内存中上一次有效配置（冷启动即无效则回退默认配置），并通过事件上报精确行号、列号与错误字段，在界面展示错误横幅及“打开 `settings.yaml` 修复”入口。
- 后果：消除双格式维护负担，同时给予手改配置最高级别的安全兜底。
- 否决替代：混用 `settings.yaml` 与 `settings.json`、配置解析失败时直接覆写重置为默认文件（造成用户手写配置瞬间丢失）。

## D-0009 IPC 类型唯一来源为 Rust（`export_bindings.rs` → `src/bindings/types.ts`），手写封装归入 `src/lib/ipc/`

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C1.2`、`C1.4`、`C3.1`、`C3.2`
- 背景：AI 跨会话修改前后端代码时，若允许手写双端类型或把胶水函数混入生成物目录，极易造成序列化字段名与可选性漂移。
- 决策：
  - 全部 IPC 结构体与枚举只在 Rust 侧定义，统一加 `#[serde(rename_all = "camelCase")]` 与 `#[derive(specta::Type)]`。
  - 导出器固定位于 `src-tauri/src/bin/export_bindings.rs`，由 `pnpm gen:bindings` 驱动生成唯一文件 `src/bindings/types.ts`（首行必须为 `/* GENERATED — DO NOT EDIT. */`）。
  - 手写 `invoke` 调用与事件监听封装集中于 `src/lib/ipc/commands.ts` 与 `src/lib/ipc/events.ts`，只导入 `@/bindings/types`，不反向依赖任何 `features/*`；UI 组件严禁直接引入 `@tauri-apps/api`。
- 后果：Rust 字段改动若不同步生成绑定，前端 `tsc` 与门禁校验会立刻失败拦截。
- 否决替代：前后端各自手写 TS 接口、把手写命令封装直接放进 `src/bindings/`（破坏生成物单一职责）。

## D-0010 Rust 全部测试统一落位 `src-tauri/tests/**`，生产代码零 `unwrap()` / `expect()`

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C1.1`、`C2.1`、`C9.1`、`C9.2`
- 背景：在生产文件内混写 `#[cfg(test)] mod tests` 会干扰静态扫描器对生产代码 `unwrap()` / `expect()` 的严格拦截，也不利于共享静态 Fixture 库的组织。
- 决策：
  - Rust 生产源码（`src-tauri/src/**`）严禁出现 `unwrap()` 与 `expect()`，错误统一通过 `?` 与 `AppError` 向上传递。
  - 所有 Rust 单元测试与集成测试统一放置于 `src-tauri/tests/*.rs`（经由 library crate 公开接口或测试专用纯函数模块测试），测试数据统一引用 `src-tauri/tests/fixtures/vault/`。
- 后果：静态扫描命令可无例外地精确校验生产代码健壮性，避免 AI 在生产路径留下 panic 隐患。
- 否决替代：允许生产文件中内联 `#[cfg(test)]` 并靠复杂 AST 白名单豁免检查。

## D-0011 macOS 原生桥接采用 Objective-C 静态桥接库（`cc` + `bridge.m`）结合 Tauri v2 托盘与菜单能力

- 日期：2026-11-23
- 状态：accepted（`C11.3`）
- 关联：`C1.2`、`C2.4`、`C3.4`、`D-0001`、`D-0002`、`D-0003`、`D-0004`
- 背景：QuickNote 需要将 Tauri `WebviewWindow` 转为非激活 `NSPanel`（`nonactivatingPanel`、`collectionBehavior = 256`、`canBecomeKeyWindow` 按窗口区分）、监听 `NSWorkspaceActiveSpaceDidChangeNotification` 复位切 Space 浮层，并通过 Carbon `RegisterEventHotKey` 实现 `⌃⌥Space` 全局热键注册、首启自检与热重注册。同时需要托盘常驻、动态顶部菜单栏、`.accessory ↔ .regular` 激活策略切换与单实例唤起。
- 决策：
  - 将 Tauri 不直接暴露的底层 AppKit/Carbon 能力（`NSPanel` 类转换与行为控制、`NSWorkspace` Space 切换监听、Carbon `RegisterEventHotKey` / `UnregisterEventHotKey`）收敛于原生 Objective-C 静态桥接文件 `src-tauri/src/infra/macos/bridge.m`，通过 `src-tauri/build.rs` 调用 `cc`（启用 `-fobjc-arc`，链接 `Cocoa` 与 `Carbon` framework）编译并经由 `extern "C"` FFI 暴露给 `src-tauri/src/infra/macos/`。
  - 托盘图标（`tauri` `tray-icon` 特性）、原生顶部菜单栏（`tauri::menu`）、动态激活策略（`AppHandle::set_activation_policy`）与单实例唤起（`tauri-plugin-single-instance`）复用 Tauri v2 原生能力。
  - `infra/macos` 对外提供 `NativeWindowBridge` trait 抽象与非 macOS / 无图形界面测试兜底实现（`NoopNativeBridge`），保证 `cargo test --all-features` 在 headless 环境下零 panic 运行。
- 后果：原生 AppKit/Carbon 调用直接对齐 Apple 官方头文件，无宏样板与特征开关膨胀风险；`domain` 层保持纯内存状态机与热键解析，严格遵守 `C1.2` 单向分层。
- 否决替代：使用 `objc2` / `objc2-app-kit` 纯 Rust 宏定义 `NSPanel` 子类（需维护大量 feature flag 与生命周期样板，且 Carbon 热键仍需手写 C FFI）；使用 `tauri-plugin-global-shortcut`（无法实现 `D-0004` 所需的 Carbon 热键自检与精确热重注册控制）。
