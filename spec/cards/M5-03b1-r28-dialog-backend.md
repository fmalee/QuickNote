# M5-03b1：系统文件选择窗口——依赖与后端（R28 前半）

- 所属里程碑：M5
- 关联方案与契约：`v5` §1.1、§1.2；C1.3、C1.4、C3.1、C4.2、C6.3；D-0008、D-0009
- 前置依赖：M5-01a（契约 C6.3 已含 `backupPaths`，C11.1 已登记 dialog 依赖）
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：接入官方 `tauri-plugin-dialog`（Rust 插件 + npm 包 + 权限），并在 `ui-state.json` 里新增 `backupPaths` 用于记住上次导出 / 导入路径。
- 不做：不改设置页界面（M5-03b2 负责）；不新增 IPC 命令。

## 2. 允许修改文件白名单（全部是 C1.3 共享串行文件，开工前确认无其他会话在改）

- `package.json`、`pnpm-lock.yaml`
- `src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`
- `src-tauri/capabilities/default.json`
- `src-tauri/src/lib.rs`
- `src-tauri/src/domain/ui_state.rs`
- `src-tauri/src/bindings.rs`

测试文件（不计入上限）：`src-tauri/tests/persistence_test.rs`

## 3. 具体实现要求

### 3.1 依赖

- Rust：`Cargo.toml` 的 `[dependencies]` 增加 `tauri-plugin-dialog = "2"`。
- npm：在 `package.json` 的 `dependencies` 增加 `"@tauri-apps/plugin-dialog": "^2"`（以 `pnpm add @tauri-apps/plugin-dialog` 的实际写入为准），同时更新 `pnpm-lock.yaml`。

### 3.2 注册插件

`lib.rs` 的 `run()` 里 `tauri::Builder::default()` 之后紧跟一行：

```rust
.plugin(tauri_plugin_dialog::init())
```

### 3.3 权限

`capabilities/default.json` 的 `permissions` 数组增加 `"dialog:default"`（包含打开与保存面板权限）。保持 `windows` 数组不变。

### 3.4 `ui-state.json` 新键

`domain/ui_state.rs`：

1. 新增结构体（放在 `PanelGroupUiState` 之后）：

```rust
/// Remembered paths for settings backup file dialogs (`C6.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupPathsUiState {
    pub last_export_path: Option<String>,
    pub last_import_path: Option<String>,
}
```

2. `UiState` 增加字段 `#[serde(default)] pub backup_paths: BackupPathsUiState`。
3. `UiStatePatchKey` 增加变体 `BackupPaths`，`as_str` 返回 `"backupPaths"`。
4. `apply_patch` 增加分支：`UiStatePatchKey::BackupPaths => self.backup_paths = parse_patch_value(key_name, value)?`。

`bindings.rs`：在 `use crate::domain::ui_state::{ ... }` 中加入 `BackupPathsUiState`。

随后运行 `pnpm gen:bindings` 更新 `src/bindings/types.ts`。

### 3.5 测试

`persistence_test.rs` 新增一个用例：

1. `ui_state_get` 取初始状态，断言 `backup_paths == Default::default()`。
2. `ui_state_set(BackupPaths, json!({ "lastExportPath": "/tmp/a.yaml" }))`。
3. 再次读取，断言 `backup_paths.last_export_path == Some("/tmp/a.yaml")`，`last_import_path` 为 `None`。
4. 在临时目录手写一份不含 `backupPaths` 的旧版 `ui-state.json`（含 `windows`、`tabs`、`panelGroup`、`filters`、`hotkey`），用 `PersistenceManager::open_with_dirs` 打开后 `ui_state_get` 成功且 `backup_paths` 为默认值。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿，其中 `pnpm gen:bindings && git diff --exit-code src/bindings` 在重新生成后零差异（即生成物已一并提交）。

## 5. 人工点验清单

- 无界面改动。`pnpm tauri dev` 能正常启动即可。

## 上下文提示

- 只读 `ui_state.rs` 约 100–260 行、`bindings.rs` 的 `use` 区、`lib.rs` 的 `run()` 开头几行。
- 忽略：前端代码本卡一行都不改。
