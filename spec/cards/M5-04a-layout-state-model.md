# M5-04a：布局新状态模型（与旧状态并存）

- 所属里程碑：M5
- 关联方案与契约：`v5` §6；C3.1、C3.4、C4.2、C6.3、C7.4、C7.5；D-0012
- 前置依赖：M5-01a
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：为「左侧栏可拖宽 + 顶层视图切换」与「编辑器底部终端面板」建立持久化状态：`ui-state.json` 的 `leftSidebar` 与 `terminalPanel`，以及前端 `panels` store 的对应状态与动作。本卡只**新增**，旧的 `panelGroup` 状态原样保留（M5-04d 再清理），保证每一步都能编译通过。
- 不做：不改界面布局（M5-04b 负责）；不删除任何旧代码。

## 2. 允许修改文件白名单

- `src-tauri/src/domain/ui_state.rs`
- `src-tauri/src/bindings.rs`
- `src/features/panels/store.ts`
- `src/features/panels/api.ts`

测试文件（不计入上限）：`src-tauri/tests/persistence_test.rs`、`src/features/panels/__tests__/left-sidebar-state.test.ts`（新建）

## 3. 具体实现要求

### 3.1 Rust：新结构体（`domain/ui_state.rs`）

在 `PanelGroupUiState` 的 `impl Default` 之后追加：

```rust
/// Top-level view shown in the left sidebar (`C6.3`, `C7.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum LeftViewId {
    #[default]
    Workspace,
    Related,
    Outline,
}

/// Resizable left sidebar state (`C6.3`, `C7.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LeftSidebarUiState {
    pub width: u32,
    pub active_view: LeftViewId,
    pub collapsed: bool,
}

impl Default for LeftSidebarUiState {
    fn default() -> Self {
        Self { width: 300, active_view: LeftViewId::Workspace, collapsed: false }
    }
}

/// Terminal dock below the editor (`C6.3`, `C7.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalPanelUiState {
    pub visible: bool,
    pub height: u32,
    pub collapsed: bool,
}

impl Default for TerminalPanelUiState {
    fn default() -> Self {
        Self { visible: false, height: 240, collapsed: false }
    }
}
```

`UiState` 增加两个带默认值的字段（放在 `panel_group` 之后）：

```rust
#[serde(default)]
pub left_sidebar: LeftSidebarUiState,
#[serde(default)]
pub terminal_panel: TerminalPanelUiState,
```

`UiStatePatchKey` 增加变体 `LeftSidebar`（`"leftSidebar"`）、`TerminalPanel`（`"terminalPanel"`）；`apply_patch` 增加对应分支。

`bindings.rs`：`use crate::domain::ui_state::{ ... }` 中加入 `LeftSidebarUiState`、`LeftViewId`、`TerminalPanelUiState`。随后 `pnpm gen:bindings`。

### 3.2 前端：`panels/api.ts` 追加

```ts
import type { ..., LeftSidebarUiState, TerminalPanelUiState } from '@/bindings/types';

export async function patchLeftSidebarUiState(value: LeftSidebarUiState): Promise<UiState> {
  return uiStateSet({ key: 'leftSidebar', value });
}

