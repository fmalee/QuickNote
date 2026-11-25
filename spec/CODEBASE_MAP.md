# CODEBASE_MAP · QuickNote 省流代码地图

> 面向 AI 维护/迭代会话的 3 秒精准定位索引（不写易过期的行号，仅保留模块职责、文件反查、修改菜谱与实机暗坑）。动手前配合 `AGENTS.md` 阅读，无需加载 1150 行的 `quicknote-design-v4.md` 或 745 行的 `engineering-contract.md`。

---

## 1. 单向分层架构与多窗口入口

- **Rust 分层（`C1.2`）**：`commands` → `application` → `domain`，`infra` → `domain`
  - `src-tauri/src/domain/`：纯结构体与纯算法，**严禁依赖** `tauri`、`std::fs`、`rusqlite`、`portable_pty`、`libc`。
  - `src-tauri/src/infra/`：文件系统原子写入、SQLite、macOS ObjC/Carbon/WebKit 桥接、`portable-pty`、`notify`、`mdfind`。
  - `src-tauri/src/application/`：业务用例、并发状态机、持久化与事件协调。
  - `src-tauri/src/commands/`：薄 Tauri IPC 命令入口；全量注册于 `src-tauri/src/lib.rs`。
- **前端分层（`C1.2`）**：`lib` → `components` → `features` → `app`
  - `src/bindings/types.ts`：**自动生成禁止手改**（首行 `/* GENERATED — DO NOT EDIT. */`），唯一来源为 `src-tauri/src/bindings.rs`。
  - `src/lib/ipc/`：唯一允许直接导入 `@tauri-apps/api/*` 的目录（`commands.ts`、`events.ts`）。
  - 跨 feature 只能通过 `@/features/<name>`（`index.ts`）导入，禁止深层导入。
- **多窗口路由（`src/app/App.vue` + `src-tauri/src/lib.rs`）**：
  - `index.html` → 主工作台（`MainWorkbenchShell.vue`）
  - `index.html?window=quickbar` → Quick Bar 原生 `NSPanel`（`QuickBarView.vue`）
  - `index.html?window=toast` → 5 秒非激活撤销 Toast `NSPanel`（`ToastView.vue`）
  - `index.html?window=settings` → 独立设置窗口（`SettingsView.vue`）

---

## 2. 按功能域反查文件表（改什么去哪找）

