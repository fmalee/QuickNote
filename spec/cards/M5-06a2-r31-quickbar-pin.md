# M5-06a2：固定 Quick Bar——后端、菜单与 ADR 流转（R31）

- 所属里程碑：M5
- 关联方案与契约：`v5` §5-1；C3.1、C3.4、C4.2、C6.2、C11.3；D-0002、D-0013
- 前置依赖：M5-06a1
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - `settings.yaml` 新增 `quick_bar.pinned`（默认 `false`）。
  - 新增 IPC 命令 `window_set_quick_bar_pinned`。
  - 原生「窗口」菜单增加勾选项「固定 Quick Bar」；固定后失焦不隐藏。
  - `D-0013` 由 `proposed` 转 `accepted`；`D-0002` 标注被取代；同步 `AGENTS.md`「已定形态」。
- 不做：Quick Bar 界面上的固定图标（M5-06a3 负责）。

## 2. 允许修改文件白名单

- `src-tauri/src/domain/settings.rs`
- `src-tauri/src/domain/window.rs`
- `src-tauri/src/bindings.rs`
- `src-tauri/src/application/window.rs`
- `src-tauri/src/commands/window.rs`
- `src-tauri/src/lib.rs`

文档同步（必做，与代码同批提交）：

- `DECISIONS.md`
- `AGENTS.md`

测试文件（不计入上限）：`src-tauri/tests/persistence_test.rs`、`src-tauri/tests/window_policy_test.rs`

## 3. 具体实现要求

### 3.1 `settings.rs`

`QuickBarSettings` 增加 `pinned: bool`。用 `grep -n destroy_after_hidden_minutes src-tauri/src/domain/settings.rs` 取得该字段出现的全部位置（结构体字段、`Default`、Patch 结构体、`apply`、YAML 结构体字段、`Default → YAML`、`YAML → Settings`、`Settings → YAML`），为 `pinned` 逐处补上对应的一行，默认 `false`。声明方式与同结构体里其他字段完全一致，保证旧 `settings.yaml` 能加载。

### 3.2 `domain/window.rs`

```rust
/// Arguments for `window_set_quick_bar_pinned` (`C4.2`, `D-0013`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetQuickBarPinnedArgs {
    pub pinned: bool,
}
```

`bindings.rs`：`use crate::domain::window::{ ... }` 加入 `SetQuickBarPinnedArgs`，并在类型收集函数里 `.register::<SetQuickBarPinnedArgs>()`。随后 `pnpm gen:bindings`。

### 3.3 `application/window.rs`

- `WindowManager` 增加字段 `quick_bar_pin_menu_item: Mutex<Option<CheckMenuItem<tauri::Wry>>>`（`new` 里初始化为 `None`）。
- 新增：

```rust
pub fn register_quick_bar_pin_menu_item(&self, item: CheckMenuItem<tauri::Wry>) -> Result<(), AppError>;

pub fn set_quick_bar_pinned(
    &self,
    app: &AppHandle,
    pm: &PersistenceManager,
    pinned: bool,
) -> Result<bool, AppError> {
    // 1. 写 settings.yaml
    let snapshot = pm.settings_update(SettingsPatch {
        quick_bar: Some(QuickBarSettingsPatch { pinned: Some(pinned), ..Default::default() }),
        ..Default::default()
    })?;
    let applied = snapshot.settings.quick_bar.pinned;
    // 2. 同步菜单勾选态
    if let Ok(guard) = self.quick_bar_pin_menu_item.lock() {
        if let Some(item) = guard.as_ref() {
            let _ = item.set_checked(applied);
        }
    }
    // 3. 通知所有窗口重新读取配置（settings:status 会触发前端刷新快照）
    let _ = app.emit("settings:status", &snapshot.status);
    Ok(applied)
}
```

- `on_quick_bar_lost_focus`（M5-06a1 加的方法）改为带 `pm` 参数：读取 `pm.settings_get()?.settings.quick_bar.pinned`，为 `true` 时直接返回不隐藏。调用点在 `lib.rs`。

### 3.4 `commands/window.rs`

新增命令：

```rust
#[tauri::command]
pub fn window_set_quick_bar_pinned(
    pm: State<'_, Arc<PersistenceManager>>,
    app: AppHandle,
    args: SetQuickBarPinnedArgs,
) -> Result<bool, AppError> {
    app.state::<Arc<WindowManager>>()
        .set_quick_bar_pinned(&app, &pm, args.pinned)
}
```

