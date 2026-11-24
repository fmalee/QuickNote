# M4-01b：后端 M4 契约补全（下）——配置导入导出、macOS 原生 PDF 导出与 45% 侧栏辅助窗口桥接

- 所属里程碑：M4
- 关联方案与契约：`v4` §3.1、§5.5、§5.8、§5.11、§6 M4；`C1.2`、`C1.3`、`C1.4`、`C2.1`、`C2.3`、`C2.4`、`C3.1`–`C3.4`、`C4.2`、`C6.2`、`C6.3`、`C9.1`、`D-0003`、`D-0008`、`D-0009`、`D-0010`、`D-0011`
- 前置依赖：`M4-01a` 已合并至 `dev`
- 建议分支：`feat/M4-01b-backend-export-and-auxiliary-window`
- 会话策略：独立新会话执行（零大文档读取，所有 Rust 结构体、校验规则、ObjC 桥接接口与 IPC 签名已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **新增 IPC 结构体定义（`C3.4`、`C4.2`）**：
   - 在 `src-tauri/src/domain/settings.rs` 新增（派生 `Serialize, Deserialize, Type` + `#[serde(rename_all = "camelCase")]`）：
     ```rust
     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct ExportPathArgs {
         pub dest_path: String,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct ImportPathArgs {
         pub src_path: String,
     }
     ```
   - 在 `src-tauri/src/domain/note.rs` 新增：
     ```rust
     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct ExportPdfArgs {
         pub html_document: String,
         pub dest_path: String,
     }
     ```
   - 在 `src-tauri/src/domain/window.rs` 新增纯算法辅助函数（零 IO / 零平台依赖）：
     - `pub fn clamp_auxiliary_width_percent(percent: u8) -> u8`：限制范围 `[30, 60]`，默认值 `45`。
     - `pub fn compute_auxiliary_frame(screen_x: f64, screen_y: f64, screen_w: f64, screen_h: f64, width_percent: u8) -> (f64, f64, f64, f64)`：
       - 计算宽度 `w = (screen_w * (clamped as f64 / 100.0)).round().max(480.0).min(screen_w)`；
       - 计算右侧贴边坐标 `x = screen_x + (screen_w - w)`，`y = screen_y`，`h = screen_h`；
       - 返回 `(x, y, w, h)`（兼容 AppKit 左下角坐标系与逻辑像素边界）。

2. **配置导出备份与导入恢复（`settings_export` & `settings_import`）**：
   - 在 `src-tauri/src/application/persistence.rs` 新增：
     - `pub fn settings_export(&self, dest_path: &str) -> Result<(), AppError>`：
       - 校验 `dest_path` 非空，支持 `~` / `~/` 展开为 `$HOME`；
       - 若当前 `settings.yaml` 有效且存在，导出当前有效配置的规范化 YAML（通过 `serialize_settings_yaml(&current_settings)`，确保导出文件可无损再次导入）；
       - 若目标父目录不存在则自动 `create_dir_all`，使用 `atomic_write_str` 原子落盘。
     - `pub fn settings_import(&self, src_path: &str) -> Result<SettingsSnapshot, AppError>`：
       - 校验 `src_path` 非空，支持 `~` 展开；文件不存在返回 `NotFound`；
       - 读取原文并用 `parse_settings_yaml(&raw, None)` 严格校验（`#[serde(deny_unknown_fields)]`）；
       - **严格保护红线（`D-0008`）**：若 `status.valid == false`，**绝不修改现有 `settings.yaml` 与内存生效配置**，直接返回 `AppError::with_detail(ErrorCode::SettingsParseError, status.error_message..., detail)`（保留行列号或未知字段名）；
       - 若校验通过：先将现有 `settings.yaml` 备份至 `$APP_DATA_DIR/backups/settings.yaml.<YYYYMMDD-HHMMSS>.bak`（调用 `prune_backups` 滚动清理），再使用 `atomic_write_str` 将校验通过的规范化 YAML 写入 `$APP_DATA_DIR/settings.yaml`，更新内存 `settings` / `last_valid_settings` / `settings_status`，返回最新 `SettingsSnapshot`。
   - 在 `src-tauri/src/commands/settings.rs` 注册 `settings_export` 与 `settings_import`（`settings_import` 成功后广播 `settings:status` 事件，并在热键配置变更时同步通知 `HotkeyManager` / 前端保持一致）。

