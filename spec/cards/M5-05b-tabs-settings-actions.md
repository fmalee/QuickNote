# M5-05b：显示关闭标签按钮开关、上一个 / 下一个标签（R22、R27）

- 所属里程碑：M5
- 关联方案与契约：`v5` §5-2；C1.4、C3.1、C4.2、C6.2、C7.2；D-0008
- 前置依赖：M5-04e
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - R22：新增设置项「显示标签页关闭按钮」，默认**不显示**标签上的 `×`。`⌘W` 与鼠标中键关闭不受影响。
  - R27：`⌃⇥` 下一个标签、`⇧⌃⇥` 上一个标签；原生「窗口」菜单增加这两项。
- 不做：不改标签的拖拽、右键菜单（M5-05c 负责）。

## 2. 允许修改文件白名单

- `src-tauri/src/domain/settings.rs`
- `src/features/settings/SettingsAppearancePanel.vue`
- `src/features/settings/store.ts`
- `src/features/editor/EditorTabBar.vue`
- `src/lib/actions/catalog.ts`
- `src/lib/actions/registry.ts`
- `src-tauri/src/lib.rs`

测试文件（不计入上限）：`src-tauri/tests/persistence_test.rs`、`src/lib/actions/__tests__/action-registry.test.ts`、`src/features/settings/__tests__/` 下相关测试

## 3. 具体实现要求

### 3.1 Rust：`settings.yaml` 新字段

`domain/settings.rs`：给 `EditorSettings` 增加 `show_tab_close_button: bool`。`single_click_link` 字段在文件里出现的每一处都要为它补上对应的一行（用 `grep -n single_click_link src-tauri/src/domain/settings.rs` 取得完整清单，基线有 8 处：结构体字段、`Default` 实现、`EditorSettingsPatch` 字段、`apply` 处、YAML 结构体字段、`Default → YAML`、`YAML → Settings`、`Settings → YAML`）。新字段默认 `false`。

**兼容性硬要求**：老用户的 `settings.yaml` 没有这个键，必须能正常加载。确认 `EditorSettings` 对应的 YAML 结构体在结构体级有 `#[serde(default)]`（与 `single_click_link` 的声明方式保持完全一致），否则补上属性级 `#[serde(default)]`。

随后 `pnpm gen:bindings`。

`persistence_test.rs` 新增用例：`settings_update` 传入 `EditorSettingsPatch { show_tab_close_button: Some(true), ..Default::default() }` 后重新读取为 `true`；缺省配置读取为 `false`。

### 3.2 前端：设置

- `settings/store.ts`：新增并导出计算属性 `showTabCloseButton = computed(() => settingsStore.snapshot?.settings.editor.showTabCloseButton ?? false)`。
- `SettingsAppearancePanel.vue`：在「单击打开链接」开关（`onToggleSingleClickLink`）之后增加一个同样的开关「显示标签页关闭按钮」，`data-testid="toggle-tab-close-button"`，`@change` 调 `store.patchSettings({ editor: { showTabCloseButton: !store.showTabCloseButton } })`（参数结构参照该文件 `onToggleSingleClickLink` 的写法）。

### 3.3 前端：`EditorTabBar.vue`

- 引入 `useSettingsStore`（`@/features/settings`），关闭按钮加 `v-if="settingsStore.showTabCloseButton"`。
- 在 `onMounted` 里按仓库里的 `actionRegistry.bindHandler(id, handler)` 模式绑定两个处理器，并在 `onUnmounted` 释放：
  - `window.nextTab` → `tabsStore.activateTabByIndex(nextIndex(tabsStore.tabs, currentIndex, 1))`
  - `window.prevTab` → `tabsStore.activateTabByIndex(nextIndex(tabsStore.tabs, currentIndex, -1))`

  其中 `nextIndex` 是组件内的纯函数：按 `tabs` 顺序循环取下一个 / 上一个（到尾回到 0，到头回到末尾）；当前标签不在列表里时返回 0。`actionRegistry` 的引入方式参照 `MarkdownEditor.vue` 顶部。

### 3.4 动作目录

`catalog.ts` 在 `window.toggleAuxiliary` 之后追加：

```ts
{ id: 'window.nextTab', title: '下一个标签', category: 'window', defaultKeys: 'ctrl+tab', scope: 'main-window', when: (ctx) => Boolean(ctx.hasActiveDoc), run: () => undefined },
{ id: 'window.prevTab', title: '上一个标签', category: 'window', defaultKeys: 'ctrl+shift+tab', scope: 'main-window', when: (ctx) => Boolean(ctx.hasActiveDoc), run: () => undefined },
```

`run` 的写法与同文件既有动作一致。

`registry.ts`：`NATIVE_MENU_ACTION_IDS` 加入 `'window.nextTab'`、`'window.prevTab'`。

### 3.5 原生菜单（`lib.rs`）

- 「窗口」菜单（基线约 257–262）在「缩放」之后、分隔线之前插入：
  - `MenuItemBuilder::with_id("window.nextTab", "下一个标签").accelerator("Ctrl+Tab")`
  - `MenuItemBuilder::with_id("window.prevTab", "上一个标签").accelerator("Ctrl+Shift+Tab")`
- `on_menu_event` 的转发白名单加入这两个 id。

### 3.6 测试

- `action-registry.test.ts`：`getEffectiveKeys('window.nextTab') === 'ctrl+tab'`、`'window.prevTab' === 'ctrl+shift+tab'`；`detectConflicts` 对两者为空；`keyboardEventToCombo` 对 `code: 'Tab' + ctrlKey` 得到 `ctrl+tab`。
- 设置测试：`showTabCloseButton` 默认 `false`，`patchSettings` 后变为 `true`。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 5. 人工点验清单

- [ ] 默认设置下标签上没有 `×`；`⌘W` 与中键仍能关闭。
- [ ] 设置「外观」里打开开关后 `×` 立即出现；重启应用后仍保持。
- [ ] `⌃⇥` / `⇧⌃⇥` 切换标签，首尾循环。
- [ ] 原生「窗口」菜单显示「下一个标签 ⌃⇥」「上一个标签 ⇧⌃⇥」，点击可用。
- [ ] **若 `⌃⇥` 在 WebView 里被系统或 WebView 自身抢走而无效**：保留菜单项，在提交说明里如实记录现象，由用户决定是否换备用键（如 `⌘⌥→` / `⌘⌥←`）。不要自行换键。

## 上下文提示

- 只读 `settings.rs` 中 `single_click_link` 的各处、`SettingsAppearancePanel.vue` 约 100–115 与模板里的开关、`EditorTabBar.vue` 全文（452 行）、`catalog.ts` 约 160–175、`registry.ts` 约 27–50、`lib.rs` 约 257–262 与 414–453。
- 忽略：标签右键菜单与拖拽。
