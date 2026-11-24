# M3-01b：后端 M3 契约补全（下）——可取消 Spotlight 搜索、路径补全、统一外部动作与菜单桥接

- 所属里程碑：M3
- 关联方案与契约：`v4` §3.5.2、§3.7.1、§4.7、§5.3 Spotlight、§5.10、§6 M3；`C1.2`、`C1.3`、`C1.4`、`C2.1`、`C2.3`、`C3.1`–`C3.4`、`C4.2`、`C9.1`、`D-0008`、`D-0009`、`D-0010`
- 前置依赖：`M3-01a` 已合并至 `dev`
- 建议分支：`feat/M3-01b-backend-spotlight-path-external`
- 会话策略：独立新会话执行（零大文档读取，所有 Rust 结构体、优先级规则、安全参数数组与 IPC 签名已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **新增 IPC 结构体定义（`src-tauri/src/domain/external.rs` 与 `domain/note.rs`，纯类型与纯规则零 IO）**：
   - 在 `src-tauri/src/domain/note.rs`（或 `domain/search.rs`）定义（均派生 `Serialize, Deserialize, Type` + `#[serde(rename_all = "camelCase")]`）：
     ```rust
     pub struct SpotlightArgs {
         pub query: String,
         #[specta(type = specta_typescript::Number)]
         pub request_id: u64,
     }

     pub struct PathCompleteArgs {
         pub prefix: String,
         #[specta(type = specta_typescript::Number)]
         pub request_id: u64,
     }

     pub struct PathEntry {
         pub name: String,
         pub abs_path: String,
         pub is_dir: bool,
     }
     ```
   - 新增 `src-tauri/src/domain/external.rs`（在 `domain/mod.rs` 导出）：
     - 精确枚举与常量（对齐 `C3.4`）：
       ```rust
       /// Sentinel `absPath` allowing `SettingsErrorBanner` to open/reveal local `settings.yaml` safely (`D-0008`).
       pub const SETTINGS_FILE_SENTINEL: &str = "$SETTINGS_YAML";

       #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
       pub enum ExternalActionId {
           #[serde(rename = "external.reveal")]
           ExternalReveal,
           #[serde(rename = "external.terminal")]
           ExternalTerminal,
           #[serde(rename = "external.vscode")]
           ExternalVscode,
           #[serde(rename = "external.sublime")]
           ExternalSublime,
           #[serde(rename = "external.workspace")]
           ExternalWorkspace,
           #[serde(rename = "external.default")]
           ExternalDefault,
           #[serde(rename = "external.copyPath")]
           ExternalCopyPath,
           #[serde(rename = "external.pin")]
           ExternalPin,
       }

       #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
       #[serde(rename_all = "camelCase")]
       pub struct ExternalActionState {
           pub id: ExternalActionId,
           pub enabled: bool,
           pub disabled_reason: Option<String>,
           pub resolved_path: Option<String>,
       }

       #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
       #[serde(rename_all = "camelCase")]
       pub struct ResolveExternalArgs {
           pub abs_path: Option<String>,
           pub workspace_key: Option<String>,
           pub command_id: Option<String>,
       }

       #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
       #[serde(rename_all = "camelCase")]
       pub struct RunExternalArgs {
           pub id: ExternalActionId,
           pub abs_path: Option<String>,
           pub dir_path: Option<String>,
       }
       ```
     - 纯函数 `pub fn is_spotlight_blacklisted(path: &str) -> bool`：
       - 按 `/` 或 `\` 拆分路径段，任一段命中黑名单集合（`.git`、`node_modules`、`target`、`dist`、`build`、`.venv`、`.DS_Store`）返回 `true`（`v4` §5.3）。
     - 纯函数 `pub fn resolve_external_action_states(ctx: &ExternalResolveContext) -> Vec<ExternalActionState>`（严格执行 `v4` §3.7.1 与 §3.5.3，且接受 `path_exists` / `is_dir` / `is_file` 回调或预先探测字段以便 `domain` 零 `std::fs` 依赖）：
       - 固定顺序返回全部 8 个 `ExternalActionId` 状态；
       - **严格禁用法则（红线）**：
         - `external.reveal` / `external.default` / `external.copyPath`：有本地存在文件路径（或 `$SETTINGS_YAML` 映射至 `settings.yaml`）即启用，`resolved_path = Some(file_path)`；否则禁用并给 `disabled_reason`。
         - `external.pin`：**仅对库外已存在文件启用**；库内文档返回 `enabled = false, disabled_reason = Some("仅支持固定库外常用文件")`。
         - `external.terminal`（目标目录解析优先级）：
           - 若传入 `command_id`：当前上下文文档/工作区目录 → 命令书自身 `terminal_dir`/`open_dir`/工作区目录 → 最近工作区目录（若有）→ **无则禁用**（`disabled_reason = Some("未配置项目目录，请手动选择目录")`），**严禁回退库根或用户主目录**。
           - 普通文档：单篇 frontmatter `terminal_dir` → `open_dir` → 关联工作区 `terminal_dir`/`open_dir`/`root` → 若为**库外文件**取其父目录；**若为库内文档且既无 `terminal_dir`/`open_dir` 也未关联工作区，严格 `enabled = false` + `disabled_reason = Some("库内文档未配置 open_dir 或关联工作区，请先配置")`，严禁回退库根或主目录**！
           - 若解析出目录但磁盘不存在，返回 `enabled = false` + `disabled_reason = Some("目标目录不存在")`（同时在 `resolved_path` 保留路径方便 UI 提示）。
         - `external.vscode` / `external.sublime`：单篇 `open_dir` → 工作区 `open_dir`/`root` → 库外文件父目录；库内无配置严禁回退；目录必须存在且对应编辑器配置有效。
         - `external.workspace`：单篇 `code_workspace` → 关联工作区 `code_workspace`；必须存在且为文件才启用，否则给出明确 `disabled_reason`（未配置 / `.code-workspace` 文件不存在）。

2. **可取消 `mdfind` Spotlight 搜索（`infra/spotlight.rs` + `application/external.rs`）**：
   - 新增 `src-tauri/src/infra/spotlight.rs`（封装 `SpotlightRunner`，以 `Arc<SpotlightRunner>` 注入 Tauri `State`）：
     - 内部持有 `Mutex<Option<ActiveSpotlightProcess>>`（存 `request_id` 与正在运行的子进程 PID / `Child` 句柄）与原子代次 `AtomicU64`。
     - 新请求到来时：先递增代次并**立即 `kill()` 上一次未完成的 `mdfind` 子进程**（防止快速打字造成并发子进程堆积，`v4` §5.3）。
     - 对 `settings.search_roots` 中展开 `~` 后实际存在的目录执行参数化 `Command::new("mdfind")`（严禁 shell 拼接！）：
       - 构造安全的参数数组：`["-onlyin", root_path, "-name", query]`（若 `query` 含控制字符或以 `-` 开头需安全校验/拒绝或防参数注入）；
       - 流式读取 stdout 行，应用 `is_spotlight_blacklisted` 过滤黑名单路径段、去重，并在收集满 **50 条**（`SPOTLIGHT_HARD_LIMIT = 50`）时立即终止子进程并设置 `truncated = true`；
       - 若运行期间被更新的请求取代（代次变化），立即 `kill()` 当前子进程并返回空结果（`truncated = false`），绝不残留僵尸进程（必须 `wait()` 回收）。
     - 为支持自动化并发取消测试，`SpotlightRunner` 提供可注入命令工厂的构造器（默认调用 `/usr/bin/mdfind`，测试可注入慢速子进程如 `sleep`/`sh -c` 验证并发到来时旧进程在毫秒级被 kill 回收且不堆积）。
   - 新增 `search_spotlight` IPC 命令（`commands/search.rs`）：空查询直接返回空 `SearchResponse`；否则调用 `SpotlightRunner` 并封装 `SearchResultItem { kind: File/Dir, key: format!("abs:{path}"), title: file_name, subtitle: path, rel_path: None, abs_path: path, score, source: SearchSource::Spotlight }`。

3. **本地路径补全（`path_complete`）**：
   - 在 `application/external.rs` 实现 `path_complete(args: PathCompleteArgs) -> Result<Vec<PathEntry>, AppError>`：
     - 支持 `~` / `~/` 展开为 `$HOME`，仅接受绝对路径（展开后以 `/` 开头；非 `/` 且非 `~` 返回空数组）。
     - 拆分为 `(parent_dir, name_prefix)`：若以 `/` 结尾则列出该目录下子项；否则列出父目录中文件名小写前缀匹配 `name_prefix` 的子项。
     - 过滤 `.DS_Store`；隐藏文件（以 `.` 开头）仅当 `name_prefix` 也以 `.` 开头时展示；忽略单条目无权限错误不崩溃。
     - 目录排在文件之前，再按名称不区分大小写升序，上限 **50 条**。

4. **常用外部文件持久化（`pinned-files.json`）与 `search_notes_and_files` 联动**：
   - 在 `PersistenceManager` 新增 `pinned_files_path() -> PathBuf`（`<app_data_dir>/pinned-files.json`）及方法：
     - `pub fn list_pinned_files(&self) -> Result<Vec<String>, AppError>`
     - `pub fn toggle_pinned_file(&self, abs_path: &str) -> Result<bool, AppError>`（返回切换后是否已固定，使用 `atomic_write_str` 落盘，自动过滤重复项）
   - 扩展 `application/notes.rs::search_notes_and_files`：
     - 当 `query` 为空或关键词匹配时，将 `pinned-files.json` 中磁盘仍存在的外部文件合入结果（`kind: SearchItemKind::File`, `source: SearchSource::Recent`, `key: format!("abs:{path}")`, `subtitle: format!("[已固定] {path}")`, `score: 1000.0` 置顶排序），使 Quick Bar 打开模式、主窗口 `⌘K` 与常用文件列表无缝共享真源。

5. **统一外部动作执行（`external_action_run`）与 OS 安全进程调用（`infra/external_open.rs`）**：
   - 新增 `src-tauri/src/infra/external_open.rs`，提供 `trait ExternalLauncher: Send + Sync`（默认 `MacosExternalLauncher`，测试可注入 Mock 验证精确参数与行为）：
     - 全部基于 `std::process::Command::new(...).args([...])` 参数数组调用（`C2.1`、`v4` §5.10），**严禁把路径拼进 shell 命令字符串**！
     - `external.reveal`：`open -R <abs_path>`（文件不存在返回 `ErrorCode::NotFound`）。
     - `external.default`：`open <abs_path>`（文件不存在返回 `ErrorCode::NotFound`）。
     - `external.vscode` / `external.sublime`：按 `settings.editors.{vscode,sublime}.bundle_id` 执行 `open -b <bundle_id> <dir_path>`；若打开失败且应用缺失返回 `ErrorCode::ExternalAppMissing`（带清晰提示）。
     - `external.workspace`：使用 VS Code（或工作区 `default_editor` 对应的 `bundle_id`）打开 `.code-workspace` 文件：`open -b <bundle_id> <workspace_file>`。
     - `external.terminal`：校验目标目录 `dir_path` 存在且为目录（缺失返回 `ErrorCode::WorkspaceDirMissing`）；按 `settings.terminal.bundle_id`（兼容 `com.apple.Terminal` / `com.googlecode.iterm2` / `dev.warp.Warp-Stable` / `com.mitchellh.ghostty` 等）参数化启动对应终端并打开到目标目录（macOS 上 `open -b <bundle_id> <dir_path>` 对 Terminal.app / iTerm2 / Warp / Ghostty 均原生支持以目录参数启动并自动 `cd` 到该目录，零 AppleScript 字符串注入风险！如特定终端不接受 `bundle_id` 回退 `open -a <name> <dir_path>`）。
     - `external.copyPath`：使用 `pbcopy` 通过 stdin 管道写入 UTF-8 路径字节（零命令行参数注入，日志严禁打印路径外剪贴板内容）。
     - `external.pin`：校验非库内文件且存在，调用 `pm.toggle_pinned_file(abs_path)`。
   - **支持 `$SETTINGS_YAML` 哨兵路径**：当 `args.abs_path.as_deref() == Some(SETTINGS_FILE_SENTINEL)` 时，后端自动映射为 `pm.settings_path()`，供 `SettingsErrorBanner` 调用 `external.default` / `external.reveal` 直接打开或定位本机 `settings.yaml`（`D-0008`）。

6. **原生菜单桥接补齐（`src-tauri/src/lib.rs`）**：
   - 将菜单项 `menu_quick_open` ID 改为 `window.quickOpen`（或同时转发 `menu_quick_open` / `menu_command_book` / `menu_tools_commands`）、在“视图”菜单补齐 `panel.toggle.related`（`Ctrl+CmdOrCtrl+R`）与 `panel.toggle.recent`（`Ctrl+CmdOrCtrl+E`），并在 `on_menu_event` 中转发给前端 `menu:action`（保持主窗聚焦守卫），使菜单栏与动作注册表同源一致。

7. **注册 IPC 命令与导出绑定（`C1.3`、`C1.4`）**：
   - `bindings.rs` 注册 `SpotlightArgs`、`PathCompleteArgs`、`PathEntry`、`ExternalActionId`、`ExternalActionState`、`ResolveExternalArgs`、`RunExternalArgs`。
   - `lib.rs` 注册 `SpotlightRunner`、`Arc<dyn ExternalLauncher>` State 及 4 条命令：`commands::search::search_spotlight`、`commands::search::path_complete`、`commands::external::external_actions_resolve`、`commands::external::external_action_run`。
   - 运行 `pnpm gen:bindings`，并在 `src/lib/ipc/commands.ts` 补齐 `searchSpotlight`、`pathComplete`、`externalActionsResolve`、`externalActionRun`。

### 明确不做什么
- 本卡**不做**前端 UI（设置页工作区表单、Quick Bar 面板、右键菜单绑定均归 `M3-02`–`M3-04`）。
- 严禁触碰 `UiState`（`C6.3`）已有字段，固定外部文件统一存 `pinned-files.json`。

## 2. 允许修改文件白名单

- 共享串行文件（`C1.3`）：
  - `src-tauri/src/bindings.rs`
  - `src/bindings/types.ts`（仅由 `pnpm gen:bindings` 自动生成）
  - `src/lib/ipc/commands.ts`
- Rust 源码：
  - `src-tauri/src/domain/external.rs`（新增）
  - `src-tauri/src/domain/note.rs`
  - `src-tauri/src/domain/mod.rs`
  - `src-tauri/src/infra/spotlight.rs`（新增）
  - `src-tauri/src/infra/external_open.rs`（新增）
  - `src-tauri/src/infra/mod.rs`
  - `src-tauri/src/application/persistence.rs`
  - `src-tauri/src/application/notes.rs`
  - `src-tauri/src/application/external.rs`（新增）
  - `src-tauri/src/application/mod.rs`
  - `src-tauri/src/commands/search.rs`
  - `src-tauri/src/commands/external.rs`（新增）
  - `src-tauri/src/commands/mod.rs`
  - `src-tauri/src/lib.rs`
- 测试文件：
  - `src-tauri/tests/m3_spotlight_path_external.rs`（新增）
  - `src/lib/ipc/__tests__/smoke.test.ts`（如需补充新封装导出断言）

## 3. 具体实现要求

1. **严格单向分层与零 `unwrap()`（`C1.2`、`C2.1`）**：
   - `domain/external.rs` 严禁依赖 `std::fs`、`std::process`、`tauri`。
   - 所有子进程调用严禁经过 `sh -c` 拼接路径；严禁在日志打印剪贴板/笔记内容（`C2.3`）。
2. **`mdfind` 并发取消与 50 条上限（`v4` §5.3）**：
   - 必须保证当请求 B 到达时，仍在运行的请求 A 的子进程被立即 `kill()` 并 `wait()` 回收；请求 A 返回空结果（不报错崩溃）；单次搜索输出超过 50 条时在第 50 条即截断并结束子进程，置 `truncated: true`。
3. **库内文档外部动作绝不回退红线（`v4` §3.7.1）**：
   - 当对库内某篇未设 `open_dir`/`terminal_dir` 且 `project` 未匹配任何 `workspaces` 的文档调用 `external_actions_resolve` 时：`external.terminal`、`external.vscode`、`external.sublime`、`external.workspace`、`external.pin` 全部必须返回 `enabled = false` 且附带清晰中文 `disabled_reason`，`resolved_path` 绝不能是 `library_root` 或 `$HOME`！
   - 当对库外文件调用 `external_actions_resolve` 时：`external.reveal`、`external.terminal`、`external.vscode`、`external.sublime`、`external.default`、`external.copyPath`、`external.pin` 均正常启用且目录解析为其父目录。
   - 当直接对 `external_action_run(ExternalTerminal, ...)` 传入缺失/不存在目录时返回 `ErrorCode::WorkspaceDirMissing`。

## 4. 自动化验收标准（必须全绿）

1. `src-tauri/tests/m3_spotlight_path_external.rs` 至少覆盖以下集成测试：
   - **黑名单与上限测试**：验证 `.git`、`node_modules`、`target`、`dist`、`build`、`.venv`、`.DS_Store` 全部过滤，且超过 50 条时硬截断至 50 条并 `truncated == true`；
   - **`mdfind` 并发取消测试**：注入慢速模拟子进程，先后在两个线程触发 `request_id = 1` 与 `request_id = 2`，断言请求 1 的进程被立即取消并快速返回（远小于慢进程睡眠时长），请求 2 正常返回结果，无僵尸残留；
   - **`path_complete` 测试**：验证目录优先排序、前缀匹配、`~` 展开、隐藏文件规则、无效路径容错；
   - **外部动作禁用法则与优先级全矩阵测试**：
     - 库内无 `open_dir` 且无工作区文档 → 终端/VS Code/Sublime/Workspace/Pin 全部 `enabled == false`（绝不回退库根或主目录）；
     - 库内文档有 `project` 关联工作区 → 继承工作区 `terminal_dir`/`open_dir`/`root`/`code_workspace`；
     - 库内文档自身 `terminal_dir` / `open_dir` 覆盖工作区配置；
     - 库外文件 → 终端/编辑器解析为其父目录且 `external.pin` 启用；
     - 命令上下文 `commandId` 解析优先级（当前文档工作区 → 命令书工作区 → 无则禁用不回退主目录）；
     - `$SETTINGS_YAML` 哨兵路径经 `external_action_run` 精确路由至 `pm.settings_path()`；
     - `external.pin` 切换持久化至 `pinned-files.json` 并在 `search_notes_and_files` 置顶返回。
2. 运行 `pnpm check:all`（全部 9 项门禁）必须 100% 通过。

## 5. 人工点验清单

- 待 `M3-02`–`M3-04` 接入前端后一并完成 macOS 实机点验（Finder `open -R`、外部终端 `cd`、VS Code / Sublime、`mdfind` 搜索与 `settings.yaml` 打开修复）。
