# M3-01a：后端 M3 契约补全（上）——命令书解析、`note_links` 索引、关联打分、标签批量重命名与非 Markdown 读写

- 所属里程碑：M3
- 关联方案与契约：`v4` §4.8、§5.3、§5.5.1、§5.6.6、§6 M3；`C1.2`、`C1.3`、`C1.4`、`C2.1`、`C3.1`–`C3.4`、`C4.2`、`C5.2`、`C6.4`、`C8.6`、`C9.1`、`C9.2`、`D-0006`、`D-0009`、`D-0010`
- 前置依赖：`m2-baseline`（`dev`）
- 建议分支：`feat/M3-01a-backend-commands-related-tags`
- 会话策略：独立新会话执行（零大文档读取，所有 Rust 结构体、SQL 列名、解析算法与 IPC 签名已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **命令书解析领域引擎（`src-tauri/src/domain/command_book.rs`，纯函数零 IO）**：
   - 定义并导出 IPC 结构体（`C3.4`、`C4.2`，均派生 `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]` 与 `#[serde(rename_all = "camelCase")]`）：
     ```rust
     pub struct CommandEntry {
         pub id: String,
         pub note_rel_path: String,
         pub section: String,
         pub title: String,
         pub description: String,
         pub code: String,
         pub language: Option<String>,
         pub keywords: Vec<String>,
         pub is_danger: bool,
         pub line: u32,
     }

     pub struct CommandSearchArgs {
         pub query: String,
     }
     ```
   - 实现 `pub fn parse_command_book(note_rel_path: &str, body: &str) -> Vec<CommandEntry>`（严格按 `v4` §4.8）：
     - 仅在 `doc_type == NoteType::CommandBook` 或路径位于 `commands/` 下（`rel_path.starts_with("commands/")`）时触发提取；入参 `body` 为**完整 Markdown 正文**，注意 `line` 必须为**1-based 源文件行号**（包含 frontmatter 所占行数偏移 `body_start_line: u32`，签名可为 `pub fn parse_command_book(note_rel_path: &str, body: &str, body_start_line: u32) -> Vec<CommandEntry>`，使得前端 `⌘O` 跳转源文件时可直接滚到该 H3/代码块真实行号）。
     - `## <section>`（围栏代码块外部的二级标题）更新当前 `section`（默认为 `"默认"` 或文档一级标题）。
     - `### <title>`（围栏代码块外部的三级标题）开启新命令条目并记录 `heading_line`；`###` 与首个围栏代码块之间的非空段落文本拼接为 `description`（过滤掉 HTML 注释行）。
     - 围栏代码块（`` ``` `` 或 `~~~`，含可选 info string 如 `` ```bash ``）提取 `code`（去除首尾围栏行）与 `language`（info string 首词，空则 `None`）。
     - 紧跟代码块下方（或同一 H3 块内代码后、下个标题/代码块前）的可选 HTML 注释：
       - `<!-- keywords: ... -->`（不区分大小写，支持空格/逗号/分号分隔）解析为 `keywords: Vec<String>`（去重去空）。
       - `<!-- danger -->`（或 `<!-- danger: ... -->`，不区分大小写）将 `is_danger = true`。
     - **无 H3 兼容模式**：若某代码块前没有未消费的 `###` 标题，取该代码块上方最近一段非空自然段（在当前 H2 内且非注释）作为 `title`，`description` 为空，`line` 取该自然段行号（无自然段则取代码块首行号）。
     - 确定性 `id` 生成：`format!("{note_rel_path}:{line}:{idx}")`（保证同文件多条目唯一且重建稳定）。
   - 实现纯检索打分函数 `pub fn filter_and_rank_commands(entries: &[CommandEntry], query: &str) -> Vec<CommandEntry>`：
     - 空查询返回全部条目（保持 `note_rel_path ASC, line ASC` 顺序）。
     - 非空查询按空白拆分为多词元（全部转小写，AND 语义匹配），综合匹配 `title`（最高权 +40）、`keywords`（+35）、`section`（+25）、`description`（+15）、`code`（+10）与 `language`（+5），过滤 0 分项并按总分降序、`line` 升序返回。

