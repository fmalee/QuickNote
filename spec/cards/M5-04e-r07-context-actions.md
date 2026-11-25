# M5-04e：外部动作进右键菜单（R07）

- 所属里程碑：M5
- 关联方案与契约：`v5` §4-1；C7.2、C7.3；D-0007
- 前置依赖：M5-04d
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - R07：编辑器「外部动作 ▾」里的全部按钮，在编辑器内容区右键菜单与「同级文件」列表右键菜单中同步提供。原按钮保留。
  - R05 第二处：「同级文件」弹出层的摘要改为单行。
- 不做：不改外部动作的 Rust 实现（`external_actions_resolve` / `external_action_run`）；不改目录右键菜单（已有 `folder.*`）。

## 2. 允许修改文件白名单

- `src/features/editor/useExternalActions.ts`
- `src/features/editor/MarkdownEditor.vue`
- `src/features/editor/SiblingFilesPopover.vue`

测试文件（不计入上限）：`src/features/editor/__tests__/external-actions-integration.test.ts`

## 3. 具体实现要求

### 3.1 现状

- `useExternalActions.ts`：`EXTERNAL_SPECS`（基线约 23–30）定义了 8 个外部动作；`buildExternalContextMenuItems(states)`（约 106–125）把状态列表转成右键菜单条目；`resolveForTarget({ absPath, workspaceKey })` 解析某路径的可用状态；`executeAction(id, { overrideStates, pinned })` 执行。标签页右键菜单（`EditorTabBar.vue` 约 86–135）已经用这一套。
- `MarkdownEditor.vue` 的 `handleContextMenu`（基线约 825–896）：当前只有「打开链接 / 复制链接」（命中链接时）、`format` 分组、`editor-context` 分组。
- `SiblingFilesPopover.vue`（410 行）：文档头「同级文件」弹出层，条目目前只有点击切换。

### 3.2 编辑器内容区右键菜单

`MarkdownEditor.vue`：

1. 从 `./useExternalActions` 引入 `useExternalActions`，组件内 `const externalActions = useExternalActions()`。
2. `handleContextMenu` 里，在 `contextEntries` 之后追加：

```ts
const doc = editorTabsStore.activeDoc;
if (doc) {
  const states = await externalActions.resolveForTarget({
    absPath: doc.absPath,
    workspaceKey: doc.meta.project ?? null,
  });
  if (states.length > 0) {
    entries.push({ type: 'separator', id: 'ctx-ext-sep' });
    entries.push(...buildExternalContextMenuItems(states));
  }
}
```

因多了一次异步解析，把 `handleContextMenu` 改为 `async`，并加请求序号守卫（快速连点右键时丢弃过期结果），写法参照 `EditorTabBar.vue` 的 `openTabContextMenu` / `tabMenuSeq`。

3. `handleContextMenuSelect`：`isExternalActionId(id)` 时调用 `externalActions.executeAction(id, { overrideStates: <本次菜单用的状态列表>, pinned: doc.meta.pinned })`。把本次菜单的状态列表存到一个 `ref` 里供选择时读取。
4. 外部文件（非库内）同样适用，不额外判断。

### 3.3 「同级文件」列表右键菜单

`SiblingFilesPopover.vue`：

1. 条目加 `@contextmenu.prevent="openEntryContextMenu($event, entry)"`。
2. 菜单条目：`打开`（与单击相同）、`在新标签打开`、分隔线、外部动作（对条目的 `absPath` 调 `resolveForTarget`）。
3. 从 `@/features/editor` 的 `index.ts` 确认 `useExternalActions` 的导出方式：本卡不改 `index.ts`；若该 hook 未导出，就在 `SiblingFilesPopover.vue` 里用相对路径 `./useExternalActions` 引入（同一 feature 内允许）。
4. 摘要 / 副标题改为单行省略：`white-space: nowrap; overflow: hidden; text-overflow: ellipsis;`，并加 `:title`。

### 3.4 `useExternalActions.ts`

本卡只允许为上述两处调用补充必要的导出或参数（例如 `resolveForTarget` 已满足则不动）。禁止改动动作的启用判定与执行逻辑。

### 3.5 测试

`external-actions-integration.test.ts` 新增：

1. `resolveForTarget({ absPath, workspaceKey })` 返回带 `enabled` 状态的列表后，`buildExternalContextMenuItems` 产出的条目包含全部 8 个动作，且禁用的带 `disabled: true`。
2. 编辑器右键菜单条目构造：命中时外部动作条目位于分隔线之后（若组件级渲染在该测试文件不易实现，改为对构造函数的纯函数测试，并在提交说明注明）。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 5. 人工点验清单

- [ ] 编辑器内容区右键：菜单下半部分列出「在访达中显示 / 在外部终端中打开 / 在 VS Code 中打开 / 在 Sublime Text 中打开 / 在外部编辑器打开工作区 / 使用系统默认应用打开 / 复制文件路径 / 固定或取消固定常用文件」，可用性与「外部动作 ▾」一致。
- [ ] 逐个点击，行为与「外部动作 ▾」里同名按钮一致（终端目录按三级级联落到正确目录）。
- [ ] 外部文件（库外）标签页上右键同样可用。
- [ ] 「同级文件」弹层里右键任一条目出现菜单并可执行；摘要单行省略。
- [ ] 右键链接时仍有「打开链接 / 复制链接」。

## 上下文提示

- 只读 `useExternalActions.ts` 全文（305 行）、`MarkdownEditor.vue` 约 300–360 与 820–900、`SiblingFilesPopover.vue` 全文、`EditorTabBar.vue` 约 80–135（参考写法）。
- 忽略：DocHeaderBar 的「外部动作 ▾」按钮本身（保留不动）。
