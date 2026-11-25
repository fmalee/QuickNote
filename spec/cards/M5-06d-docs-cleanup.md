# M5-06d：文档记录类收尾

- 所属里程碑：M5
- 关联方案与契约：`v5` 全文（104 行，可整篇读）；C9.1、C11.2；R42
- 前置依赖：M5-02a 至 M5-06e2 全部合入
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：让四份记录类文档与 M5 完成后的代码一致：`spec/CODEBASE_MAP.md`、`spec/PROJECT_STATE.md`、`README.md`、`spec/quicknote-design-v5.md`。
- 不做：不改任何源码；不改 `AGENTS.md`、`DECISIONS.md`、`spec/engineering-contract.md`（它们已在各自的变更卡里同步过）；**禁读禁改 `spec/quicknote-design-v4.md`**。

## 2. 允许修改文件白名单

- `spec/CODEBASE_MAP.md`
- `spec/PROJECT_STATE.md`
- `README.md`
- `spec/quicknote-design-v5.md`

## 3. 具体实现要求

原则：以**当前代码为准**写文档（C11.2：用户最新指示 > DECISIONS > 契约 > v5 > 代码；本轮用户已确认的口径见下）。每改一处前先 `grep` 代码确认事实，不要凭印象写。

### 3.1 `spec/CODEBASE_MAP.md`

- §1 目录树：
  - `src/features/panels/` 一行改为实际剩余文件（`OutlinePanel.vue`、`RelatedNotesPanel.vue`、`registry.ts`、`store.ts`、`api.ts`、`outlineExtractor.ts`），去掉 `RightPanelGroup.vue`、`RecentNotesPanel.vue`。
  - `src/features/editor/extensions/` 增加 `search.ts`。
  - `src/components/` 去掉已删除的组件。
  - `src-tauri/src/infra/` 去掉 `spotlight.rs`，增加 `trash.rs`。
- §2 核心数据流：第 2 条补充「`ui-state.json` 的 `leftSidebar` / `terminalPanel` / `backupPaths`」；新增一条「外部写入回声抑制：写前预登记 + 前端同哈希忽略」。
- §3 功能反查表：按下面的对应关系逐行更新。
  - 「主工作台壳、多标签…」行：前端文件注明底部终端面板在 `MainWorkbenchShell.vue`。
  - 「右侧面板组」行改名为「左侧栏视图（大纲 / 相关文档）与底部终端」。
  - 「笔记导航树」行：去掉收件箱与工作区标签页的描述，改称「目录」。
  - 「快速打开」行：注明引擎与 Quick Bar「打开」共用。
  - 「设置窗口」行：文件选择走 `src/lib/ipc/commands.ts` 的 `pickBackupSavePath` / `pickBackupOpenPath`。
  - 删除与 Spotlight 相关的测试文件名。
- §4 修改菜谱：第 1 条补充「`ui_state_set` 新键要先在 `domain/ui_state.rs` 的 `UiStatePatchKey` 登记并同步 `bindings.rs`」。
- §5 实机暗坑：第 3 条改为「原生 `NSPanel` 零私有 API：Quick Bar 与 Toast 严禁开 `.transparent(true)`；`collectionBehavior` 保持 `256`，不跨 Space、切 Space 自动复位、默认失焦关闭（固定态例外）」；新增一条「编辑器自动保存在输入法组合态不触发，组合结束后补排保存」。

### 3.2 `README.md`

逐条核对并修正（行号为基线参考）：

