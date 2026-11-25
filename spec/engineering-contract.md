# QuickNote 工程契约（`C1`–`C12`）

> 本文档为 QuickNote 的机器级静态接口与工程边界契约，面向全程 AI 分模块构建。产品取舍以 [quicknote-design-v5.md](quicknote-design-v5.md) 为准，架构决策以 [DECISIONS.md](../DECISIONS.md) 为准。

---

## C1. 仓库结构、分层边界与共享文件

### C1.1 标准目录结构

```text
QuickNote/
├── AGENTS.md
├── DECISIONS.md
├── package.json
├── pnpm-lock.yaml
├── tsconfig.json
├── vite.config.ts
├── eslint.config.js
├── .prettierrc / .prettierignore
├── scripts/
│   ├── check-architecture.mjs          # 分层、token、unwrap、生成物检查脚本
│   └── check-all.sh                    # pnpm check:all 入口
├── spec/
│   ├── quicknote-design-v5.md
│   ├── CODEBASE_MAP.md
│   ├── engineering-contract.md
│   └── cards/                          # 分阶段施工任务卡
├── src/
│   ├── app/                            # 多窗口入口（main / quickbar / toast / settings）与全局样式
│   ├── styles/tokens.css               # 浅/深双主题 --qn-* CSS 变量定义
│   ├── lib/
│   │   ├── actions/                    # 统一动作注册表、快捷键映射与作用域分发
│   │   ├── ipc/                        # commands.ts / events.ts（唯一调用 @tauri-apps/api 的手写层）
│   │   └── utils/                      # 纯前端工具（无业务状态）
│   ├── bindings/
│   │   └── types.ts                    # 唯一由 Rust 导出的 TS 类型生成物（禁止手改）
│   ├── components/                     # 纯 UI 组件（禁连 IPC 与跨 feature 状态）
│   └── features/                       # 按业务垂直拆分（commands / editor / notes / onboarding / panels / quickbar / quickopen / settings / terminal / toast）
└── src-tauri/
    ├── Cargo.toml / Cargo.lock
    ├── tauri.conf.json
    ├── capabilities/
    ├── migrations/                     # 只追加的 SQLite 迁移文件（0001_init.sql, 0002_directory_workspace_cache.sql...）
    ├── src/
    │   ├── lib.rs
    │   ├── main.rs
    │   ├── commands/                   # 薄层：Tauri IPC 命令与参数校验
    │   ├── application/                # 用例编排、只读对账、配置/缓存隔离持久化、写入防回环、撤销快照、终端生命周期调度
    │   ├── domain/                     # 纯领域算法与模型（零 tauri / std::fs / rusqlite 依赖）
    │   ├── infra/                      # SQLite FTS5、文件读写、受控外部文件系统直扫（fs_browser）、notify 监听、portable-pty、macOS NSPanel/外部动作
    │   ├── bindings.rs                 # specta::Type 汇总导出列表
    │   └── bin/export_bindings.rs      # 绑定导出可执行入口
    └── tests/
        ├── fixtures/vault/             # 共享静态 Fixture 库
        └── *.rs                        # Rust 全部单元与集成测试
```

### C1.2 单向依赖边界与静态扫描

1. **Rust 后端单向依赖**：
   - 允许流向：`commands → application → domain`，`application → infra`，`infra → domain`。
   - **绝对红线**：`src-tauri/src/domain/**` 严禁引用 `tauri`、`std::fs`、`std::process`、`rusqlite`、`notify`、`portable_pty` 或原生系统 API。所有核心算法（目标真值表、分区插入、前置去重、YAML 最小补丁、Slug、混合分词、防回环纯判定）必须在 `domain` 内保持纯内存函数。
2. **前端单向依赖**：
   - 允许流向：`lib → components → features → app`。
   - **绝对红线**：
     - 除 `src/lib/ipc/**` 外，任何前端文件（尤其是 `src/components/**` 与 `src/features/**/components/**`）严禁直接 `import` `@tauri-apps/api/*`；业务功能须经各自 `features/<name>/api.ts`、`stores/` 或 `src/lib/ipc/` 调用。
     - 禁止跨 feature 深层导入（禁止 `import ... from '@/features/A/internal/...'`）；跨模块通信通过共享 Store、事件总线或顶层 `features/<name>/index.ts` 公共导出。
3. **自动扫描**：`scripts/check-architecture.mjs` 在每次提交前以静态正则/AST 检查上述禁令，违例即退出非零码阻断门禁。

### C1.3 共享串行文件清单

以下文件涉及全局契约或构建配置，改动须在任务卡中显式声明并保持串行修改：

- `package.json`、`pnpm-lock.yaml`、`tsconfig.json`
- `src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json`、`src-tauri/capabilities/**`
- `src-tauri/migrations/**`
- `src-tauri/src/bindings.rs`、`src/bindings/types.ts`、`src/lib/ipc/**`
- `src/styles/tokens.css`
- `DECISIONS.md`、`spec/engineering-contract.md`

### C1.4 生成物保护

- `src/bindings/` 目录仅允许存在 `types.ts`（或受同一导出器管辖的生成文件），且**每个文件首行必须精确为**：
  `/* GENERATED — DO NOT EDIT.`
- 禁止人类或 AI 直接编辑 `src/bindings/**`；修改 IPC 数据模型时只改 Rust 结构体并运行 `pnpm gen:bindings`。

---

## C2. 代码质量、安全与可观测性红线

### C2.1 Rust 质量与进程调用红线

- **零 Panic 容忍**：`src-tauri/src/**`（含 `commands`、`application`、`domain`、`infra`、`bin`）严禁使用 `.unwrap()` 与 `.expect()`（除非极其明确由门禁排除的 `tests/` 目录，按 `D-0010` 测试全放 `src-tauri/tests/**`）。错误全部经 `Result<T, AppError>` 与 `?` 传递。
- **参数化外部命令**：调用外部工具（`open -R`、`osascript`、编辑器 CLI、shell）必须使用 `std::process::Command::new(bin).args(&[...])` 数组传参；**严禁拼接任何包含用户路径或用户输入字符串的 shell 命令字串**。外部文件检索严禁依赖 Spotlight（`mdfind`），统一使用纯 Rust 受控文件系统探测与目录直扫（`infra::fs_browser`）。

### C2.2 前端 TypeScript 质量与防竞态红线

- `tsconfig.json` 开启 `"strict": true`、`"noUncheckedIndexedAccess": true`；禁止使用 `any`（`eslint` 配置 `@typescript-eslint/no-explicit-any: error`）。
- 搜索框、Quick Bar 实时匹配与异步路径补全必须实现请求版本校验（递增 `requestId` 或 `AbortController`）与 **80ms** 防抖，过期响应必须丢弃，防止乱序覆盖最新 UI。

### C2.3 安全与隐私日志红线

- **日志脱敏硬红线**：任何 Rust `tracing` 日志、前端 `console` 或错误详情严禁写入或上报：
  - 笔记正文、标题片段或搜索关键词全文；
  - 系统剪贴板内容；
  - 终端 PTY 输入/输出字节流；
  - 本机敏感路径外的密钥/环境变量。
- **能力最小化**：Tauri capability 仅授权访问当前 `library_root`、用户在 `settings.yaml` 显式添加的 `search_roots` 与用户主动打开的文件路径；正文内点击本地绝对路径或 `file://` 链接前必须弹二次确认；外部 `https://` / `mailto:` 交系统默认程序打开；生产构建关闭 DevTools 且零遥测。

### C2.4 macOS 原生桥接结构化调试日志