| 功能域 | 前端核心文件 | Rust 后端核心文件 | 对应测试文件 |
| --- | --- | --- | --- |
| **Quick Bar 捕捉 / 打开 / 命令三模式** | `src/features/quickbar/` (`QuickBarView.vue`, `store.ts`, `QuickBarTargetPicker.vue`) | `domain/capture.rs`, `application/capture.rs`, `commands/capture.rs` | `src/features/quickbar/__tests__/*`, `src-tauri/tests/capture_truth_table_test.rs`, `append_and_undo_test.rs` |
| **全局 5 秒非激活撤销 Toast** | `src/features/toast/` (`ToastView.vue`, `store.ts`) | `application/window.rs`, `infra/macos/bridge.m` (`QuickNoteToastPanel`) | `src/features/toast/__tests__/toast-view.test.ts` |
| **主工作台壳、多标签、菜单同步与 45% 辅助窗** | `src/app/MainWorkbenchShell.vue`, `src/features/editor/EditorTabBar.vue` | `domain/window.rs`, `application/window.rs`, `commands/window.rs`, `infra/macos/bridge.m`, `src-tauri/src/lib.rs` | `src/app/__tests__/auxiliary-window-mode.test.ts`, `src-tauri/tests/window_policy_test.rs`, `m4_export_and_auxiliary.rs` |
| **笔记导航树、标签树、列表、模板新建与批量重命名标签** | `src/features/notes/` (`NotesSidebar.vue`, `store.ts`, `NewNoteTemplateModal.vue`, `templateUtils.ts`) | `domain/note.rs`, `domain/template.rs`, `application/notes.rs`, `application/index.rs`, `commands/notes.rs` | `src/features/notes/__tests__/*`, `src-tauri/tests/note_crud_and_index_test.rs`, `m3_commands_related_tags.rs` |
| **CM6 编辑器、文档头、`[属性]` Popover 与收件箱提取** | `src/features/editor/` (`MarkdownEditor.vue`, `store.ts`, `DocHeaderBar.vue`, `DocPropertiesPopover.vue`, `InboxBloatBanner.vue`, `InboxExtractModal.vue`) | `domain/frontmatter.rs`, `domain/markdown.rs`, `application/notes.rs`, `commands/notes.rs` | `src/features/editor/__tests__/doc-meta-and-inbox-extract.test.ts`, `editor-autosave-and-conflict.test.ts`, `src-tauri/tests/m2_backend_test.rs` |
| **CM6 扩展（缩进/待办/格式/链接/图片/多语言高亮与诊断）** | `src/features/editor/extensions/` (`markdownIndent.ts`, `markdownTodo.ts`, `markdownFormat.ts`, `markdownLinks.ts`, `markdownImages.ts`, `codeLanguages.ts`, `codeDiagnostics.ts`, `scrollTarget.ts`) | `domain/markdown.rs`（中文 Slug、base64 图片解码）, `application/notes.rs`（`note_save_image`） | `src/features/editor/__tests__/cm6-markdown-editing.test.ts`, `cm6-links-and-images.test.ts`, `code-languages-and-diagnostics.test.ts` |
| **外部修改监听、防回环、冲突横幅与 CM6 `MergeView`** | `src/features/editor/` (`EditorConflictBanner.vue`, `EditorMergeView.vue`, `store.ts`) | `domain/watch.rs`, `application/watch.rs`, `infra/vault_watch.rs` | `src/features/editor/__tests__/merge-view-conflict.test.ts`, `merge-view-real-smoke.test.ts`, `src-tauri/tests/self_write_guard_and_watch_test.rs` |
| **内嵌 `xterm.js` 终端、折叠保活与 30 分钟空闲回收** | `src/features/terminal/` (`TerminalPanel.vue`, `store.ts`, `theme.ts`, `api.ts`) | `domain/terminal.rs`, `infra/pty.rs`, `application/terminal.rs`, `commands/terminal.rs` | `src/features/terminal/__tests__/terminal-panel.test.ts`, `src-tauri/tests/m4_terminal_lifecycle.rs` |
| **右侧面板组（大纲 / 终端 / 关联前 20 / 最近前 20）** | `src/features/panels/` (`RightPanelGroup.vue`, `store.ts`, `registry.ts`, `OutlinePanel.vue`, `RelatedNotesPanel.vue`, `RecentNotesPanel.vue`, `outlineExtractor.ts`) | `application/notes.rs`（`notes_get_related`） | `src/features/panels/__tests__/*`, `src-tauri/tests/m3_commands_related_tags.rs` |
| **快速打开（`⌘K`）、路径补全与可取消 Spotlight** | `src/features/quickopen/` (`QuickOpenPalette.vue`, `useQuickOpen.ts`, `queryParser.ts`) | `infra/spotlight.rs`, `application/external.rs`, `application/notes.rs`, `commands/search.rs` | `src/features/quickopen/__tests__/quick-open-engine.test.ts`, `src-tauri/tests/m3_spotlight_path_external.rs` |
| **命令书解析与命令库面板（`⌘⇧K`）** | `src/features/commands/` (`CommandBookPalette.vue`, `useCommandPalette.ts`) | `domain/command_book.rs`, `application/index.rs`, `commands/search.rs` | `src/features/commands/__tests__/command-book-palette.test.ts`, `src-tauri/tests/m3_commands_related_tags.rs` |
| **外部动作（Finder / 外部终端 / VS Code / Sublime / 固定）** | `src/features/editor/useExternalActions.ts` | `domain/external.rs`, `infra/external_open.rs`, `application/external.rs`, `commands/external.rs` | `src/features/editor/__tests__/external-actions-integration.test.ts`, `src-tauri/tests/m3_spotlight_path_external.rs` |
| **复制富文本 HTML 与离屏 `WKWebView` 导出 PDF** | `src/features/editor/export/markdownExport.ts`, `ExportPdfModal.vue` | `commands/notes.rs` (`note_export_pdf`), `infra/macos/bridge.m` (`qn_render_html_to_pdf`), `infra/macos/mod.rs` | `src/features/editor/__tests__/export-rich-text-and-pdf.test.ts`, `src-tauri/tests/m4_export_and_auxiliary.rs` |
| **统一动作注册表、快捷键作用域分发与右键菜单** | `src/lib/actions/` (`catalog.ts`, `registry.ts`, `keymap.ts`, `types.ts`), `src/components/QnContextMenu.vue` | `commands/app.rs` (`app_set_menu_accelerators`), `src-tauri/src/lib.rs`（原生系统菜单） | `src/lib/actions/__tests__/action-registry.test.ts` |
| **设置窗口、配置备份导入导出、`settings.yaml` 容错与首次选库** | `src/features/settings/` (`SettingsView.vue`, `store.ts`, 各子面板), `src/features/onboarding/` | `domain/settings.rs`, `domain/ui_state.rs`, `application/persistence.rs`, `infra/settings_watch.rs`, `commands/settings.rs`, `commands/library.rs` | `src/features/settings/__tests__/*`, `src-tauri/tests/persistence_test.rs`, `m4_export_and_auxiliary.rs` |
| **SQLite FTS5 混合分词索引与性能压测** | — | `domain/tokenizer.rs`, `infra/sqlite.rs`, `src-tauri/migrations/0001_init.sql`, `application/index.rs` | `src-tauri/tests/domain_note_and_tokenizer_test.rs`, `sqlite_migration_test.rs`, `m4_performance_budget.rs` |
| **设计 Token 与基础通用组件** | `src/styles/tokens.css`, `src/components/Qn*.vue` | — | `scripts/__tests__/check-architecture.test.ts` |

