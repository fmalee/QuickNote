# M5-01b：门禁基线转绿与版本号对齐

- 所属里程碑：M5
- 关联方案与契约：`v5` §1；C9.1
- 前置依赖：无（可与 M5-01a 并行）
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：让 `pnpm check:all` 在基线上 9 步全绿；把 `tauri.conf.json` 的版本号对齐到 `2.0.0`。
- 不做：不做任何逻辑改动。格式化只允许产生空白 / 换行 / 引号风格差异。

## 2. 允许修改文件白名单

纯格式化，由 `pnpm format` 产生（明示例外，共 10 个）：

- `scripts/check-architecture.mjs`
- `src/features/editor/DocHeaderBar.vue`
- `src/features/editor/SiblingFilesPopover.vue`
- `src/features/quickbar/QuickBarView.vue`
- `src/features/quickbar/store.ts`
- `src/features/quickopen/__tests__/quick-open-engine.test.ts`
- `src/features/quickopen/QuickOpenPalette.vue`
- `src/features/quickopen/useQuickOpen.ts`
- `src/features/settings/SettingsExternalPanel.vue`
- `src/features/settings/SettingsWorkspacesPanel.vue`

纯格式化，由 `cargo fmt` 产生（明示例外，共 10 个；门禁第 7 步 `cargo fmt --check` 在基线上就不过）：

- `src-tauri/src/application/external.rs`
- `src-tauri/src/application/index.rs`
- `src-tauri/src/application/reconciler.rs`
- `src-tauri/src/application/terminal.rs`
- `src-tauri/src/domain/bookmark.rs`
- `src-tauri/src/domain/workspace_context.rs`
- `src-tauri/src/infra/fs_browser.rs`
- `src-tauri/src/infra/sqlite.rs`
- `src-tauri/tests/m3_spotlight_path_external.rs`
- `src-tauri/tests/note_crud_and_index_test.rs`

版本号：

- `src-tauri/tauri.conf.json`

跳过一个已知失败的用例（明示例外，唯一的非格式化改动，见第 3 节第 6 步）：

- `src-tauri/tests/m3_spotlight_path_external.rs`（上面已列，这里额外允许加一行 `#[ignore]`）
- `spec/PROJECT_STATE.md`（只在第 40 行末尾补一句登记）

合计 21 个代码 / 配置文件加 1 个文档。除 `#[ignore]` 那一行和版本号外，全部必须是纯格式化改动，不得含任何逻辑改动。

## 3. 具体实现要求

1. 先运行 `pnpm format:check`，确认输出恰好是上面 10 个文件。若清单不同，停下并报告差异，不要继续。
2. 运行 `pnpm format`。
3. 前端格式化结果自查。`git diff --stat` 确认只有这 10 个文件变化，然后逐个看 `git diff -w <file>`。
   注意：`-w` 不忽略换行，prettier 会重排换行，所以差异不是 0 行是正常的。判断标准改为：
   - 允许出现的差异只有这几类：换行位置变化、引号风格（单引号变双引号）、补尾逗号、
     多行联合类型去掉行首多余的 `|`、补或去一层括号、行尾分号。
   - 出现以上任何一类之外的差异（标识符、字面量、运算符、调用、语句顺序有变化），
     用 `git checkout -- <file>` 还原该文件并报告，不要手工修。
   - 「没有逻辑改动」最终以门禁里的类型检查、lint、测试三步全绿为准。
4. Rust 格式化。在项目根目录运行：

   ```bash
   cargo fmt --manifest-path src-tauri/Cargo.toml --all
   ```

   然后 `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` 应无输出。
   `git diff --stat` 里新增的变化文件必须恰好是上面列的 10 个 Rust 文件；多出来的文件先还原并报告。
   `rustfmt` 只调整排版，不需要逐行比对，抽查两三个 diff 确认只是换行位置变化即可。
5. `src-tauri/tauri.conf.json` 的 `"version": "1.0.0"` 改为 `"2.0.0"`（与 `package.json`、`src-tauri/Cargo.toml` 一致）。
6. 跳过已知失败的 Spotlight 兼容用例。背景：`spec/PROJECT_STATE.md` 第 40 行已登记
   `m3_spotlight_path_external::spotlight_concurrent_request_cancels_previous_process` 在基线上就 10 秒超时。
   Spotlight 主检索已于 v5 废止，整段代码和这个用例会在卡片 `M5-06e1` 里删除，所以这里不修取消逻辑，只跳过。
   - 在该用例的 `#[test]` 下一行加：

     ```rust
     #[ignore = "已知遗留：并发取消 10s 超时；Spotlight 已于 v5 废止，随 M5-06e1 删除 Spotlight 时一并移除"]
     ```

     只加这一行，用例体一个字符都不要改。
   - `spec/PROJECT_STATE.md` 第 40 行那句末尾补：「`M5-01b` 起已标 `#[ignore]`，待 `M5-06e1` 删除。」
7. 最后 `git diff --stat`：代码 / 配置 21 个文件，加 `spec/PROJECT_STATE.md`。
   本卡片文件 `spec/cards/M5-01b-gate-baseline.md` 的改动是用户对卡片的勘误，一并纳入本次提交。
   仓库根目录下的 `.workbuddy/` 目录是本地工具状态，不要纳入提交，也不要删除它。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 9 步全绿。这是本项目当前基线上第一次能全绿，请把 9 步的最终输出摘要写进提交说明。
- 第 8 步 Clippy 和第 9 步 Rust 测试如果在基线上还有别的失败：停下并把失败的用例名或告警原文报告给用户，不要自行修复或跳过。

## 5. 人工点验清单

- 无。

## 上下文提示

- 只需关注格式化结果与版本号。
- 忽略：这些文件里的任何业务逻辑；不要「顺手修」任何东西。Rust 那 10 个文件里能看到
  旧命名和待清理代码，都不要动，后面的卡片会处理。