为方便开发阶段诊断全屏 Space、焦点切换与动态激活行为，macOS 原生桥接层须输出固定键值的结构化诊断日志（仅记录元数据，不含内容）：

- `event`: `"panel_show" | "panel_hide" | "space_changed" | "policy_switch" | "hotkey_fired" | "toast_show" | "focus_lost" | "vault_change_echo_ignored"`
- `activationPolicy`: `"accessory" | "regular"`
- `collectionBehavior`: integer（Quick Bar 与 Toast 恒为 `256`）
- `isKeyWindow`: boolean
- `hideReason`: `"esc" | "hotkey" | "space_changed" | "focus_lost" | null`

---

## C3. IPC 类型体系与统一错误模型

### C3.1 类型单一来源与命名约定

- Rust 为所有 IPC 入参、返回值、错误与事件的**唯一结构定义源**。
- 所有暴露给前端的 Rust `struct` / `enum` 必须派生 `serde::Serialize`、`serde::Deserialize`、`specta::Type`，并标注 `#[serde(rename_all = "camelCase")]`。
- 路径类型：库内文件一律使用 POSIX 风格相对路径 `relPath`（无前导 `/`）；库外文件与配置根使用绝对路径 `absPath`。

### C3.2 绑定生成与零差异校验

- 命令：`pnpm gen:bindings`（底层执行 `cargo run --manifest-path src-tauri/Cargo.toml --bin export_bindings`）。
- 门禁要求运行后 `git diff --exit-code src/bindings` 返回退出码 `0`。

### C3.3 统一错误模型（`AppError` 与 `ErrorCode`）

所有 Tauri 命令返回 `Result<T, AppError>`：

```rust
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>, // 结构化附加信息，如冲突哈希、YAML 行列号，禁止放正文
}

pub enum ErrorCode {
    ValidationError,
    LibraryNotConfigured,
    NotFound,
    PermissionDenied,
    Conflict,            // 乐观并发 bodyHash 不一致
    UndoUnavailable,     // 快照过期、超 5MB 或撤销校验哈希不匹配
    InvalidFrontmatter,
    SettingsParseError,  // settings.yaml 解析失败（含行列号，不覆盖原文件）
    ExternalAppMissing,
    WorkspaceDirMissing,
    TerminalLimitReached,
    TerminalSpawnFailed,
    InternalError,
}
```

### C3.4 共享核心结构体定义

> 以下字段名（转换为 `camelCase` 后）为前后端契约基线，生成时保持一一对应。

1. **应用与库信息**：
   - `AppInfo { version: String, platform: String, libraryRoot: Option<String>, activationPolicy: ActivationPolicy }`（`ActivationPolicy = "accessory" | "regular"`）
   - `LibraryInfo { root: Option<String>, noteCount: u32, lastIndexedAt: Option<i64> }`（`root = None` 代表首次启动尚未选库）
   - `IndexStatus { phase: IndexPhase, notesCount: u32, ftsDegraded: bool, lastCompletedAt: Option<i64> }`（`IndexPhase = "idle" | "scanning" | "indexing" | "error"`）

2. **笔记与 `meta + body` 分离（`D-0006`、`D-0007`）**：
   - `NoteType = "note" | "doc" | "command-book" | "template"`
   - `NoteMeta { id: Option<String>, title: String, docType: NoteType, tags: Vec<String>, project: Option<String>, openDir: Option<String>, codeWorkspace: Option<String>, terminalDir: Option<String>, created: Option<String>, pinned: bool, archived: bool, metaError: Option<String> }`
   - `NoteDoc { key: String, relPath: Option<String>, absPath: String, isLibrary: bool, isInbox: bool, meta: NoteMeta, body: String, bodyHash: String, mtimeMs: i64, sizeBytes: u64 }`（`key` 对库内文件取 `id` 或 `path:<relPath>`，对外部文件取 `abs:<absPath>`）
   - `NoteSummary { key: String, relPath: Option<String>, absPath: String, title: String, docType: NoteType, tags: Vec<String>, project: Option<String>, excerpt: String, pinned: bool, archived: bool, isInbox: bool, mtimeMs: i64, metaError: Option<String> }`
   - `WriteNoteArgs { absPath: String, body: String, baseBodyHash: String }`
   - `WriteResult { bodyHash: String, mtimeMs: i64, updatedMeta: Option<NoteMeta> }`（仅缺失 ULID 补写时返回 `updatedMeta`）
   - `ConflictInfo { absPath: String, currentBody: String, currentBodyHash: String, currentMtimeMs: i64 }`
   - `NoteMetaPatch { title: Option<String>, docType: Option<NoteType>, tags: Option<Vec<String>>, project: Option<Option<String>>, openDir: Option<Option<String>>, codeWorkspace: Option<Option<String>>, terminalDir: Option<Option<String>>, pinned: Option<bool>, archived: Option<bool> }`
   - `NoteUpdateMetaArgs { absPath: String, patch: NoteMetaPatch }`
   - `NotesFilterArgs { tab: String, dirRelPath: Option<String>, tags: Vec<String>, workspaceKey: Option<String>, filterText: Option<String> }`
   - `NavTreeSnapshot { dirs: Vec<DirNode>, tags: Vec<TagNode>, workspaces: Vec<WorkspaceSummary> }`（`DirNode { relPath, name, count, children }`、`TagNode { tag, count, children }`、`WorkspaceSummary { key, name, root, openDir }`）

3. **捕捉、目标解析、追加与撤销（`C8.1`–`C8.3`）**：
   - `CaptureMode = "list" | "todo" | "prose"`
   - `QuickBarMode = "capture" | "open" | "command"`
   - `TargetResolveInput { mainVisible: bool, activeDocKey: Option<String>, selectedWorkspaceKey: Option<String>, isInboxView: bool }`
   - `ResolvedTarget { scenario: String, kind: TargetKind, targetRelPath: Option<String>, targetTitle: String, sectionHint: Option<String>, workspaceKey: Option<String>, workspaceName: Option<String>, isCrossWorkspaceFallback: bool, recommendedMode: CaptureMode, chipLabel: String }`（`TargetKind = "unconfigured" | "note" | "inbox"`）
   - `AppendNoteArgs { targetRelPath: String, content: String, mode: CaptureMode, workspaceKey: Option<String> }`
   - `AppendResult { relPath: String, title: String, addedLines: u32, sectionHeading: Option<String>, bodyHash: String, mtimeMs: i64, undoAvailable: bool }`
   - `UndoAppendResult { relPath: String, restoredBodyHash: String, mtimeMs: i64 }`
   - `InboxExtractArgs { fromLine: u32, toLine: u32, newTitle: String, targetDirRelPath: String, templateName: String }`

4. **搜索、纯 Rust 外部文件直扫、外部书签与命令书**：
   - `SearchItemKind = "note" | "file" | "dir" | "action"`
   - `SearchResultItem { kind: SearchItemKind, key: String, title: String, subtitle: String, relPath: Option<String>, absPath: String, score: f64, source: SearchSource }`（外部检索采用纯 Rust 受控探测与目录直扫，支持 `.xxx` 隐藏目录与 `/usr/local/etc` 等系统路径）
   - `SearchResponse { requestId: u64, items: Vec<SearchResultItem>, truncated: bool }`
   - `PathEntry { name: String, absPath: String, isDir: bool }`
   - `ExternalPathProbeResult`、`ExternalDirectoryListing`、`ExternalDirectoryEntry`、`ExternalPathEntryKind`：受控文件系统路径探测与目录直扫强类型返回（带深度与条目数熔断）
   - `ExternalBookmark`：持久化于应用配置目录的外部常用文件/目录书签模型
   - `CommandEntry { id: String, noteRelPath: String, section: String, title: String, description: String, code: String, language: Option<String>, keywords: Vec<String>, isDanger: bool, line: u32 }`
   - `RelatedNoteItem { key: String, relPath: String, title: String, reason: RelatedReason, score: f64 }`（`RelatedReason = "backlink" | "shared_tags" | "same_project"`）