3. **macOS 原生离屏 `WKWebView` PDF 导出（`note_export_pdf`）**：
   - 在 `src-tauri/build.rs` 的 `#[cfg(target_os = "macos")]` 块中新增链接：`println!("cargo:rustc-link-lib=framework=WebKit");`。
   - 在 `src-tauri/src/infra/macos/bridge.m` 新增原生 C 接口：
     - `int32_t qn_render_html_to_pdf(const char *html_utf8, const char *dest_path_utf8, char *err_buf, size_t err_buf_len);`
     - 实现要点：
       - 在主线程创建离屏 `WKWebView`（`frame = CGRectMake(0, 0, 794, 1123)`，即 A4 96dpi 逻辑尺寸），设置内部 `WKNavigationDelegate`；
       - 加载 `html_utf8`（`baseURL: nil`），等待 `webView:didFinishNavigation:`（通过 `CFRunLoopRunInMode` 泵事件并设置 `15s` 超时保护，防死锁）；
       - 调用 `[webView createPDFWithConfiguration:pdfConfig completionHandler:^(NSData *pdfData, NSError *error) { ... }]`（macOS 11+ 标准公开 API，零私有 API）；
       - 将 `pdfData` 原子写入 `dest_path_utf8`（写同目录临时文件后替换，或返回字节交给 Rust `atomic_write_bytes` 落盘，推荐返回堆内存字节或由桥接写临时文件再 rename，避免路径竞态）；
       - **隐私红线（`C2.3`）**：绝不在日志或 `err_buf` 中打印 `html_utf8` 正文内容！
   - 在 `src-tauri/src/infra/macos/mod.rs` 的 `NativeWindowBridge` trait 新增方法：
     - `fn render_html_to_pdf(&self, html_document: &str, dest_path: &Path) -> Result<(), AppError>;`
     - `NoopNativeBridge` 实现：校验 `html_document` 非空后，使用原子写入写出最小合法 `%PDF-1.4\n%%EOF\n` 桩文件至 `dest_path`，使无头 `cargo test` 完整覆盖参数校验、路径展开、父目录创建与错误分支。
   - 在 `src-tauri/src/commands/notes.rs` 新增 `note_export_pdf(bridge, args: ExportPdfArgs) -> Result<(), AppError>`：
     - 校验 `html_document` 非空且不超过 20MB、`dest_path` 非空；
     - 展开 `~`；若未以 `.pdf`（不区分大小写）结尾可自动校验或拒绝非 `.pdf` 后缀（推荐：若无 `.pdf` 后缀自动补 `.pdf` 并在文档中写明，或要求调用方传 `.pdf` 并兜底补齐）；确保父目录存在；调用 `bridge.render_html_to_pdf(...)`。

4. **45% 侧栏辅助窗口原生桥接（`WindowMode::Auxiliary`）**：
   - 在 `src-tauri/src/infra/macos/bridge.m` 新增原生 C 接口：
     - `void qn_set_main_window_mode(void *ns_window_ptr, uint8_t mode, uint8_t auxiliary_width_percent, QnPanelDiag *out_diag);`
       - `mode = 0 (Standard)`：恢复 `NSNormalWindowLevel`、`NSWindowCollectionBehaviorFullScreenPrimary`（标准主窗口行为）；若保存过切入辅助模式前的标准窗口 `NSRect`，恢复原位置与尺寸（经当前屏幕 `visibleFrame` 合法性夹取）；
       - `mode = 1 (Fullscreen)`：恢复标准 `level` 与 `NSWindowCollectionBehaviorFullScreenPrimary`（由上层 Tauri `set_fullscreen(true)` 触发原生全屏）；
       - `mode = 2 (Auxiliary)`：若尚未处于辅助态先缓存当前标准窗口 `frame`；设置 `collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary`（`256`，不含 `canJoinAllSpaces`）、`level = NSFloatingWindowLevel`；按当前鼠标/窗口所在 `NSScreen` 的 `visibleFrame`（全屏空间下取 `frame` 与 `visibleFrame` 合理覆盖）计算右侧 `clamp(auxiliary_width_percent, 30, 60)%` 宽度的矩形并 `setFrame:display:animate:YES`，**不触发 Split View、不改变其他 App 窗口尺寸**。
   - 在 `src-tauri/src/infra/macos/mod.rs` 的 `NativeWindowBridge` trait 新增：
     - `fn configure_main_window_mode(&self, ns_window_ptr: usize, mode: WindowMode, auxiliary_width_percent: u8) -> Result<PanelDiagnostics, AppError>;`
   - 在 `src-tauri/src/application/window.rs` 的 `show_main` 中：
     - 接受 `auxiliary_width_percent: u8`（从 `PersistenceManager::settings_get()` 读取，默认 `45`），当 `mode == Some(WindowMode::Auxiliary)` 或切回 `Standard`/`Fullscreen` 时：
       - 若从全屏切往 `Auxiliary`，先 `win.set_fullscreen(false)`；
       - 调用 `bridge.configure_main_window_mode(ns_ptr, target_mode, auxiliary_width_percent)`；
       - 记录 `C2.4` 结构化诊断日志（`event = "main_mode_switch"`，不含内容）；
       - 保持 `D-0003` 动态激活策略不变（主窗口显示为 `.regular`，隐藏且设置窗隐藏为 `.accessory`），并将最新 `WindowState`（含 `mode: Some(target_mode)`）通过 `window:state` 广播至前端。

