# M5-05c：标签拖拽排序、删除子菜单、去掉「关闭其他」按钮（R23、R24、R25）

- 所属里程碑：M5
- 关联方案与契约：`v5` §5-2；C2.2、C6.3、C7.1；D-0008
- 前置依赖：M5-03a（删除走废纸篓）、M5-05b
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - R23：标签可拖动排序，顺序写入 `ui-state.json` 的 `tabs.items`，重启后保留。
  - R24：删除标签栏右侧的「关闭其他」按钮（右键菜单里的「关闭其他标签」保留）。
  - R25：标签右键菜单增加「删除」子菜单，内容是「删除文件（移到废纸篓）」。
- 不做：不改关闭标签的逻辑；不改标签 LRU 池。

## 2. 允许修改文件白名单

- `src/components/QnContextMenu.vue`
- `src/features/editor/EditorTabBar.vue`
- `src/features/editor/store.ts`

测试文件（不计入上限）：`src/features/editor/__tests__/tabbar-and-focus-guard.test.ts`、`src/components/__tests__/` 下的右键菜单测试（若不存在则新建 `qn-context-menu.test.ts`）

## 3. 具体实现要求

### 3.1 `QnContextMenu.vue` 支持子菜单

现有类型（文件顶部导出）只有 `item` 与 `separator`。增加第三个变体：

```ts
| {
    type: 'submenu';
    id: string;
    title: string;
    disabled?: boolean;
    items: ContextMenuEntry[];   // 只允许 item 与 separator
  }
```

渲染与交互：

- 子菜单行右侧显示 `▸`。鼠标悬停（或键盘聚焦）到该行时，在其右侧展开下一级菜单；鼠标移出且不在子菜单上时收起。
- 点击下一级里的条目时，照常 `emit('select', 子条目 id)` 并关闭整个菜单。
- 子菜单绝对定位，`z-index: var(--qn-z-popover)`；颜色只用 `--qn-*` Token。
- `disabled` 的子菜单行不响应悬停。
- 其他组件传入现有的 `item` / `separator` 时行为完全不变。

### 3.2 R24 去掉按钮

`EditorTabBar.vue` 模板里 `div.qn-tabbar__actions`（基线约 275–283，含「关闭其他」按钮）整块删除及其样式。`contextMenuItems` 里的 `tab.closeOthers` 条目保留。

### 3.3 R23 拖拽排序

`store.ts` 新增并导出：

```ts
function reorderTab(fromKey: string, toKey: string, position: 'before' | 'after'): void {
  const fromIndex = tabs.value.findIndex((tab) => tab.key === fromKey);
  const toIndex = tabs.value.findIndex((tab) => tab.key === toKey);
  if (fromIndex < 0 || toIndex < 0 || fromIndex === toIndex) return;
  const [moved] = tabs.value.splice(fromIndex, 1);
  if (!moved) return;
  const insertAt = tabs.value.findIndex((tab) => tab.key === toKey) + (position === 'after' ? 1 : 0);
  tabs.value.splice(insertAt, 0, moved);
  persistTabsDebounced();
}
```

`activeKey`、池、计时器都以 `key` 为准，不需要迁移。

`EditorTabBar.vue`：

- 标签元素加 `draggable="true"`，并绑定 `@dragstart`、`@dragover.prevent`、`@drop.prevent`、`@dragend`。
- 状态：`draggingKey = ref<string | null>(null)`、`dropTarget = ref<{ key: string; position: 'before' | 'after' } | null>(null)`。
- `dragstart`：记录 `draggingKey`，设置 `event.dataTransfer.effectAllowed = 'move'`。
- `dragover`：取目标标签的中点，鼠标在前半为 `before`、后半为 `after`；更新 `dropTarget`。
- `drop`：调用 `tabsStore.reorderTab(draggingKey, 目标 key, position)`。
- `dragend`：清空两个状态。
- 给目标标签加类名 `qn-tabbar__item--drop-before` / `--drop-after`，用一条 `2px` 的 `var(--qn-accent)` 侧边线指示落点。

### 3.4 R25 删除子菜单

`EditorTabBar.vue` 的 `contextMenuItems` 在「关闭其他标签」之后追加：

```ts
{
  type: 'submenu',
  id: 'tab.delete',
  title: '删除',
  disabled: !canDeleteFile(contextTab.value),
  items: [
    {
      type: 'item',
      id: 'tab.deleteFile',
      title: '删除文件（移到废纸篓）',
      danger: true,
    },
  ],
},
```

- `canDeleteFile(tab)`：`tab.relPath` 存在且 `tab.relPath !== 'inbox.md'`。
- `handleContextMenuSelect` 增加 `tab.deleteFile` 分支：通过 `@/features/notes` 引入 `useNotesStore`，调用 `notesStore.deleteNoteItem(tab.relPath)`。删除成功后该标签由既有的 `handleNoteDeleted` 流程标记为「已删除」。
- 外部文件（库外）与 `inbox.md` 的右键菜单里「删除」为禁用。

## 4. 测试

- `tabbar-and-focus-guard.test.ts`：
  1. `reorderTab('a', 'c', 'after')` 后 `tabs` 的 key 顺序符合预期；`reorderTab` 对不存在的 key 不改变顺序。
  2. 右键菜单条目里存在 `type: 'submenu'` 且 `id === 'tab.delete'` 的条目；`inbox.md` 标签的该条目 `disabled === true`。
  3. 组件不再渲染 `editor-tab-close-others`。
- 右键菜单测试：传入含 `submenu` 的条目能渲染出子条目标题；点击子条目 `emit('select', 子条目 id)`。

## 5. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 6. 人工点验清单

- [ ] 拖动一个标签到另一个标签的前 / 后：落点指示线出现，松手后顺序改变；重启应用后顺序保留。
- [ ] 拖到标签栏空白处松手：顺序不变。
- [ ] 标签栏不再有「关闭其他」按钮；右键菜单仍有「关闭其他标签」。
- [ ] 右键「删除」展开子菜单；点「删除文件（移到废纸篓）」：文件进废纸篓，标签显示「已删除」。
- [ ] `inbox.md` 与外部文件标签上「删除」为灰色不可点。
- [ ] 其他使用 `QnContextMenu` 的地方（编辑器、目录、标签云、常用外部文件）行为不变。

## 上下文提示

- 只读 `QnContextMenu.vue` 全文（251 行）、`EditorTabBar.vue` 全文、`store.ts` 的 `closeTab` / `closeOtherTabs` 附近与 `persistTabsDebounced`。
- 忽略：查找面板、底部终端、左侧栏。