5. **窗口、状态、目录级工作区配置、`settings.yaml` 容错与外部动作**：
   - `WindowMode = "standard" | "fullscreen" | "auxiliary"`
   - `HideReason = "esc" | "hotkey" | "space_changed" | "focus_lost"`
   - `WindowState { windowId: String, visible: bool, fullscreen: bool, mode: Option<WindowMode>, reason: Option<HideReason> }`
   - `HotkeyStatus { combo: String, backupCombo: String, selfCheck: HotkeySelfCheck }`（`HotkeySelfCheck = "pending" | "ok" | "conflict"`）
   - `DirectoryWorkspaceConfig { vscodeWorkspacePath?: Option<String>, projectRoot?: Option<String>, defaultTerminalCwd?: Option<String>, externalAppOverrides?: Option<HashMap<String, String>> }` 与 `DirectoryWorkspaceConfigEntry { vaultRelDir: String, config: Option<DirectoryWorkspaceConfig>, mtimeMs: i64, parseError: Option<String> }`（笔记库子目录 `.quicknote.yaml` 绑定模型，替代已废弃的独立全局工作区）
   - `Settings` 按 `C6.2` Schema 严格映射（`#[serde(deny_unknown_fields)]`）；`SettingsPatch` 各顶层及子字段均为 `Option<...>`
   - `SettingsLoadStatus { valid: bool, activeSource: String, errorLine: Option<u32>, errorColumn: Option<u32>, errorMessage: Option<String> }`
   - `ExternalActionId = "external.reveal" | "external.terminal" | "external.vscode" | "external.sublime" | "external.workspace" | "external.default" | "external.copyPath" | "external.pin"`
   - `ExternalActionState { id: ExternalActionId, enabled: bool, disabledReason: Option<String>, resolvedPath: Option<String> }`

6. **内嵌终端（`D-0005`）**：
   - `TerminalSpawnArgs { workspaceKey: String, cwd: String, cols: u16, rows: u16, shell: Option<String> }`
   - `TerminalSessionInfo { sessionId: String, workspaceKey: String, cwd: String, hasActiveForegroundChild: bool, outputPaused: bool, state: TerminalLifecycleState }`（`TerminalLifecycleState = "active" | "hidden_alive" | "recycled"`）
   - `TerminalData { sessionId: String, dataBase64: String }`
   - `TerminalExitPayload { sessionId: String, exitCode: Option<i32>, reason: TerminalExitReason }`（`TerminalExitReason = "process_exit" | "manual_close" | "idle_recycled" | "app_exit"`）

---

## C4. IPC 命令表与事件契约

### C4.1 命令命名规范

全部 Tauri 命令采用 `snake_case`，按 `<domain>_<verb>` 命名，统一注册在 `src-tauri/src/commands/`，通过 `src/lib/ipc/commands.ts` 封装暴露给前端。

### C4.2 IPC 命令全表

| 模块 | 命令名 | 入参 | 返回值 | 所属里程碑 |
| --- | --- | --- | --- | --- |
| 应用与库 | `app_get_info` | — | `AppInfo` | M0 |
| 应用与库 | `library_get_info` | — | `LibraryInfo` | M0 |
| 应用与库 | `library_set_root` | `SetRootArgs { root: String, createIfMissing: bool }` | `LibraryInfo` | M0 |
| 应用与库 | `index_get_status` | — | `IndexStatus` | M1 |
| 应用与库 | `index_rebuild` | — | `IndexStatus` | M1 |
| 窗口与热键 | `window_show_main` | `MainWindowArgs { mode: Option<WindowMode> }` | `WindowState` | M0 |
| 窗口与热键 | `window_toggle_auxiliary_sidebar` | — | `WindowState` | v5 |
| 窗口与热键 | `window_hide_main` | — | `WindowState` | M0 |
| 窗口与热键 | `window_show_quick_bar` | `QuickBarArgs { mode: Option<QuickBarMode> }` | `WindowState` | M0 |
| 窗口与热键 | `window_hide_quick_bar` | `HideQuickBarArgs { reason: HideReason }` | `WindowState` | M0 |
| 窗口与热键 | `window_open_settings` | — | `WindowState` | M0 |
| 窗口与热键 | `window_close_settings` | — | `()` | v5 |
| 窗口与热键 | `window_dismiss_toast` | — | `()` | v5 |
| 窗口与热键 | `hotkey_update` | `HotkeyUpdateArgs { combo: String }` | `HotkeyStatus` | M0 |
| 窗口与热键 | `window_set_quick_bar_pinned` | `SetQuickBarPinnedArgs { pinned: bool }` | `bool` | M5 |
| 配置与状态 | `settings_get` | — | `SettingsSnapshot { settings: Settings, status: SettingsLoadStatus }` | M0 |
| 配置与状态 | `settings_update` | `SettingsPatch` | `SettingsSnapshot` | M2 |
| 配置与状态 | `settings_export` | `ExportPathArgs { destPath: String }` | `()` | M4 |
| 配置与状态 | `settings_import` | `ImportPathArgs { srcPath: String }` | `SettingsSnapshot` | M4 |
| 配置与状态 | `ui_state_get` | — | `UiState` | M0 |
| 配置与状态 | `ui_state_set` | `UiStateArgs { key: UiStatePatchKey, value: Value }` | `UiState` | M0 |
| 笔记与列表 | `notes_list` | `NotesFilterArgs` | `Vec<NoteSummary>` | M1 |
| 笔记与列表 | `notes_get_tree_and_tags` | — | `NavTreeSnapshot` | M1 |
| 笔记与列表 | `note_read` | `ReadNoteArgs { absPath: String }` | `NoteDoc` | M1 |
| 笔记与列表 | `note_create` | `CreateNoteArgs { title: String, dirRelPath: String, templateName: Option<String>, body: Option<String> }` | `NoteDoc` | M1 |
| 笔记与列表 | `note_write` | `WriteNoteArgs` | `WriteResult` | M1 |
| 笔记与列表 | `note_update_meta` | `NoteUpdateMetaArgs` | `NoteDoc` | M1 |
| 笔记与列表 | `note_rename` | `RenameNoteArgs { relPath: String, newRelPath: String }` | `NoteDoc` | M1 |
| 笔记与列表 | `note_delete` | `RelPathArgs { relPath: String }` | `()`（将文件移动到系统废纸篓（M5）） | M1 |
| 笔记与列表 | `note_save_image` | `SaveImageArgs { sourcePath: Option<String>, bytesBase64: Option<String>, ext: String, contextRelPath: Option<String> }` | `SavedImageResult { relPath: String }` | M2 |
| 捕捉与收件箱 | `capture_resolve_target` | `TargetResolveInput` | `ResolvedTarget` | M1 |
| 捕捉与收件箱 | `note_append` | `AppendNoteArgs` | `AppendResult` | M1 |
| 捕捉与收件箱 | `note_undo_last_append` | — | `UndoAppendResult` | M1 |
| 捕捉与收件箱 | `inbox_extract_to_note` | `InboxExtractArgs` | `NoteDoc` | M2 |
| 搜索与命令库 | `search_notes_and_files` | `SearchQueryArgs { query: String, limit: Option<u32>, requestId: u64 }` | `SearchResponse` | M1 |
| 搜索与命令库 | `path_complete` | `PathCompleteArgs { prefix: String, requestId: u64 }` | `Vec<PathEntry>` | M3 |
| 搜索与命令库 | `commands_search` | `CommandSearchArgs { query: String }` | `Vec<CommandEntry>` | M3 |
| 搜索与命令库 | `tags_batch_rename` | `TagRenameArgs { oldTag: String, newTag: String }` | `TagRenameResult { updatedFiles: u32 }` | M3 |
| 搜索与命令库 | `search_spotlight` | `SpotlightArgs` | `Vec<SearchResultItem>` | deprecated（Spotlight 已废止，M5-06e 删除） |
| 外部直扫、书签与目录工作区 | `probe_external_path` | `{ rawPath: String }` | `ExternalPathProbeResult` | v5 |
| 外部直扫、书签与目录工作区 | `list_external_directory` | `{ dirPath: String, query: Option<String>, includeHidden: bool }` | `ExternalDirectoryListing` | v5 |
| 外部直扫、书签与目录工作区 | `list_current_doc_siblings` | `{ docPath: String }` | `ExternalDirectoryListing` | v5 |
| 外部直扫、书签与目录工作区 | `upsert_external_bookmark` | `{ bookmark: ExternalBookmark }` | `Vec<ExternalBookmark>` | v5 |
| 外部直扫、书签与目录工作区 | `remove_external_bookmark` | `{ path: String }` | `Vec<ExternalBookmark>` | v5 |
| 外部直扫、书签与目录工作区 | `list_directory_workspace_configs` | — | `Vec<DirectoryWorkspaceConfigEntry>` | v5 |
| 外部直扫、书签与目录工作区 | `set_directory_workspace_config` | `{ vaultRelDir: String, config: Option<DirectoryWorkspaceConfig> }` | `()` | v5 |
| 关联、外部与导出 | `notes_get_related` | `RelPathArgs { relPath: String }` | `Vec<RelatedNoteItem>` | M3 |
| 关联、外部与导出 | `external_actions_resolve` | `ResolveExternalArgs { absPath: Option<String>, workspaceKey: Option<String>, commandId: Option<String> }` | `Vec<ExternalActionState>` | M3 |
| 关联、外部与导出 | `external_action_run` | `RunExternalArgs { id: ExternalActionId, absPath: Option<String>, dirPath: Option<String> }` | `()` | M3 |
| 关联、外部与导出 | `note_export_pdf` | `ExportPdfArgs { htmlDocument: String, destPath: String }` | `()` | M4 |
| 内嵌终端 | `terminal_spawn` | `TerminalSpawnArgs`, `onData: Channel<TerminalData>` | `TerminalSessionInfo` | M4 |
| 内嵌终端 | `terminal_write` | `TerminalWriteArgs { sessionId: String, dataBase64: String }` | `()` | M4 |
| 内嵌终端 | `terminal_resize` | `TerminalResizeArgs { sessionId: String, cols: u16, rows: u16 }` | `()` | M4 |
| 内嵌终端 | `terminal_set_visibility` | `TerminalVisibilityArgs { sessionId: String, visible: bool }` | `()` | M4 |
| 内嵌终端 | `terminal_kill` | `TerminalKillArgs { sessionId: String }` | `()` | M4 |
| 内嵌终端 | `terminal_list_sessions` | — | `Vec<TerminalSessionInfo>` | M4 |

