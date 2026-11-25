# M5-06a1：Quick Bar 失焦关闭——原生层与窗口状态机（R30）

- 所属里程碑：M5
- 关联方案与契约：`v5` §5-1；C2.3、C2.4、C3.4、C4.3、C9.3；D-0001、D-0002、D-0011、D-0013
- 前置依赖：M5-01a（`DECISIONS.md` 已有 `D-0013`，状态 `proposed`）
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：Quick Bar 面板失去键窗口焦点时通知 Rust，由窗口状态机决定是否隐藏并上报新的关闭原因 `focus_lost`。
- 不做：「固定」开关（M5-06a2 / M5-06a3 负责）。本卡完成后，失焦**总是**隐藏——这是预期中间态，两张卡连续合入即可。ADR 状态流转也由 M5-06a2 完成。

## 2. 允许修改文件白名单

- `src-tauri/src/infra/macos/bridge.m`
- `src-tauri/src/infra/macos/mod.rs`
- `src-tauri/src/application/window.rs`
- `src-tauri/src/domain/window.rs`
- `src-tauri/src/lib.rs`

测试文件（不计入上限）：`src-tauri/tests/window_policy_test.rs`

## 3. 具体实现要求

### 3.1 `domain/window.rs`

```rust
#[serde(rename_all = "camelCase")]
pub enum HideReason {
    Esc,
    Hotkey,
    #[serde(rename = "space_changed")]
    SpaceChanged,
    #[serde(rename = "focus_lost")]
    FocusLost,
}
```

### 3.2 `bridge.m`

- 文件顶部回调变量区增加：`static QnVoidCallback g_quickbar_resign_cb = NULL;` 与 `static id g_quickbar_resign_observer = nil;`。
- 在 `qn_configure_quick_bar_panel` 里，面板创建完成后（`if (g_quickbar_panel == nil) { ... }` 块之后、`diag = qn_read_diag(...)` 之前）注册一次性观察者：

```objc
if (g_quickbar_resign_observer == nil) {
    g_quickbar_resign_observer = [[NSNotificationCenter defaultCenter]
        addObserverForName:NSWindowDidResignKeyNotification
                    object:g_quickbar_panel
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(__unused NSNotification *note) {
        if (g_quickbar_resign_cb != NULL) {
            g_quickbar_resign_cb();
        }
    }];
}
```

- 新增导出函数（放在 `qn_register_space_observer` 旁）：

```objc
void qn_register_quick_bar_resign_handler(QnVoidCallback on_resign) {
    g_quickbar_resign_cb = on_resign;
}
```

只能用 AppKit 公开 API，不得新增 `NSLog`。

### 3.3 `mod.rs`

- `extern "C"` 声明区增加：`fn qn_register_quick_bar_resign_handler(on_resign: Option<extern "C" fn()>);`
- `NativeWindowBridge` trait 增加方法：

```rust
fn register_quick_bar_resign_handler(&self, on_resign: extern "C" fn());
```

- `MacosNativeBridge` 与 `NoopNativeBridge` 各实现一次（Noop 为空实现）。

### 3.4 `application/window.rs`

- `hide_quick_bar` 里的 `event_name` 匹配增加 `HideReason::FocusLost => "focus_lost"`。
- 新增公开方法：

```rust
/// Called when the Quick Bar panel loses key-window focus (`D-0013`).
/// No-op when the panel is already hidden (programmatic hides also trigger resign-key).
pub fn on_quick_bar_lost_focus(&self, app: &AppHandle) -> Result<(), AppError> {
    if !self.is_quick_bar_visible()? {
        return Ok(());
    }
    self.hide_quick_bar(app, HideReason::FocusLost)?;
    Ok(())
}
```

### 3.5 `lib.rs`

- 新增：

```rust
extern "C" fn on_quick_bar_resign() {
    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        let wm = app.state::<Arc<WindowManager>>();
        if let Err(err) = wm.on_quick_bar_lost_focus(app) {
            eprintln!("[quickbar] focus_lost hide failed: code={:?}", err.code);
        }
    }
}
```

- `setup` 里在 `bridge.register_quick_bar_esc_handler(on_quick_bar_esc);` 之后增加 `bridge.register_quick_bar_resign_handler(on_quick_bar_resign);`。
- 注意注册顺序：`configure_native_panels`（约 582 行）先于本注册执行，观察者已建好；观察者回调读取的是全局变量，届时已有值。

### 3.6 测试

`window_policy_test.rs`：新增用例，`serde_json::to_string(&HideReason::FocusLost)` 得到 `"\"focus_lost\""`；`hide_quick_bar(HideReason::FocusLost)` 后 `reason == Some(HideReason::FocusLost)`。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（`cargo clippy -D warnings` 会重新编译 `bridge.m`，不得有新警告）。
- `pnpm gen:bindings` 后零 diff 差异（`HideReason` 新增变体已体现在生成物里并一并提交）。

## 5. 人工点验清单（本卡为中间态）

- [ ] 呼出 Quick Bar 后点击任意其他窗口：Quick Bar 立即消失，草稿保留；再次呼出时草稿还在。
- [ ] 按 Esc 或热键隐藏：只隐藏一次，无重复事件、无闪烁。
- [ ] 呼出后立刻再按热键切换：行为正常，无「刚显示就被隐藏」。
- [ ] 在 Quick Bar 内点击自己的按钮、打开目标选择器、打开「另存为」弹窗：**不应**触发隐藏（这些都在面板内，面板保持键窗口）。
- [ ] 在 Quick Bar 输入框里用中文输入法选词：选词候选窗弹出期间不触发隐藏。
- [ ] 切换 Space：仍自动复位。

## 上下文提示

- 只读 `bridge.m` 约 1–70 与 166–275、约 377–392；`mod.rs` 约 60–130 与约 230–270；`application/window.rs` 约 560–640；`domain/window.rs` 全文；`lib.rs` 约 30–66 与 580–592。
- 忽略：Toast 面板、45% 侧栏辅助模式。
