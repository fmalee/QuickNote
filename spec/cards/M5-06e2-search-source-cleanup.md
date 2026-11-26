# M5-06e2：清理 Spotlight 残留的搜索来源变体、黑名单函数与前端过滤

- 所属里程碑：M5
- 关联方案与契约：`v5` §3；C3.4、C4.2；D-0012
- 前置依赖：M5-06e1
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：删除 `SearchSource::Spotlight` 枚举变体、只为 Spotlight 结果服务的黑名单常量与函数，以及前端里「过滤掉 spotlight 结果」的分支和相关注释。做完后全仓代码里不再出现 Spotlight。
- 不做：不改搜索排序与评分；不改 `fs_browser.rs` 自己的目录黑名单（`BLACKLISTED_DIR_NAMES`、`BLACKLISTED_DIR_PAIRS`，纯 Rust 直扫在用）。

## 2. 允许修改文件白名单

- `src-tauri/src/domain/note.rs`
- `src-tauri/src/domain/external.rs`
- `src-tauri/src/bindings.rs`（仅在类型注册需要调整时）
- `src/features/quickopen/useQuickOpen.ts`
- `src/features/quickopen/queryParser.ts`（只改一处注释）
- `src/features/quickbar/store.ts`（条件项，见 3.1 第 4 点；不需要就不改、不提交）

文档同步：`spec/engineering-contract.md`

测试文件（不计入上限）：`src/features/quickopen/__tests__/quick-open-engine.test.ts`、`src/features/quickbar/__tests__/quickbar-open-mode.test.ts`

## 3. 具体实现要求

### 3.1 逐处改动

1. `src-tauri/src/domain/note.rs`：`SearchSource` 枚举删掉 `Spotlight` 变体。删之前执行
   `grep -rn 'SearchSource::Spotlight' src-tauri/src src-tauri/tests`，应无结果（唯一的生产者在 M5-06e1 已随 `search_spotlight` 删除）；有结果则停下报告。
2. `src-tauri/src/domain/external.rs`：
   - 删除 `pub const SPOTLIGHT_BLACKLIST` 整个常量及其文档注释。
   - 删除 `pub fn is_spotlight_blacklisted` 及其文档注释。
   - 文件第 1 行模块注释 `//! Pure external action resolution rules and Spotlight path blacklist (...)` 去掉 `and Spotlight path blacklist`。
   - 依据：M5-06e1 删除 `spotlight.rs` 和对应测试后，这两项在全仓零引用（纯 Rust 直扫用的是 `infra/fs_browser.rs` 自己的黑名单）。动手前执行
     `grep -rn 'is_spotlight_blacklisted\|SPOTLIGHT_BLACKLIST' src-tauri/src src-tauri/tests | grep -v 'domain/external.rs'`，应无结果；有结果则停下报告。
3. `src/features/quickopen/useQuickOpen.ts`：
   - `QuickOpenResultSource` 的定义 `Exclude<SearchSource, 'spotlight'> | ...` 改为 `SearchSource | ...`。
   - `mapSearchResultItem` 开头的 `if (item.source === 'spotlight') { return null; }` 删除，函数直接走后面的返回。
4. `src/features/quickbar/store.ts`：M5-06b 之后这个文件里已经没有 spotlight 条件。执行 `grep -n -i 'spotlight' src/features/quickbar/store.ts` 确认；无结果就不碰这个文件。
5. `src/features/quickopen/queryParser.ts`：约第 15 行注释「其余 -> `all`（默认混合检索 + 异步 Spotlight）」改为「其余 -> `all`（默认混合检索）」。只改注释。
6. `pnpm gen:bindings`，确认 `src/bindings/types.ts` 里 `SearchSource` 变为 `"library" | "recent" | "path"`。

### 3.2 测试

- `quick-open-engine.test.ts`：约 161 行的用例标题去掉「filters spotlight items」；mock 数据里 `key: 'spot:legacy'`、`source: 'spotlight'` 的那一项删除；对应的「Spotlight 结果被忽略」断言删除；其余断言（防抖、合并、丢弃过期响应）保留，涉及条数的地方按少一项调整。
- `quickbar-open-mode.test.ts`：约 1007–1014 行 `key: 'sp1'`、`source: 'spotlight'` 的那一项删除（它带 `satisfies SearchResponse`，变体删除后这里是类型错误）；注释「Spotlight result must be excluded」删除；`openGroups` 的断言不变。
- Rust 侧若还有测试引用 `is_spotlight_blacklisted`，说明 M5-06e1 没清理干净，停下报告，不要自行扩大范围。

### 3.3 文档同步

`spec/engineering-contract.md`：

- C3.4：若列举了 `SearchSource` 的取值，去掉 `spotlight`；若列出 `SpotlightArgs`，删除。
- C2.1 里「严禁依赖 Spotlight」是禁令，保留。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- `grep -rni 'spotlight' src src-tauri/src src-tauri/tests | grep -v __tests__` 无结果。
  唯一允许保留的例外：`src/features/settings/__tests__/settings-workspaces-and-external.test.ts` 里断言设置页文案不含 `Spotlight` 的那一句，它在 `__tests__` 下，已被上面的过滤排除。

## 5. 人工点验清单

- [ ] 主窗口快速打开（`⌘K`）与 Quick Bar「打开 ⌘K」结果正常，笔记库文档、书签、路径补全分组都在。
- [ ] 搜索一个位于 `node_modules` 里的文件名，确认纯 Rust 直扫仍不返回该目录下的结果（验证删掉的是 Spotlight 专用黑名单，不是直扫的黑名单）。

## 上下文提示

- 纯删除卡，不改功能。
- 忽略：`src-tauri/src/infra/fs_browser.rs`、`settings.yaml` 的 `search_roots`。
