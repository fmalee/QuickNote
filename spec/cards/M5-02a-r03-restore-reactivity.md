# M5-02a：修复启动后当前标签内容不加载（R03）

- 所属里程碑：M5
- 关联方案与契约：`v5` §5；C2.2、C7.5、C9.1；D-0012
- 前置依赖：无
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：修复「启动后编辑器停在『从左侧选择或新建一篇笔记。』，切换标签再切回才加载」。
- 不做：不改 LRU 池（`editorPool.ts`）、不改 `MarkdownEditor.vue` 的挂载逻辑、不改零标签时自动打开收件箱的行为。

## 2. 允许修改文件白名单

- `src/features/editor/store.ts`
- `src/features/editor/__tests__/editor-autosave-and-conflict.test.ts`

## 3. 具体实现要求

### 3.1 根因（已定位，照此修）

`restoreTabsFromUiState`（基线约 1326–1420 行）里：

```ts
tabs.value = restored;                         // restored 是本地普通数组
...
const activeTarget =
  (preferredKey ? restored.find((tab) => tab.key === preferredKey) : undefined) ??
  restored[0] ?? null;                        // 取到的是「非响应式的原始对象」
activeKey.value = activeTarget?.key ?? null;
if (activeTarget) {
  await hydrateTabFromDisk(activeTarget);     // 对原始对象赋值，Vue 收不到更新
```

`hydrateTabFromDisk` 内部给 `tab.doc`、`tab.draftBody` 赋值发生在原始对象上，界面依赖的计算属性 `activeDoc` 不会重新求值，所以停在占位文案。切换标签会改变 `activeKey`，计算属性才重算。

### 3.2 修法

在 `tabs.value = restored` 之后，`activeTarget` 必须从响应式代理取：

```ts
const preferredKey = uiState.tabs.activeKey;
const activeTarget =
  (preferredKey ? tabs.value.find((tab) => tab.key === preferredKey) : undefined) ??
  tabs.value[0] ??
  null;
```

其余逻辑不动。同一个函数里 `restored.length === 0` 分支（自动打开收件箱）不受影响，不要改。

### 3.3 回归测试（先写测试，确认在旧代码上失败，再改）

在 `editor-autosave-and-conflict.test.ts` 新增一个用例：

1. 按该文件既有用例的方式 mock `@/lib/ipc/commands`（`uiStateGet`、`notesList`、`noteRead`）与 `@/lib/ipc/events`。
2. `uiStateGet` 返回 `tabs.items` 含两个条目 A、B，`activeKey` 为 A。
3. `notesList` 返回 A、B 两条摘要（含 `relPath`、`absPath`、`key`）。
4. `noteRead` 返回一个**手动控制的延迟 Promise**（保存其 `resolve`）。
5. 调用 `restoreTabsFromUiState()`（不 await），等一个微任务让代码跑到 `noteRead` 之前，先读一次 `store.activeDoc` 并断言为 `null`（让计算属性缓存下来）。
6. resolve `noteRead`（返回含正文的 `NoteDoc`），await 恢复完成。
7. 断言 `store.activeDoc?.body` 等于正文、`store.activeTab?.key` 等于 A 的 key、`store.activeTab?.evicted` 为 `false`。

修复前第 7 步应失败（`activeDoc` 仍为 `null`），修复后通过。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- 新增用例通过，原有用例不回归。

## 5. 人工点验清单

- [ ] 带 2 个以上标签的状态重启应用：激活标签立即显示正文，无需切换。
- [ ] 零标签状态首次启动：仍自动打开 `inbox.md`。
- [ ] `ui-state.json` 里的标签在磁盘上已被移走时：显示「文件已移走或失联」横幅。

## 上下文提示

- 只读 `store.ts` 的 `restoreTabsFromUiState` 与 `hydrateTabFromDisk` 两段，以及测试文件头部和一个现有用例的写法。
- 忽略：保存 / 冲突相关逻辑。
