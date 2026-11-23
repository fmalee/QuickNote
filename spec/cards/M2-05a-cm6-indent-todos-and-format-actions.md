# M2-05a：CM6 Markdown 增强（上）——上下文 Tab 缩进、行首复选框 Widget 与全套格式动作/右键菜单

- 所属里程碑：M2
- 关联方案与契约：`v4` §3.8、§5.6.1、§5.6.3、§5.6.5、§6 M2、附录 A；`C1.2`、`C2.2`、`C7.1`、`C7.2`、`C7.3`、`C9.1`
- 前置依赖：`M2-02`、`M2-04`
- 会话策略：独立新会话执行（零大文档读取，所有 CM6 扩展规格、缩进宽度规则、IME 安全约束与全套格式行为已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 上下文感知 Tab / ⇧Tab 缩进扩展（`src/features/editor/extensions/markdownIndent.ts`，`v4` §5.6.1）**：
    - 导出纯配置接口 `IndentConfig { isMarkdown: boolean; tabSizeProse: number; tabSizeCode: number }`（默认 `tabSizeProse = 2`、`tabSizeCode = 4`）与纯辅助/CM6 命令：
      - `isLineInFencedCodeBlock(doc: Text, lineNumber: number): boolean`：从第 1 行向下扫描 `` ``` `` / `~~~` 围栏代码块状态，准确判断指定行是否位于代码块内部（不含围栏开闭行本身）。
      - `detectNonMarkdownIndentUnit(doc: Text): string`：针对非 Markdown 文件，扫描前 100 行既有缩进（若 tab 占优返回 `'\t'`，否则估算常见空格步长 `2` 或 `4`），绝不套用 Markdown 列表/引用规则。
      - `createMarkdownIndentCommand(getConfig: () => IndentConfig): StateCommand`（`Tab`）与 `createMarkdownOutdentCommand(getConfig: () => IndentConfig): StateCommand`（`⇧Tab`）：
        - **多行选区支持**：当选区跨越多行（或单行命中列表/引用）时，对选区覆盖的每一行按上下文统一整块缩进或反缩进，并保持选区范围连贯。
        - **场景 1 — 列表项（无序 `- ` / `* ` / `+ `、有序 `1. ` / `10. `、待办 `- [ ] ` / `- [x] `）**：
          - 缩进宽度按当前/上一父级列表 marker 宽度动态计算：`marker.length`（例如 `- ` 为 `2`、`1. ` 为 `3`、`10. ` 为 `4`）；
          - `Tab` 在行首插入对应宽度空格；`⇧Tab` 移除行首至多对应宽度空格（不足则清空全部前导空格）。
        - **场景 2 — 引用块（`^\s*>+`）**：
          - `Tab` 在首个 `>` 处增加一层 `> `；`⇧Tab` 移除一层 `> `（或单独的 `>`）。
        - **场景 3 — 围栏代码块内部**：
          - `Tab` 在光标处（或每行行首，多行选区时）插入 `tabSizeCode`（默认 `4`）空格；`⇧Tab` 每行行首减少至多 `tabSizeCode` 空格（或 1 个 `\t`）。
        - **场景 4 — 普通段落**：
          - `Tab` 在光标处插入 `tabSizeProse`（默认 `2`）软空格，**绝不因按 Tab 误触发 4 空格缩进代码块**；`⇧Tab` 移除行首至多 `tabSizeProse` 空格。
        - **场景 5 — 非 Markdown 文件（`isMarkdown === false`）**：
          - 使用 `detectNonMarkdownIndentUnit(state.doc)` 缩进/反缩进。
  - **2. 行首 `Decoration.widget` 待办复选框与待办快捷键（`src/features/editor/extensions/markdownTodo.ts`，`v4` §5.6.5）**：
    - 待办行正则：`TODO_LINE_RE = /^(\s*)([-*+])(\s+)\[([ xX])\](?=\s|$)/`（忽略围栏代码块内的行）。
    - **`TodoCheckboxWidget extends WidgetType` + `ViewPlugin`**：
      - 在每个匹配待办行的行首（`line.from + leadingSpaces.length`，`side: -1`）注入只读可点击复选框 Widget（`<input type="checkbox" class="qn-cm-todo-checkbox" ... />` 或 `<button role="checkbox" aria-checked="...">`）：
        - **IME 与选区保护红线（`v4` §5.6.5 / §8.1）**：**严禁使用 `Decoration.replace` 隐藏或替换底层 `- [ ] ` 源文本**！源文本始终完整保留 `- [ ] ` / `- [x] `，Widget 仅作行首视觉控件（`ignoreEvent` 拦截点击并 `mousedown` `preventDefault` 防止抢焦点破坏中文拼音 IME 选词）；
        - 点击复选框直接派发单步事务将对应位置 `[ ]` ↔ `[x]`（保留原缩进与后续文本）。
    - **三态循环切换命令 `toggleTodoCommand: StateCommand`（`⌥⌘X`）**：
      - 支持单行与多行选区：
        - 普通文本行（含普通无序列表 `- text`）→ `- [ ] text`（保留前导缩进）；
        - 未完成待办 `- [ ] text` → `- [x] text`；
        - 已完成待办 `- [x] text` / `- [X] text` → 普通文本行 `text`（去除 `- [x] ` 前缀，保留缩进）。
    - **待办回车续写 `todoEnterCommand: StateCommand`（`Enter`）**：
      - 当单光标位于待办行且非代码块内部：
        - 若待办行正文为空（如 `  - [ ] ` 或 `- [ ]`）：一键清除该行待办前缀恢复为空白行，返回 `true`；
        - 若待办行正文非空且光标在前缀之后：插入 `\n${indent}${bullet} [ ] ` 自动续写同级缩进未完成待办，返回 `true`；
        - 其余场景返回 `false` 放行给默认 `Enter`。
    - **空待办退格清除 `todoBackspaceCommand: StateCommand`（`Backspace`）**：
      - 当单光标（空选区）正好位于待办前缀末尾（如 `- [ ] |` 且该行后面无内容）时，一键删除整段 `- [ ] ` 前缀（保留或清至行首），返回 `true`；其余场景返回 `false` 放行。
  - **3. 全套 Markdown 格式命令（`src/features/editor/extensions/markdownFormat.ts`，`v4` §5.6.3、`C7.2`、`C7.3`）**：
    - 所有常规格式命令均**不依赖选区**（无选区按规范模板插入并精准定位光标）：
      - `formatBold`（`editor.bold`，`⌘B`）：无选区插入 `****` 并将光标置中（`anchor = from + 2`）；有选区则包裹 `**sel**`（若已包裹 `**...**` 则智能反包裹）。
      - `formatItalic`（`editor.italic`，`⌘I`）：无选区插入 `**` 光标置中（`anchor = from + 1`）；有选区包裹/反包裹 `*sel*`。
      - `formatCodeBlock`（`editor.codeBlock`，`⌥⌘C`）：
        - 无选区：在光标当前行下方插入 ```` \n```\n\n``` ````（若当前行为空行则直接在当前行插入 ```` ```\n\n``` ````），光标置于中间空行；
        - 有选区：前后包裹 ```` ```\n${sel}\n``` ````。
      - `formatTable`（`editor.table`，`⌥⌘G`）：插入 `| 列1 | 列2 |\n| --- | --- |\n|  |  |`，选中首格 `列1` 便于直接键入覆盖。
      - `formatLink`（`editor.link`，`⌥⌘K`）：
        - 无选区：插入 `[](url)`，光标落入 `[]` 内；
        - 有选区且选区匹配 `/^https?:\/\//i` 或 `mailto:`：插入 `[](${sel})`，光标落入 `[]` 内；
        - 有选区（普通文本）：插入 `[${sel}]()`，光标落入 `()` 内。
      - `formatImage`（`editor.image`，`⌥⌘I`）：插入 `![]()`，光标落入 `[]` 内（有选区时放入 alt `![${sel}]()` 并落入 `()` 内）。
      - `formatBulletList`（`editor.bulletList`，`⌥⌘L`）：选区覆盖的各行切换 `- ` 前缀（若所有非空行均已有 `- ` 则移除，否则补加）。
      - `formatBlockquote`（`editor.blockquote`，`⌥⌘Q`）：选区覆盖的各行切换 `> ` 前缀。
      - `formatHeading(level: 1 | 2 | 3 | 4 | 5 | 6)`（`editor.heading1`..`6`，`⌥⌘1..6`）：将当前行设为 `#`..`######` 标题（已有其他级别则替换级别；若已是相同级别则移除 `#` 恢复普通段落）。
    - 导出 `executeEditorFormatAction(view: EditorView, actionId: string, indentConfig: IndentConfig): boolean` 统一分发入口。
  - **4. 将扩展、统一动作注册表与同源右键菜单接入 `MarkdownEditor.vue`**：
    - 在 `createEditorState` 中挂载：
      - `todoCheckboxPlugin`（行首只读 `Decoration.widget`）；
      - 高优先级 `keymap`（`Tab`、`Shift-Tab`、`Enter` 待办续写、`Backspace` 空待办清除；格式动作统一经 `actionRegistry` 分发，确保用户在设置窗口自定义改键立即生效且遵循 `C7.3` 作用域优先级！）；
    - 在 `MarkdownEditor.vue` `onMounted` / `onUnmounted` 中调用 `actionRegistry.bindHandler(...)` 绑定全部 `editor.bold`、`editor.italic`、`editor.codeBlock`、`editor.table`、`editor.toggleTodo`、`editor.link`、`editor.image`、`editor.bulletList`、`editor.blockquote`、`editor.heading1`..`6`、`editor.indent`、`editor.outdent` 到当前活跃 `EditorView`（**注意**：同步打通顶部原生 macOS「格式」系统菜单经 `onMenuAction` 触发执行！）；
    - 在 `MarkdownEditor.vue` 监听 `contextmenu` 事件打开 `QnContextMenu`：
      - 菜单项直接由 `actionRegistry.getMenuItems('format', ctx)` + `actionRegistry.getMenuItems('editor-context', ctx)` 同源生成（包含常用格式、切换待办、收件箱有选区时的“移到新笔记…”），每项自动显示最新自定义快捷键文案！
