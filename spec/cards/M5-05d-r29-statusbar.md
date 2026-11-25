# M5-05d：状态栏重排（R29）

- 所属里程碑：M5
- 关联方案与契约：`v5` §1.2；C7.1、C7.5
- 前置依赖：M5-04b
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - 把编辑器底部提示框里的信息（文本类型、当前缩进、语法诊断提示等）移到窗口底部状态栏的右侧。
  - 删除状态栏右侧的「热键：…」提示。
- 不做：不改诊断计算本身；不改状态栏左侧的保存状态 / 字数 / 行列。

## 2. 允许修改文件白名单

- `src/features/editor/MarkdownEditor.vue`
- `src/features/editor/store.ts`
- `src/app/MainWorkbenchShell.vue`

测试文件（不计入上限）：`src/features/editor/__tests__/editor-status.test.ts`（新建）

## 3. 具体实现要求

### 3.1 `store.ts`：新增编辑器状态快照

新增并导出：

```ts
export interface EditorStatusInfo {
  docKindLabel: string;     // 如 'Markdown' / '代码 · TypeScript' / '纯文本'
  indentLabel: string;      // 如 '缩进 2 空格'
  diagnosticsLabel: string; // 如 '诊断：2 个错误，1 个警告'；无诊断为空串
}
const editorStatus = ref<EditorStatusInfo>({ docKindLabel: '', indentLabel: '', diagnosticsLabel: '' });
function setEditorStatus(next: EditorStatusInfo): void { editorStatus.value = next; }
```

### 3.2 `MarkdownEditor.vue`

- `updateStatusAndDiagnostics`（基线约 223–253）在更新底部提示框的同时，把同样的三项文本写入 `editorTabsStore.setEditorStatus(...)`。文本内容直接复用现有提示框里的文案生成逻辑，不要重新发明。
- 底部提示框（模板里编辑器下方的状态条，约 1078–1120）删除，只保留顶部 `editorNotice` 横幅（打开链接确认等提示仍需要它）。
- 标签切换到无文档时写入三个空串。

### 3.3 `MainWorkbenchShell.vue` 状态栏

- 删除右侧的 `<span>热键：{{ settingsStore.hotkeyStatus?.combo ?? 'ctrl+alt+space' }}</span>`（基线约 798）。若 `settingsStore.hotkeyStatus` 在该文件别无引用，去掉相关引用。
- 右侧新增三个 `span`，依次显示 `editorTabsStore.editorStatus` 的三项，为空时不渲染。`data-testid`：`statusbar-doc-kind`、`statusbar-indent`、`statusbar-diagnostics`。
- 布局：`status-left` 保持，`status-right` 用已有的弹性样式靠右。

## 4. 测试

`editor-status.test.ts`：

1. `setEditorStatus` 后 `editorStatus` 反映新值。
2. `handleExternalChange` 等既有流程不会把 `editorStatus` 置为 `undefined`。

## 5. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 6. 人工点验清单

- [ ] 打开 Markdown 文档：状态栏右侧显示「Markdown」「缩进 2 空格」。
- [ ] 打开代码文件：显示语言名与代码缩进；写入一处语法错误后出现诊断数量提示。
- [ ] 状态栏右侧不再出现热键提示。
- [ ] 45% 侧栏辅助窄窗口下，状态栏三项不溢出、不换行遮挡。
- [ ] 编辑器下方不再有原来的提示条。

## 上下文提示

- 只读 `MarkdownEditor.vue` 约 200–270 与模板末尾、`MainWorkbenchShell.vue` 的 `footer.qn-main-shell__statusbar`（基线约 777–808）。
- 忽略：查找面板、终端面板。
