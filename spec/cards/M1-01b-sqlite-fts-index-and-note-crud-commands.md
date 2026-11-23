# M1-01b：SQLite FTS5 派生索引入库、笔记 CRUD 用例与 11 条 IPC 命令封装

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.3、§4.1–§4.6、§4.9、§5.3、§6 M1；`C1.2`、`C1.3`、`C2.1`、`C2.3`、`C3.3`、`C3.4`、`C4.1`–`C4.3`、`C5.1`–`C5.3`、`C6.1`、`C6.4`、`C9.1`、`C9.2`、`D-0006`、`D-0007`、`D-0009`、`D-0010`
- 前置依赖：`M1-01a`
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - 在 `src-tauri/src/infra/sqlite.rs` 与 `src-tauri/src/application/index.rs` 实现库目录 `.md` 扫描、SQLite `notes` / `note_tags` / `notes_fts` 派生索引读写与 FTS5 混合分词查询：
    - 忽略 `.` 开头隐藏文件/目录、`assets/`、`_templates/`（或标记 `_templates/` 为 `NoteType::Template`）。
    - 检测重复 ULID（如 `duplicate-id-a.md` 与 `duplicate-id-b.md`）：标记 `meta_error` 且绝不篡改磁盘文件 `id`，冲突项 `key` 回退为 `path:<rel_path>`。
    - `notes_fts` 仅索引 `title_tokens`、`tag_tokens` 与剥离 YAML 后的纯正文 `body_tokens`（调用 `domain::tokenizer::tokenize_mixed`）。
    - 实现 `index_get_status`、`index_rebuild`（向前端 emit `index:status` 事件；并在 `library_set_root` 成功后自动触发全量索引重建）。
  - 在 `src-tauri/src/application/notes.rs` 实现笔记 CRUD 与导航/搜索用例，并在 `src-tauri/src/commands/{notes,index,search}.rs` 及 `src-tauri/src/lib.rs` 注册全部 11 条 M1 笔记与索引 IPC 命令（`index_get_status`、`index_rebuild`、`notes_list`、`notes_get_tree_and_tags`、`note_read`、`note_create`、`note_write`、`note_update_meta`、`note_rename`、`note_delete`、`search_notes_and_files`）：
    1. `index_get_status() -> Result<IndexStatus, AppError>`
    2. `index_rebuild() -> Result<IndexStatus, AppError>`
    3. `notes_list(args: NotesFilterArgs) -> Result<Vec<NoteSummary>, AppError>`（支持 `tab: "inbox" | "projects" | "tags" | "recent" | "workspaces"`、`dir_rel_path` 前缀过滤、`tags` 多标签 AND 且 `dev` 匹配 `dev/*`、`workspace_key`、`filter_text`；按 `pinned DESC, mtime_ms DESC` 排序）
    4. `notes_get_tree_and_tags() -> Result<NavTreeSnapshot, AppError>`（返回 `inboxCount`、目录树 `dirs`、层级标签树 `tags`、配置工作区 `workspaces`）
    5. `note_read(args: ReadNoteArgs) -> Result<NoteDoc, AppError>`（返回剥离 YAML 后的 `body`、`bodyHash` 与结构化 `meta`）
    6. `note_create(args: CreateNoteArgs) -> Result<NoteDoc, AppError>`（生成 ULID、ISO `created` 与 frontmatter 原子写入磁盘并同步索引）
    7. `note_write(args: WriteNoteArgs) -> Result<WriteResult, AppError>`（校验磁盘当前 `bodyHash == args.base_body_hash`，不一致返回 `ErrorCode::Conflict` 并在 `detail` 填入 `ConflictInfo` JSON；一致时保留磁盘现有 YAML 原样拼接新 `body` 原子写入；**唯一例外 `D-0006`**：当库内笔记缺失 `id` 时，自动生成新 ULID 补入 frontmatter 并在 `WriteResult.updated_meta` 返回更新后的 `NoteMeta`）
    8. `note_update_meta(args: NoteUpdateMetaArgs) -> Result<NoteDoc, AppError>`（调用 `domain::frontmatter` 最小补丁器原地更新 YAML，不校验 `baseBodyHash`；若原 YAML 损坏无法原地补丁，先备份原文件至 `$APP_DATA_DIR/backups/` 再重写最小合法 frontmatter）
    9. `note_rename(args: RenameNoteArgs) -> Result<NoteDoc, AppError>`（移动/重命名库内 `.md`，保持 `id` 不变，更新索引 `rel_path`）
    10. `note_delete(args: RelPathArgs) -> Result<(), AppError>`（禁止删除 `inbox.md`，其余库内笔记删除文件并清理 SQLite 索引）
    11. `search_notes_and_files(args: SearchQueryArgs) -> Result<SearchResponse, AppError>`（结合 `notes_fts` BM25 与标题/路径匹配，原样回传 `request_id`）
  - 在 `src/lib/ipc/commands.ts` 与 `src/lib/ipc/events.ts` 补齐上述 11 条命令函数与 `onIndexStatus` 事件监听封装。