> 说明：`ui_state_set` 的键白名单涵盖：`windows`、`tabs`、`leftSidebar`、`terminalPanel`、`filters`、`backupPaths`、`selectedWorkspace`、`hotkey`、`lastActiveDoc`、`workspaceModes`。

### C4.3 事件与流式通道契约

| 事件 / 通道 | 载荷类型 | 方向 | 说明 |
| --- | --- | --- | --- |
| `window:state` | `WindowState` | Rust → 前端 | 通知 Quick Bar / 主窗口显隐、模式与关闭原因（含 `space_changed`） |
| `toast:append-undo` | `AppendResult` | Rust → Toast/主窗 | 触发 5 秒可撤销 Toast 通知 |
| `fs:external-change` | `ExternalChangeEvent { absPath: String, kind: "modified" \| "deleted", newBody: Option<String>, newBodyHash: Option<String>, newMeta: Option<NoteMeta> }` | Rust → 前端 | 已剔除自身写入回环；干净态前端静默重载，脏态弹冲突横幅 |
| `index:status` | `IndexStatus` | Rust → 前端 | 后台索引进度、FTS 降级状态推送 |
| `settings:status` | `SettingsLoadStatus` | Rust → 前端 | 手改 `settings.yaml` 热重载结果与错误行列定位 |
| `terminal:session-state` | `TerminalSessionInfo` | Rust → 前端 | 前台子进程指示点变更、10MB 暂停状态与 30 分钟空闲回收通知 |
| `terminal:exit` | `TerminalExitPayload` | Rust → 前端 | PTY 会话退出与终止原因 |
| `Channel<TerminalData>` | `{ sessionId, dataBase64 }` | Rust → 前端 | 终端流式输出（16ms 或 64KB 批处理，唯一高吞吐通道） |

---

## C5. SQLite 派生索引 DDL 与混合分词契约

### C5.1 唯一真相源、缓存目录隔离与迁移原则

- **磁盘文档为唯一真相源**：笔记库内的 `.md` 与目录级 `.quicknote.yaml` 是唯一真相源。启动时由 `application::reconciler` 执行全量/增量只读对账，运行期由文件系统监听单向收敛至 SQLite；无 Frontmatter `id` 的 `.md` 必须以基于 `rel_path` 的确定性兜底键（`path:<rel_path>`）收录，扫描与对账过程**只读不改写用户文件**。
- **缓存目录强制隔离与可重建**：SQLite 数据库文件（含 `-wal` / `-shm`）强制存于应用缓存目录（`$APP_CACHE_DIR/<vault_hash>.db`），按 Vault 路径哈希隔离，**严禁放入笔记库根目录**；严禁存储任何无法从磁盘文档或配置目录重建的原始状态，必须支持随时删库全量重建。
- **只追加迁移**：迁移文件存于 `src-tauri/migrations/`（`0001_init.sql`、`0002_directory_workspace_cache.sql` 等），遵循**只追加、不修改历史迁移文件**。
- **`.quicknote.yaml` 隔离**：目录级 `.quicknote.yaml` 必须排除出 `notes` 列表与 `notes_fts` 全文索引，仅作为派生缓存记录在 `directory_workspace_configs` 表中。

### C5.2 核心表结构 DDL

