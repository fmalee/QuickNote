# M1-01a：共享静态 Fixture 库、`domain` 纯算法（混合分词 / YAML 最小补丁 / `meta + body`）与绑定导出

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.3、§4.3–§4.4、§4.9、§5.3、§6 M1；`C1.2`、`C1.3`、`C1.4`、`C2.1`、`C3.1`–`C3.4`、`C5.3`、`C6.4`、`C9.1`、`C9.2`、`D-0006`、`D-0007`、`D-0009`、`D-0010`
- 前置依赖：`M0-04`
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - 按 `C9.2` 在 `src-tauri/tests/fixtures/vault/` 补齐 M1 共享静态测试样本（`inbox.md` 超 500 行膨胀样本、`missing-id.md`、`duplicate-id-a.md` & `duplicate-id-b.md`、`broken-yaml.md`、`unknown-keys-comments.md`、`code-fence-heading.md`、`cn-headings-anchors.md`、`commands/常用命令.md`、`projects/` 及大体积边界测试种子）。
  - 在 `src-tauri/src/domain/tokenizer.rs` 实现 `C5.3` CJK + ASCII 混合分词纯函数（`tokenize_mixed`、`tokenize_mixed_vec`、`build_fts_query`、`tokenize_tags`）。
  - 在 `src-tauri/src/domain/frontmatter.rs` 实现 `D-0006` / `C6.4` 的 `split_frontmatter`、`parse_yaml_mapping` 与 YAML 最小补丁器（`apply_yaml_patch` / `patch_frontmatter_block`）。
  - 在 `src-tauri/src/domain/note.rs` 实现 `D-0006` / `D-0007` 的 `NoteType`、`NoteMeta`、`NoteDoc`、`NoteSummary`、`ReadNoteArgs`、`CreateNoteArgs`、`WriteNoteArgs`、`WriteResult`、`ConflictInfo`、`NoteMetaPatch`、`NoteUpdateMetaArgs`、`RenameNoteArgs`、`RelPathArgs`、`NotesFilterArgs`、`DirNode`、`TagNode`、`WorkspaceSummary`、`NavTreeSnapshot`、`IndexPhase`、`IndexStatus`、`SearchItemKind`、`SearchSource`、`SearchResultItem`、`SearchQueryArgs`、`SearchResponse`，以及纯函数 `parse_note`、`compose_note`、`meta_patch_ops`、`format_ulid`、`compute_body_hash`、`make_excerpt`、`count_list_items`、`tag_matches`。
  - 在 `src-tauri/src/bindings.rs` 注册上述全部新增 `specta::Type` 结构体与枚举，运行 `pnpm gen:bindings` 导出至 `src/bindings/types.ts`。
  - 编写 `src-tauri/tests/domain_note_and_tokenizer_test.rs`，基于 `src-tauri/tests/fixtures/vault/` 真实样本验证分词、YAML 最小补丁与 `meta + body` 纯函数。
- 不做：
  - 不编写 `infra/sqlite.rs`、`application/notes.rs`、`application/index.rs` 或 Tauri `commands`（全部归 `M1-01b`）。
  - 不修改 `src/lib/ipc/commands.ts` 或 `src/lib/ipc/events.ts`（归 `M1-01b`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（自动生成）
- Rust 领域层：`src-tauri/src/domain/mod.rs`、`src-tauri/src/domain/note.rs`、`src-tauri/src/domain/frontmatter.rs`、`src-tauri/src/domain/tokenizer.rs`
- 测试与 Fixture：`src-tauri/tests/fixtures/vault/**`、`src-tauri/tests/domain_note_and_tokenizer_test.rs`

## 3. 具体实现要求
- **`domain` 纯净红线（`C1.2`）**：`domain/{tokenizer,frontmatter,note}.rs` 零 `tauri`、`std::fs`、`rusqlite` 依赖，生产代码零 `unwrap()` / `expect()`（`C2.1`）。
- **绑定零漂移（`C3.2`、`D-0009`）**：`src-tauri/src/bindings.rs` 的 `collect_types()` 必须注册 `domain::note` 中全部带 `#[derive(Type)]` 的 IPC 类型，运行 `pnpm gen:bindings` 后 `git diff --exit-code src/bindings` 为 `0`。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/domain_note_and_tokenizer_test.rs`
  - 验证 `tokenize_mixed` 对中英混排、路径/命令连接符（`nginx.conf` → `nginx.conf nginx conf`）、单字与多字中文 2-gram 分词结果，确认不对 ASCII 做双字切分。
  - 读取 `src-tauri/tests/fixtures/vault/` 下的 `inbox.md`（>500 行）、`missing-id.md`、`broken-yaml.md`、`unknown-keys-comments.md`，验证 `parse_note`、`compose_note`、`compute_body_hash`、`format_ulid`、`meta_patch_ops` 与 YAML 最小补丁原样保留注释、未知键与字段顺序。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡为纯 `domain` 算法、Fixture 样本与 TS 类型绑定生成，无原生窗口交互，以 `pnpm check:all` 全绿为准。