- 不做：
  - 不实现 `capture_resolve_target`、`note_append`、`note_undo_last_append`（归 `M1-02`）。
  - 不实现 `notify` 实时监听与防回环（归 `M1-03`）。
  - 不改动任何前端 Vue 组件（归 `M1-04`、`M1-05`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（如有补充导出）、`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`
- Rust 应用、基础设施与命令层：`src-tauri/src/domain/note.rs`（仅允许补充纯辅助函数）、`src-tauri/src/application/mod.rs`、`src-tauri/src/application/notes.rs`、`src-tauri/src/application/index.rs`、`src-tauri/src/application/library.rs`、`src-tauri/src/infra/fs.rs`、`src-tauri/src/infra/sqlite.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/commands/notes.rs`、`src-tauri/src/commands/index.rs`、`src-tauri/src/commands/search.rs`、`src-tauri/src/lib.rs`
- Rust 集成测试：`src-tauri/tests/note_crud_and_index_test.rs`

## 3. 具体实现要求
- **单向分层与零 Panic（`C1.2`、`C2.1`）**：`commands` 只做薄转发；`src-tauri/src/**` 严禁出现 `.unwrap()` / `.expect()`；日志严禁打印笔记正文或搜索词全文（`C2.3`）。
- **ULID 补写唯一例外（`D-0006`）**：对 `missing-id.md` 执行 `note_write` 时必须自动生成 26 位 ULID 写入 frontmatter，且 `WriteResult.updated_meta` 返回 `Some(NoteMeta { id: Some(..), .. })`；而对已有 `id` 的笔记执行 `note_write` 时 `updated_meta` 为 `None` 且原有 frontmatter 字节 100% 不变。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/note_crud_and_index_test.rs`
  - 将 `src-tauri/tests/fixtures/vault/` 复制到临时目录并调用 `set_library_root` / `index_rebuild`：
    1. 验证 `index_get_status`、`notes_get_tree_and_tags`（含 `inboxCount`、目录树与层级标签树）、`notes_list`（多标签 AND、`dev` 匹配 `dev/*`）。
    2. 验证 `duplicate-id-a.md` 与 `duplicate-id-b.md` 均入库且标记 `meta_error`，磁盘原文件 `id` 未被篡改。
    3. 验证 `missing-id.md` 调用 `note_write` 后自动补写 ULID 并返回 `updated_meta`；`base_body_hash` 不一致时返回 `ErrorCode::Conflict`。
    4. 验证 `unknown-keys-comments.md` 调用 `note_update_meta` 后保留注释与未知键；`broken-yaml.md` 调用 `note_update_meta` 时先在 `backups/` 生成 `.bak` 备份。
    5. 验证 `note_create`、`note_rename`、`note_delete` 及 `search_notes_and_files`（搜 `git` 不命中 `digit`、搜中文词组准确命中、原样回传 `request_id`）。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡为后端 SQLite 索引、笔记 CRUD 与 `src/lib/ipc/` 封装，无原生窗口交互改动，以 `pnpm check:all` 全绿为准。