```sql
CREATE TABLE IF NOT EXISTS library_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
  key             TEXT PRIMARY KEY,          -- id 或 path:<rel_path>
  id              TEXT,                      -- frontmatter ULID
  rel_path        TEXT NOT NULL UNIQUE,      -- POSIX 相对路径
  title           TEXT NOT NULL,
  doc_type        TEXT NOT NULL,             -- note | doc | command-book | template
  project         TEXT,
  mtime_ms        INTEGER NOT NULL,
  size_bytes      INTEGER NOT NULL,
  body_hash       TEXT NOT NULL,
  frontmatter_json TEXT NOT NULL,
  excerpt         TEXT NOT NULL,
  meta_error      TEXT,
  pinned          INTEGER NOT NULL DEFAULT 0,
  archived        INTEGER NOT NULL DEFAULT 0,
  is_inbox        INTEGER NOT NULL DEFAULT 0,
  updated_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_notes_id ON notes(id);
CREATE INDEX IF NOT EXISTS idx_notes_project ON notes(project);

CREATE TABLE IF NOT EXISTS note_tags (
  note_key TEXT NOT NULL,
  tag      TEXT NOT NULL,
  PRIMARY KEY (note_key, tag),
  FOREIGN KEY (note_key) REFERENCES notes(key) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag);

CREATE TABLE IF NOT EXISTS note_links (
  source_rel_path TEXT NOT NULL,
  target_rel_path TEXT NOT NULL,
  PRIMARY KEY (source_rel_path, target_rel_path)
);
CREATE INDEX IF NOT EXISTS idx_note_links_target ON note_links(target_rel_path);

CREATE TABLE IF NOT EXISTS commands (
  id            TEXT PRIMARY KEY,
  note_rel_path TEXT NOT NULL,
  section       TEXT NOT NULL,
  title         TEXT NOT NULL,
  description   TEXT NOT NULL,
  code          TEXT NOT NULL,
  language      TEXT,
  keywords      TEXT NOT NULL,
  is_danger     INTEGER NOT NULL DEFAULT 0,
  line          INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
  note_key UNINDEXED,
  title_tokens,
  tag_tokens,
  body_tokens,
  tokenize = 'unicode61'
);

-- 0002_directory_workspace_cache.sql
CREATE TABLE IF NOT EXISTS directory_workspace_configs (
  vault_rel_dir TEXT PRIMARY KEY,
  config_json   TEXT NOT NULL,
  mtime_ms      INTEGER NOT NULL,
  parse_error   TEXT
);
```

### C5.3 CJK + ASCII 混合分词契约

`domain::tokenizer::tokenize_mixed(input: &str) -> String` 用于写入 `notes_fts` 与改写用户检索查询：

1. 对输入按字符类别连续扫描（统一转小写）：
   - **ASCII 连续字母数字与路径/命令分隔符**（`[a-z0-9_]` 组成主词元；遇到 `.` / `-` / `/` / `:` 时同时保留完整带连接符词元及拆分后的独立词元，如 `nginx.conf` 产 `nginx.conf`、`nginx`、`conf`），**禁止对 ASCII 字符做双字 bigram**。
   - **CJK 汉字连续片段**：若片段长度 `L == 1`，输出该单字；若 `L >= 2`，输出全部连续 2-gram（滑动窗口步长为 `1`），不插入人工空格干扰词首词尾匹配。
2. 写入 `notes_fts` 的 `body_tokens` **必须使用剥离 YAML frontmatter 后的纯正文**。

---

## C6. 持久化文件 Schema 与应用数据落盘

### C6.1 应用配置目录、缓存目录隔离与目录级 `.quicknote.yaml`

1. **配置目录（`$APP_CONFIG_DIR/`，严格不入笔记库，不以 SQLite 为唯一存储）**：
   - `settings.yaml`：应用主配置文件（唯一配置文件格式，见 `D-0008`）。
   - `ui-state.json`：窗口、标签、滚动、面板与热键状态。
   - `bookmarks.json`：外部文件/目录书签持久化文件（同时与 `settings.yaml` 的 `external_bookmarks` 保持规范化同步）。
   - `quickbar-draft.json`：Quick Bar 未提交草稿 `{ mode, targetRelPath, selectedWorkspaceKey, text, updatedAt }`，成功提交或用户显式清空时删除。
   - `recovery/<hash>.json`：脏标签每 2 秒增量落盘的崩溃恢复快照，正常保存或干净关闭时删除。
   - `backups/<name>.<YYYYMMDD-HHmmss>.bak`：冲突强制覆盖或损坏 YAML 重写前自动备份；每次写新备份后执行滚动清理，仅保留最近 **30 天**内且总数不超过 **50 份**的备份，超限按修改时间淘汰最旧文件。
2. **缓存目录（`$APP_CACHE_DIR/`，随时可删除并无损重建）**：
   - `<vault_hash>.db`（含 `-wal` / `-shm`）：按笔记库路径哈希隔离的 SQLite 派生全文与元数据索引库，严禁放入笔记库根目录。
3. **目录级工作区配置文件（`<vault>/<dir>/.quicknote.yaml`）**：
   - 废弃脱离笔记库的独立全局工作区，改为笔记库子目录 `.quicknote.yaml` 绑定（支持 `vscodeWorkspacePath`、`projectRoot`、`defaultTerminalCwd`、`externalAppOverrides`）。
   - 外部动作与终端 `cwd` 严格按 **`文档 Frontmatter > 最近祖先目录 .quicknote.yaml > 全局默认`** 三级级联解析。
   - `.quicknote.yaml` 必须排除出笔记列表与 FTS；应用内经 `set_directory_workspace_config` 写入或删除时必须走原子写 + 自写守卫。

### C6.2 `settings.yaml` 完整 Schema 与容错

```yaml
library_root: ~/Documents/QuickNote       # 可空或首次引导设置
hotkey:
  quick_bar: ctrl+alt+space
  quick_bar_backup: ctrl+alt+cmd+space
quick_bar:
  default_target: workspace_last_active_or_inbox
  last_active_ttl_hours: 24
  destroy_after_hidden_minutes: 10
  auto_create_missing_section: false
  pinned: false                           # 固定后失焦不关闭
main_window:
  mode: standard                          # standard | fullscreen | auxiliary
  auxiliary_width_percent: 45
editor:
  theme: system                           # system | light | dark
  font_size: 14
  line_height: 1.6
  max_width: 900                          # px，0 表示不限制
  tab_size_prose: 2
  tab_size_code: 4
  auto_save_debounce_ms: 600
  auto_save_max_wait_ms: 2000
  single_click_link: false
  show_tab_close_button: false
panels:
  default: outline
  width: 260
  terminal:
    visible_by_default: false
    width: 360
    height: 240                           # 底部终端面板高度 px
    shell: /bin/zsh
    idle_recycle_minutes: 30              # 0 表示禁用空闲自动回收
    max_concurrent_sessions: 3
inbox:
  bloat_threshold_lines: 500
terminal:
  name: iTerm
  bundle_id: com.googlecode.iterm2
editors:
  vscode:
    name: Visual Studio Code
    bundle_id: com.microsoft.VSCode
  sublime:
    name: Sublime Text
    bundle_id: com.sublimetext.4
search_roots:
  - ~/Sites/Projects
  - ~/.config
  - /opt/homebrew/etc
max_active_editor_renders: 8              # 主编辑器 CM6 EditorState LRU 池上限
scan_hidden_files: true                   # 受控目录直扫是否允许 .xxx 隐藏目录
external_bookmarks: []                    # 常用外部文件/目录书签（持久化于配置目录）
```

- 解析使用 `#[serde(deny_unknown_fields)]`（历史 `workspaces` 字段仅作只读向后兼容保留，不再作为工作区主模型）；手改错误时保持原 `settings.yaml` **只读不动**，运行时回退上次有效配置或默认配置，并通过 `SettingsLoadStatus` 返回准确行列号与提示文案（`D-0008`）。

### C6.3 `ui-state.json` 完整 Schema

`leftSidebar`、`terminalPanel`、`backupPaths` 在旧版 `ui-state.json` 缺失时取默认值；旧键 `panelGroup` 读取时忽略。