### v5 Refactor · Phase A 新增模块

| 模块路径 | 职责一句话说明 |
| --- | --- |
| `src-tauri/src/domain/bookmark.rs` | 外部文件/目录书签领域模型与纯内存规范化、去重及排序规则 |
| `src-tauri/src/domain/workspace_context.rs` | 笔记库目录级 `.quicknote.yaml` 工作区配置模型与三级级联解析规则 |
| `src-tauri/migrations/0002_directory_workspace_cache.sql` | 只追加 SQLite 迁移，新增 `directory_workspace_configs` 目录级工作区派生缓存表 |
| `src-tauri/src/application/reconciler.rs` | 只读 Vault 全量/增量对账引擎，将磁盘 `.md` 与 `.quicknote.yaml` 单向收敛至 SQLite 且不改写用户文件 |
| `src-tauri/src/infra/fs_browser.rs` | 纯 Rust 受控外部文件系统探测与目录直扫引擎（带深度/条目数熔断，支持隐藏目录与系统路径） |
| `src/features/editor/core/docSession.ts` | 与框架解耦的编辑器标签轻量元数据桩 `DocTabStub`（仅存元数据与光标/滚动快照，不存全文与 CM6 实例） |
| `src/features/editor/core/editorPool.ts` | 与框架解耦的泛型 `EditorStatePool<TState>` LRU 状态池，严格保护激活态与脏数据标签不被淘汰 |

---

## 3. 高频改动操作菜谱（最少文件集）

