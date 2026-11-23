# M1-01b：SQLite FTS5 派生索引入库、笔记 CRUD 用例与 11 条 IPC 命令封装

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.3、§4.1–§4.6、§4.9、§5.3、§6 M1；`C1.2`、`C1.3`、`C2.1`、`C2.3`、`C3.3`、`C3.4`、`C4.1`–`C4.3`、`C5.1`–`C5.3`、`C6.1`、`C6.4`、`C9.1`、`C9.2`、`D-0006`、`D-0007`、`D-0009`、`D-0010`
- 前置依赖：`M1-01a`
- 会话策略：独立新会话执行（零大文档读取，所有契约已内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **SQLite 索引与混合 FTS5 入库**（`src-tauri/src/infra/sqlite.rs`、`src-tauri/src/application/index.rs`）：
    - 表结构（已在 `0001_init.sql` 创建好，无需写新迁移）：
      - `notes(key TEXT PRIMARY KEY, id TEXT, rel_path TEXT NOT NULL UNIQUE, title TEXT NOT NULL, doc_type TEXT NOT NULL, project TEXT, mtime_ms INTEGER NOT NULL, size_bytes INTEGER NOT NULL, body_hash TEXT NOT NULL, frontmatter_json TEXT NOT NULL, excerpt TEXT NOT NULL, meta_error TEXT, pinned INTEGER NOT NULL DEFAULT 0, archived INTEGER NOT NULL DEFAULT 0, is_inbox INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL)`
      - `note_tags(note_key TEXT NOT NULL, tag TEXT NOT NULL, PRIMARY KEY (note_key, tag))`
      - `notes_fts(note_key UNINDEXED, title_tokens, tag_tokens, body_tokens)`（FTS5 虚表，`body_tokens` 只用剥离 YAML 后的纯正文经 `domain::tokenizer::tokenize_mixed` 生成）
    - 库扫描规则：递归扫描 `library_root` 下全部 `.md` 文件，忽略 `.` 开头隐藏目录/文件、`assets/`；`_templates/` 下的 `.md` 若无显式 `type` 则默认视为 `NoteType::Template`（或在常规笔记列表中默认排除 `doc_type == "template"`）。
    - 主键 `key` 与重复 ULID 规则：正常有 `id` 时 `key = id`，无 `id` 时 `key = format!("path:{rel_path}")`；当全库扫描发现重复 `id`（如 `duplicate-id-a.md` 与 `duplicate-id-b.md`）时，两篇都标记 `meta_error`（提示重复 `id`），绝不篡改磁盘文件，且后续重复项的 `key` 回退为 `path:<rel_path>` 避免主键冲突覆写前一篇。
    - 实现 `index_get_status` 与 `index_rebuild`（更新 `library_meta` 的 `last_indexed_at`，如有 `AppHandle` 则 emit `index:status` 事件；并在 `application::library::set_library_root` 成功后自动触发 `index_rebuild`）。
  - **笔记 CRUD、导航树与全文搜索用例**（`src-tauri/src/application/notes.rs`）及 **11 条 Tauri IPC 命令**（`index_get_status`、`index_rebuild`、`notes_list`、`notes_get_tree_and_tags`、`note_read`、`note_create`、`note_write`、`note_update_meta`、`note_rename`、`note_delete`、`search_notes_and_files`；落位 `src-tauri/src/commands/{notes,index,search}.rs`、`src-tauri/src/lib.rs`）：
    1. `index_get_status`：无参 → `Result<IndexStatus, AppError>`
    2. `index_rebuild`：无参 → `Result<IndexStatus, AppError>`
    3. `notes_list(args: NotesFilterArgs) -> Result<Vec<NoteSummary>, AppError>`：
       - `args.tab`：`"inbox"`（仅 `is_inbox == true`）、`"projects"`（排除 `doc_type == Template`，按 `dir_rel_path` 目录前缀过滤）、`"tags"`（按 `args.tags` 多标签 AND 过滤，且父标签 `dev` 匹配 `dev` 与 `dev/*`，可调用 `domain::note::tag_matches`）、`"recent"`（排除 `Template`，按 `mtime_ms DESC`）、`"workspaces"`（按 `args.workspace_key` 匹配 `project == workspace_key`）；若 `args.filter_text` 非空则对 `title` / `rel_path` / `tags` 做不区分大小写子串过滤；默认排序 `pinned DESC, mtime_ms DESC`。
    4. `notes_get_tree_and_tags() -> Result<NavTreeSnapshot, AppError>`：
       - `inbox_count`：读取 `inbox.md` 正文调用 `domain::note::count_list_items(&body)`；
       - `dirs`：从库内非 `_templates/`、非 `inbox.md` 笔记的 `rel_path` 构建层级目录树 `Vec<DirNode>`（含子目录笔记计数）；
       - `tags`：从 `note_tags` 构建层级标签树 `Vec<TagNode>`（如 `dev/quicknote` 自动挂在父节点 `dev` 的 `children` 下，父节点 `count` 为匹配 `tag_matches(t, "dev")` 的去重笔记数）；
       - `workspaces`：从当前 `Settings.workspaces` 映射 `Vec<WorkspaceSummary>`。
    5. `note_read(args: ReadNoteArgs) -> Result<NoteDoc, AppError>`：支持库内文件与库外 `.md` 绝对路径，返回剥离 YAML 后的 `body`、`body_hash` 与 `meta`。
    6. `note_create(args: CreateNoteArgs) -> Result<NoteDoc, AppError>`：校验 `title` 非空，生成 ULID 与本地 ISO 时间 `created`，在 `dir_rel_path` 下创建 `<slug-or-title>.md`（重名时返回 `ValidationError` 或自动区分），原子写盘并同步 SQLite 索引。
    7. `note_write(args: WriteNoteArgs) -> Result<WriteResult, AppError>`：
       - 读取磁盘当前文件并 `parse_note` 计算当前 `disk_body_hash`；若 `disk_body_hash != args.base_body_hash`，返回 `AppError::with_detail(ErrorCode::Conflict, "Document modified on disk", serde_json::to_string(&ConflictInfo { abs_path, current_body, current_body_hash, current_mtime_ms })?)`。
       - 若一致：原样保留磁盘当前 YAML frontmatter 块与新 `args.body` 拼接（`compose_note`）并原子写入；**唯一例外（`D-0006`）**：当该文件属于库内笔记（`is_library == true`）且 `parsed.meta.id.is_none()` 且 frontmatter 可补丁（或无 frontmatter）时，自动生成新 ULID 通过 `patch_frontmatter_block` 补入 `id` 字段，写盘并同步索引，此时 `WriteResult.updated_meta = Some(new_meta)`；其余情况 `updated_meta = None`。
    8. `note_update_meta(args: NoteUpdateMetaArgs) -> Result<NoteDoc, AppError>`：
       - 调用 `meta_patch_ops(&args.patch)` 与 `patch_frontmatter_block` 原地最小补丁 YAML，不改动 `body`、不校验 `base_body_hash`；
       - 若原文件 YAML 语法损坏（`!parsed.yaml_patchable` 且含损坏 frontmatter），先将原始全文备份至 `$APP_DATA_DIR/backups/`（调用 `format_backup_filename` + `prune_backups`），再以新 `NoteMeta` 生成合法 frontmatter 拼接原 `body` 写回并更新索引。
    9. `note_rename(args: RenameNoteArgs) -> Result<NoteDoc, AppError>`：将库内 `rel_path` 重命名/移动至 `new_rel_path`（禁止重命名 `inbox.md`），保持 `id` 不变，更新磁盘与 SQLite 索引。
    10. `note_delete(args: RelPathArgs) -> Result<(), AppError>`：禁止删除 `inbox.md`（返回 `ValidationError`），删除目标库内 `.md` 文件并从 SQLite `notes` / `note_tags` / `notes_fts` 移除。
    11. `search_notes_and_files(args: SearchQueryArgs) -> Result<SearchResponse, AppError>`：用 `build_fts_query(&args.query)` 查询 `notes_fts` 并结合标题/相对路径子串加权排序，按 `args.limit.unwrap_or(30)` 截断，原样回传 `args.request_id`。
  - **前端手写 IPC 封装**（`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`）：
    - 在 `commands.ts` 新增：`indexGetStatus`、`indexRebuild`、`notesList`、`notesGetTreeAndTags`、`noteRead`、`noteCreate`、`noteWrite`、`noteUpdateMeta`、`noteRename`、`noteDelete`、`searchNotesAndFiles`。
    - 在 `events.ts` 新增：`onIndexStatus(handler: (status: IndexStatus) => void)`。