```typescript
// 本表对应 Rust UiState 结构体导出形态（camelCase）
interface UiState {
  windows: { main: { x: number; y: number; width: number; height: number; mode: "standard" | "fullscreen" | "auxiliary"; isOpen: boolean } };
  tabs: {
    items: Array<{
      key: string;
      relPath: string | null;
      absPath: string;
      isPinned: boolean;
      isPreview: boolean;
      scrollTop: number;
      cursorPos: number;
    }>;
    activeKey: string | null;
    closedStack: string[]; // 最近关闭 absPath，供 ⌘⇧T 恢复
  };
  leftSidebar: {
    width: number;                                   // px，默认 300
    activeView: "workspace" | "related" | "outline";  // 左侧栏顶层视图
    collapsed: boolean;
  };
  terminalPanel: { visible: boolean; height: number; collapsed: boolean }; // 编辑器底部终端
  filters: {
    activeTab: "projects" | "tags" | "recent";       // 旧值 "inbox" 读取时映射为 "projects"
    selectedDir: string | null;
    selectedTags: string[];
    listScrollTop: number;
  };
  backupPaths: { lastExportPath: string | null; lastImportPath: string | null };
  selectedWorkspaceKey: string | null; // 已废弃，M5-06f 移除
  lastActiveDoc: { relPath: string; workspaceKey: string | null; touchedAtMs: number } | null;
  workspaceModes: Record<string, "list" | "todo" | "prose">;
  hotkey: { combo: string; backupCombo: string; selfCheck: "pending" | "ok" | "conflict" };
}
```

### C6.4 YAML frontmatter 最小补丁与序列化

- 读取时保留原文本切片及注释、字段排序、未知扩展键。
- `note_update_meta` 与 ULID 补写必须走最小补丁器：只增删改目标 key 所在 YAML 片段。
- 序列化规范：ISO 日期字符串必须带双引号；`tags` 统一输出 YAML 块列表（每行 `- tag`）。若原 YAML 损坏无法原地补丁，先备份原文件至 `backups/`，再在前端确认 diff 后覆盖。

---

## C7. UI Token、动作注册表与快捷键分发

### C7.1 `--qn-*` CSS Token 清单与双主题

`src/styles/tokens.css` 统一提供浅色（默认）与 `[data-theme="dark"]` 两套主题变量，任何组件禁用裸十六进制色值、像素硬编码与任意 `z-index`：

- **色彩 Token**：`--qn-bg-base`、`--qn-bg-elevated`、`--qn-bg-sidebar`、`--qn-bg-hover`、`--qn-bg-active`、`--qn-border-subtle`、`--qn-border-strong`、`--qn-text-primary`、`--qn-text-secondary`、`--qn-text-muted`、`--qn-accent`、`--qn-accent-soft`、`--qn-danger`、`--qn-warning`、`--qn-success`。
- **布局与排版 Token**：`--qn-font-sans`、`--qn-font-mono`、`--qn-text-xs`..`--qn-text-lg`、`--qn-space-1`..`--qn-space-8`、`--qn-radius-sm` / `md` / `lg`、`--qn-editor-max-width`（绑定设置项，默认 `900px`）。
- **层级 Token**：`--qn-z-base`、`--qn-z-drawer`、`--qn-z-popover`、`--qn-z-modal`、`--qn-z-toast`。

### C7.2 统一动作注册表与命名空间

所有前端可执行动作集中注册，结构：

```typescript
interface ActionDef {
  id: string;
  title: string;
  category: "global" | "quickbar" | "window" | "editor" | "external" | "panel";
  defaultKeys: string | null;
  scope: "global" | "modal" | "quickbar" | "terminal" | "editor" | "list" | "main-window";
  when?: (ctx: ActionContext) => boolean;
  swallowWhenDisabled?: boolean;
  menuGroup?: "format" | "view" | "tools" | "editor-context";
  run: (ctx: ActionContext) => void | Promise<void>;
}
```

核心命名空间：

- `global.*`：`global.toggleQuickBar`（`⌃⌥Space`）、`global.toggleMainWindow`
- `quickbar.*`：`quickbar.mode.capture`（`⌘N`）、`quickbar.mode.open`（`⌘K`）、`quickbar.mode.command`（`⌘⇧K`）、`quickbar.captureMode.list`（`⌘1`）、`quickbar.captureMode.todo`（`⌘2`）、`quickbar.captureMode.prose`（`⌘3`）、`quickbar.selectTarget`（`⌘T`）、`quickbar.submitAppend`（`⌘↵`）、`quickbar.submitOpenMain`（`⇧⌘↵`）、`quickbar.saveAsNote`（`⌥⌘↵`）、`quickbar.openCommandSource`（`⌘O`）、`quickbar.togglePin`
- `editor.*`：`editor.bold`（`⌘B`）、`editor.italic`（`⌘I`）、`editor.codeBlock`（`⌥⌘C`）、`editor.table`（`⌥⌘G`）、`editor.toggleTodo`（`⌥⌘X`）、`editor.link`（`⌥⌘K`）、`editor.image`（`⌥⌘I`）、`editor.bulletList`（`⌥⌘L`）、`editor.blockquote`（`⌥⌘Q`）、`editor.heading1`..`6`（`⌥⌘1..6`）、`editor.indent`（`Tab`）、`editor.outdent`（`⇧Tab`）、`editor.moveSelectionToNewNote`（`when: editor.hasSelection`）、`editor.find`（`⌘F`）、`editor.replace`（`⌥⌘F`）
- `panel.*`：`panel.toggle.outline`（`⌃⌘O`，左侧栏「大纲」视图）、`panel.toggle.related`（`⌃⌘R`，左侧栏「相关文档」视图）、`panel.toggle.terminal`（`⌃⌘T`，编辑器底部终端）
- `external.*`：同 `C3.4` `ExternalActionId`
- `folder.*`：`folder.bindWorkspaceConfig`（绑定/编辑目录 `.quicknote.yaml`）、`folder.openWorkspaceInVscode`（以该目录绑定的工作区在 VS Code 中打开）
- `window.*`：`window.nextTab`（`⌃⇥`）、`window.prevTab`（`⇧⌃⇥`）、`window.pinQuickBar`、`window.toggleAuxiliary`

### C7.3 作用域优先级、放行与显式吞键

1. **分发优先级**：`modal` > `quickbar` > `terminal` > `editor` > `list` > `main-window` > `global`。
2. **`when` 未命中放行**：若匹配到动作但 `when(ctx) === false` 且 `swallowWhenDisabled !== true`，按键事件放行给下层作用域或系统默认行为（如 `editor.moveSelectionToNewNote` 无选区时不阻拦）。常规格式动作（`⌘B`、`⌘I`、`⌥⌘C` 等）**不依赖选区**，无选区时按各自模板插入。
3. **高优先级显式吞键**：
   - 当焦点位于 `terminal` 时，`⌥⌘C` 显式设为 `swallowWhenDisabled: true` 或在 `terminal` 作用域拦截吞掉，防止穿透至 `editor` 插入代码块；`⌘C` 仅在终端有选区时复制文本且不写入 PTY，无选区或 `⌃C`/`⌃D`/`⌃Z` 放行直传 PTY。

### C7.4 左侧栏视图注册表与底部终端面板

- 左侧栏顶层视图顺序：`workspace`（笔记导航：目录 / 标签 / 最近）、`related`（相关文档）、`outline`（大纲）。
- 左侧栏宽度：默认 `300px`，最小 `220px`，可拖动并持久化于 `ui-state.json` 的 `leftSidebar.width`；收起为 `36px` 图标条。
- 底部终端面板：默认隐藏，高度默认 `240px`，范围 `120–600px`，可拖动并持久化于 `terminalPanel.height`；折叠时终端会话保活。
- 响应式断点：`< 1000px` 左侧栏收为图标条；`< 900px`（含 45% 侧栏辅助窄窗口）左侧栏以覆盖式抽屉浮于编辑区之上，编辑区最小宽度 `480px`。

### C7.5 常驻内存模型、单视图 LRU 池与 45% 侧栏辅助模式

