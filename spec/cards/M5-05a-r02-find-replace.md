# M5-05a：编辑器查找与替换（R02）

- 所属里程碑：M5
- 关联方案与契约：`v5` §1.2；C1.3、C2.2、C7.1、C7.2、C7.3；D-0011
- 前置依赖：M5-01a（契约 C7.2 已登记 `editor.find` / `editor.replace`，C11.1 已登记 `@codemirror/search`）
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：`⌘F` 打开查找面板，`⌥⌘F` 打开并聚焦替换输入框；原生「编辑」菜单增加「查找…」「替换…」。
- 不做：不做项目级 / 全库搜索（那是 `⌘K`）；不改侧栏里 `⌘F` 聚焦过滤框的行为（焦点在侧栏时仍是过滤框）。

## 2. 允许修改文件白名单

- `package.json`、`pnpm-lock.yaml`（C1.3 共享文件）
- `src/features/editor/extensions/search.ts`（新建）
- `src/features/editor/MarkdownEditor.vue`
- `src/lib/actions/catalog.ts`
- `src-tauri/src/lib.rs`

测试文件（不计入上限）：`src/features/editor/__tests__/editor-search.test.ts`（新建）、`src/lib/actions/__tests__/action-registry.test.ts`

## 3. 具体实现要求

### 3.1 依赖

`pnpm add @codemirror/search`（运行时依赖，写入 `dependencies`）。

### 3.2 `extensions/search.ts`（新建）

```ts
import { openSearchPanel, replaceKeymap, search, searchKeymap } from '@codemirror/search';
import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';

/** 查找 / 替换面板样式：只用 --qn-* Token（C7.1）。 */
export const qnSearchPanelTheme = EditorView.theme({
  '.cm-panels': {
    backgroundColor: 'var(--qn-bg-elevated)',
    color: 'var(--qn-text-primary)',
    borderBottom: '1px solid var(--qn-border-subtle)',
    fontSize: 'var(--qn-text-sm)',
  },
  '.cm-panels.cm-panels-top': { borderBottom: '1px solid var(--qn-border-subtle)' },
  '.cm-search': { padding: 'var(--qn-space-2)', display: 'flex', flexWrap: 'wrap', gap: 'var(--qn-space-2)', alignItems: 'center' },
  '.cm-search input': {
    backgroundColor: 'var(--qn-bg-base)',
    color: 'var(--qn-text-primary)',
    border: '1px solid var(--qn-border-strong)',
    borderRadius: 'var(--qn-radius-sm)',
    padding: 'var(--qn-space-1) var(--qn-space-2)',
    font: 'inherit',
  },
  '.cm-search button': {
    backgroundColor: 'transparent',
    color: 'var(--qn-text-secondary)',
    border: '1px solid var(--qn-border-subtle)',
    borderRadius: 'var(--qn-radius-sm)',
    padding: 'var(--qn-space-1) var(--qn-space-2)',
    font: 'inherit',
  },
  '.cm-search button:hover': { backgroundColor: 'var(--qn-bg-hover)', color: 'var(--qn-text-primary)' },
  '.cm-searchMatch': { backgroundColor: 'var(--qn-accent-soft)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'var(--qn-bg-active)' },
});

/** 打开查找面板（⌘F）。 */
export function openFindPanel(view: EditorView): void {
  openSearchPanel(view);
}

/** 打开查找面板并聚焦「替换为」输入框（⌥⌘F）。 */
export function openReplacePanel(view: EditorView): void {
  openSearchPanel(view);
  const replaceInput = view.dom.querySelector<HTMLInputElement>('.cm-search input[name="replace"]');
  replaceInput?.focus();
}

/**
 * 查找扩展集合。内置键位里去掉 `Mod-f`（由统一动作注册表的 `editor.find` 接管），
 * 其余（⌘G 下一个、⇧⌘G 上一个、F3、回车下一个、⇧回车上一个、替换确认等）保留。
 */
export function createSearchExtensions(): Extension[] {
  const findKeymap = searchKeymap.filter((binding) => binding.key !== 'Mod-f');
  return [search({ top: true }), qnSearchPanelTheme, keymap.of([...findKeymap, ...replaceKeymap])];
}
```

若安装版本的 `searchKeymap` / `replaceKeymap` 导出名不同，以该版本的类型定义为准做等价替换。

### 3.3 `MarkdownEditor.vue`

