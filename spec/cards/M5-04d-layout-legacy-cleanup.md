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

删除类例外（仅当第 3.1 步确认全仓零引用时才删除，否则不动）：`src/features/settings/VisualBaselinePreview.vue`

测试文件（不计入上限）：`src/lib/actions/__tests__/action-registry.test.ts`、`src/features/panels/__tests__/` 下的受影响用例。

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

若有非预期引用（即不在本卡白名单文件内、也不是马上要删的定义），停下并报告。

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

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- 第 3.1 步的五条 `grep` 全部无结果（`VisualBaselinePreview` 若被设置页使用则只允许出现在设置页）。

## 5. 人工点验清单

- [ ] 行为与上一张卡完成时完全一致：左侧栏视图切换、底部终端开关与拖高、45% 侧栏辅助、窄窗口抽屉均正常。
- [ ] 删除前先备份 `ui-state.json`，启动应用后确认旧键 `panelGroup` 被忽略、新键 `leftSidebar` / `terminalPanel` 正常读写。

## 上下文提示

- 这是一张纯删除卡：不改功能、不重命名对外行为。
- 忽略：`settings.yaml` 里的 `panels.default` 与 `panels.width`。