- **窗口生命周期**：主窗口与 Quick Bar 常驻内存；Settings、Toast 等低频副窗口按需创建并在闲置/关闭后销毁释放 WebView 内存。
- **主编辑器单视图 LRU 池**：主工作台全局复用单一 CM6 `EditorView` 实例，切换标签时经由 `DocTabStub` 与可配置上限（`settings.maxActiveEditorRenders`）的 `EditorState` LRU 池挂载/缓存状态，**绝不淘汰当前激活态或 `isDirty` 未保存标签**。
- **45% 侧栏辅助模式**：复用主工作台 WebView，macOS 原生层配置全屏 Space 悬浮叠加行为，系统托盘与 Quick Bar 均提供切换入口。

---

## C8. 核心确定性算法与状态机契约

### C8.1 Quick Bar `S1–S8` 目标真值表

`domain::capture::resolve_target(input, state)` 为无副作用纯函数，严格按以下真值表输出（任何实现不得偏离）：

| 场景 | 条件 | 输出目标与说明 |
| --- | --- | --- |
| **S1** | `libraryRoot == None` | `kind = unconfigured`（禁用提交） |
| **S2** | `mainVisible && activeDoc != None` | `kind = note/inbox`，`target = activeDoc`（工作区随 `activeDoc`） |
| **S3** | `mainVisible && activeDoc == None && selectedWorkspace == Some(W) && lastActiveDoc in 24h && lastActiveDoc.workspace == W` | `target = lastActiveDoc` |
| **S4** | `mainVisible && activeDoc == None && selectedWorkspace == Some(W)` 且 `lastActiveDoc` 缺失或属于异区 `W'` | 有 `W.defaultEntryDoc` 则取之，否则 `inbox.md`；若因异区触发标记 `isCrossWorkspaceFallback = true` |
| **S5** | `mainVisible && activeDoc == None && selectedWorkspace == None` | 若 `isInboxView` 取 `inbox.md`；否则 `lastActiveDoc in 24h` 存在则取之，无则 `inbox.md` |
| **S6** | `!mainVisible && selectedWorkspace == Some(W) && lastActiveDoc in 24h && lastActiveDoc.workspace == W` | 忽略隐藏态 `activeDoc`，`target = lastActiveDoc` |
| **S7** | `!mainVisible && selectedWorkspace == Some(W)` 且 `lastActiveDoc` 缺失或属于异区 `W'` | 有 `W.defaultEntryDoc` 则取之，否则 `inbox.md`；若因异区触发标记 `isCrossWorkspaceFallback = true` |
| **S8** | `!mainVisible && selectedWorkspace == None` | `lastActiveDoc in 24h` 存在则取之，否则 `inbox.md` |

### C8.2 分区追加与前缀去重算法

1. **时间戳基准**：调用提交函数时读取当前系统本地时区时间 `Local::now()`，生成 `YYYY-MM-DD` 与 `HH:mm`。
2. **前缀去重规则**：
   - `list` 模式：已以 `- `、`* `、`+ ` 或 `^\d+\.\s` 开头时不再前置 `- `；`inbox.md` 下若缺失 `HH:mm ` 则在列表符后补入时间戳。
   - `todo` 模式：已以 `- [ ] ` 或 `- [x] ` 开头保留原样；以 `- ` / `* ` 开头替换为 `- [ ] `；其余加 `- [ ] `。
   - `prose` 模式：原样保留。
3. **分区扫描与插入**：按行解析正文并追踪围栏代码块（`` ``` `` / `~~~`），仅在代码块外部匹配首个精确二级标题（`inbox.md` 为 `## YYYY-MM-DD`；普通文档 `list` 为 `## 想法`、`todo` 为 `## 待办`）；找到后向后推进至下一个一级/二级标题或 EOF，回退跳过尾随空行后插入内容；`inbox.md` 缺失当日标题则在文末追加新日期标题与条目，普通文档缺失标题默认追加至文末（除非开启 `auto_create_missing_section`）。

### C8.3 追加 5 秒快照撤销契约

- 每次 `note_append` 执行前若原正文 `before_body.len() <= 5 * 1024 * 1024`，内存保存单槽快照 `LastAppendUndo { relPath, afterBodyHash, beforeBody, createdAt }`（新追加覆盖旧快照）。
- `note_undo_last_append` 校验当前文件磁盘正文哈希严格等于 `afterBodyHash`，满足时保留磁盘当前 `meta` 原样写回 `beforeBody`，否则返回 `ErrorCode::UndoUnavailable`。

### C8.4 中文锚点 Slug 算法

`domain::markdown::heading_to_slug(raw_heading: &str, seen_counts: &mut HashMap<String, usize>) -> String`：

