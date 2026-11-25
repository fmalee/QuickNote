# M5-06e2：清理 Spotlight 残留的搜索来源变体与前端过滤

- 所属里程碑：M5
- 关联方案与契约：`v5` §3；C3.4、C4.2；D-0012
- 前置依赖：M5-06e1
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：删除 `SearchSource::Spotlight` 枚举变体，以及前端里「过滤掉 spotlight 结果」的分支。
- 不做：不改搜索排序与评分。

## 2. 允许修改文件白名单

- `src-tauri/src/domain/note.rs`
- `src-tauri/src/bindings.rs`（仅在类型注册需要调整时）
- `src/features/quickopen/useQuickOpen.ts`
- `src/features/quickbar/store.ts`

文档同步：`spec/engineering-contract.md`（C3.4 若列举了 `SearchSource` 取值，去掉 `spotlight`）

测试文件（不计入上限）：`src/features/quickopen/__tests__/quick-open-engine.test.ts`、`src/features/quickbar/__tests__/` 下相关测试

## 3. 具体实现要求

1. 先执行 `grep -rn 'spotlight\|Spotlight' src src-tauri/src --include=*.ts --include=*.vue --include=*.rs | grep -v __tests__`，把结果逐条分类：
   - `domain/note.rs` 的 `SearchSource::Spotlight` 变体：删除。
   - `useQuickOpen.ts`：`Exclude<SearchSource, 'spotlight'>` 改回 `SearchSource`；`if (item.source === 'spotlight') { ... }` 的跳过分支删除。
   - `quickbar/store.ts`：`item.source === 'spotlight'` 相关条件去掉该子句。
   - `domain/external.rs` 里与「Spotlight 黑名单」相关的纯函数与注释（如路径段黑名单过滤）：若仍被纯 Rust 直扫复用则保留，只把注释里的 `v4` 字样改成 v5 对应说法；若零引用则**不在本卡白名单，不要删**，写进提交说明的遗留项。
2. `pnpm gen:bindings`，确认 `SearchSource` 变为 `"library" | "recent" | "path"`。
3. 测试里凡是构造 `source: 'spotlight'` 数据的用例，改用其他来源或删除。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- 第 3.1 步的 `grep` 在 `src/` 与 `src-tauri/src/domain/note.rs` 中无结果。

## 5. 人工点验清单

- [ ] 主窗口快速打开与 Quick Bar「打开」结果正常，无缺失分组。

## 上下文提示

- 纯删除卡，不改功能。