`lib.rs` 的 `invoke_handler!` 里在 `commands::window::hotkey_update` 之后注册 `commands::window::window_set_quick_bar_pinned`。

### 3.5 `lib.rs`：菜单

- `build_app_menu` 的「窗口」菜单在「显示主窗口」之后追加一个勾选项，id 为 `window.pinQuickBar`，标题「固定 Quick Bar」，初始勾选状态从配置读取：`pm.settings_get().map(|s| s.settings.quick_bar.pinned).unwrap_or(false)`。写法参照「显示主窗口」一项与托盘的 `tray_toggle_auxiliary`（基线约 307–312）。
- `build_app_menu` 返回的菜单需要让 `setup_tray_and_menu` 拿到这个勾选项的句柄：把该项在 `setup_tray_and_menu` 里创建，再传入 `build_app_menu`（给函数加一个参数），或改为 `build_app_menu` 返回 `(Menu, CheckMenuItem)`，二选一。
- 创建后调用 `wm.register_quick_bar_pin_menu_item(item.clone())`。
- `on_menu_event` 新增分支（放在通用转发分支**之前**，因为该 id 必须由 Rust 直接处理、不能转发给前端）：

```rust
"window.pinQuickBar" => {
    let pinned = app_handle
        .try_state::<Arc<PersistenceManager>>()
        .and_then(|pm| pm.settings_get().ok())
        .map(|s| s.settings.quick_bar.pinned)
        .unwrap_or(false);
    if let (Some(pm), Some(wm)) = (
        app_handle.try_state::<Arc<PersistenceManager>>(),
        app_handle.try_state::<Arc<WindowManager>>(),
    ) {
        if let Err(err) = wm.set_quick_bar_pinned(app_handle, &pm, !pinned) {
            eprintln!("[menu] {menu_id} failed: code={:?}", err.code);
        }
    }
}
```

### 3.6 文档同步

`DECISIONS.md`：

- `D-0013` 的状态行改为 `- 状态：accepted（C11.3）`。
- `D-0002` 的状态行改为 `- 状态：accepted（C11.3）— 其中「点击面板外部不关闭」条款已 superseded by D-0013（`collectionBehavior = 256`、不跨 Space、切 Space 自动复位继续有效）`。

`AGENTS.md`「已定形态」第一条，把 `Quick Bar 与全局 Toast` 一句中的「点击外部不关闭（`D-0001`、`D-0002`）」改为「默认失焦即关闭，「固定」态例外（`D-0001`、`D-0002`、`D-0013`）」。

`spec/engineering-contract.md` 不用改（C3.4 / C4.2 / C6.2 / C2.4 / C9.3 已在 M5-01a 完成）。提交前确认 `grep -n 'focus_lost\|window_set_quick_bar_pinned\|pinned: false' spec/engineering-contract.md` 有结果。

### 3.7 测试

- `persistence_test.rs`：`settings_update` 写 `quick_bar.pinned = true` 后读回为 `true`；缺省为 `false`。
- `window_policy_test.rs`：无需新增（菜单与 IPC 依赖 Tauri 运行时）。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- `grep -n 'D-0013' DECISIONS.md` 可见状态为 `accepted`；`grep -n 'superseded by D-0013' DECISIONS.md` 有结果。
- `grep -n 'D-0013' AGENTS.md` 有结果。

## 5. 人工点验清单（本卡为中间态：固定开关只能从菜单操作）

- [ ] 原生「窗口」菜单出现「固定 Quick Bar」，初始未勾选。
- [ ] 勾选后呼出 Quick Bar，点击其他窗口：Quick Bar **不**消失；按 Esc 或热键仍可隐藏。
- [ ] 取消勾选后恢复失焦即关闭。
- [ ] 重启应用后勾选状态保留（`settings.yaml` 里 `quick_bar.pinned` 对应正确）。
- [ ] 菜单里的勾选状态与实际状态一致。

## 上下文提示

- 只读 `settings.rs` 中 `destroy_after_hidden_minutes` 的各处、`application/window.rs` 的结构体定义与 `new`、`hide_quick_bar`、`on_quick_bar_lost_focus`、`lib.rs` 约 84–125、257–262、288–360 与 414–453。
- 忽略：Quick Bar 前端界面。
