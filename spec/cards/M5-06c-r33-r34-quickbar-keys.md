# M5-06c：取消键直接隐藏、记录模式快捷键 ⌘N（R33、R34）

- 所属里程碑：M5
- 关联方案与契约：`v5` §5-1；C7.2、C7.3；D-0013
- 前置依赖：M5-06b
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - R33：在「打开 ⌘K」「命令 ⌘⇧K」等非记录模式下按 Esc（取消键），直接隐藏 Quick Bar，不再先退回「记录」。
  - R34：「记录」模式增加快捷键 `⌘N`。
- 不做：不改原生层的 Esc 处理；不改热键。

## 2. 允许修改文件白名单

- `src/features/quickbar/store.ts`
- `src/lib/actions/catalog.ts`

测试文件（不计入上限）：`src/features/quickbar/__tests__/` 下相关测试、`src/lib/actions/__tests__/action-registry.test.ts`

## 3. 具体实现要求

### 3.1 R33

`quickbar/store.ts` 的 `handleEscape`（基线约 1328–1343，M5-06b 后行号会偏移，按函数名找）当前逻辑：

```ts
if (saveAsModalOpen.value) { closeSaveAsNoteDialog(); return; }
if (targetPickerOpen.value) { closeTargetPicker(); return; }
if (mode.value !== 'capture') { mode.value = 'capture'; focusTick.value += 1; return; }   // ← 退回记录
await hide('esc');
```

删掉第三段（退回记录模式）。即：另存为弹窗打开时 Esc 关弹窗；目标选择器打开时 Esc 关选择器；其余情况一律 `await hide('esc')`。

`hide` 内部已经把模式复位为 `capture` 并保留草稿，无需改动。

入口核对：`QuickBarView.vue` 里三处调用 `store.handleEscape()`（记录输入框 keydown、打开输入框 keydown、顶部「Esc」按钮）都会自动获得新行为，不需要改。

### 3.2 R34

`catalog.ts` 的 `quickbar.mode.capture` 定义（基线约 27–30）把 `defaultKeys: null` 改为 `defaultKeys: 'cmd+n'`。

作用域：该动作 `scope` 为 `quickbar`，与主窗口的 `window.newNote`（`cmd+n`，作用域 `main-window`）不在同一作用域，不构成冲突（C7.3）。

### 3.3 测试

- quickbar 测试：
  1. `mode = 'open'` 时调用 `handleEscape()`：断言 `hideQuickBarWindow` 被调用且参数为 `'esc'`，`mode` 复位为 `'capture'`。
  2. `mode = 'command'` 时同上。
  3. 目标选择器打开时调用：只关闭选择器，不隐藏。
  4. 另存为弹窗打开时调用：只关闭弹窗，不隐藏。
- `action-registry.test.ts`：`getEffectiveKeys('quickbar.mode.capture') === 'cmd+n'`；`detectConflicts('quickbar.mode.capture', 'cmd+n')` 为空。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 5. 人工点验清单

- [ ] Quick Bar 切到「打开」，按 Esc：面板一次即隐藏；再次呼出时回到「记录」且草稿保留。
- [ ] 「命令」模式下同样一次隐藏。
- [ ] 顶部「Esc」按钮在任意模式下一次隐藏。
- [ ] 打开目标选择器时按 Esc：只关选择器；再按一次才隐藏。
- [ ] 「打开」里 Esc 若被打开输入框自身的逻辑处理（例如先清空路径），确认最终仍能通过再一次 Esc 隐藏，不会卡住。
- [ ] Quick Bar 里按 `⌘N`：切到「记录」模式。设置「快捷键」里显示该默认键。

## 上下文提示

- 只读 `quickbar/store.ts` 的 `handleEscape`、`hide`、`setMode`；`catalog.ts` 约 27–45。
- 忽略：失焦关闭、固定开关。
