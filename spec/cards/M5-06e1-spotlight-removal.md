# M5-06e1：清理 Spotlight 死代码——命令与 Runner

- 所属里程碑：M5
- 关联方案与契约：`v5` §3；C2.1、C3.4、C4.2；D-0012
- 前置依赖：M5-06c
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：删除已废止的 Spotlight 主检索遗留：`search_spotlight` 命令、`SpotlightRunner`、`SpotlightArgs`，以及只为它们存在的代码与测试；同步契约表。
- 不做：**保留** `settings.yaml` 的 `search_roots` 字段（它还被纯 Rust 的 `external_search_roots` 使用，且从 Schema 里删除会让老用户的配置解析失败）；**保留** `SearchSource::Spotlight` 枚举变体（由 M5-06e2 与前端过滤一起删）。

## 2. 允许修改文件白名单

- `src-tauri/src/infra/spotlight.rs`（删除）
- `src-tauri/src/infra/mod.rs`
- `src-tauri/src/commands/search.rs`
- `src-tauri/src/application/external.rs`
- `src-tauri/src/domain/note.rs`
- `src-tauri/src/bindings.rs`
- `src-tauri/src/lib.rs`
- `src/lib/ipc/commands.ts`（C1.3 共享文件，仅删除一个函数）

文档同步（与代码同批提交）：`spec/engineering-contract.md`

测试文件（不计入上限）：`src-tauri/tests/m3_spotlight_path_external.rs`、`src/lib/ipc/__tests__/smoke.test.ts`

## 3. 具体实现要求

### 3.1 逐处删除

| 文件 | 操作 |
| --- | --- |
| `infra/spotlight.rs` | 整个文件删除。其中的 `expand_tilde` 返回 `Option<PathBuf>`，与 `infra/fs.rs` 的 `expand_tilde`（返回 `PathBuf`）签名不同，见 3.2 |
| `infra/mod.rs` | 删除 `pub mod spotlight;` |
| `commands/search.rs` | 删除 `search_spotlight` 命令，以及 `use` 里的 `search_spotlight as search_spotlight_uc`、`SpotlightArgs`、`SpotlightRunner` |
| `application/external.rs` | 删除 `search_spotlight` 函数（约 350–390）；`use` 行去掉 `SpotlightArgs`；把 `use crate::infra::spotlight::{expand_tilde, SpotlightRunner, SPOTLIGHT_HARD_LIMIT};` 改为从 `crate::infra::fs` 引入 `expand_tilde` |
| `domain/note.rs` | 删除 `SpotlightArgs` 结构体（约 343–350）及其文档注释。**`SearchSource::Spotlight` 变体保留不动** |
| `bindings.rs` | `use` 区去掉 `SpotlightArgs`；删除 `.register::<SpotlightArgs>()` |
| `lib.rs` | 删除 `use crate::infra::spotlight::SpotlightRunner;`、`let spotlight_runner = Arc::new(SpotlightRunner::new()); app.manage(spotlight_runner);`、`invoke_handler!` 里的 `commands::search::search_spotlight` |
| `commands.ts` | 删除 `searchSpotlight` 函数 |

### 3.2 `expand_tilde` 签名差异

`application/external.rs` 约 710 行有一处 `expand_tilde(raw)` 调用。原 `spotlight.rs` 版本返回 `Option<PathBuf>`（无法展开时为 `None`），`infra/fs.rs` 版本直接返回 `PathBuf`。改用后者后，该调用处的「无法展开」分支（`let Some(path) = ... else { ... }`）不再需要：用 `grep -n -B3 -A8 'expand_tilde(raw)' src-tauri/src/application/external.rs` 看清上下文，做等价最小改写（原 `None` 分支若直接跳过该目录，现在改为判断展开后的路径是否非空）。在提交说明里注明这处行为差异。

### 3.3 测试

- `m3_spotlight_path_external.rs`：删除只测试 Spotlight 的用例（包括一直并发超时的 `spotlight_concurrent_request_cancels_previous_process`）。其余关于外部直扫、书签、路径补全的用例保留；若它们 `use` 了被删类型，做最小适配。删完后把文件重命名为 `m3_external_path_bookmarks.rs`（`cargo test` 按目录自动发现，无需登记）。
- `smoke.test.ts`：去掉对 `searchSpotlight` 的断言。

### 3.4 文档同步

`spec/engineering-contract.md`：

- C4.2：删除 `search_spotlight` 行。
- C9.2 或测试相关说明里若提到 `m3_spotlight_path_external`，改为新文件名。
- C2.1 已规定「外部文件检索严禁依赖 Spotlight」，无需改。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（`cargo test` 不再出现 Spotlight 超时用例）。
- `grep -rn 'SpotlightRunner\|SpotlightArgs\|search_spotlight' src-tauri/src src/lib` 无结果。

## 5. 人工点验清单

- [ ] 快速打开（`⌘K`）输入路径与关键字：外部文件检索正常（纯 Rust 直扫路径不受影响）。
- [ ] 设置「外部与搜索」里的 `search_roots` 配置保存、加载正常。

## 上下文提示

- 只读上表列出的片段；`application/external.rs` 只看 `use` 区、约 77–100、约 350–390、约 700–715。
- 忽略：`external_search_roots` 与 `fs_browser.rs`。
