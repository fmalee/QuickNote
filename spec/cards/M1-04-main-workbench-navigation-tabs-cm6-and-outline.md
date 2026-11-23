# M1-04：两层主界面（导航树 / 标签树 / 收件箱 500 行提醒 / 多标签）与 CM6 编辑器 + 右侧大纲面板

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.2–§3.4、§3.9（F/G/H）、§4.6、§4.11、§5.5、§5.6.2、§5.7、§5.9、§6 M1；`C1.2`、`C1.3`、`C2.2`、`C4.2`、`C4.3`、`C6.2`、`C6.3`、`C7.1`、`C7.4`、`C8.4`、`C8.5`、`C9.1`、`D-0006`、`D-0007`、`D-0009`
- 前置依赖：`M1-03`
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - 安装 CodeMirror 6 核心基础依赖（`@codemirror/state`、`@codemirror/view`、`@codemirror/commands`、`@codemirror/language`、`@codemirror/lang-markdown`、`@lezer/highlight`），严守 `--qn-*` CSS Token 定制浅/深双主题编辑器样式与阅读最大宽度（`--qn-editor-max-width`，默认 `900px` 居中）。
  - 在 `src/features/notes/` 落地主界面**第一层（导航 + 列表）**：
    - 顶部切换“收件箱 / 项目 / 标签 / 最近 / 工作区”，调用 `notes_get_tree_and_tags` 与 `notes_list` 渲染目录树、派生标签树（支持 `a/b` 层级展开与多标签 AND 筛选）及笔记卡片列表（标题、摘要、标签、相对时间、置顶图标、`metaError` 徽标）。
    - 支持列表筛选框（`80ms` 防抖 + 递增 `requestId` 防竞态，列表聚焦时 `⌘F` 激活过滤）、新建笔记（`note_create`）、删除笔记（`note_delete`）与双击/操作菜单重命名（调用 `note_rename`，同步更新已打开标签页的 `relPath`）。
    - **收件箱 500 行被动膨胀提醒（`v4` §4.11、`C6.2`）**：当打开 `inbox.md` 且行数超过 `settings.inbox.bloat_threshold_lines`（默认 `500` 行）时，在顶部显示温和非阻塞提醒横幅“收件箱已超过 500 行，建议整理”。
  - 在 `src/features/editor/` 落地主界面**第二层（多标签 + 文档头 + CM6 单实例多状态编辑器）**：
    - 多标签栏：单击列表项打开**预览标签（斜体）**，双击、编辑或回车转为**固定标签**；支持关闭标签、关闭其他、`⌘1…9` 切换标签、`⌘⇧T` 从 `closedStack` 重开最近关闭标签。
    - 单一 `EditorView` 实例搭配每标签独立 `EditorState`，切换标签通过 `view.setState()` 瞬切并独立缓存恢复 `scrollTop` 与 `cursorPos`；编辑器仅渲染 `NoteDoc.body`（**绝不显示 YAML frontmatter**，`D-0006`）。
    - 自动保存与防回环协同：变更后 `600ms` 防抖、持续打字最长 `2000ms` 强制调用 `note_write`（若返回 `updatedMeta` 则无感更新内存 `meta` 與标签 `key`），切标签或失焦前冲刷保存；脏态下每 2 秒触发崩溃恢复快照保护。
    - 外部变更三态 UI（`C8.5`）：监听 `fs:external-change`，**干净态（`!isDirty`）**静默重置编辑器 `body`/`meta` 并保持原滚动位置，状态栏提示“已同步外部修改”；**脏态（`isDirty`）**立即暂停自动保存并弹出冲突横幅（提供“重新加载磁盘版本”“备份并覆盖磁盘”“暂缓处理”按钮）；**外部删除（`kind == "deleted"`）**绝不关闭标签，保留正文并标红警告“磁盘文件已丢失”，提供“写回原路径 / 关闭标签”。
  - 在 `src/features/panels/` 落地右侧默认**大纲面板（`outline`，默认宽 `260px`，`⌃⌘O` 切换 / `⌃⌘0` 折叠面板组）**：
    - 基于 CM6 `syntaxTree(state)`（或 Markdown 标题解析器）提取 `H1..H6` 标题文本、行号与 `C8.4` 中文 Slug，编辑后 **`150ms` 防抖**刷新；随编辑器视口滚动自动高亮当前章节，点击大纲项滚动聚焦到对应标题行；非 Markdown 或无标题时展示空状态。
  - 状态恢复与搬库重定位（`v4` §5.9、`C6.3`）：主窗口启动或搬库后从 `ui-state.json` 恢复 `tabs`、`filters`、`panelGroup` 与 `lastActiveDoc`（优先匹配 `relPath`，找不到则按 `id` 匹配，彻底缺失则显示失联占位标签）。