- 不做：
  - 不实现语法树链接装饰、`⌘`+单击锚点跳转或图片粘贴/拖入落盘 `assets/`（全部归下一张卡 `M2-05b`）。
  - 不安装新 npm 依赖（`@codemirror/state`、`@codemirror/view`、`@codemirror/language`、`@codemirror/commands` 已在 `package.json` 就绪）。

## 2. 允许修改文件白名单
- 共享串行文件：无
- 前端业务模块：
  - `src/features/editor/extensions/markdownIndent.ts`（新增）
  - `src/features/editor/extensions/markdownTodo.ts`（新增）
  - `src/features/editor/extensions/markdownFormat.ts`（新增）
  - `src/features/editor/MarkdownEditor.vue`
  - `src/features/editor/index.ts`
- 前端单元测试：`src/features/editor/__tests__/cm6-markdown-editing.test.ts`（新增）

## 3. 具体实现要求
- **零裸色值与魔法 `z-index`（`C7.1`）**：复选框 Widget 样式在 `EditorView.theme(...)` 或 `<style scoped>` 中仅使用 `--qn-*` CSS Token（例如 `accentColor: 'var(--qn-accent)'`、`borderColor: 'var(--qn-border-strong)'`）。
- **IME 组合安全**：所有自定义 `keydown` / `Enter` / `Backspace` / `Tab` 逻辑遇到 `event.isComposing` 或 `view.composing` 时必须直接放行，绝不吞掉中文拼音选词按键！
- **无 `any` 与单测纯净**：全部命令基于 CM6 `EditorState` / `StateCommand`（`{ state, dispatch }`）编写，无需真实浏览器即可在 Vitest 中直接测试文档变更与选区游标位置。

