# M5-02e：修复 45% 侧栏在全屏 Space 下顶部未贴边（R35）

> **状态：已完成并实机通过（2026-11-26）。**
>
> 最终落地设计（已与用户确认并完成全场景实机验证）：
> 1. **临时去标题栏并放行键焦点**：进入 45% 侧栏时保存原 `styleMask` 并移除 `NSWindowStyleMaskTitled`（隐藏红绿灯），同时在辅助模式期间放行 `canBecomeKeyWindow` 返回 `YES`，以绕过 AppKit `constrainFrameRect:toScreen:` 对 `visibleFrame` 的顶部截断；退出时恢复原 `styleMask`（并兜底保证补回 `NSWindowStyleMaskTitled`）。
> 2. **跨进程全屏 Space 判定**：通过 `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, ...)` 检查当前屏幕是否存在覆盖整块 `screen.frame` 的其他进程 `layer == 0` 窗口，在托盘 / Quick Bar / 热键三种入口下均稳定区分桌面与全屏 Space。
> 3. **Space 切换自动重算**：监听 `NSWorkspaceActiveSpaceDidChangeNotification`，在桌面与全屏 Space 之间切换时自动重算并调整侧栏高度。
> 4. **全屏 Space 退出侧栏回桌面不回弹**：`window.rs` 在 macOS 退出辅助模式时交由原生层控制上屏；`qn_exit_auxiliary_mode_locked` 先 `orderOut` 并保留 `CanJoinAllSpaces`（去掉 `FullScreenAuxiliary`），切 `.regular` 触发系统回到桌面 Space，待 `NSWorkspaceActiveSpaceDidChangeNotification` 后在桌面 Space 上屏再剥离 `CanJoinAllSpaces`，彻底避免窗口被绑定在原全屏 Space。
> 5. **全屏主窗口切侧栏状态竞态修复**：`domain/window.rs` 的 `set_main_fullscreen` 在 `main_mode == Auxiliary` 时不再被退全屏动画中的 `Resized` 事件覆盖；`NSWindowDidExitFullScreenNotification` 退全屏完成后补存标准窗口 `frame` / `styleMask`，退出侧栏时经 `qn_clamp_rect_to_visible` 兜底约束在 `visibleFrame` 内。

- 所属里程碑：M5
- 关联方案与契约：`v5` §6；C2.4、C9.1、C9.3；D-0011、D-0012
- 前置依赖：M5-01a
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：45% 侧栏辅助模式在「桌面（有菜单栏）」与「全屏 Space（无菜单栏）」两种场景下都正确贴靠：桌面不盖住菜单栏，全屏 Space 顶部贴到屏幕顶边。
- 不做：不改退出全屏的异步等待逻辑、不改 `collectionBehavior`、不改宽度计算。

## 2. 允许修改文件白名单

- `src-tauri/src/infra/macos/bridge.m`

## 3. 具体实现要求

### 3.1 现状

`qn_apply_auxiliary_docking`（基线约 610–655 行）用 `screen.visibleFrame` 计算目标 frame（约 641–650 行）：

```objc
NSRect visible = screen.visibleFrame;
CGFloat w = round(visible.size.width * ((CGFloat)clamped / 100.0));
...
CGFloat x = visible.origin.x + (visible.size.width - w);
NSRect target = NSMakeRect(x, visible.origin.y, w, visible.size.height);
```

`visibleFrame` 在任何 Space 下都扣掉了菜单栏高度。全屏 Space 里其实没有可见菜单栏，于是顶部空出一截。

### 3.2 修法

把 frame 计算改为「顶部按 `screen.frame` 顶边，底部按 `visibleFrame.origin.y`」：

```objc
NSRect frame = screen.frame;
NSRect visible = screen.visibleFrame;
CGFloat w = round(visible.size.width * ((CGFloat)clamped / 100.0));
// 原有的 w 上下限裁剪保持不变
CGFloat x = frame.origin.x + (frame.size.width - w);
CGFloat top = frame.origin.y + frame.size.height;       // 屏幕物理顶边
CGFloat bottom = visible.origin.y;                       // 仍避开 Dock
NSRect target = NSMakeRect(x, bottom, w, top - bottom);
```

然后在「桌面场景」避免盖住菜单栏：目标 frame 的顶部若高于 `visible` 顶边，且当前屏幕**不是**处于另一个应用的全屏 Space，则把顶部夹回 `visible` 顶边。

是否处于全屏 Space 的判定由你在实机上确定，建议方案（任选其一，以实测为准）：

- 方案 A：`NSApp.windows` 与 `[[NSWorkspace sharedWorkspace] runningApplications]` 中前台应用的窗口存在 `styleMask & NSWindowStyleMaskFullScreen`。
- 方案 B：比较贴靠后 `[NSScreen mainScreen].frame` 与 `[[NSApp mainWindow] screen]` 的关系。

把最终采用的判定方式与实测现象写进提交说明。

### 3.3 约束

- 只能用 AppKit 公开 API（D-0011）。
- 编译受 `build.rs` 的 `-Wall -Wextra` 约束，不得产生新警告。
- 不新增 `NSLog`；若需要诊断，沿用文件内已有的结构化诊断字段，不得输出窗口标题等内容。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（其中 `cargo clippy -D warnings` 会重新编译 `bridge.m`）。

## 5. 人工点验清单（C9.3 第 4 条扩展，两种场景都必须验）

- [ ] 桌面（可见菜单栏）：切到 45% 侧栏，顶部不遮挡菜单栏，底部不遮挡 Dock，宽度为设定百分比。
- [ ] 全屏 Space（如 Safari 全屏）：切到 45% 侧栏，顶部贴屏幕顶边、无空白，不改变 Safari 窗口尺寸，不切换 Space。
- [ ] 两种场景下退出侧栏辅助：窗口恢复到进入前的位置与大小。
- [ ] 从全屏的主工作台切换到 45% 侧栏：等待退全屏动画结束后才贴靠，无错位。

## 上下文提示

- 只读 `bridge.m` 约 586–660 行与约 123–135 行（`qn_active_screen`）。
- 忽略：Quick Bar / Toast 面板代码、热键代码。