- 不做：
  - 不实现 Markdown 格式快捷键（`⌘B`/`⌘I`/`⌥⌘C`）、上下文 Tab 缩进、行首 `Decoration.widget` 待办复选框、图片粘贴落盘、文档头 `[属性]` Popover 完整表单或收件箱“移到新笔记…”（全部归 `M2`）。
  - 不实现右侧 `related`、`recent`、`terminal` 三个面板的具体内容（`related`/`recent` 归 `M3`，`terminal` 归 `M4`）。
  - 不实现 CM6 `MergeView` 并排对比视图（归 `M4`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`package.json`、`pnpm-lock.yaml`
- 前端工具与组件：`src/lib/utils/**`、`src/components/**`
- 前端业务模块：`src/features/notes/**`、`src/features/editor/**`、`src/features/panels/**`、`src/app/MainWorkbenchShell.vue`、`src/app/bootstrapStore.ts`
- 前端单元测试：`src/features/notes/__tests__/**`、`src/features/editor/__tests__/**`、`src/features/panels/__tests__/**`

## 3. 具体实现要求
- **严格遵守前端分层与 Token 规范（`C1.2`、`C7.1`）**：`src/components/**` 与各 `.vue` 组件严禁直接导入 `@tauri-apps/api`；跨 feature 仅通过顶层 `src/features/<name>/index.ts` 导入；所有样式 100% 使用 `var(--qn-*)` CSS 变量，禁止任何十六进制裸色值或硬编码数字 `z-index`。
- **预览标签 vs 固定标签状态机**：全局最多同时存在 1 个未固定的预览标签（`isPreview: true`）；单击另一篇笔记时直接替换当前预览标签；一旦用户双击列表/标签头或产生正文编辑（`isDirty`），该预览标签立即晋升为固定标签（`isPreview: false, isPinned: true`）。
- **防竞态与防抖（`C2.2`）**：笔记列表搜索过滤必须带 `80ms` 防抖与递增 `requestId` 校验；大纲重提取必须带 `150ms` 防抖；自动保存严格遵守 `settings.editor.auto_save_debounce_ms`（默认 `600ms`）与 `auto_save_max_wait_ms`（默认 `2000ms`）。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：
  - `src/features/notes/__tests__/notes-tree-and-list.test.ts`：验证目录树/标签树筛选、多标签 AND 过滤、`inbox.md` 超 500 行膨胀提醒横幅显隐、列表重命名 `note_rename` 联动更新打开标签。
  - `src/features/editor/__tests__/tabs-and-external-change.test.ts`：验证预览标签替换与编辑晋升固定标签、`⌘⇧T` 恢复关闭标签、600ms/2s 自动保存与 `updatedMeta` 回写、收到 `fs:external-change` 时干净态静默重载 / 脏态弹冲突横幅且暂停自动保存 / 删除态标红保留正文、以及 `ui-state.json` 标签恢复与 `id` 兜底重定位。
  - `src/features/panels/__tests__/outline-panel.test.ts`：验证 150ms 防抖提取 `H1..H6` 标题、剥离行内修饰符、中文 Slug 生成及视口滚动高亮计算。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] 启动主窗口，在左侧切换“收件箱 / 项目 / 标签 / 最近”，单击笔记以斜体预览标签打开、输入文字自动晋升为固定标签并在 600ms 后状态栏显示“已保存”。
- [ ] 打开超过 500 行的 `inbox.md`，确认顶部出现温和的“收件箱已超过 500 行，建议整理”提醒横幅；在右侧大纲面板点击各级标题，确认编辑器平滑滚动定位且 `⌃⌘O` / `⌃⌘0` 可折叠面板。
- [ ] 在外部编辑器修改当前打开的笔记并保存：确认主窗口干净态下无感同步新内容；在主窗口输入未保存文字的同时外部修改文件，确认顶部弹出冲突横幅。