1. **新增 / 修改 IPC 命令**：
   1. `src-tauri/src/domain/<mod>.rs` 定义结构体（`Serialize, Deserialize, Type` + `#[serde(rename_all = "camelCase")]`）
   2. `src-tauri/src/bindings.rs` 注册新类型 → 运行 `pnpm gen:bindings` 更新 `src/bindings/types.ts`
   3. `src-tauri/src/application/` + `src-tauri/src/commands/` 实现并在 `src-tauri/src/lib.rs` `invoke_handler` 注册
   4. `src/lib/ipc/commands.ts`（或 `events.ts`）添加前端强类型封装
2. **新增前端动作 / 快捷键 / 原生菜单项**：
   1. `src/lib/actions/catalog.ts` 注册 `ActionDef`（明确 `scope`、`when`、`swallowWhenDisabled`）
   2. 若需挂入 macOS 顶部菜单：在 `src-tauri/src/lib.rs` `build_app_menu` 添加同 `id` 菜单项并在 `setup_tray_and_menu` 放行转发至 `menu:action`；若支持改键同步菜单，加入 `src/lib/actions/registry.ts::NATIVE_MENU_ACTION_IDS`
   3. 在 `src/app/MainWorkbenchShell.vue` 绑定 `actionRegistry.bindHandler(id, ...)`
3. **修改 UI 样式**：
   - 仅使用 `src/styles/tokens.css` 已定义的 `--qn-*` CSS Token（禁硬编码 hex/rgba 色值与魔法 `z-index`）；新增 Token 必须同时补齐浅色、`:root[data-theme='dark']` 和 `@media (prefers-color-scheme: dark)` 三处。

---

## 4. 已验证实机暗坑与不可破红线（动手前必看）

1. **macOS `WKWebView` 剪贴板粘贴气泡**：调用 `navigator.clipboard.readText()` 会在鼠标处弹出系统原生 `Paste` 确认气泡。终端粘贴严禁拦截 `⌘V` 调 `readText()`，必须放行给浏览器原生 `paste` 事件流向 xterm textarea。
2. **`@xterm/xterm` v6 自绘滚动条**：使用 VS Code `DomScrollableElement` 自绘 `.xterm-scrollable-element > .scrollbar`（非原生 `::-webkit-scrollbar`），隐藏滚动条须同时覆盖该 DOM 选择器。
3. **原生 `NSPanel` 零私有 API**：Quick Bar 与 Toast 严禁开 Tauri `.transparent(true)`；保持 `collectionBehavior = 256`（`fullScreenAuxiliary`，不含 `canJoinAllSpaces`），点击外部不关、切 Space 自动复位。
4. **全屏 ↔ 45% 侧栏辅助窗异步竞态**：原生全屏退全屏是异步动画，`bridge.m` 仅在非全屏态保存标准 frame，且通过一次性 `NSWindowDidExitFullScreenNotification` 观察者等待动画完成后再贴靠右侧 45%。
5. **macOS `Option` 组合键**：`⌥⌘C` / `⌥⌘1` 等组合键会让 `KeyboardEvent.key` 变特殊字符，快捷键匹配与录制一律优先基于 `event.code` 归一化。
6. **外链与文件拖放**：`WKWebView` 拦截 `window.open`，外链必须走参数化 Rust `app_open_external_url`；Finder/截图拖入走 `onWindowFileDrop`。
7. **`meta + body` 分离与坐标系**：编辑器内所有行号（大纲 / 收件箱提取 / 命令书 `line`）均为剥离 frontmatter 后的 `NoteDoc.body` 1-based 行号；`note_write` 绝不改写 YAML（缺失 ULID 补写为唯一例外）。
8. **Rust 与安全底线**：`src-tauri/src/**` 零 `.unwrap()`/`.expect()`；所有测试落位 `src-tauri/tests/**`；外部命令严禁 shell 拼接；日志严禁打印笔记正文、剪贴板、终端字节流或 PDF HTML；`git commit` / `git merge` 必须带 `--no-gpg-sign`；收工必过 `pnpm check:all`。
