# M5-01b：门禁基线转绿与版本号对齐

- 所属里程碑：M5
- 关联方案与契约：`v5` §1；C9.1
- 前置依赖：无（可与 M5-01a 并行）
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：让 `pnpm check:all` 在基线上 9 步全绿；把 `tauri.conf.json` 的版本号对齐到 `2.0.0`。
- 不做：不做任何逻辑改动。格式化只允许产生空白 / 换行 / 引号风格差异。

## 2. 允许修改文件白名单

纯格式化（由 `pnpm format` 产生，属明示例外，共 10 个）：

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

版本号：

- `src-tauri/tauri.conf.json`

## 3. 具体实现要求

1. 先运行 `pnpm format:check`，确认输出恰好是上面 10 个文件。若清单不同，停下并报告差异，不要继续。
2. 运行 `pnpm format`。
3. `git diff --stat` 确认只有这 10 个文件变化。逐个用 `git diff -w --stat <file>` 检查：忽略空白后差异应为 0 行。若某个文件存在非空白差异，用 `git checkout -- <file>` 还原该文件并报告。
4. `src-tauri/tauri.conf.json` 的 `"version": "1.0.0"` 改为 `"2.0.0"`（与 `package.json`、`src-tauri/Cargo.toml` 一致）。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 9 步全绿。这是本项目当前基线上第一次能全绿，请把 9 步的最终输出摘要写进提交说明。

## 5. 人工点验清单

- 无。

## 上下文提示

- 只需关注格式化结果与版本号。
- 忽略：这 10 个文件里的任何业务逻辑；不要「顺手修」任何东西。