2. **相对链接提取与 `C8.6` 关联文档打分（`domain::markdown` + `domain::note`）**：
   - 在 `src-tauri/src/domain/markdown.rs` 实现纯函数 `pub fn extract_relative_note_links(source_rel_path: &str, body: &str) -> Vec<String>`：
     - 忽略围栏代码块（`` ``` `` / `~~~`）与行内反引号代码；
     - 提取标准 Markdown 链接 `[text](target)`（含可选 `<target>`、`target "title"`）以及引用定义 `[ref]: target`；
     - 跳过 `http://`、`https://`、`mailto:`、`file://`、纯 `#anchor`、以 `/` 开头的绝对路径；
     - 剥离 `#anchor` 与 `?query`，仅保留目标扩展名为 `.md` / `.markdown` 的相对路径；基于 `source_rel_path` 所在目录执行 POSIX 路径规范化（解析 `./` 与 `../`，越出库根丢弃、排除指向自身的链接），去重排序返回。
   - 在 `src-tauri/src/domain/note.rs` 新增关联与标签重命名 IPC 结构体（`C3.4`、`C4.2`）：
     ```rust
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "snake_case")]
     pub enum RelatedReason {
         Backlink,
         SharedTags,
         SameProject,
     }

     #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct RelatedNoteItem {
         pub key: String,
         pub rel_path: String,
         pub title: String,
         pub reason: RelatedReason,
         #[specta(type = specta_typescript::Number)]
         pub score: f64,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TagRenameArgs {
         pub old_tag: String,
         pub new_tag: String,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TagRenameResult {
         pub updated_files: u32,
     }
     ```
     > 注意：`RelatedReason` 的 `C3.4` 精确字符串值为 `"backlink" | "shared_tags" | "same_project"`，必须用 `#[serde(rename_all = "snake_case")]`！
   - 在 `domain::note` 实现纯打分函数 `pub fn score_related_notes(...) -> Vec<RelatedNoteItem>`（严格执行 `C8.6`）：
     - 遍历库内除当前文档 `C` 与 `_templates/` 外的所有文档 `D`：
       - 若 `D` 在 `note_links` 中存在指向 `C.rel_path` 的反向相对链接：`+100.0`；
       - 共有标签数 `shared = |D.tags ∩ C.tags|`：`+15.0 * shared as f64`；
       - 若 `C.project` 非空且 `D.project == C.project`：`+10.0`；
     - `reason` 取最高权重命中项（有反向链接取 `Backlink`，否则有共有标签取 `SharedTags`，否则取 `SameProject`）；
     - 过滤 `score <= 0.0` 者，按 `score` 降序、`mtime_ms` 降序、`rel_path` 升序取前 **20** 条。

3. **SQLite 索引同步扩展（`infra/sqlite.rs` + `application/index.rs`）**：
   - `0001_init.sql` 已建好 `note_links(source_rel_path, target_rel_path)` 与 `commands(id, note_rel_path, section, title, description, code, language, keywords, is_danger, line)` 表（**严禁修改 `0001_init.sql` 历史迁移**）。
   - 扩展 `NoteIndexRecord` 增加 `pub links: Vec<String>` 与 `pub commands: Vec<CommandEntry>`：
     - `replace_all_notes_index`：清空并重建 `note_links` 与 `commands`；
     - `upsert_note_index`：先按 `rec.rel_path` 删除旧 `note_links`（`WHERE source_rel_path = ?1`）与旧 `commands`（`WHERE note_rel_path = ?1`），再批量写入新记录；
     - `delete_note_index_by_rel_path`：级联删除对应 `note_links`（`WHERE source_rel_path = ?1`）与 `commands`（`WHERE note_rel_path = ?1`）。
   - 在 `infra/sqlite.rs` 新增查询函数：
     - `pub fn list_all_commands(conn: &Connection) -> Result<Vec<CommandEntry>, AppError>`（`keywords` 列存 JSON 数组字符串，反序列化兼容旧空格分隔格式）；
     - `pub fn list_backlink_sources(conn: &Connection, target_rel_path: &str) -> Result<HashSet<String>, AppError>`（查 `SELECT source_rel_path FROM note_links WHERE target_rel_path = ?1`）。
   - 在 `application/index.rs::build_note_index_record` 中计算 `body_start_line`（`raw` 中 `body` 的 1-based 起始行号），调用 `extract_relative_note_links` 与（若 `doc_type == CommandBook` 或 `rel_path.starts_with("commands/")`）`parse_command_book` 填充记录。

