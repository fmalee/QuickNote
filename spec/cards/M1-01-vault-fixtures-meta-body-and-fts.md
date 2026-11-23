# M1-01：共享 Fixture 库、`meta + body` 分离、YAML 最小补丁与混合 FTS5 索引引擎

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.3、§4.1–§4.6、§4.9、§5.3、§6 M1；`C1.2`、`C1.3`、`C2.1`、`C2.3`、`C3.1`–`C3.4`、`C4.2`、`C4.3`、`C5.1`–`C5.3`、`C6.1`、`C6.4`、`C9.1`、`C9.2`、`D-0006`、`D-0007`、`D-0009`、`D-0010`
- 前置依赖：`M0-04`
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - 按 `C9.2` 在 `src-tauri/tests/fixtures/vault/` 补齐 M1 所需共享静态 Fixture 测试样本（`inbox.md` 超 500 行膨胀样本、`missing-id.md`、`duplicate-id-a.md` & `duplicate-id-b.md`、`broken-yaml.md`、`unknown-keys-comments.md`、`code-fence-heading.md`、`cn-headings-anchors.md`、`commands/常用命令.md` 及大体积边界测试生成辅助）。
  - 在 `domain::tokenizer` 实现 `C5.3` CJK + ASCII 混合分词纯函数 `tokenize_mixed(input: &str) -> String`（ASCII 保持完整词元并拆分 `.` / `-` / `/` / `:` 子词元，严禁对 ASCII 做双字 bigram；CJK 单字保留、长度 `>= 2` 连续滑动 2-gram；FTS5 `body_tokens` 仅输入剥离 frontmatter 后的纯正文）。
  - 在 `domain::note` 与 `domain::frontmatter` 实现 `D-0006` / `D-0007` / `C6.4` 的 `meta + body` 强分离解析、`bodyHash` 计算、宽容 YAML 读取（损坏 YAML 或未知 `type` 不阻塞正文打开，记录 `metaError`）与 YAML 最小补丁器（只增删改目标 key，原样保留未知字段、注释与字段顺序；ISO 日期带双引号、`tags` 输出 YAML 块列表 `- tag`）。
  - 实现笔记 CRUD 与元数据 IPC 命令：`note_read`、`note_create`、`note_write`（校验 `baseBodyHash` 乐观并发；**唯一例外**：库内文档缺失 `id` 时在 `note_write` 自动生成新 ULID 补入 `meta` 并于 `WriteResult.updatedMeta` 返回）、`note_update_meta`（走最小补丁器，不校验 `baseBodyHash`；若原 YAML 损坏无法补丁则先备份至 `backups/`）、`note_rename`（保持 `id` 不变，更新 `rel_path`，1.0 不自动改写其他笔记相对链接）、`note_delete`。
  - 实现库扫描、SQLite FTS5 派生索引与查询命令：`index_get_status`、`index_rebuild`（推送 `index:status` 事件；联动 `library_set_root` 搬库时重扫并维持 `rel_path` / `id` 定位）、`notes_list`（支持 `NotesFilterArgs` 的 `tab`、目录、多标签 AND、`dev` 匹配 `dev/*`、工作区与关键词过滤）、`notes_get_tree_and_tags`（返回 `NavTreeSnapshot` 含 `inboxCount`、目录树、标签树与工作区摘要）、`search_notes_and_files`（库内 FTS5 + 标题/路径匹配，携带递增 `requestId` 原样回传）。
  - 通过 `pnpm gen:bindings` 导出上述结构体至 `src/bindings/types.ts`，并在 `src/lib/ipc/commands.ts` 与 `src/lib/ipc/events.ts` 补齐对应手写封装（含 `onIndexStatus`）。
- 不做：
  - 不实现 Quick Bar `S1–S8` 目标解析、`note_append` 与 `note_undo_last_append`（归 `M1-02`）。
  - 不实现 `notify` 文件系统实时监听、写入防回环缓存与 `recovery/` 崩溃恢复命令（归 `M1-03`）。
  - 不编写主窗口导航树、CM6 编辑器或 Quick Bar 前端视图组件（归 `M1-04`、`M1-05`）。
  - 不实现 Spotlight `mdfind`、路径补全、命令书检索或图片保存（归 `M2` / `M3`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（自动生成）、`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`
