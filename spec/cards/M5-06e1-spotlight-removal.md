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
- `src/lib/ipc/commands.ts`（C1.3 共享文件，只动两处：删 `searchSpotlight` 函数、删类型导入里的 `SpotlightArgs`）

文档同步（与代码同批提交）：

- `spec/engineering-contract.md`
- `spec/PROJECT_STATE.md`（只删「已知非阻断遗留项」里关于 Spotlight 用例超时的那一句，因为本卡删除了该用例）

测试文件（不计入上限）：

- `src-tauri/tests/m3_spotlight_path_external.rs`（删用例后重命名）
- `src/lib/ipc/__tests__/smoke.test.ts`
- `src/features/quickbar/__tests__/quickbar-command-mode.test.ts`
- `src/features/quickbar/__tests__/quickbar-open-mode.test.ts`

全仓引用已核对（基线 `dev` 上实测）：`SpotlightArgs`、`SpotlightRunner`、`search_spotlight`、`SPOTLIGHT_HARD_LIMIT` 只出现在上列文件和这些测试里。`capabilities/`、`Cargo.toml`、`tauri.conf.json`、`build.rs` 均无 Spotlight 相关内容。

## 3. 具体实现要求

### 3.1 逐处删除

| 文件 | 操作 |
| --- | --- |
| `infra/spotlight.rs` | 整个文件删除。其中的 `expand_tilde` 返回 `Option<PathBuf>`，与 `infra/fs.rs` 的 `expand_tilde`（返回 `PathBuf`）签名不同，见 3.2 |
| `infra/mod.rs` | 删除 `pub mod spotlight;` |
| `commands/search.rs` | 删除 `search_spotlight` 命令，以及 `use` 里的 `search_spotlight as search_spotlight_uc`、`SpotlightArgs`、`SpotlightRunner` |
| `application/external.rs` | 删除 `search_spotlight` 函数（约 350–390）；`use` 行去掉 `SpotlightArgs`；把 `use crate::infra::spotlight::{expand_tilde, SpotlightRunner, SPOTLIGHT_HARD_LIMIT};` 改为 `use crate::infra::fs::expand_tilde;`（先确认文件里没有别的地方已经从 `infra::fs` 引入 `expand_tilde`，有就合并）。`external_search_roots` 上方注释里的「(replaces Spotlight as the primary external directory search path in v5)」删掉括号这一段 |
| `domain/note.rs` | 删除 `SpotlightArgs` 结构体（约 343–350）及其文档注释。**`SearchSource::Spotlight` 变体保留不动** |
| `bindings.rs` | `use` 区去掉 `SpotlightArgs`；删除 `.register::<SpotlightArgs>()` |
| `lib.rs` | 删除 `use crate::infra::spotlight::SpotlightRunner;`、`let spotlight_runner = Arc::new(SpotlightRunner::new()); app.manage(spotlight_runner);`、`invoke_handler!` 里的 `commands::search::search_spotlight` |
| `commands.ts` | 删除 `searchSpotlight` 函数；顶部类型导入列表里删掉 `SpotlightArgs,`（留着会成为未使用的导入，lint 会报） |

本卡**不动** `src-tauri/src/domain/external.rs` 里的 `SPOTLIGHT_BLACKLIST` 与 `is_spotlight_blacklisted`：本卡删掉 `spotlight.rs` 后它们就没有任何非测试引用了，由 M5-06e2 删除。不要提前删。

### 3.2 `expand_tilde` 签名差异

`application/external.rs` 约 777 行的 `expand_optional_path` 用了 `spotlight.rs` 的 `expand_tilde`（返回 `Option<PathBuf>`）：

```rust
fn expand_optional_path(raw: &str) -> String {
    expand_tilde(raw)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| raw.to_string())
}
```

改用 `crate::infra::fs::expand_tilde`（返回 `PathBuf`）后，函数体改成一行：

```rust
fn expand_optional_path(raw: &str) -> String {
    expand_tilde(raw).to_string_lossy().to_string()
}
```

改之前读 `src-tauri/src/infra/fs.rs` 的 `expand_tilde`（约 125–138 行）确认：不是以 `~` 开头的输入原样返回。两个版本已对比如下，等价时才照上面改：两者都会 `trim`；`~` 和 `~/` 在 `HOME` 未设置时都原样返回；空串都得到空串。唯一的差别是旧版多包了一层 `Option`。若你读到的实现与此不符，停下并报告。在提交说明里写明这处改写。

### 3.3 测试

- `m3_spotlight_path_external.rs`：
  - 整个删除两个用例：`spotlight_blacklist_and_hard_limit_50`（连同它的 `#[test]`，它同时断言黑名单函数和 Spotlight 硬上限）和带 `#[ignore]` 的 `spotlight_concurrent_request_cancels_previous_process`（连同 `#[ignore]` 那行）。
  - 顶部 `use` 里删掉因此不再使用的名字：`search_spotlight`、`SpotlightArgs`、`is_spotlight_blacklisted`。`SearchSource` 仍被保留的用例用到（断言 `SearchSource::Recent`），不要删。
  - 其余五个用例（`path_complete_*`、`external_actions_*`、`three_tier_*`、`external_fs_browser_*`）原样保留。
  - 删完后 `git mv` 重命名为 `m3_external_path_bookmarks.rs`（`cargo test` 按目录自动发现，无需登记）。
- `smoke.test.ts`：删掉 `searchSpotlight` 的导入、断言调用，以及 `it(...)` 标题里的 `searchSpotlight`；同一个 `it` 里其余三个调用保留。
- `quickbar-command-mode.test.ts`、`quickbar-open-mode.test.ts`：删掉 `mockSearchSpotlight` 的声明与导出、mock 对象里的 `searchSpotlight: mockSearchSpotlight`，以及 `expect(mockSearchSpotlight).not.toHaveBeenCalled()` 这一条断言（约第 878 行）；该用例本身和其余断言保留。这两处是 06e1 删函数后的残留 mock，不删虽然不影响运行，但留着是死代码。
- `src/features/settings/__tests__/settings-workspaces-and-external.test.ts` 里有一句 `not.toContain('Spotlight')`，是设置页文案断言，与本卡无关，不要动。

### 3.4 文档同步

`spec/engineering-contract.md`：

- C4.2：删除 `search_spotlight` 那一行（命令表里「搜索与命令库」下、状态为 deprecated 的那一行）。
- C2.1 里「外部文件检索严禁依赖 Spotlight（`mdfind`）」是禁令，保留不改。
- C9.2 或测试相关说明里若提到 `m3_spotlight_path_external`，改为新文件名。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（`cargo test` 不再出现 Spotlight 超时用例）。
- `grep -rn 'SpotlightRunner\|SpotlightArgs\|search_spotlight\|SPOTLIGHT_HARD_LIMIT' src-tauri/src src-tauri/tests src/lib` 无结果。
- `grep -rn 'searchSpotlight\|mockSearchSpotlight' src | grep -v 'settings-workspaces-and-external'` 无结果。
- `pnpm gen:bindings` 后 `src/bindings/types.ts` 里不再有 `SpotlightArgs`。

## 5. 人工点验清单

- [ ] 快速打开（`⌘K`）输入路径与关键字：外部文件检索正常（纯 Rust 直扫路径不受影响）。
- [ ] 设置「外部与搜索」里的 `search_roots` 配置保存、加载正常。

## 上下文提示

- 只读上表列出的片段；`application/external.rs` 只看 `use` 区、约 74–82、约 350–390、约 768–782。
- 忽略：`external_search_roots` 与 `fs_browser.rs`。