| 位置 | 改成 |
| --- | --- |
| 约 12–13 行窗口表 | 45% 侧栏辅助模式的入口改为「视图 > 切换 45% 侧栏辅助模式」或 Quick Bar 的「45% 侧栏」按钮（标题栏按钮已删除）；补一句「侧栏内右上角『退出侧栏辅助』」 |
| 约 31 行 | `⌘K` 说明删除「以及 Spotlight 外部搜索」，改为「`/` 路径补全、`#` 标签、`@` 工作区、`>` 动作，纯 Rust 受控探测与目录直扫」；补一句 Quick Bar「打开 ⌘K」具备同样能力 |
| 快捷键表 | 增加 `⌘F` 查找、`⌥⌘F` 查找并替换、`⌃⇥` / `⇧⌃⇥` 切换标签、Quick Bar `⌘N` |
| 约 47–53 行「右侧面板组与内嵌终端」 | 整节改写为「左侧栏视图与底部终端」：导航顺序 `目录｜标签｜最近｜相关文档｜大纲`；`⌃⌘O` / `⌃⌘R` 切左侧视图；`⌃⌘T` 开关编辑器下方终端并可拖高；删除 `⌃⌘E` 与 `⌃⌘0` |
| Quick Bar 一节 | 补充「默认失焦即关闭；点图钉或『窗口 > 固定 Quick Bar』固定后失焦不关闭」；非记录模式下 Esc 直接隐藏 |
| 约 63 行收件箱清理 | 保留（`inbox.md` 追加与提取能力未变），补一句「`inbox.md` 默认置顶显示在『目录』里」 |
| 约 79 行 | 删除「在『设置 > 工作区』配置项目目录…」与「与 Spotlight `search_roots`」的过期说法，改为「在左侧目录树右键『绑定工作区配置』编辑该目录的 `.quicknote.yaml`」 |
| 约 85 行 | 「设置 > 库与收件箱」按设置页当前的实际分区名修改 |
| 约 91 行 | `ui-state.json` 说明补「左侧栏宽度与视图、底部终端高度」 |
| 约 92 行 | `index.sqlite` 改为「缓存目录下按笔记库哈希隔离的 `<vault_hash>.db`（含 `-wal` / `-shm`），可随时删库重建，严禁放入笔记库根目录」（路径以 `application/persistence.rs` 的实际实现为准） |
| 约 95 行 | `pinned-files.json` 改为「`bookmarks.json`：外部文件 / 目录书签」 |
| 删除操作说明 | 补充「应用内删除均移到系统废纸篓，可还原」 |
| 标签页 | 补充「右键菜单可删除文件到废纸篓；标签可拖动排序；关闭按钮默认隐藏，可在设置 > 外观打开」 |

### 3.3 `spec/quicknote-design-v5.md`

- §2 第 2 点末尾补一句回声抑制规则（引用 `C8.5`）。
- §3 「Quick Open（`⌘K`）」后补「Quick Bar 的『打开』复用同一引擎」。
- §4 第 1 点「左侧项目目录树」改为「左侧目录树」。
- §5 第 1 点「常驻窗口」一句改为「…不跨 Space、切 Space 自动复位、默认失焦关闭，固定态例外（`D-0013`）」。
- §6 「统一入口与窄屏适配」一段改为：「系统托盘菜单、Quick Bar 及动作注册表（`window.toggleAuxiliary` → `window_toggle_auxiliary_sidebar`）均提供切换入口；侧栏模式下编辑器区右上角提供『拉出导航 / 退出侧栏辅助』；窗口宽度 `< 900px` 时左侧栏以覆盖式抽屉浮于编辑区之上，编辑区最小宽度不低于 `480px`；内嵌终端位于编辑器下方，可拖动高度并持久化。」
- §6 末尾补一句左侧栏顶层视图：工作区（目录 / 标签 / 最近）、相关文档、大纲。

### 3.4 `spec/PROJECT_STATE.md`

- 第 1 节「当前状态」改为 M5 迭代完成态，并用一句话概述本轮内容。
- 新增一节「M5 迭代任务清单」：表格列出 `spec/cards/README.md` 索引里的全部卡片与状态（已合入 `dev` 的标 `[x]`；M5-06f 若未执行标「未执行（可选）」）。状态以 `git log --oneline dev` 里各卡 `Card:` 行为准。
- 第 3 节回归验证结果表：按本卡收工时 `pnpm check:all` 的实际输出重新填写日期与数字（测试文件数 / 用例数 / Rust 用例数）。
- 「已知非阻断遗留项」：删除「10 个历史文件待格式化」与「Spotlight 用例超时」两条（均已解决）；如本卡执行中发现新的非阻断遗留，如实列出。
- 第 4 节「已验证实机约定」：把 Quick Bar 辅助窗口 URL 一条保留，新增「`ui-state.json` 旧键 `panelGroup` 读取时忽略」「`settings.yaml` 新字段均有默认值，旧配置可直接加载」。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（四份文档都在 `.prettierignore` 中，不影响格式检查）。
- `grep -n 'Spotlight\|spotlight' README.md spec/CODEBASE_MAP.md spec/quicknote-design-v5.md` 只剩「已废止」语境的说明。
- `grep -rn 'quicknote-design-v4' README.md spec/CODEBASE_MAP.md spec/quicknote-design-v5.md spec/PROJECT_STATE.md` 无结果。
- `grep -n '收件箱\|工作区' README.md | head` 逐条确认无把「收件箱 / 工作区」当作左侧标签页的说法（`.quicknote.yaml` 的「目录工作区」是现行概念，保留）。

## 5. 人工点验清单

- [ ] 用户通读四份文档的 diff。

## 上下文提示

- 本卡以「代码 → 文档」方向核对为主，遇到文档与代码不一致、且不属于本轮用户已确认口径的情况，按 C11.2 判定并**修正文档**，不要改代码。
