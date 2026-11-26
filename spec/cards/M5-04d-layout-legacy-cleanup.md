# M5-04d：清理旧右侧栏状态与死组件

- 所属里程碑：M5
- 关联方案与契约：`v5` §6；C3.4、C4.2、C6.3、C7.4、C7.5；D-0012
- 前置依赖：M5-04c
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：删除已被取代的右侧栏状态（`ui-state.json` 的 `panelGroup`、`panels` store 旧状态、右侧面板注册表、「最近编辑」面板、未使用的视觉基线组件），保证不留死代码。
- 不做：不改任何现有功能的表现；不改 `settings.yaml` 的 `panels.default`（由 M5-06f 或后续处理）。

## 2. 允许修改文件白名单

- `src-tauri/src/domain/ui_state.rs`
- `src-tauri/src/bindings.rs`
- `src/features/panels/store.ts`
- `src/features/panels/api.ts`
- `src/features/panels/registry.ts`
- `src/features/panels/index.ts`
- `src/features/panels/RecentNotesPanel.vue`（删除）
- `src/lib/actions/registry.ts`（只改 `NATIVE_MENU_ACTION_IDS` 一处）
- `src/features/quickbar/store.ts`（只动 `panelGroup` 的两处，见 3.6；本卡因此共 9 个代码文件，属明示例外）

删除类例外（仅当第 3.1 步确认全仓零引用时才删除，否则不动）：`src/features/settings/VisualBaselinePreview.vue`（设置页 `SettingsView.vue` 仍在用，预期结果是保留）

测试文件（不计入上限）：

- `src/lib/actions/__tests__/action-registry.test.ts`、`src/features/panels/__tests__/` 下的受影响用例。
- 全仓所有因 `UiState` 类型变化而报错的测试文件（基线实测约 16 个，分布在 `src/app/__tests__/`、`src/features/settings/__tests__/`、`src/features/editor/__tests__/`、`src/features/notes/__tests__/`、`src/features/quickbar/__tests__/`）。只允许删掉 mock 里的 `panelGroup` 字段、`PanelGroupUiState` 类型导入，以及针对已删功能的断言；其余内容一个字符都不改。以类型检查和单测实际报错的清单为准，不必和这里的文件数一一对应。

## 3. 具体实现要求

### 3.1 先确认无引用

逐一执行，必须全部为空才继续：

```sh
grep -rn 'panelGroup' src/ src-tauri/src/ | grep -v __tests__
grep -rn 'RecentNotesPanel\|recentItems\|refreshRecentNotes\|fetchRecentNotes\|RECENT_NOTES_LIMIT' src/ | grep -v __tests__
grep -rn 'RIGHT_PANEL_REGISTRY\|getRightPanelDef\|RightPanelDef\|BP_COLLAPSE_RIGHT_PANEL\|isNarrowRightBreakpoint\|autoCollapsedByNarrowRight\|toggleCollapseAll\|isPanelGroupExpanded\|isOutlineVisible\|applyPanelGroupState\|selectPanel\b\|togglePanel\b\|setPanelWidth\|commitPanelWidth\|MIN_PANEL_WIDTH\|DEFAULT_PANEL_WIDTH\|PANEL_WIDTH_PERSIST_DEBOUNCE_MS' src/ | grep -v __tests__
grep -rn 'VisualBaselinePreview' src/
grep -rn 'panel.toggle.recent\|panel.collapseGroup' src/ src-tauri/src/
```

若有引用落在白名单之外、也不是马上要删的定义，停下并报告。已知并已列入白名单的例外：`src/features/quickbar/store.ts` 的两处（见 3.6）。

`src-tauri/tests/persistence_test.rs` 里旧版 `ui-state.json` 样本中的 `"panelGroup"` 键是兼容性样本，保留不删；只有当该文件里有对 `panel_group` 的断言报错时才做最小适配。

### 3.2 Rust

`ui_state.rs`：删除 `PanelGroupUiState` 结构体及其 `Default`；`UiState` 删除 `panel_group` 字段；`UiStatePatchKey` 删除 `PanelGroup` 变体及 `as_str`、`apply_patch` 分支。

旧 `ui-state.json` 里的 `panelGroup` 键在反序列化时自动被忽略（`UiState` 没有 `deny_unknown_fields`），无需兼容代码。

`bindings.rs`：`use crate::domain::ui_state::{ ... }` 删除 `PanelGroupUiState`。随后 `pnpm gen:bindings`。

### 3.3 `panels` 模块