4. **新增 3 条 IPC 命令（`C4.2`）**：
   - `commands_search(args: CommandSearchArgs) -> Vec<CommandEntry>`（注册于 `commands/search.rs`）
   - `notes_get_related(args: RelPathArgs) -> Vec<RelatedNoteItem>`（注册于 `commands/notes.rs`）
   - `tags_batch_rename(args: TagRenameArgs) -> TagRenameResult`（注册于 `commands/notes.rs`）：
     - 校验 `old_tag` / `new_tag` 规范化后（trim、去前导 `#`、去首尾 `/`）均非空，且不相等；非法返回 `ErrorCode::ValidationError`。
     - 遍历库内所有带 `old_tag` 或以 `format!("{old_tag}/")` 为前缀子标签的笔记（排除 `_templates/` 与 `meta_error` 不可 patch 的损坏 YAML 文件，不可 patch 的记录跳过不破坏原文件）：
       - 精确匹配 `old_tag` 替换为 `new_tag`；子标签 `old_tag/sub` 替换为 `new_tag/sub`；
       - 按原有顺序去重（当目标 `new_tag` 已存在于该笔记时自然完成**标签合并**）；
       - 复用 `apply_yaml_patch` 最小补丁仅更新 `tags`（完整保留注释、未知键与正文，遵守 `D-0006` / `C6.4`），`atomic_write_str` 落盘，调用 `index_single_note` 与 `pm.record_self_write` 防回环；
       - 返回实际更新文件数 `TagRenameResult { updated_files }`。

5. **非 Markdown 外部文件读写安全增强（`application/notes.rs`）**：
   - 抽取辅助判断 `fn is_markdown_path(path: &Path) -> bool`（扩展名忽略大小写为 `md` 或 `markdown`）。
   - `note_read`：对非 Markdown 文件跳过 `parse_note` frontmatter 剥离，原样将整文件作为 `body`，`meta` 使用文件名作为 `title`、`doc_type = NoteType::Note`、`meta_error = None`、`tags = vec![]`；同时支持对 `~/` 前缀路径展开。
   - `note_write`：对非 Markdown 文件跳过 frontmatter 剥离、不尝试补写 ULID，校验 `compute_body_hash(&raw) == args.base_body_hash` 后直接原样 `atomic_write_str(&abs_path, &args.body)` 并调用 `record_self_write`。

6. **绑定导出与前端 IPC 封装（`C1.3`、`C1.4`、`C3.2`）**：
   - 在 `src-tauri/src/bindings.rs` 注册 `CommandEntry`、`CommandSearchArgs`、`RelatedReason`、`RelatedNoteItem`、`TagRenameArgs`、`TagRenameResult`。
   - 在 `src-tauri/src/lib.rs` 的 `invoke_handler!` 注册 `commands::search::commands_search`、`commands::notes::notes_get_related`、`commands::notes::tags_batch_rename`。
   - 运行 `pnpm gen:bindings` 生成 `src/bindings/types.ts`（零手改）。
   - 在 `src/lib/ipc/commands.ts` 新增 `commandsSearch`、`notesGetRelated`、`tagsBatchRename`。

### 明确不做什么
- 本卡**不做** `search_spotlight`、`path_complete`、`external_actions_resolve`、`external_action_run`（归 `M3-01b`）。
- 本卡**不做**任何前端 Vue 界面、store 或样式改动（归 `M3-02`–`M3-05b`）。
- 严禁修改 `src-tauri/migrations/0001_init.sql`。

## 2. 允许修改文件白名单

- 共享串行文件（`C1.3`）：
  - `src-tauri/src/bindings.rs`
  - `src/bindings/types.ts`（仅由 `pnpm gen:bindings` 自动生成）
  - `src/lib/ipc/commands.ts`
- Rust 源码：
  - `src-tauri/src/domain/command_book.rs`（新增）
  - `src-tauri/src/domain/markdown.rs`
  - `src-tauri/src/domain/note.rs`
  - `src-tauri/src/domain/mod.rs`
  - `src-tauri/src/infra/sqlite.rs`
  - `src-tauri/src/application/index.rs`
  - `src-tauri/src/application/notes.rs`
  - `src-tauri/src/commands/search.rs`
  - `src-tauri/src/commands/notes.rs`
  - `src-tauri/src/lib.rs`