1. 剥离行内 Markdown 语法（`**`、`*`、`` ` ``、`[text](url)` 取 `text`）。
2. 全字符转小写；空白字符连续折叠为单个 `-`；去除 ASCII 与常见中文标点符号，**完整保留所有汉字、假名、韩文及字母数字与 `-`/`_`**，去除首尾多余 `-`。
3. 查 `seen_counts`：首次出现记 `0` 返回 `base`；第 `k` 次重复（`k >= 1`）返回 `format!("{base}-{k}")`。不支持 `{#custom-id}` 语法。

### C8.5 防回环与外部变更状态机

- 应用每次写入前（`note_write` / `note_append` / `note_update_meta` / `note_rename` / `tags_batch_rename` / 撤销追加 / `set_directory_workspace_config` 的写入与删除）先以「预计落盘 body_hash」在内存写入缓存表登记 `(canonical_abs_path, body_hash, expires_at = now + 1500ms)`；写入失败必须立即清除该登记。`.quicknote.yaml` 的应用内写入 / 清除同样走原子写 + 自写守卫。
- `notify` 收到磁盘修改事件后先重读正文计算 `disk_body_hash`：若与写入缓存中的近期哈希相同（或处于抑制窗口且内容未变），仅更新索引 `mtime` 并静默丢弃事件；若哈希确实不同，发送 `fs:external-change(modified)`。
- 前端收到 `modified` 事件时，若 `newBodyHash` 等于该标签的 `baseBodyHash` 且 `newMeta` 与标签当前 `meta` 一致，视为应用自身写入的回声：记诊断事件 `vault_change_echo_ignored` 并忽略，不重置编辑器状态、不弹冲突横幅。其余情况按原规则：干净态静默重载，脏态激活冲突横幅。
- `notify` 收到删除或移出路径事件：发 `fs:external-change(deleted)`，前端绝不关闭标签、保留编辑器完整正文并标红警告。

### C8.6 `related` 面板加权打分

遍历库内除当前笔记 `C` 外的其余文档 `D` 计算关联分：

- `+100`：若 `D` 在 `note_links` 中存在指向 `C` 的反向相对链接；
- `+15 * shared_tag_count`：按两者共有标签数线性累加；
- `+10`：若 `D.project` 非空且等于 `C.project`。

过滤得分 `0` 者，按分数降序、更新时间降序取前 **20** 条。

### C8.7 终端保活与闲置自动回收状态机

- 每个工作区至多 1 个会话，全局并发上限 3 个，状态转换如下：
  - `active`（面板可见） ⇄ `hidden_alive`（面板折叠/切到其他板块：保持 PTY 存活，暂停前端写入并以 Rust 1MB 环形缓冲蓄水，重新可见时一次性吐回缓冲并恢复流式写入）。
  - 在 `hidden_alive` 状态下：后台每 60s 巡检一次。仅当同时满足 `settings.panels.terminal.idle_recycle_minutes > 0`、`tcgetpgrp(master_fd) == shell_pgid`（无前台活跃子进程）且 `now - last_io_at >= idle_recycle_minutes` 时，自动终止 PTY、清理资源并推送 `terminal:exit(reason = "idle_recycled")`。
  - 任意状态下点击关闭按钮或程序退出：按 `kill(-pgid, SIGHUP) → wait ≤ 500ms → 200ms SIGKILL` 回收并发送 `terminal:exit`。

---

## C9. 自动化验收门禁与共享 Fixture 库

### C9.1 一键门禁 `pnpm check:all`

所有任务卡收工前必须运行并全绿（返回码 `0`）：

1. `pnpm gen:bindings && git diff --exit-code src/bindings`（Rust↔TS 类型零漂移）
2. `node scripts/check-architecture.mjs`（验证 `C1.2` 分层、`C1.4` 首行生成标记、`C2.1` 零 `unwrap/expect`、`C7.1` 无硬编码色值/魔法 `z-index`）
3. `pnpm format:check`（Prettier 格式校验）
4. `pnpm lint`（ESLint + 禁止 `any`）
5. `pnpm typecheck`（`vue-tsc --noEmit`）
6. `pnpm test:unit --run`（Vitest 前端全部单元测试）
7. `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
9. `cargo test --manifest-path src-tauri/Cargo.toml --all-features`

### C9.2 共享静态 Fixture 库清单

`src-tauri/tests/fixtures/vault/` 必须预置以下 10 类标准测试样本（测试严禁绕过样本自行 mock 核心磁盘解析）：

1. `inbox.md`：包含多天日期分区、已超 500 行的膨胀样例、混合 `- ` 与 `- [ ] ` 项。
2. `missing-id.md`：无 `id` 字段的旧库 Markdown（验证补写例外与 `note_append` 零篡改）。
3. `duplicate-id-a.md` & `duplicate-id-b.md`：重复 ULID 样本（验证 `meta_error` 与不静默改写）。
4. `broken-yaml.md`：语法损坏的 YAML frontmatter（验证宽容读取正文与补丁保护）。
5. `unknown-keys-comments.md`：含注释、非标准序与自定义字段（验证最小补丁原样保留）。
6. `code-fence-heading.md`：代码块内包含 `## 想法`、`## 待办` 假标题（验证跳过围栏插入真实分区）。
7. `cn-headings-anchors.md`：中英混排、标点及重复标题样本（验证 Slug 与大纲去重）。
8. `commands/常用命令.md`：含 H2、H3、代码块、`<!-- keywords -->`、`<!-- danger -->` 与无 H3 兼容段。
9. `large-note-6mb.md`（或测试动态生成脚本）：验证 `>2MB` 关诊断、`>5MB` 退纯文本及撤销快照体积边界。
10. `settings-invalid.yaml`：含语法错误与未知字段的配置样本（验证 `settings.yaml` 只读保护与精确行列报错）。

### C9.3 macOS 原生人工点验清单

每当任务卡涉及原生窗口、热键或输入法交互时，由用户在真实 macOS 桌面完成 5 项点验：

1. 全屏 App（如 Safari / 终端全屏）下 `⌃⌥Space` 呼出 Quick Bar 覆盖当前空间、不切 Space；未固定时点击面板外立即关闭且草稿保留，固定后点击外侧不关；切 Space 自动复位且留存草稿。
2. `⌘↵` 提交后 5 秒非激活 Toast 正常出现于当前空间且点击撤销不抢原 App 焦点。
3. 打开主窗口/关闭主窗口时 `.accessory ↔ .regular` 切换正常、系统顶栏菜单完备、全屏工作台 Mission Control 拖拽排序持久化。
4. 45% 侧栏辅助窗口覆盖全屏右侧不挤压/改变其他 App 尺寸。
5. 中文拼音输入法在 CM6 待办复选框行与内嵌 xterm.js 终端内的组合态选词与上屏、快捷键拦截全部正常。

---

## C10. Git 工作流、提交规范与会话隔离

### C10.1 分支规范

- `master` 为稳定主线，禁止直接在 `master` 上提交功能开发代码。
- 日常全部开发在 `dev` 分支上拉取短生命周期任务分支：`feat/<card-id>-<slug>`、`fix/<slug>`、`contract/<slug>`；完成验收并通过 `pnpm check:all` 后合并回 `dev`。

### C10.2 中文提交规范

- 严禁使用 `--no-verify` 跳过钩子检查。
- 提交信息首行用中文说清「改了什么及为什么」，正文末尾必须包含契约与任务卡引用：

```text
feat(capture): 实现 Quick Bar S1-S8 目标真值表与跨工作区保护

将激活文档与选中工作区彻底解耦，防止主窗口隐藏或跨项目切换时误把想法追加到旧项目文档。

Contract: C8.1, C3.4
Card: M1-02
```

### C10.3 一卡一干净会话与独立审查

- 严格推行**一卡一干净会话**：每张任务卡在独立新会话中从阅读 `AGENTS.md` 与当前卡片开始；卡片门禁全绿并提交后即终止会话。
- **里程碑独立只读审查**：每个里程碑全部卡片合入后，另起一个全新只读审查会话，仅输入 `spec/quicknote-design-v5.md`、`spec/engineering-contract.md`、`DECISIONS.md` 与当前代码 diff，核查是否存在未实现边界、隐性偏差或自测漏洞。

---

## C11. 契约与决策变更流程

### C11.1 触发条件

施工中发现以下情况必须立刻停下当前编码，先执行变更流程：

- 现有 `C3`–`C8` 接口定义、字段类型或算法无法满足真实边界；
- 发现两处契约描述与 [quicknote-design-v5.md](quicknote-design-v5.md) 存在冲突；
- 需要在契约之外新增未定义依赖、数据表字段或产品交互规则。

M5 登记的新依赖：Rust `trash`（删除到废纸篓）、`tauri-plugin-dialog` 与 npm `@tauri-apps/plugin-dialog`（系统文件选择窗口）、npm `@codemirror/search`（查找替换）。

### C11.2 冲突仲裁优先级

当多份文件发生细节不一致时，按以下从高到低优先级仲裁，并立即修正下位文档消除不一致：

1. 用户在当前会话的最新明确指示（涉及产品行为时须先询问用户拍板）；
2. [DECISIONS.md](../DECISIONS.md) 中状态为 `accepted` 的架构决策；
3. [spec/engineering-contract.md](engineering-contract.md) 中的静态接口、DDL 与算法契约；
4. [spec/quicknote-design-v5.md](quicknote-design-v5.md) 的产品框架说明；
5. 任务卡正文与既有代码实现（不得以代码现状倒逼修改上位契约）。

### C11.3 ADR 状态流转

- 新决策写入 `DECISIONS.md` 末尾（顺延编号 `D-xxxx`），初始标注 `proposed`，经确认实施后改为 `accepted（C11.3）`。
- 若新决策取代旧决策，在旧条目状态中注明 `superseded by D-yyyy` 并在新条目背景中说明演进原因，不删除历史条目。

---

## C12. 任务卡标准模板

`spec/cards/` 下所有施工卡片必须遵循以下结构：

```markdown
# <Card-ID>：<简明中文标题>

- 所属里程碑：M0 / M1 / M2 / M3 / M4 / M5
- 关联方案与契约：`v5` §x.y、`Cx.y`、`D-xxxx`
- 前置依赖：<已完成卡片 ID 或无>
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：...
- 不做：...

## 2. 允许修改文件白名单
- `...`（严格限制；如需扩充必须先更新本卡再动代码）

## 3. 具体实现要求
- ...

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/...`、`src/.../*.test.ts`
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] ...
```