- `registry.ts`：删除 `RightPanelDef` 接口、`OUTLINE_PANEL_DEF`、`RIGHT_PANEL_REGISTRY`、`getRightPanelDef`、`BP_COLLAPSE_RIGHT_PANEL`、`MIN_PANEL_WIDTH`（左侧栏最小宽度已由 M5-04a 在 `store.ts` 里以 `MIN_SIDEBAR_WIDTH` 定义）。保留 `MIN_EDITOR_WIDTH`、`BP_OVERLAY_DRAWER`、`BP_COLLAPSE_LEFT_SIDEBAR`、`SIDEBAR_EXPANDED_WIDTH`、`COLLAPSED_PANEL_STRIP_WIDTH`。
- `api.ts`：删除 `fetchRecentNotes`、`RECENT_NOTES_LIMIT`、`patchPanelGroupUiState` 与 `PanelGroupUiState` 类型引用。
- `store.ts`：删除 `panelGroup`、`preferredVisible`、`autoCollapsedByNarrowRight`、`recentItems`、`recentLoading`、`recentRequestSeq`、`isPanelGroupExpanded`、`isOutlineVisible`、`isNarrowRightBreakpoint`，以及 `applyPanelGroupState`、`selectPanel`、`togglePanel`、`toggleCollapseAll`、`computeMaxPanelWidth`、`setPanelWidth`、`commitPanelWidth`、`refreshRecentNotes` 和旧宽度防抖计时器。
  - `applyViewportWidth` 里「右侧栏断点」整段（`BP_OVERLAY_DRAWER` 与 `BP_COLLAPSE_RIGHT_PANEL` 两段对 `panelGroup` 的操作）删除；只保留「左侧栏断点 < 1000px」部分，并追加：窗口宽度小于 `BP_OVERLAY_DRAWER` 时若底部终端展开则自动折叠（`collapsed = true`，不持久化）。
  - `setForceOverlayDrawer` 里对 `panelGroup` 的操作删除。
  - `refreshActivePanelData` 只保留 `related` 分支，改为读 `leftSidebar.activeView`。
  - `dispose()` 清理剩余计时器。
- `index.ts`：删除 `RecentNotesPanel`、`RightPanelGroup`、`RightPanelDef`、`RIGHT_PANEL_REGISTRY`、`getRightPanelDef`、`BP_COLLAPSE_RIGHT_PANEL`、`MIN_PANEL_WIDTH`、`DEFAULT_PANEL_WIDTH`、`PANEL_WIDTH_PERSIST_DEBOUNCE_MS` 的导出；`toggleCollapseAll` 若被导出也删除。
- 删除 `RecentNotesPanel.vue`。
- `VisualBaselinePreview.vue`：若第 3.1 步显示全仓零引用则删除；若设置页仍在用则保留不动。

### 3.4 动作注册表

`src/lib/actions/registry.ts` 的 `NATIVE_MENU_ACTION_IDS` 删除 `'panel.toggle.recent'` 与 `'panel.collapseGroup'` 两项。

### 3.5 测试

删除只针对已删功能的用例；其余用例因导出变化做最小适配。不得为了过测试而恢复已删代码。

`src/features/quickbar/__tests__/quickbar-capture-flow.test.ts` 里对 `key: 'panelGroup'` 的 `uiStateSet` 断言随行为一起删除，不要改成别的键。

### 3.6 `src/features/quickbar/store.ts` 的两处 `panelGroup`

1. 默认 `UiState` 对象里的 `panelGroup: { ... }` 整块删除。类型检查若要求补 `leftSidebar` / `terminalPanel`，按生成的类型补上带默认值的字段即可。
2. 追加笔记并选择在主窗口打开（`openInMain`）时调用的 `patchQuickBarUiState('panelGroup', { visible: true, activeId: 'outline', width: 260, collapsed: false })`：整个 `await Promise.resolve(...).catch(...)` 语句删除，不替换成别的键。
   原意是强制展开右侧的大纲面板；右侧栏已不存在，主窗口打开后左侧栏保持用户上次的状态。保留后面的 `hideQuickBarWindow('esc')` 与 `showMainWindowForOnboarding()` 不变。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- 第 3.1 步的五条 `grep` 全部无结果（`VisualBaselinePreview` 若被设置页使用则只允许出现在设置页）。

## 5. 人工点验清单

- [ ] 行为与上一张卡完成时完全一致：左侧栏视图切换、底部终端开关与拖高、45% 侧栏辅助、窄窗口抽屉均正常。
- [ ] 删除前先备份 `ui-state.json`，启动应用后确认旧键 `panelGroup` 被忽略、新键 `leftSidebar` / `terminalPanel` 正常读写。

## 上下文提示

- 这是一张纯删除卡：不改功能、不重命名对外行为。
- 忽略：`settings.yaml` 里的 `panels.default` 与 `panels.width`。
