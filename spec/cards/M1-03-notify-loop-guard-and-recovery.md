# M1-03：`notify` 自身写入防回环、外部变更状态机与持久化辅助（`recovery/` & `quickbar-draft.json`）

- 所属里程碑：M1
- 关联方案与契约：`v4` §1.5、§3.9（G/H）、§4.1、§5.7、§6 M1；`C1.2`、`C1.3`、`C2.1`、`C2.3`、`C3.1`–`C3.4`、`C4.3`、`C6.1`、`C8.4`、`C8.5`、`C9.1`、`C9.2`、`D-0006`、`D-0009`、`D-0010`
- 前置依赖：`M1-02`
- 会话策略：独立新会话执行（零大文档读取，所有结构体、防回环状态机、Slug 算法与持久化规格已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 在 `src-tauri/src/domain/watch.rs` 定义外部变更事件 IPC 结构体与 `C8.5` 自身写入防回环纯判定状态机（零 IO / 零框架依赖）**：
    - IPC 结构体（均加 `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]` 与 `#[serde(rename_all = "camelCase")]`）：
      - `ExternalChangeKind`：枚举 `Modified | Deleted`（序列化为 `"modified" | "deleted"`）
      - `ExternalChangeEvent { pub abs_path: String, pub kind: ExternalChangeKind, pub new_body: Option<String>, pub new_body_hash: Option<String>, pub new_meta: Option<NoteMeta> }`
    - 防回环纯数据结构 `SelfWriteGuard`（默认抑制窗口 `SELF_WRITE_SUPPRESS_MS: i64 = 1500`）：
      - 记录条目：`SelfWriteRecord { pub body_hash: String, pub expires_at_ms: i64 }`，内部 `HashMap<String, SelfWriteRecord>`（key 为规范化绝对路径字符串）。
      - `record_write(&mut self, canonical_abs_path: &str, body_hash: &str, now_ms: i64)`：存入 `expires_at_ms = now_ms + SELF_WRITE_SUPPRESS_MS` 并顺带清理已过期条目。
      - `should_suppress_modify(&mut self, canonical_abs_path: &str, disk_body_hash: &str, now_ms: i64) -> bool`：
        - 若存在该路径的缓存记录且 `disk_body_hash == record.body_hash`（无论是否刚超过 1500ms，或处于 `now_ms <= record.expires_at_ms` 窗口内且哈希未变），判定为**自身写入回环**，返回 `true`（静默忽略事件）；
        - 若 `disk_body_hash != record.body_hash`，说明磁盘发生了真实的外部修改，移除旧缓存记录并返回 `false`（允许发出 `ExternalChangeEvent { kind: Modified, ... }`）。
      - `clear_path(&mut self, canonical_abs_path: &str)`：在文件被删除或重命名移出时移除该路径缓存。
  - **2. 在 `src-tauri/src/domain/markdown.rs` 实现 `C8.4` 中文锚点 Slug 纯函数與大纲标题提取辅助**：
    - `pub fn heading_to_slug(raw_heading: &str, seen_counts: &mut HashMap<String, usize>) -> String`：
      1. 剥离行内 Markdown 语法修饰符：去掉 `**`、`__`、`*`、`_`、`` ` ``、以及将 `[text](url)` 替换为 `text`（忽略 `{#custom-id}`，不支持非标自定义锚点语法）。
      2. 全字符转小写（`to_lowercase()`）；所有空白字符连续折叠为单个 `-`；去除 ASCII 标点与常见中文标点符号（如 `：:（()）、，。！？!?,.;'"“”‘’[]{}<>《》【】/\|~@#$%^&*+=` 等），**完整保留所有汉字、假名、韩文（`domain::tokenizer::is_cjk`）及 ASCII 字母数字与 `-` / `_`**；折叠连续 `--`（若由空格+标点产生）并去除首尾多余 `-`。
      3. 查 `seen_counts`：首次出现记 `0` 并返回 `base`；第 `k` 次重复出现（`k >= 1`）更新计数为 `k` 并返回 `format!("{base}-{k}")`。
    - `pub struct OutlineHeading { pub level: u8, pub text: String, pub slug: String, pub line: u32 }` 与 `pub fn extract_headings(body: &str) -> Vec<OutlineHeading>`：跳过 `` ``` `` / `~~~` 围栏代码块，扫描 ATX 标题 `#`..`######`，剥离行内修饰符生成显示文本与 `heading_to_slug` 锚点（1-based 行号）。
  - **3. 在 `src-tauri/src/infra/vault_watch.rs` 与 `src-tauri/src/application/watch.rs` 实现 `notify` 库目录递归监听与防回环协同**：
    - 在 `PersistenceManager` 中内嵌 `self_write_guard: Mutex<SelfWriteGuard>`（并提供 `record_self_write(path: &Path, body_hash: &str)`、`should_suppress_external_modify(path: &Path, disk_body_hash: &str) -> bool` 辅助方法，路径统一 `std::fs::canonicalize` 兜底原路径规范化，解决 macOS `/var` 与 `/private/var` 软链差异）。
    - 在 `application::notes::{note_create, note_write, note_update_meta, note_rename}` 与 `application::capture::{note_append, note_undo_last_append}` 每次原子写盘成功后立即调用 `pm.record_self_write(&abs_path, &new_body_hash)`（`note_delete` / `note_rename` 旧路径调用 `clear_self_write`），并确保正常保存 `note_write` 成功后自动清理对应的 `$APP_DATA_DIR/recovery/<hash>.json` 快照。
    - 实现 `VaultWatcher`（参考 `infra/settings_watch.rs` 的 `notify::RecommendedWatcher` + 100ms 防抖线程模式）：
      - 递归监听当前 `library_root` 下 `.md` 文件变更（忽略 `.` 隐藏文件/目录、`assets/` 与 `.tmp.` 临时原子写文件）。
      - 实现 `application::watch::process_external_fs_path(pm: &PersistenceManager, path: &Path) -> Result<Option<ExternalChangeEvent>, AppError>`（可被 `VaultWatcher` 后台线程与集成测试直接调用）：
        - 若 `path` 在磁盘上已不存在（被外部删除或移走）：从 SQLite 索引移除该 `rel_path`（`delete_note_index_by_rel_path`），清理 `self_write_guard`，返回 `Some(ExternalChangeEvent { abs_path, kind: ExternalChangeKind::Deleted, new_body: None, new_body_hash: None, new_meta: None })`。
        - 若 `path` 存在且为 `.md` 文件：读取磁盘全文并 `parse_note` 计算 `disk_body_hash`；若 `pm.should_suppress_external_modify(path, &disk_body_hash)` 为 `true`，仅静默刷新索引 `mtime` 并返回 `Ok(None)`（**零自激**）；若为 `false`，调用 `index_single_note` 刷新 SQLite + FTS5 索引，并返回 `Some(ExternalChangeEvent { abs_path, kind: ExternalChangeKind::Modified, new_body: Some(parsed.body), new_body_hash: Some(disk_body_hash), new_meta: Some(parsed.meta) })`。
      - 在 `VaultWatcher` 检测到返回 `Some(event)` 时，通过 `AppHandle::emit("fs:external-change", &event)` 推送给前端；支持在 `library_set_root` 切换库目录时动态重绑监听目录。
  - **4. 在 `src-tauri/src/application/persistence.rs` 落地 `C6.1` `$APP_DATA_DIR/quickbar-draft.json` 与 `$APP_DATA_DIR/recovery/<hash>.json` 持久化能力**：
    - 在 `domain::watch`（或 `domain::ui_state`）定义：
      - `QuickBarDraft { pub mode: CaptureMode, pub target_rel_path: Option<String>, pub selected_workspace_key: Option<String>, pub text: String, #[specta(type = specta_typescript::Number)] pub updated_at: i64 }`
      - `RecoverySnapshot { pub abs_path: String, pub rel_path: Option<String>, pub dirty_body: String, pub base_body_hash: String, #[specta(type = specta_typescript::Number)] pub updated_at: i64 }`
    - 在 `PersistenceManager` 提供公开方法：
      - `save_quickbar_draft(&self, draft: &QuickBarDraft) -> Result<(), AppError>`、`load_quickbar_draft(&self) -> Result<Option<QuickBarDraft>, AppError>`、`clear_quickbar_draft(&self) -> Result<(), AppError>`（并在 `note_append` 成功后自动调用 `clear_quickbar_draft`）。
      - `save_recovery_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), AppError>`（文件名 `<compute_body_hash(&snapshot.abs_path)>.json` 存于 `$APP_DATA_DIR/recovery/`）、`list_recovery_snapshots(&self) -> Result<Vec<RecoverySnapshot>, AppError>`、`clear_recovery_snapshot(&self, abs_path: &str) -> Result<(), AppError>`（并在 `note_write` 成功后自动调用 `clear_recovery_snapshot`）。
  - **5. 绑定导出与前端事件监听封装**：
    - 在 `src-tauri/src/bindings.rs` 注册 `ExternalChangeKind`、`ExternalChangeEvent`、`QuickBarDraft`、`RecoverySnapshot`，运行 `pnpm gen:bindings` 更新 `src/bindings/types.ts`。
    - 在 `src/lib/ipc/events.ts` 新增 `onFsExternalChange(handler: (event: ExternalChangeEvent) => void): Promise<UnlistenFn>`。
- 不做：
  - **严禁超出 `C4.2` 新增非契约 Tauri IPC 命令名**。
  - 不编写主窗口或 Quick Bar 的 Vue 组件（归 `M1-04a`、`M1-04b`、`M1-05`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（自动生成）、`src/lib/ipc/events.ts`
- Rust 领域、应用与基础设施层：`src-tauri/src/domain/mod.rs`、`src-tauri/src/domain/watch.rs`、`src-tauri/src/domain/markdown.rs`、`src-tauri/src/application/mod.rs`、`src-tauri/src/application/watch.rs`、`src-tauri/src/application/notes.rs`、`src-tauri/src/application/capture.rs`、`src-tauri/src/application/persistence.rs`、`src-tauri/src/application/library.rs`、`src-tauri/src/infra/mod.rs`、`src-tauri/src/infra/vault_watch.rs`、`src-tauri/src/lib.rs`
- Rust 测试：`src-tauri/tests/self_write_guard_and_watch_test.rs`、`src-tauri/tests/slug_and_recovery_test.rs`

## 3. 具体实现要求
- **自身写入 100% 零自激（`C8.5`）**：注意 macOS 临时目录 `/var/folders/...` 与 `/private/var/folders/...` 软链接差异，所有登记到 `SelfWriteGuard` 与 `process_external_fs_path` 比较的路径必须经过统一的规范化函数处理。
- **单向分层与零 Panic（`C1.2`、`C2.1`）**：`domain/watch.rs` 与 `domain/markdown.rs` 严禁依赖 `tauri`、`std::fs`、`rusqlite` 或 `notify`；生产代码零 `.unwrap()` / `.expect()` / `panic!()`。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/self_write_guard_and_watch_test.rs`、`src-tauri/tests/slug_and_recovery_test.rs`
  - `self_write_guard_and_watch_test.rs`：
    1. 验证应用调用 `note_write`、`note_append`、`note_update_meta` 后对目标文件执行 `process_external_fs_path`（以及真实 `VaultWatcher` 监听回调）被防回环状态机 100% 抑制，返回 `None`（产生 `0` 条 `fs:external-change` 事件）。
    2. 验证直接用 `std::fs::write` 模拟外部进程修改同一文件正文后，准确产出 `ExternalChangeEvent { kind: ExternalChangeKind::Modified }`（含剥离 YAML 后的 `new_body`、`new_body_hash` 与 `new_meta`），且 SQLite FTS5 索引立即能搜到外部新增词。
    3. 验证外部 `std::fs::remove_file` 删除文件后，产出 `ExternalChangeEvent { kind: ExternalChangeKind::Deleted }` 且 SQLite 索引移除该记录。
  - `slug_and_recovery_test.rs`：
    1. 读取 `src-tauri/tests/fixtures/vault/cn-headings-anchors.md` 检验 `heading_to_slug` 与 `extract_headings`：验证 `# 中文标题与 Anchor 混排` → `"中文标题与-anchor-混排"`、第一个 `## 安装 Install 指南` → `"安装-install-指南"`、第二个重复 `## 安装 Install 指南` → `"安装-install-指南-1"`、`## API：接口说明（v2）` → `"api接口说明v2"`（或去标点规范结果），且跳过代码块内 `#` 注释与剥离 `**`/`[text](url)`。
    2. 验证 `save_quickbar_draft` / `load_quickbar_draft` 在 `note_append` 成功后自动清除；验证 `save_recovery_snapshot` / `list_recovery_snapshots` 在 `note_write` 成功后自动清除对应快照。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡聚焦后端 `notify` 防回环状态机、中文 Slug 算法与本地快照持久化，UI 联动点验在 `M1-04b` 与 `M1-05` 进行；本卡以 `pnpm check:all` 全绿为准。