- 不做：
  - 不实现 `capture_resolve_target`、`note_append`、`note_undo_last_append`（归 `M1-02`）。
  - 不实现 `notify` 实时监听与防回环（归 `M1-03`）。
  - 不改动任何前端 Vue 组件（归 `M1-04a`、`M1-04b`、`M1-05`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`（注：`src/bindings/types.ts` 已在 `M1-01a` 生成完毕，除非确需微调否则无需改 `bindings.rs`）
- Rust 应用、基础设施与命令层：`src-tauri/src/domain/note.rs`（仅限补充纯辅助函数）、`src-tauri/src/application/mod.rs`、`src-tauri/src/application/notes.rs`、`src-tauri/src/application/index.rs`、`src-tauri/src/application/library.rs`、`src-tauri/src/infra/fs.rs`、`src-tauri/src/infra/sqlite.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/commands/notes.rs`、`src-tauri/src/commands/index.rs`、`src-tauri/src/commands/search.rs`、`src-tauri/src/lib.rs`
- Rust 集成测试：`src-tauri/tests/note_crud_and_index_test.rs`

## 3. 具体实现要求
- **单向分层与零 Panic（`C1.2`、`C2.1`）**：`commands` 只做薄转发；`src-tauri/src/**` 严禁出现 `.unwrap()` / `.expect()` / `panic!()`；日志严禁打印笔记正文或搜索词全文（`C2.3`）。
- **Tauri 命令入参约定**：与现有 `src/lib/ipc/commands.ts` 保持一致——有入参的命令统一使用 `args: XxxArgs` 作为参数名（前端 `invoke('note_read', { args })`），无参命令不传 `args`。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/note_crud_and_index_test.rs`
  - 将 `src-tauri/tests/fixtures/vault/` 复制到临时测试目录并调用 `set_library_root` / `index_rebuild`：
    1. 验证 `index_get_status`、`notes_get_tree_and_tags`（含 `inboxCount`、目录树与层级标签树）、`notes_list`（多标签 AND、`dev` 匹配 `dev/*`）。
    2. 验证 `duplicate-id-a.md` 与 `duplicate-id-b.md` 均入库且标记 `meta_error`，磁盘原文件 `id` 未被篡改。
    3. 验证 `missing-id.md` 调用 `note_write` 后自动补写 ULID 并返回 `updated_meta`；`base_body_hash` 不一致时返回 `ErrorCode::Conflict`。
    4. 验证 `unknown-keys-comments.md` 调用 `note_update_meta` 后保留注释与未知键；`broken-yaml.md` 调用 `note_update_meta` 时先在 `backups/` 生成 `.bak` 备份。
    5. 验证 `note_create`、`note_rename`、`note_delete` 及 `search_notes_and_files`（搜 `git` 不命中 `digit`、搜中文词组准确命中、原样回传 `request_id`）。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡为后端 SQLite 索引、笔记 CRUD 与 `src/lib/ipc/` 封装，无原生窗口交互改动，以 `pnpm check:all` 全绿为准。