export async function patchTerminalPanelUiState(value: TerminalPanelUiState): Promise<UiState> {
  return uiStateSet({ key: 'terminalPanel', value });
}
```

### 3.3 前端：`panels/store.ts` 追加（旧代码一律不动）

新状态：

```ts
const leftSidebar = ref<LeftSidebarUiState>({ width: 300, activeView: 'workspace', collapsed: false });
const terminalPanel = ref<TerminalPanelUiState>({ visible: false, height: 240, collapsed: false });
```

新计算属性：

- `isTerminalPanelShown = computed(() => terminalPanel.value.visible && !terminalPanel.value.collapsed)`
- `leftSidebarWidth = computed<number>`：抽屉模式（`isOverlayDrawer`）下取 `Math.max(MIN_SIDEBAR_WIDTH, viewportWidth - 48)`，否则取 `leftSidebar.width`；两者上限都是 `Math.max(MIN_SIDEBAR_WIDTH, viewportWidth - MIN_EDITOR_WIDTH - SIDEBAR_COLLAPSED_STRIP_WIDTH)`。

新常量：在 `store.ts` 文件内（`DEFAULT_PANEL_WIDTH` 旁）定义并导出 `MIN_SIDEBAR_WIDTH = 220`、`MIN_TERMINAL_DOCK_HEIGHT = 120`、`MAX_TERMINAL_DOCK_HEIGHT = 600`。本卡不要动 `registry.ts`。

新动作（都要在 `return` 里导出）：

| 函数 | 行为 |
| --- | --- |
| `setLeftView(view: LeftViewId)` | `leftSidebar.activeView = view`；立即持久化 `leftSidebar` |
| `toggleLeftView(view: 'related' \| 'outline')` | 当前就是该视图则切回 `'workspace'`，否则切到该视图；立即持久化 |
| `setLeftSidebarWidth(width: number)` | 夹到 `[220, viewportWidth - 480 - 36]`（下限不得高于上限）后赋值；防抖 180ms 持久化（另起一个计时器变量，写法参照现有的 `schedulePersistPanelGroup`） |
| `commitLeftSidebarWidth()` | 立即持久化一次 |
| `toggleTerminalPanel()` | `visible` 与 `collapsed` 同时取反展开：若当前 `isTerminalPanelShown` 则 `visible = false`，否则 `visible = true; collapsed = false`；立即持久化 |
| `toggleTerminalCollapse()` | `visible = true` 且 `collapsed` 取反；立即持久化 |
| `setTerminalPanelHeight(height: number)` | 夹到 `[120, 600]`；防抖 180ms 持久化 |
| `commitTerminalPanelHeight()` | 立即持久化一次 |

`initialize()` 末尾追加：`uiState.leftSidebar` 存在则赋给 `leftSidebar`，`uiState.terminalPanel` 存在则赋给 `terminalPanel`（先做数值夹取）。

相关文档刷新：现有针对 `panelGroup.activeId === 'related'` 触发 `refreshRelatedNotes()` 的 `watch`，追加对 `leftSidebar.activeView === 'related'` 的同样触发。

`dispose()` 里清理新增的计时器。

### 3.4 测试

- `persistence_test.rs`：新增用例，分别 `ui_state_set` 写入 `leftSidebar`（`{ width: 320, activeView: "outline", collapsed: false }`）与 `terminalPanel`（`{ visible: true, height: 300, collapsed: false }`），再 `ui_state_get` 断言读回一致。
- `left-sidebar-state.test.ts`（新建）：mock `@/lib/ipc/commands` 的 `uiStateGet` / `uiStateSet`，断言：
  1. `toggleLeftView('outline')` 两次后 `activeView` 回到 `'workspace'`，且每次都调用了 `uiStateSet` 且 `key` 为 `'leftSidebar'`。
  2. `toggleTerminalPanel()` 一次后 `isTerminalPanelShown` 为 `true`，再一次为 `false`。
  3. `setTerminalPanelHeight(9999)` 后 `height === 600`；`setTerminalPanelHeight(1)` 后为 `120`。
  4. `setLeftSidebarWidth(50)` 后宽度为 `220`。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（生成物已重新生成并提交）。

## 5. 人工点验清单

- 无界面变化。`pnpm tauri dev` 能启动、`ui-state.json` 里出现 `leftSidebar` 与 `terminalPanel` 即可。

## 上下文提示

- 只读 `ui_state.rs` 约 100–260、`bindings.rs` 的 `use` 区、`panels/store.ts` 全文（458 行）、`panels/api.ts` 全文。
- 忽略：右侧面板组相关旧逻辑（下一张卡才动）。
