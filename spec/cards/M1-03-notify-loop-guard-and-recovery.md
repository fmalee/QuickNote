# M1-03：`notify` 自身写入防回环、外部变更状态机与持久化辅助（`recovery/` & `quickbar-draft.json`）

- 所属里程碑：M1
- 关联方案与契约：`v4` §1.5、§3.9（G/H）、§4.1、§5.7、§6 M1；`C1.2`、`C1.3`、`C2.1`、`C2.3`、`C3.1`–`C3.4`、`C4.3`、`C6.1`、`C8.4`、`C8.5`、`C9.1`、`C9.2`、`D-0006`、`D-0009`、`D-0010`
- 前置依赖：`M1-02`
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - 在 `domain::watch` 实现 `C8.5` 自身写入防回环纯判定状态机：每次应用内部写入（`note_write`、`note_append`、`note_update_meta`、`note_create`、`note_undo_last_append`）成功后登记 `(canonical_abs_path, body_hash, expires_at = now + 1500ms)`。
  - 在 `infra::vault_watch` 与 `application::watch` 接入 `notify` 库目录递归监听器（随启动与 `library_set_root` 动态绑定当前 `library_root`）：
    - 收到磁盘 `.md` 修改事件后重读文件计算 `disk_body_hash`：若命中防回环缓存（哈希一致或处于抑制窗口且内容未变），仅静默同步索引 `mtime` 并**丢弃事件，零自激**；
    - 若哈希确实变化，更新 SQLite 索引并向前端推送 `fs:external-change`（`ExternalChangeEvent { absPath, kind: "modified", newBody, newBodyHash, newMeta }`）；
    - 收到磁盘文件删除或移出事件时，从 SQLite 索引移除对应条目并推送 `fs:external-change`（`kind: "deleted"`）。
  - 在 `domain::markdown` 提前落地 `C8.4` 中文锚点 Slug 纯函数 `heading_to_slug(raw_heading: &str, seen_counts: &mut HashMap<String, usize>) -> String`（供大纲与后续锚点共用，并用 `cn-headings-anchors.md` 验证）。
  - 在 `PersistenceManager` 落地 `C6.1` 本地应用数据目录 `$APP_DATA_DIR` 下 `quickbar-draft.json`（`{ mode, targetRelPath, selectedWorkspaceKey, text, updatedAt }` 原子读写与提交后清理）与 `recovery/<hash>.json`（脏标签每 2 秒崩溃恢复快照的写入、列举、单篇清理与全量恢复读取）内部支撑能力，并在 `src/lib/ipc/events.ts` 封装 `onFsExternalChange`。
- 不做：
  - **严禁超出 `C4.2` 新增非契约 Tauri IPC 命令名**（若前端需同步 Quick Bar 草稿或脏标签恢复快照，应通过既有命令生命周期或 `C6.1` 后端自动落盘/恢复机制协同，若确需扩展契约须先走 `C11` 登记）。
  - 不在本卡实现 CM6 `@codemirror/merge` `MergeView` 并排对比 UI（`MergeView` 完整交互归 `M4`；M1 前端冲突横幅提供重载磁盘 / 备份后覆盖 / 暂缓，并预留并排对比入口）。
  - 不编写主窗口或 Quick Bar 的 Vue 组件（归 `M1-04`、`M1-05`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（自动生成）、`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`
- Rust 领域、应用与基础设施层：`src-tauri/src/domain/mod.rs`、`src-tauri/src/domain/watch.rs`、`src-tauri/src/domain/markdown.rs`、`src-tauri/src/application/mod.rs`、`src-tauri/src/application/watch.rs`、`src-tauri/src/application/notes.rs`、`src-tauri/src/application/capture.rs`、`src-tauri/src/application/persistence.rs`、`src-tauri/src/application/library.rs`、`src-tauri/src/infra/mod.rs`、`src-tauri/src/infra/vault_watch.rs`、`src-tauri/src/lib.rs`
- Rust 测试：`src-tauri/tests/self_write_guard_and_watch_test.rs`、`src-tauri/tests/slug_and_recovery_test.rs`

## 3. 具体实现要求
- **自身写入 100% 零自激（`C8.5`）**：`note_write`、`note_append`、`note_update_meta`、`note_undo_last_append` 落盘前/后必须向写入抑制表注册 canonical 绝对路径与最新 `bodyHash`；哪怕 `notify` 在 macOS FSEvents 下延迟推送多次事件，只要磁盘正文哈希等于缓存 `bodyHash`，一律静默吞掉不发 `fs:external-change`。
- **外部真实修改与删除精准上报（`C4.3`、`C8.5`）**：当外部进程改写文件内容（`disk_body_hash != cached_body_hash`）时，后端必须解析最新 `NoteMeta` 与 `newBody` 并发出 `ExternalChangeEvent { kind: Modified, ... }`，同时自动刷新 SQLite `notes` 与 `notes_fts` 索引；当外部删除文件时发出 `ExternalChangeEvent { kind: Deleted, ... }`。
- **中文锚点 Slug 契约（`C8.4`）**：剥离行内 `**`、`*`、`` ` ``、`[text](url)` 修饰符，转小写，空白折叠为单 `-`，剔除中英文标点，完整保留汉字、假名、韩文、字母数字与 `-`/`_`，重复标题依次追加 `-1`、`-2`，不支持 `{#custom-id}`。
- **`recovery/` 与 `quickbar-draft.json` 落盘位置（`C6.1`）**：严格写入 `$APP_DATA_DIR/recovery/<hash>.json` 与 `$APP_DATA_DIR/quickbar-draft.json`，绝不写进用户 `library_root` 笔记库目录。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/self_write_guard_and_watch_test.rs`、`src-tauri/tests/slug_and_recovery_test.rs`
  - `self_write_guard_and_watch_test.rs`：
    1. 验证应用调用 `note_write` 与 `note_append` 后触发的文件监听回调被防回环状态机 100% 抑制，产生 `0` 条 `fs:external-change` 事件。
    2. 验证直接用 `std::fs::write` 模拟外部编辑器修改同一文件正文后，监听器准确捕获并产出 `ExternalChangeEvent { kind: Modified }`（含剥离 YAML 后的 `newBody`、`newBodyHash` 与 `newMeta`），且 SQLite FTS5 索引同步更新。
    3. 验证外部 `std::fs::remove_file` 删除文件后，监听器产出 `ExternalChangeEvent { kind: Deleted }` 且索引移除该记录。
  - `slug_and_recovery_test.rs`：
    1. 使用 `src-tauri/tests/fixtures/vault/cn-headings-anchors.md` 检验 `heading_to_slug` 对中英混排、行内代码/加粗/链接、特殊标点与同名重复标题（`-1`、`-2`）的输出。
    2. 验证 `recovery/<hash>.json` 与 `quickbar-draft.json` 的原子写入、重启恢复读取与提交/保存后的自动清理。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡聚焦后端 `notify` 防回环状态机、Slug 算法与本地快照持久化，UI 联动点验在 `M1-04` 与 `M1-05` 进行；本卡以 `pnpm check:all` 全绿为准。
