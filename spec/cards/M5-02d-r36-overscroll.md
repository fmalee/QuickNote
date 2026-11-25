# M5-02d：修复滚动到顶/底时整页橡皮筋晃动（R36）

- 所属里程碑：M5
- 关联方案与契约：`v5` §1.2；C7.1、C9.1
- 前置依赖：M5-01a
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：鼠标在编辑器、左侧列表、终端里滚到顶或底时，主界面不再跟手位移。
- 不做：不改任何滚动位置逻辑、不改滚动条样式。

## 2. 允许修改文件白名单

- `src/styles/tokens.css`
- `src/app/MainWorkbenchShell.vue`
- `src/features/editor/MarkdownEditor.vue`
- `src/features/terminal/TerminalPanel.vue`

## 3. 具体实现要求

1. `src/styles/tokens.css`：`html, body, #app { ... }`（基线约 166–175 行）里追加 `overflow: hidden;` 与 `overscroll-behavior: none;`。
2. `MainWorkbenchShell.vue`：`<style scoped>` 里所有带 `overflow-y: auto` 或 `overflow: auto` 的规则（基线约 945、1046 行两处）各追加 `overscroll-behavior: contain;`。
3. `MarkdownEditor.vue`：`<style>`（含 `EditorView.theme` 或 scoped style）中给 `.cm-scroller` 追加 `overscroll-behavior: contain;`。若该选择器尚不存在，新增一条只含这一条声明与已有 `scrollbar` 规则同处的样式。
4. `TerminalPanel.vue`：给 `.xterm-viewport` 追加 `overscroll-behavior: contain;`（与现有的 `.xterm-scrollable-element > .scrollbar` 规则放在同一处）。

约束：不得出现硬编码色值与魔法 `z-index`（门禁第 2 步会扫描）。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 5. 人工点验清单

- [ ] 编辑器滚到顶 / 底后继续滚动：编辑器内容可有惯性，但主界面不上下位移。
- [ ] 左侧列表、标签树滚到边界：同上。
- [ ] 终端内滚到边界：同上。
- [ ] 45% 侧栏辅助模式下重复以上三项。

## 上下文提示

- 只需关注 `overflow` 相关样式声明。
- 忽略：布局尺寸、颜色、间距。