- `createEditorState` 的扩展数组里加入 `...createSearchExtensions()`（放在 `keymap.of([...defaultKeymap, ...historyKeymap])` 之后）。
- 按 `EDITOR_FORMAT_ACTION_IDS` 的绑定写法（基线约 990–1000），在 `onMounted` 里增加两个处理器并把释放函数推进 `actionDisposers`：
  - `editor.find` → `openFindPanel(view); view.focus();`
  - `editor.replace` → `openReplacePanel(view);`
  - 两者都要先判断 `view` 存在，且沿用现有的 `buildEditorActionContext().isMarkdown` 判断（非 Markdown 的外部文件同样允许查找：把条件放宽为「有 `view` 且当前标签有文档」）。
- `onUnmounted` 里 `actionDisposers` 的释放逻辑已有，无需改。

### 3.4 `catalog.ts`

在 `editor.outdent` 之后追加：

```ts
{
  id: 'editor.find',
  title: '查找',
  category: 'editor',
  defaultKeys: 'cmd+f',
  scope: 'editor',
  when: (ctx) => Boolean(ctx.hasActiveDoc),
  run: () => undefined,   // 由 MarkdownEditor 里绑定的处理器执行
},
{
  id: 'editor.replace',
  title: '查找并替换',
  category: 'editor',
  defaultKeys: 'alt+cmd+f',
  scope: 'editor',
  when: (ctx) => Boolean(ctx.hasActiveDoc),
  run: () => undefined,
},
```

`run` 的写法与同文件既有动作保持一致（若既有动作用 `actionRegistry.executeById` 或直接调用，照同样模式）。

**快捷键匹配**：`⌥⌘F` 会把 `KeyboardEvent.key` 变成特殊字符，仓库已有基于 `event.code` 的归一化（`lib/actions/keymap.ts` 的 `resolveKeyFromCode`）。实现后用单元测试确认 `keyboardEventToCombo` 对 `code: 'KeyF' + altKey + metaKey` 得到 `alt+cmd+f`；若不是，修正 `normalizeKeyCombo` / 映射表，使该组合被正确识别。

### 3.5 原生菜单（`lib.rs`）

- 「编辑」菜单（基线约 128–137）在「粘贴」之前插入：
  - `MenuItemBuilder::with_id("editor.find", "查找…").accelerator("CmdOrCtrl+F")`
  - `MenuItemBuilder::with_id("editor.replace", "查找并替换…").accelerator("Alt+CmdOrCtrl+F")`
- `on_menu_event` 的转发白名单（基线约 414–440）加入 `"editor.find"` 与 `"editor.replace"`。

`src/lib/actions/registry.ts` 的 `NATIVE_MENU_ACTION_IDS` 需要同步加入这两个 id 才能让改键后的菜单快捷键同步——该文件不在本卡白名单。做法：在本卡里只给 `lib.rs` 的固定 accelerator；并把「`NATIVE_MENU_ACTION_IDS` 未包含 `editor.find` / `editor.replace`」写进提交说明的遗留项，由用户决定是否单开一张小卡。

### 3.6 测试

- `editor-search.test.ts`：测试 `createSearchExtensions()` 返回非空数组，且其中的键位不包含 `key === 'Mod-f'` 的绑定。
- `action-registry.test.ts`：注册全部动作后 `getEffectiveKeys('editor.find') === 'cmd+f'`、`getEffectiveKeys('editor.replace') === 'alt+cmd+f'`；`detectConflicts` 对这两个 id 返回空。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 5. 人工点验清单

- [ ] 编辑器聚焦时按 `⌘F`：顶部出现查找面板，输入即高亮，回车跳到下一个，`⌘G` / `⇧⌘G` 可用，`Esc` 关闭面板。
- [ ] `⌥⌘F`：面板打开且光标在「替换为」输入框。
- [ ] 原生「编辑」菜单显示「查找… ⌘F」「查找并替换… ⌥⌘F」，点击可用。
- [ ] 焦点在左侧栏时按 `⌘F`：仍是聚焦过滤框，不弹查找面板。
- [ ] 外部代码文件标签页里 `⌘F` 可用。
- [ ] 查找面板在浅色与深色主题下配色正确；回车与 `Esc` 不触发自动保存冲突。
- [ ] C9.3 第 5 条：在查找输入框里用中文输入法输入并选词，正常上屏。

## 上下文提示

- 只读 `MarkdownEditor.vue` 约 620–740 与 968–1020、`catalog.ts` 约 176–200 与 326–355、`lib.rs` 约 128–137 与 414–453、`lib/actions/keymap.ts` 全文（250 行）。
- 忽略：快速打开（`⌘K`）与命令库。
