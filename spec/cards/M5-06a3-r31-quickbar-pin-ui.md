# M5-06a3：固定 Quick Bar——界面图标与动作（R31）

- 所属里程碑：M5
- 关联方案与契约：`v5` §5-1；C1.2、C7.2；D-0013
- 前置依赖：M5-06a2
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - Quick Bar 顶部右侧、「取消 / Esc」按钮左边增加一个固定图标按钮，点击切换固定状态。
  - 在动作目录登记 `quickbar.togglePin` 与 `window.pinQuickBar`，使其出现在快捷键设置里并可改键。
  - 主窗口与 Quick Bar 里的状态保持一致。
- 不做：不改失焦判定（M5-06a1 / M5-06a2 已完成）。

## 2. 允许修改文件白名单

- `src/lib/ipc/commands.ts`（C1.3 共享文件，仅追加一个函数）
- `src/features/quickbar/store.ts`
- `src/features/quickbar/QuickBarView.vue`
- `src/lib/actions/catalog.ts`
- `src/app/MainWorkbenchShell.vue`

测试文件（不计入上限）：`src/features/quickbar/__tests__/` 下相关测试、`src/lib/ipc/__tests__/smoke.test.ts`

## 3. 具体实现要求

### 3.1 `commands.ts`

```ts
export function windowSetQuickBarPinned(pinned: boolean): Promise<boolean> {
  return invoke<boolean>('window_set_quick_bar_pinned', { args: { pinned } });
}
```

### 3.2 `quickbar/store.ts`

- 通过 `@/features/settings` 引入 `useSettingsStore`。
- 新增并导出：

```ts
const settingsStore = useSettingsStore();
const isPinned = computed<boolean>(() => settingsStore.snapshot?.settings.quickBar.pinned ?? false);

async function togglePin(): Promise<void> {
  errorMessage.value = null;
  try {
    await windowSetQuickBarPinned(!isPinned.value);
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : '切换 Quick Bar 固定状态失败。';
  }
}
```

`useSettingsStore()` 必须在 `defineStore` 的 setup 函数体内调用（不要在模块顶层）。

界面状态同步：M5-06a2 的命令在成功后会广播 `settings:status`，`settings` store 收到后会重新拉取配置快照，所以无论从菜单、主窗口还是 Quick Bar 触发，`isPinned` 都会自动更新。实现后确认 Quick Bar 窗口里 `settings:status` 的监听已建立（`src/features/settings/store.ts` 的 `initialize`）。

### 3.3 `QuickBarView.vue`

在 `header-right` 里的 `<QnButton ... @click="store.handleEscape()">Esc</QnButton>` **之前**插入：

```html
<button
  type="button"
  class="qn-quickbar__chip-btn"
  :class="{ 'qn-quickbar__pin-btn--active': store.isPinned }"
  :title="store.isPinned ? '已固定：失焦不关闭' : '固定后失焦不关闭'"
  :aria-pressed="store.isPinned"
  data-testid="quickbar-pin-toggle"
  @click="store.togglePin()"
>
  📌
</button>
```

新增样式 `.qn-quickbar__pin-btn--active`：背景 `var(--qn-accent-soft)`、边框 `var(--qn-accent)`。只用 `--qn-*` Token。

### 3.4 `catalog.ts`

追加两个动作（写法与同文件一致）：

```ts
{ id: 'quickbar.togglePin', title: '固定 / 取消固定 Quick Bar', category: 'quickbar', defaultKeys: null, scope: 'quickbar', run: () => undefined },
{ id: 'window.pinQuickBar', title: '固定 Quick Bar', category: 'window', defaultKeys: null, scope: 'main-window', run: () => undefined },
```

### 3.5 处理器绑定

- `QuickBarView.vue`：按文件里已有的 `actionRegistry.bindHandler` 用法绑定 `quickbar.togglePin` → `store.togglePin()`，卸载时释放。
- `MainWorkbenchShell.vue` 的 `bindMainWindowActions` 里绑定 `window.pinQuickBar` → 调用 `windowSetQuickBarPinned(!(settingsStore.snapshot?.settings.quickBar.pinned ?? false))`（`windowSetQuickBarPinned` 从 `@/lib/ipc/commands` 引入）。

### 3.6 测试

- quickbar 测试：mock `windowSetQuickBarPinned`，`settingsStore.snapshot.settings.quickBar.pinned` 为 `false` 时调用 `togglePin()`，断言被调用且参数为 `true`。
- `smoke.test.ts` 若枚举 `commands.ts` 的导出，同步加入新函数。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 5. 人工点验清单（C9.3 第 1 条修订版）

- [ ] Quick Bar 顶部「Esc」左边有图钉按钮；点击后高亮，`settings.yaml` 的 `quick_bar.pinned` 变为 `true`。
- [ ] 固定后点击其他窗口：Quick Bar 不消失；取消固定后恢复失焦即关闭。
- [ ] 在 Quick Bar 里切换固定：主窗口「窗口 > 固定 Quick Bar」的勾选态同步变化；反向在菜单里切换，Quick Bar 的图钉高亮也同步。
- [ ] 未固定时点击外侧关闭后草稿保留；切 Space 仍自动复位。
- [ ] 设置「快捷键」里能找到「固定 / 取消固定 Quick Bar」与「固定 Quick Bar」并可改键。

## 上下文提示

- 只读 `QuickBarView.vue` 约 296–382、`quickbar/store.ts` 的 store 定义开头与 `errorMessage` 声明处、`catalog.ts` 约 27–45 与 160–175、`MainWorkbenchShell.vue` 约 200–290。
- 忽略：Quick Bar 的打开 / 命令模式。