5. **命令注册、绑定导出与前端 IPC 封装**：
   - 在 `src-tauri/src/bindings.rs` 注册 `ExportPathArgs`、`ImportPathArgs`、`ExportPdfArgs` 并运行 `pnpm gen:bindings`。
   - 在 `src-tauri/src/lib.rs` 的 `invoke_handler` 注册 `settings_export`、`settings_import`、`note_export_pdf`，并在 `commands/window.rs::window_show_main` 传入 `PersistenceManager` 获取 `auxiliary_width_percent`。
   - 在 `src/lib/ipc/commands.ts` 添加 `settingsExport`、`settingsImport`、`noteExportPdf` 封装。

### 明确不做
- 不在本卡编写前端导出按钮、设置导入导出弹窗或主工作台辅助模式 UI 适配（分别归 `M4-04` 与 `M4-05`）。
- 不引入 `tauri-plugin-dialog`（沿用项目路径输入约定）。

## 2. 允许修改文件白名单

### 共享串行文件（`C1.3` 显式声明）
- `src-tauri/src/bindings.rs`
- `src/bindings/types.ts`（仅由 `pnpm gen:bindings` 生成）
- `src/lib/ipc/commands.ts`

### 业务与测试文件
- `src-tauri/build.rs`
- `src-tauri/src/domain/settings.rs`
- `src-tauri/src/domain/note.rs`
- `src-tauri/src/domain/window.rs`
- `src-tauri/src/infra/macos/bridge.m`
- `src-tauri/src/infra/macos/mod.rs`
- `src-tauri/src/application/persistence.rs`
- `src-tauri/src/application/window.rs`
- `src-tauri/src/commands/settings.rs`
- `src-tauri/src/commands/notes.rs`
- `src-tauri/src/commands/window.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tests/m4_export_and_auxiliary.rs`（新增）
- `src/lib/ipc/__tests__/smoke.test.ts`

## 3. 具体实现要求

1. **零 Panic / 零私有 API**：`bridge.m` 仅使用公开 AppKit / WebKit API（`WKWebView`、`createPDFWithConfiguration:`、`NSWindowCollectionBehaviorFullScreenAuxiliary`、`NSFloatingWindowLevel`）；Rust 生产代码零 `unwrap()` / `expect()`。
2. **原子写入与路径展开**：导出配置、导入覆写 `settings.yaml` 与导出 PDF 均须原子写入，并正确展开前导 `~/`。
3. **不破坏现有窗口行为**：Quick Bar 与 Toast 的 `NSPanel` 逻辑、`on_space_changed` 与 `.accessory ↔ .regular` 状态机保持与 `m3-baseline` 100% 兼容。

## 4. 自动化验收标准（必须全绿）

- 新增集成测试 `src-tauri/tests/m4_export_and_auxiliary.rs`，至少覆盖：
  1. `settings_export` 导出的 YAML 可被 `parse_settings_yaml` 100% 无损解析且与当前设置一致；
  2. `settings_import` 导入合法 YAML：原 `settings.yaml` 自动在 `backups/` 生成备份、新配置落盘生效、返回 `status.valid == true`；
  3. `settings_import` 导入非法 YAML（使用 `fixtures/vault/settings-invalid.yaml` 样式的语法错与未知字段）：返回 `SettingsParseError`（含行列号或未知字段名）且原 `settings.yaml` 与内存配置零篡改；
  4. `note_export_pdf` 经 `NoopNativeBridge` 验证：空 HTML 报错、目标路径自动创建父目录并写出合法 `%PDF-` 文件头；
  5. `clamp_auxiliary_width_percent` 与 `compute_auxiliary_frame` 边界测试（`<30%` 夹取 `30%`、`>60%` 夹取 `60%`、默认 `45%`、窄屏最小 `480px` 保护、`WindowPolicyState::show_main(Some(WindowMode::Auxiliary))` 状态与激活策略验证）。
- 必过命令：`pnpm check:all`。

## 5. 人工点验清单

- 本卡后端桥接将随 `M4-04`（PDF 排版与设置导入导出 UI）和 `M4-05`（45% 侧栏辅助窗口前端形态）合入后一并在真实 macOS 桌面执行实机点验。