## 4. 自动化验收标准（必须全绿）
- 新增测试文件：`src/features/editor/__tests__/cm6-markdown-editing.test.ts`，覆盖：
  1. **上下文 Tab / ⇧Tab 缩进**：
     - 普通段落按 `Tab` 插入 2 空格（绝不插 4 空格误造代码块）；
     - 无序列表 `- item` 按 `Tab` 缩进 2 格、`1. item` 按 `Tab` 缩进 3 格、`10. item` 按 `Tab` 缩进 4 格；
     - 多行列表选区按 `Tab` / `⇧Tab` 整块缩进/反缩进；
     - 引用块 `> quote` 按 `Tab` 变 `> > quote`、`⇧Tab` 恢复 `> quote`；
     - 围栏代码块 ```` ```js\nconst a = 1\n``` ```` 内按 `Tab` 插入 4 空格；
     - 非 Markdown 文件自动侦测 `\t` 或 4 空格，不套用 Markdown 引用/列表规则。
  2. **快速待办与行首 Widget**：
     - 验证待办行源文本保持 `- [ ] 任务` 不被替换，Widget 装饰点位于行首 `side: -1`（非 `replace` 装饰）；
     - 验证 `toggleTodoCommand` 三态循环：`任务` → `- [ ] 任务` → `- [x] 任务` → `任务`；
     - 验证 `todoEnterCommand`：非空待办 `  - [ ] 写单测|` 回车续写 `  - [ ] `；空待办 `  - [ ] |` 回车清除前缀；
     - 验证 `todoBackspaceCommand`：空待办 `- [ ] |` 退格一键清空前缀。
  3. **全套格式动作**：
     - 无选区 `formatBold` 插入 `****` 且光标位于索引 `2`；有选区包裹与再次触发反包裹；
     - 无选区 `formatItalic` 插入 `**` 且光标位于索引 `1`；
     - 无选区 `formatCodeBlock` 在下方插入围栏且光标位于中间空行；有选区前后包裹；
     - `formatTable` 插入 3 行 Markdown 表格且选中表头首格；
     - `formatLink` 无选区插 `[](url)`、选区为 `https://example.com` 插 `[](https://example.com)`、选区为普通文本插 `[文本]()`；
     - `formatImage` 插 `![]()`、`formatBulletList`、`formatBlockquote`、`formatHeading(1..6)` 切换与取消。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] 在主窗口打开 Markdown 笔记，切换中文拼音输入法输入待办内容 `- [ ] 你好世界`：行首显示可点击复选框，拼音候选词与光标位置无任何错位；点击复选框原地切换 `[x]`。
- [ ] 使用顶部 macOS 系统菜单「格式 > 加粗 / 代码块 / 插入表格」及编辑器右键菜单，确认均可生效且快捷键标签与设置窗口一致。
