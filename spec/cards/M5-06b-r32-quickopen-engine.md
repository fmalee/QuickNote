# M5-06b：混合打开移植到 Quick Bar（R32）

- 所属里程碑：M5
- 关联方案与契约：`v5` §3；C1.2、C2.2、C7.2；D-0012
- 前置依赖：M5-06a3
- 会话策略：独立新会话执行；完成后提交并结束会话

> 难度偏高：涉及两个较大文件的适配。执行中如发现两侧行为无法一一对应，以**主窗口的行为为准**，并把差异写进提交说明。

## 1. 目标范围

- 要做：让 Quick Bar 的「打开 ⌘K」具备与主窗口「快速打开 ⌘K」完全相同的混合打开能力（`/`、`~/` 路径直开与 Tab 补全，`#` 标签，`@` 工作区，`>` 动作，笔记 / 外部书签混合检索），并且两边**共用同一套引擎**。
- 不做：不改 Rust 侧搜索命令；不改 Quick Bar 的记录 / 命令模式；不改主窗口快速打开面板的外观。

## 2. 允许修改文件白名单

- `src/features/quickopen/useQuickOpen.ts`
- `src/features/quickopen/index.ts`
- `src/features/quickopen/api.ts`（只扩充跨窗口待办载荷）
- `src/features/quickbar/store.ts`
- `src/features/quickbar/QuickBarView.vue`
- `src/app/MainWorkbenchShell.vue`（只扩充 `consumeAndOpenPendingFile` 的消费逻辑）

测试文件（不计入上限）：`src/features/quickbar/__tests__/quickbar-open-mode.test.ts`、`src/features/quickopen/__tests__/quick-open-engine.test.ts`

## 3. 具体实现要求

### 3.1 现状

- 主窗口：`useQuickOpen(options)`（784 行）已是完整引擎，提供回调 `onOpenItem(item, mode)`、`onExecuteAction(actionId)`、`onRevealItem(item)`、`onClose()`、`getActionItems()`，返回 `query / items / selectedIndex / activeDirectoryScope / setQuery / moveSelection / enterDirectory / exitDirectory / handleTabComplete / confirmItem / revealItem / handleKeydown / reset / dispose`。
- Quick Bar：`quickbar/store.ts` 里另有一套精简实现（约 447–800 行的 `runOpenModeSearchNow`、`scheduleOpenModeSearch`、书签分组等），只支持书签、目录下钻与路径直开，没有 `#` / `@` / `>`。四个工具函数（`getPathBaseName`、`splitPartialExternalPath`、`matchesBookmarkQuery`、`sortBookmarks`）与 `useQuickOpen.ts` 重复。

### 3.2 第一步：读清楚主窗口的回调

读 `src/features/quickopen/QuickOpenPalette.vue`，把它传给 `useQuickOpen` 的四个回调（打开条目、执行动作、访达显示、关闭）逐条抄录成笔记，作为 Quick Bar 侧行为的基准。再读 `queryParser.ts` 确认五种前缀。

### 3.3 第二步：`useQuickOpen.ts` 去重

把下面四个函数收敛为一处（保留在 `useQuickOpen.ts` 内并 `export`，或挪到 `queryParser.ts`），Quick Bar 侧不再保留副本：

- `getPathBaseName`
- `splitPartialExternalPath`
- `matchesBookmarkQuery`
- `sortBookmarks`

在 `index.ts` 导出 `useQuickOpen` 已导出；若新加了纯函数也一并导出。

若 `useQuickOpen.ts` 内部有只属于「在主窗口里打开」的硬编码（例如直接操作主窗口 store），改为通过回调完成，使该 composable 能在 Quick Bar 窗口里运行。

### 3.4 第三步：Quick Bar 接入引擎

在 `quickbar/store.ts` 的 store 定义内创建引擎：

```ts
const quickOpen = useQuickOpen({
  getActionItems: () => quickBarActionItems(),      // 见下
  onOpenItem: (item, mode) => openItemFromQuickBar(item, mode),
  onExecuteAction: (actionId) => runActionFromQuickBar(actionId),
  onRevealItem: (item) => revealItemFromQuickBar(item),
});
```

然后把 Quick Bar 现有的打开模式状态与函数改成对引擎的**薄适配**，对外名称保持不变（`QuickBarView.vue` 与测试依赖它们）：

| 现有导出 | 改为 |
| --- | --- |
| `openQuery` | `toRef(quickOpen.query)` 或等价的 `computed({ get, set })` |
| `setOpenQuery(q)` | `quickOpen.setQuery(q)` |
| `openSelectedIndex` | `quickOpen.selectedIndex` |
| `moveOpenSelection(delta)` | `quickOpen.moveSelection(delta)` |
| `enterOpenDirectory(dir, title)` | `quickOpen.enterDirectory(dir, title)` |
| `exitOpenDirectory()` | `quickOpen.exitDirectory()` |
| `activeOpenDirectoryScope` | `quickOpen.activeDirectoryScope` |
| `openLoading` / `openError` | `quickOpen.loading` / `quickOpen.errorMessage` |
| `openFlatItems` | 由 `quickOpen.items` 映射为 `QuickBarOpenItem` |
| `openGroups` | 由 `openFlatItems` 按 `group` 归并，分组顺序固定为 `note`、`file_bookmark`、`dir_bookmark`、`path_direct`，另加 `action`、`tag` 两个新分组 |
| `confirmOpenSelection()` | `quickOpen.confirmItem(quickOpen.selectedItem.value, 'open')` |
| `resetOpenModeState()` | `quickOpen.reset()` |
| `openProbeResult` | `quickOpen.pathProbeResult` |

