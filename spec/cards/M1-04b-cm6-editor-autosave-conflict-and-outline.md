# M1-04b：CodeMirror 6 单实例多状态编辑器、600ms/2s 自动保存、三态外部变更横幅与右侧大纲面板

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.2、§3.4、§3.9（G/H）、§4.4、§5.5、§5.6.2、§5.7、§6 M1；`C1.2`、`C1.3`、`C2.2`、`C4.2`、`C4.3`、`C6.1`–`C6.3`、`C7.1`、`C7.4`、`C8.4`、`C8.5`、`C9.1`、`D-0006`、`D-0007`
- 前置依赖：`M1-04a`
- 会话策略：独立新会话执行（零大文档读取，所有契约已内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - 安装 CodeMirror 6 核心基础包（`@codemirror/state`、`@codemirror/view`、`@codemirror/commands`、`@codemirror/language`、`@codemirror/lang-markdown`、`@lezer/highlight`），基于 `--qn-*` CSS Token 定制浅/深双主题编辑器样式与阅读最大宽度（`--qn-editor-max-width`，绑定 `settings.editor.maxWidth` 默认 `900px` 居中）。
  - 在 `src/features/editor/` 落地 **CM6 单一 `EditorView` + 每标签独立 `EditorState` 编辑器**：
    - 切换标签通过 `view.setState(tabState)` 实现零销毁瞬切，独立保存并恢复每个标签的 `scrollTop` 与 `cursorPos`；
    - 编辑器仅渲染与修改 `NoteDoc.body`（**绝不显示 YAML frontmatter**，`D-0006`）；用户输入产生文档变更时，若当前标签为预览标签则自动晋升为固定标签（`isPreview = false, isPinned = true`）并标记 `isDirty = true`。
    - 状态栏实时展示：保存状态（已保存 / 未保存 / 保存中 / 冲突 / 已同步外部修改）、字数统计、光标 `Ln/Col`。
  - 落地 **600ms 防抖 / 2000ms 最大等待自动保存**与 ULID 补写回填：
    - 编辑变更后按 `settings.editor.autoSaveDebounceMs`（默认 `600ms`）防抖、连续打字最长 `autoSaveMaxWaitMs`（默认 `2000ms`）强制调用 `noteWrite({ absPath, body, baseBodyHash })`，切标签或窗口失焦/隐藏前立即冲刷未保存改动，也支持 `⌘S` 立即保存。
    - 写盘成功后更新当前标签 `baseBodyHash = res.bodyHash`、`isDirty = false`；若 `res.updatedMeta` 非空（旧笔记首次保存自动补写了 ULID，`D-0006`），无感更新标签内存 `meta` 与 `key`。
  - 落地 **外部变更三态处理（`C8.5`）**：监听 `onFsExternalChange`（`fs:external-change` 事件）：
    - **干净态（`!isDirty` 且 `kind == "modified"`）**：静默使用事件携带的 `newBody`、`newBodyHash`、`newMeta` 重置该标签 `EditorState`，尽量保持原 `scrollTop` 与光标相对位置，状态栏微提示“已同步外部修改”。
    - **脏态（`isDirty` 且 `kind == "modified"`）**：立即**暂停该标签自动保存**，在编辑器顶部展示冲突横幅，提供三个动作：① **重新加载磁盘版本**（放弃本地未保存修改，载入 `newBody`/`newBodyHash`）；② **备份并覆盖磁盘**（以最新磁盘 `newBodyHash` 为 `baseBodyHash` 强制写回本地 `body`，由后端或前置备份机制保障安全）；③ **暂缓处理**（关闭横幅保持手动编辑）。
    - **外部删除（`kind == "deleted"`）**：**绝不自动关闭标签或清空编辑器正文**；暂停自动保存，标签与文档头标红警告“磁盘文件已丢失”，横幅提供“写回原路径”（调用 `noteWrite` 重新落盘）与“关闭标签”。
  - 在 `src/features/panels/` 落地右侧默认**大纲面板（`outline`，默认宽 `260px`）**：
    - 提取当前激活文档 `body` 中围栏代码块外的 `H1..H6` 标题（剥离行内 `**`、`*`、`` ` ``、`[text](url)` 修饰符），计算行号、字符偏移与 `C8.4` 中文 Slug（全小写、空格转 `-`、去标点保留中英文与数字、重复标题追加 `-1`/`-2`），编辑后 **`150ms` 防抖**更新。
    - 随编辑器视口滚动高亮当前所处章节；点击大纲项滚动编辑器并聚焦到对应标题行；支持 `⌃⌘O` 切换大纲面板、`⌃⌘0` 折叠/展开右侧面板组，并将 `panelGroup` 状态持久化至 `uiStateSet`。
- 不做：
  - 不实现 Markdown 格式快捷键（`⌘B`/`⌘I`/`⌥⌘C`）、上下文 Tab 缩进、行首复选框 widget、图片粘贴落盘或文档头 `[属性]` Popover 完整表单（全部归 `M2`）。
  - 不实现右侧 `related`、`recent`、`terminal` 面板（归 `M3`/`M4`）或 CM6 `MergeView` 并排对比（归 `M4`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`package.json`、`pnpm-lock.yaml`
- 前端业务模块：`src/features/editor/**`、`src/features/panels/**`、`src/features/notes/**`（仅限与编辑器标签联动的最小调整）、`src/app/MainWorkbenchShell.vue`
- 前端单元测试：`src/features/editor/__tests__/**`、`src/features/panels/__tests__/**`

## 3. 具体实现要求
- **严格遵守前端分层与 Token 规范（`C1.2`、`C7.1`）**：零 `@tauri-apps/api` 直连、零跨 feature 深层导入、零裸十六进制色值（含 CM6 `EditorView.theme` 必须全部引用 `var(--qn-*)`）、零魔法 `z-index`。
- **Headless / Vitest 可测性**：将大纲提取、自动保存调度器与三态外部变更状态机封装在纯 TS 模块 / Pinia Store 中，确保在 Vitest（Node / jsdom）环境下无需真实浏览器布局即可 100% 机测覆盖。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：
  - `src/features/editor/__tests__/editor-autosave-and-conflict.test.ts`：验证编辑晋升固定标签、600ms 防抖与 2000ms 最大等待自动保存、`updatedMeta` 回填更新标签 `key`、以及收到 `fs:external-change` 时干净态静默重载 / 脏态暂停自动保存并弹冲突横幅（重载 vs 覆盖） / 删除态标红保留正文。
  - `src/features/panels/__tests__/outline-panel.test.ts`：验证 150ms 防抖提取 `H1..H6`（跳过围栏代码块内 `#` 注释）、剥离行内修饰符、中文 Slug 去重（`-1`、`-2`）、视口滚动当前章节高亮计算，以及 `⌃⌘O` / `⌃⌘0` 面板折叠状态切换。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] 启动主窗口，单击笔记打开预览标签，在 CM6 编辑器中输入文字：标签自动由斜体变为固定标签，600ms 后状态栏变为“已保存”，右侧大纲面板在 150ms 内同步刷新标题层级，点击大纲可跳转对应行。
- [ ] 在外部终端修改当前干净态笔记：主窗口无感刷新最新正文；在编辑器输入未保存文字时再从外部修改该文件：顶部立即弹出冲突横幅且暂停自动保存。