- Rust 领域层：`src-tauri/src/domain/mod.rs`、`src-tauri/src/domain/note.rs`、`src-tauri/src/domain/frontmatter.rs`、`src-tauri/src/domain/tokenizer.rs`
- Rust 应用、基础设施与命令层：`src-tauri/src/application/mod.rs`、`src-tauri/src/application/notes.rs`、`src-tauri/src/application/index.rs`、`src-tauri/src/application/library.rs`、`src-tauri/src/infra/fs.rs`、`src-tauri/src/infra/sqlite.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/commands/notes.rs`、`src-tauri/src/commands/index.rs`、`src-tauri/src/commands/search.rs`、`src-tauri/src/lib.rs`
- 测试与 Fixture：`src-tauri/tests/fixtures/vault/**`、`src-tauri/tests/tokenizer_and_fts_test.rs`、`src-tauri/tests/note_meta_and_crud_test.rs`

## 3. 具体实现要求
- **严守 `domain` 纯净红线（`C1.2`）**：`domain/tokenizer.rs`、`domain/frontmatter.rs`、`domain/note.rs` 严禁引用 `tauri`、`std::fs`、`rusqlite` 或 `notify`；所有分词、YAML 切片补丁、正文哈希计算与标题推导必须为纯内存函数。
- **`meta + body` 铁律（`D-0006`、`D-0007`）**：
  - `note_read` 必须剥离 YAML frontmatter，保证 `NoteDoc.body` 不含 `---` 块，`bodyHash` 严格仅对 `body` 计算。
  - `note_write` 校验磁盘当前正文哈希与 `baseBodyHash`：不一致时返回 `ErrorCode::Conflict`（`detail` 序列化 `ConflictInfo` 元数据，遵守 `C2.3` 日志不打印正文）；一致时保留磁盘现有 YAML frontmatter 原样拼接新 `body` 原子写回，仅当库内文件缺失 `id` 时生成 ULID 补入 frontmatter 并在 `WriteResult.updatedMeta` 返回。
  - 扫描索引发现重复 `id`（如 `duplicate-id-a.md` 与 `duplicate-id-b.md`）时，标记 `meta_error` 且绝不静默篡改原文件 `id`，第二个重复项的 `key` 回退使用 `path:<rel_path>` 保证主键唯一。
- **混合分词与 FTS5 精确检索（`C5.3`）**：
  - 英文技术词如 `nginx.conf`、`git-reset`、`HEAD~1` 保持完整词元并拆分子词元，确保搜索 `git` 不会误命中 `digit`；中文按连续 2-gram 切分。
  - `notes_fts` 仅索引 `title_tokens`、`tag_tokens` 与纯 `body_tokens`，杜绝 YAML 字段名（如 `open_dir`、`archived`）污染全文搜索。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/tokenizer_and_fts_test.rs`、`src-tauri/tests/note_meta_and_crud_test.rs`
  - 验证 `tokenize_mixed` 对中英混排、路径/命令连接符、单字与多字中文分词结果；验证 FTS5 搜索 `git` 不命中 `digit`、搜索中文双字与标签层级 `dev` → `dev/quicknote` 准确命中。
  - 使用 `src-tauri/tests/fixtures/vault/` 真实样本验证：`missing-id.md` 经 `note_write` 自动补写 ULID 且返回 `updatedMeta`；`unknown-keys-comments.md` 经 `note_update_meta` 修改 `tags`/`pinned` 后原有未知键、注释与顺序 100% 保留；`broken-yaml.md` 宽容读取正文且标识 `metaError`；`duplicate-id-a/b.md` 正确标记 `metaError` 且不改写文件；`baseBodyHash` 不匹配时 `note_write` 返回 `ErrorCode::Conflict`。
  - 验证 `note_create`、`note_rename`、`note_delete`、`notes_list`、`notes_get_tree_and_tags` 与 `index_rebuild` 全链路数据一致性。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡为 Rust 核心领域、SQLite FTS5 索引与 IPC 契约层，无 macOS 原生窗口改动，以 `pnpm check:all` 全绿为准。