映射规则（`QuickOpenResultItem` → `QuickBarOpenItem`）：

| `item.kind` | `group` | `badgeLabel` | `actionKind` |
| --- | --- | --- | --- |
| `note` | `note` | 「笔记」 | `open_file` |
| `file` 且 `source` 为书签 | `file_bookmark` | 「文件书签」 | `open_file` |
| `dir` 且 `source` 为书签 | `dir_bookmark` | 「目录书签」 | `enter_directory` |
| `file` 且来自路径探测 | `path_direct` | 「路径直开」 | `open_file` |
| `tag` | `tag` | 「标签」 | `open_file` |
| `action` | `action` | 「动作」 | `open_file`（实际执行动作） |

类型 `QuickBarOpenGroupId`、`QuickBarOpenBadgeLabel` 相应增加 `action`、`tag` 与「动作」「标签」。

删除 `runOpenModeSearchNow`、`scheduleOpenModeSearch`、`clearOpenTimer`、`openRequestId` 以及 Quick Bar 自己的防抖逻辑（引擎内部已有 80ms 防抖与过期响应丢弃）。

### 3.5 Quick Bar 侧行为

- **打开笔记 / 文件**：沿用现有的 `openFileInMain`（先显示主窗口再在主窗口打开）。`mode === 'newTab'` 时按固定标签打开，否则按预览打开——与 `QuickOpenPalette` 的 `open` / `newTab` 语义一致。
- **目录条目**：进入目录下钻（引擎内部处理），不需要回调。
- **标签条目（`#`）**：与主窗口一致地跳转。主窗口的行为是切到左侧栏「标签」并选中该标签；Quick Bar 窗口无法直接操作主窗口 store，沿用现有的跨窗口待办机制（`src/features/quickopen/api.ts` 的 `PENDING_OPEN_FILE_STORAGE_KEY` 一族）：把待办载荷从「只能是文件路径」扩展为可携带 `{ kind: 'file', absPath, mode }`（默认，保持向后兼容）、`{ kind: 'tag', tag }` 与 `{ kind: 'action', actionId }`。写入用现有的 `writePendingOpenFile`，由 `MainWorkbenchShell.vue` 的 `consumeAndOpenPendingFile` 消费：
  - `tag`：显示主窗口后，把左侧栏切到「标签」并选中该标签（通过 `@/features/notes` 的 `useNotesStore`：`setActiveTab('tags')` 后 `toggleTag(tag)`；若已选中则不重复切换）。
  - `action`：显示主窗口后 `actionRegistry.executeById(actionId, ctx)`，`ctx` 用壳里现成的 `buildActionContext()`。
  - 这两处的写法参照壳里现有的「待办文件」处理与 `openMainCommandBook()`。
- **动作条目（`>`）**：`quickBarActionItems()` 提供可在 Quick Bar 执行的动作（作用域为 `quickbar` 或 `global` 的全部动作，来源与 `QuickOpenPalette` 相同的 `actionRegistry.getAll()` 过滤）；执行走 `actionRegistry.executeById(actionId, ctx)`，`ctx.activeScope` 为 `'quickbar'`。
- **访达显示**（`⌥⏎`）：沿用现有的对外部文件的 `reveal` 调用。

### 3.6 `QuickBarView.vue`

- `handleOpenKeydown`（基线约 68–100）开头先调用 `quickOpen.handleKeydown(event)` 的等价入口（在 store 上导出一个 `handleOpenKeydown(event): boolean` 转发），返回 `true` 表示已处理则直接返回；否则走原有逻辑（保留 Escape 分支）。
- 结果列表增加「动作」「标签」两个分组的渲染，样式复用现有分组。
- 输入框占位文案改为：「搜索笔记、书签；`/` 路径、`#` 标签、`@` 工作区、`>` 动作」。

### 3.7 测试

- `quickbar-open-mode.test.ts`：把依赖旧内部实现的用例改为针对引擎行为；新增：
  1. 输入 `#ta` 时列表出现 `tag` 分组。
  2. 输入 `>ref` 时列表出现 `action` 分组。
  3. 路径模式下按 Tab 触发补全。
- `quick-open-engine.test.ts`：确认引擎导出项无破坏；若新增了纯函数导出，补最小用例。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- 架构检查通过：`features/quickbar` 只能通过 `@/features/quickopen` 顶层导入使用引擎；`features/quickopen` 不得反向依赖 `features/quickbar`。

## 5. 人工点验清单

- [ ] Quick Bar「打开」里输入关键字：笔记与书签混合结果与主窗口一致。
- [ ] `/usr/lo` + Tab：补全；回车直开。
- [ ] `#标`：列出标签；回车后主窗口打开并切到对应标签筛选。
- [ ] `@`：列出工作区条目（与主窗口一致）。
- [ ] `>pin`：列出动作；回车执行。
- [ ] 进目录后空输入按 Backspace 返回上级。
- [ ] 快速连续输入：结果无乱序覆盖（80ms 防抖与过期丢弃生效）。
- [ ] 主窗口「快速打开」行为与之前完全一致。

## 上下文提示

- 必读：`useQuickOpen.ts` 全文、`queryParser.ts` 全文、`QuickOpenPalette.vue` 的 `useQuickOpen` 调用处、`quickbar/store.ts` 约 60–160 与 330–800、`QuickBarView.vue` 约 60–110 与 400–520。
- 忽略：记录模式、目标选择器。