- 测试文件：
  - `src-tauri/tests/m3_commands_related_tags.rs`（新增）
  - `src/lib/ipc/__tests__/smoke.test.ts`（如需补充新封装导出断言）

## 3. 具体实现要求

1. **严格单向分层与零 `unwrap()`（`C1.2`、`C2.1`）**：
   - `domain/command_book.rs`、`domain/markdown.rs`、`domain/note.rs` 严禁引入 `std::fs`、`rusqlite`、`tauri`。
   - 生产代码（`src-tauri/src/**`）零 `.unwrap()` / `.expect()`。
   - 全部 Rust 测试放入 `src-tauri/tests/m3_commands_related_tags.rs`，不要在 `src-tauri/src/**` 内写 `#[cfg(test)]`。
2. **Fixture 真实解析要求（`C9.2`）**：
   - 必须直接加载 `src-tauri/tests/fixtures/vault/commands/常用命令.md` 测试命令书解析与检索，验证 H2 `Git` / `Nginx`、H3 `撤销最近一次提交，但保留改动` + 描述 + `bash` 代码块 + `keywords: ["撤销", "commit", "回退", "reset"]`、H3 `强制清理未跟踪文件与目录` + `is_danger = true`、以及无 H3 兼容段 `重载 nginx.conf 配置而不中断服务`（`nginx -s reload`）全部准确解析，且 `line` 指向精确 1-based 源文件行号。
3. **标签批量重命名与合并细节**：
   - 在 `domain::note` 提取纯函数 `pub fn rewrite_tags_for_rename(tags: &[String], old_tag: &str, new_tag: &str) -> Option<Vec<String>>`（仅当至少一个标签发生变化时返回 `Some(new_tags)`，否则返回 `None` 避免无谓写盘）。
   - 子标签前缀替换示例：`old_tag = "dev"`, `new_tag = "work"` 时，`["dev", "dev/git", "devops", "work"]` → `["work", "work/git", "devops"]`（注意 `devops` 不受影响，且 `work` 自动去重合并）。
   - `tags_batch_rename` 若某篇笔记没有 frontmatter 或 YAML 损坏，不得覆盖破坏原文件（直接跳过）。
4. **隐私日志红线（`C2.3`）**：
   - 严禁在 `eprintln!` 或错误 detail 中打印命令代码内容或笔记正文。

## 4. 自动化验收标准（必须全绿）

1. `src-tauri/tests/m3_commands_related_tags.rs` 至少覆盖以下集成测试场景：
   - 加载 `fixtures/vault/commands/常用命令.md` 验证完整命令书解析（H2/H3/描述/代码块/language/keywords/danger/无 H3 兼容/1-based 行号）；
   - `index_rebuild` + `commands_search`（空查询、中文关键词、同义词 `回退`、命令体 `nginx`、排序与单文件增量更新/删除同步）；
   - `extract_relative_note_links`（跳过代码块与外链/锚点、解析 `./` 与 `../`、去重）；
   - `notes_get_related` 严格验证 `C8.6` 打分权重（反向链接 `+100` > 共有标签 `+15*n` > 同项目 `+10`）、过滤 0 分与自身、同分按 `mtime_ms` 降序、硬上限 20 条；
   - `tags_batch_rename`（普通重命名、层次子标签前缀 `dev/git -> work/git`、不误伤 `devops`、目标标签已存在时自动合并去重、保留原文件 YAML 注释与未知字段、索引同步刷新、`record_self_write` 防回环）；
   - 非 Markdown 外部文件（如以 `---` 开头的 `.yaml` 与 `.conf`）经 `note_read` / `note_write` 往返读写后字节级完整保留，不剥离 `---` 也不补写 ULID。
2. 运行 `pnpm check:all`（全部 9 项门禁：`gen:bindings` 零 diff、`check:arch`、`format:check`、`lint`、`typecheck`、`test:unit --run`、`cargo fmt --check`、`cargo clippy -D warnings`、`cargo test --all-features`）必须 100% 通过。

## 5. 人工点验清单

- 本卡为纯后端领域、索引与 IPC 契约层，无 macOS 原生窗口或 UI 改动，无需人工桌面点验。
