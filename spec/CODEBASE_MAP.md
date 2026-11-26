# CODEBASE_MAP · QuickNote 省流代码地图

> 面向 AI 维护/迭代会话的 3 秒精准定位索引（不写易过期的行号，仅保留当前真实目录树、核心模块职责、关键数据流、功能反查表、修改菜谱与实机暗坑）。动手前配合 `AGENTS.md` 阅读，无需整篇加载 `spec/quicknote-design-v5.md` 或 `spec/engineering-contract.md`。

---

## 1. 单向分层架构与前后端目录树

- **Rust 分层（`C1.2`）**：`commands` → `application` → `domain`，`application` → `infra`，`infra` → `domain`
  - `src-tauri/src/domain/`：纯结构体与纯算法，**严禁依赖** `tauri`、`std::fs`、`std::process`、`rusqlite`、`notify`、`portable_pty`、`libc`。
  - `src-tauri/src/infra/`：原子文件系统、SQLite、macOS ObjC/Carbon/WebKit 桥接、`portable-pty`、`notify`、`trash`（系统废纸篓）、`fs_browser`（纯 Rust 受控文件系统探测与目录直扫；Spotlight 主检索引擎已废止并移除）。
  - `src-tauri/src/application/`：业务用例、只读对账（`reconciler.rs`）、并发状态机、持久化（`config_dir` 与 `cache_dir` 隔离）与事件协调。
  - `src-tauri/src/commands/`：薄 Tauri IPC 命令入口；全量注册于 `src-tauri/src/lib.rs`。
- **前端分层（`C1.2`）**：`lib` → `components` → `features` → `app`
  - `src/bindings/types.ts`：**自动生成禁止手改**（首行 `/* GENERATED — DO NOT EDIT. */`），唯一来源为 `src-tauri/src/bindings.rs`。
  - `src/lib/ipc/`：唯一允许直接导入 `@tauri-apps/api/*` 的目录（`commands.ts`、`events.ts`）。
  - 跨 feature 只能通过 `@/features/<name>`（`index.ts`）导入，禁止深层导入。
- **多窗口路由与常驻内存模型（`src/app/App.vue` + `src-tauri/src/lib.rs`）**：
  - `index.html` → 主工作台（`MainWorkbenchShell.vue`，常驻；含 45% 侧栏辅助模式复用同一 WebView）
  - `index.html?window=quickbar` → Quick Bar 原生非激活 `NSPanel`（`QuickBarView.vue`，常驻）
  - `index.html?window=toast` → 5 秒非激活撤销 Toast `NSPanel`（`ToastView.vue`，按需创建并闲置销毁）
  - `index.html?window=settings` → 独立设置窗口（`SettingsView.vue`，按需创建并闲置销毁）

```text
src/
├── app/                     # App.vue, MainWorkbenchShell.vue, bootstrapStore.ts, main.ts
├── bindings/types.ts        # Rust -> TS 自动生成唯一契约（禁止手改）
├── components/              # 纯 UI 组件：QnBadge, QnBanner, QnButton, QnContextMenu, QnInput, QnModalShell, QnPanelCard
├── lib/
│   ├── actions/             # catalog.ts, registry.ts, keymap.ts, types.ts（统一动作注册与快捷键分发）
│   ├── ipc/                 # commands.ts, events.ts（唯一调用 @tauri-apps/api 的手写层）
│   └── utils/               # 纯前端工具函数
├── styles/tokens.css        # 浅/深双主题 --qn-* CSS Token
└── features/
    ├── commands/            # CommandBookPalette.vue, useCommandPalette.ts, api.ts
    ├── editor/              # MarkdownEditor.vue, EditorTabBar.vue, DocHeaderBar.vue, DocPropertiesPopover.vue,
    │                        # SiblingFilesPopover.vue, EditorConflictBanner.vue, EditorMergeView.vue,
    │                        # InboxBloatBanner.vue, InboxExtractModal.vue, ExportPdfModal.vue,
    │                        # useExternalActions.ts, store.ts, api.ts
    │                        # core/{docSession.ts, editorPool.ts} · export/markdownExport.ts · extensions/* (含 search.ts)
    ├── notes/               # NotesSidebar.vue, DirWorkspaceBindModal.vue, TagRenameModal.vue, useDirectoryWorkspaces.ts,
    │                        # NewNoteTemplateModal.vue, templateUtils.ts, store.ts, api.ts
    ├── onboarding/          # OnboardingView.vue, store.ts, api.ts
    ├── panels/              # OutlinePanel.vue, RelatedNotesPanel.vue, outlineExtractor.ts, registry.ts, store.ts, api.ts
    ├── quickbar/            # QuickBarView.vue, QuickBarTargetPicker.vue, store.ts, api.ts
    ├── quickopen/           # QuickOpenPalette.vue, useQuickOpen.ts, queryParser.ts, api.ts
    ├── settings/            # SettingsView.vue, SettingsLibraryPanel.vue, SettingsAppearancePanel.vue,
    │                        # SettingsExternalPanel.vue, SettingsShortcutsPanel.vue, SettingsWorkspacesPanel.vue,
    │                        # SettingsErrorBanner.vue, VisualBaselinePreview.vue, store.ts, api.ts
    ├── terminal/            # TerminalPanel.vue, theme.ts, store.ts, api.ts
    └── toast/               # ToastView.vue, store.ts, api.ts

src-tauri/
├── migrations/              # 0001_init.sql, 0002_directory_workspace_cache.sql（只追加）
├── src/
│   ├── lib.rs / main.rs / bindings.rs / bin/export_bindings.rs
│   ├── domain/              # app, backup, bookmark, capture, command_book, error, external, frontmatter,
│   │                        # hotkey, markdown, note, settings, template, terminal, tokenizer, ui_state,
│   │                        # watch, window, workspace_context
│   ├── infra/               # external_open, fs, fs_browser, macos/{bridge.m, mod.rs}, pty,
│   │                        # settings_watch, sqlite, trash, vault_watch
│   ├── application/         # capture, external, hotkey, index, library, notes/{mod,crud,nav,search,media,batch},
│   │                        # persistence, reconciler, terminal, watch, window
│   └── commands/            # app, capture, external, index, library, notes, search, settings, terminal, window
└── tests/                   # 全部 Rust 单元/集成测试 + fixtures/vault/ 共享静态样本库
```

---

## 2. 核心数据流与架构不变量

1. **唯一真相源与只读单向对账流**：
   - 磁盘上的 `.md` 笔记与目录级 `.quicknote.yaml` 是唯一真相源。
   - 启动时 `application/reconciler.rs` 执行只读全量/增量对账，运行期由 `infra/vault_watch.rs` + `application/watch.rs`（含自写防回环）持续收敛至 SQLite 派生缓存；无 Frontmatter `id` 的 `.md` 以 `path:<rel_path>` 确定性兜底键收录，扫描绝不改写用户文件。
2. **配置目录与缓存目录物理隔离**：
   - `application/persistence.rs` 将用户配置（`config_dir`：`settings.yaml`、`ui-state.json`（含 `leftSidebar` / `terminalPanel` / `backupPaths`）、`bookmarks.json`、`quickbar-draft.json`、`recovery/`、`backups/`）与可重建缓存库（`cache_dir`：按 Vault 路径哈希隔离的 `vaults/<vault_hash>.db` 及其 `-wal`/`-shm`）严格分离；SQLite 严禁落入笔记库根目录，且支持随时删库全量重建。
3. **目录级 `.quicknote.yaml` 工作区三级级联流**：
   - 废弃独立全局工作区，改为笔记库子目录 `.quicknote.yaml` 绑定（派生缓存于 `directory_workspace_configs` 表）。
   - 外部动作与终端 `cwd` 由 `domain/workspace_context.rs` 严格按 `文档 Frontmatter > 最近祖先目录 .quicknote.yaml > 全局默认` 三级级联解析；`.quicknote.yaml` 排除出笔记树与 FTS，应用内经 `set_directory_workspace_config` 写入时走原子写 + 自写守卫。
4. **纯 Rust 外部文件直扫与书签流**：
   - `infra/fs_browser.rs` 提供带深度与条目数熔断的受控路径探测与目录直扫（支持 `.xxx` 隐藏目录与 `/usr/local/etc` 等系统路径），驱动 `probe_external_path`、`list_external_directory`、`list_current_doc_siblings`（同级文件弹出层 `SiblingFilesPopover.vue`）与外部书签（`domain/bookmark.rs`、`upsert_external_bookmark`、`remove_external_bookmark`）。
5. **主编辑器单视图 LRU 池流**：
   - 主工作台全局复用单一 CM6 `EditorView` 实例，标签状态经 `DocTabStub`（轻量元数据 + 光标/滚动快照）与 `EditorStatePool<TState>`（上限由 `settings.maxActiveEditorRenders` 控制）在内存中按 LRU 调度，严格保护激活态与 `isDirty` 标签不被淘汰。
6. **外部写入回声抑制流**：
   - 应用内写盘前先通过自写守卫预登记目标路径与预期内容哈希，文件监听回调与前端同步层对同哈希变更自动忽略，消除自写回声导致的虚假外部修改冲突提示。

---

## 3. 按功能域反查文件表（改什么去哪找）

| 功能域 | 前端核心文件 | Rust 后端核心文件 | 对应测试文件 |
| --- | --- | --- | --- |
| **Quick Bar 捕捉 / 打开 / 命令三模式** | `src/features/quickbar/` (`QuickBarView.vue`, `store.ts`, `QuickBarTargetPicker.vue`) | `domain/capture.rs`, `application/capture.rs`, `commands/capture.rs` | `src/features/quickbar/__tests__/*`, `src-tauri/tests/capture_truth_table_test.rs`, `append_and_undo_test.rs` |
| **全局 5 秒非激活撤销 Toast** | `src/features/toast/` (`ToastView.vue`, `store.ts`) | `application/window.rs`, `commands/window.rs` (`window_dismiss_toast`), `infra/macos/bridge.m` (`QuickNoteToastPanel`) | `src/features/toast/__tests__/toast-view.test.ts` |
| **主工作台壳、多标签、菜单同步与 45% 辅助窗** | `src/app/MainWorkbenchShell.vue`（含底部终端面板挂载）, `src/features/editor/EditorTabBar.vue` | `domain/window.rs`, `application/window.rs`, `commands/window.rs` (`window_toggle_auxiliary_sidebar`), `infra/macos/bridge.m`, `src-tauri/src/lib.rs` | `src/app/__tests__/auxiliary-window-mode.test.ts`, `src-tauri/tests/window_policy_test.rs`, `m4_export_and_auxiliary.rs` |
| **左侧目录树、目录工作区绑定、标签树、最近编辑、模板新建与批量重命名标签** | `src/features/notes/` (`NotesSidebar.vue`, `DirWorkspaceBindModal.vue`, `TagRenameModal.vue`, `useDirectoryWorkspaces.ts`, `store.ts`, `api.ts`, `NewNoteTemplateModal.vue`, `templateUtils.ts`), `src/lib/actions/catalog.ts` (`folder.*`) | `domain/note.rs`, `domain/template.rs`, `domain/workspace_context.rs`, `application/notes/` (`crud.rs`, `nav.rs`, `batch.rs`), `application/reconciler.rs`, `application/index.rs`, `commands/notes.rs`, `commands/external.rs` (`list_directory_workspace_configs`, `set_directory_workspace_config`) | `src/features/notes/__tests__/*`, `src-tauri/tests/note_crud_and_index_test.rs`, `m3_commands_related_tags.rs` |
| **CM6 编辑器（单视图 LRU 池）、文档头、同级文件弹出层、`[属性]` Popover 与收件箱提取** | `src/features/editor/` (`MarkdownEditor.vue`, `store.ts`, `core/docSession.ts`, `core/editorPool.ts`, `DocHeaderBar.vue`, `SiblingFilesPopover.vue`, `DocPropertiesPopover.vue`, `InboxBloatBanner.vue`, `InboxExtractModal.vue`) | `domain/frontmatter.rs`, `domain/markdown.rs`, `application/notes/crud.rs`, `commands/notes.rs`, `commands/external.rs` (`list_current_doc_siblings`) | `src/features/editor/core/editorPool.test.ts`, `src/features/editor/__tests__/doc-meta-and-inbox-extract.test.ts`, `editor-autosave-and-conflict.test.ts`, `src-tauri/tests/m2_backend_test.rs` |
| **CM6 扩展（缩进/待办/格式/链接/图片/多语言高亮与诊断）** | `src/features/editor/extensions/` (`markdownIndent.ts`, `markdownTodo.ts`, `markdownFormat.ts`, `markdownLinks.ts`, `markdownImages.ts`, `codeLanguages.ts`, `codeDiagnostics.ts`, `scrollTarget.ts`) | `domain/markdown.rs`（中文 Slug、base64 图片解码）, `application/notes/media.rs`（`note_save_image`） | `src/features/editor/__tests__/cm6-markdown-editing.test.ts`, `cm6-links-and-images.test.ts`, `code-languages-and-diagnostics.test.ts` |
| **外部修改监听、防回环、冲突横幅与 CM6 `MergeView`** | `src/features/editor/` (`EditorConflictBanner.vue`, `EditorMergeView.vue`, `store.ts`) | `domain/watch.rs`, `application/watch.rs`, `infra/vault_watch.rs` | `src/features/editor/__tests__/merge-view-conflict.test.ts`, `merge-view-real-smoke.test.ts`, `src-tauri/tests/self_write_guard_and_watch_test.rs` |
| **内嵌 `xterm.js` 终端、折叠保活与 30 分钟空闲回收** | `src/features/terminal/` (`TerminalPanel.vue`, `store.ts`, `theme.ts`, `api.ts`) | `domain/terminal.rs`, `domain/workspace_context.rs`, `infra/pty.rs`, `application/terminal.rs`, `commands/terminal.rs` | `src/features/terminal/__tests__/terminal-panel.test.ts`, `src-tauri/tests/m4_terminal_lifecycle.rs` |
| **左侧栏视图（大纲 / 相关）与底部终端** | `src/features/panels/` (`OutlinePanel.vue`, `RelatedNotesPanel.vue`, `store.ts`, `registry.ts`, `outlineExtractor.ts`), `src/features/terminal/` (`TerminalPanel.vue`) | `application/notes/search.rs`（`notes_get_related`） | `src/features/panels/__tests__/*`, `src-tauri/tests/m3_commands_related_tags.rs` |
| **快速打开（`⌘K`，引擎与 Quick Bar「打开」共用）、纯 Rust 路径探测/目录直扫与外部书签** | `src/features/quickopen/` (`QuickOpenPalette.vue`, `useQuickOpen.ts`, `queryParser.ts`, `api.ts`) | `domain/bookmark.rs`, `infra/fs_browser.rs`, `application/external.rs`, `application/notes/search.rs`, `commands/external.rs` (`probe_external_path`, `list_external_directory`, `upsert_external_bookmark`, `remove_external_bookmark`), `commands/search.rs` | `src/features/quickopen/__tests__/quick-open-engine.test.ts`, `src-tauri/tests/m3_external_path_bookmarks.rs` |
| **命令书解析与命令库面板（`⌘⇧K`）** | `src/features/commands/` (`CommandBookPalette.vue`, `useCommandPalette.ts`) | `domain/command_book.rs`, `application/index.rs`, `commands/search.rs` | `src/features/commands/__tests__/command-book-palette.test.ts`, `src-tauri/tests/m3_commands_related_tags.rs` |
| **外部动作（Finder / 外部终端 / VS Code / Sublime / 固定）** | `src/features/editor/useExternalActions.ts`, `src/features/notes/NotesSidebar.vue` | `domain/external.rs`, `domain/workspace_context.rs`, `infra/external_open.rs`, `application/external.rs`, `commands/external.rs` | `src/features/editor/__tests__/external-actions-integration.test.ts`, `src-tauri/tests/m3_external_path_bookmarks.rs` |
| **复制富文本 HTML 与离屏 `WKWebView` 导出 PDF** | `src/features/editor/export/markdownExport.ts`, `ExportPdfModal.vue` | `commands/notes.rs` (`note_export_pdf`), `infra/macos/bridge.m` (`qn_render_html_to_pdf`), `infra/macos/mod.rs` | `src/features/editor/__tests__/export-rich-text-and-pdf.test.ts`, `src-tauri/tests/m4_export_and_auxiliary.rs` |
| **统一动作注册表、快捷键作用域分发与右键菜单** | `src/lib/actions/` (`catalog.ts`, `registry.ts`, `keymap.ts`, `types.ts`), `src/components/QnContextMenu.vue` | `commands/app.rs` (`app_set_menu_accelerators`), `src-tauri/src/lib.rs`（原生系统菜单） | `src/lib/actions/__tests__/action-registry.test.ts` |
| **设置窗口、配置备份导入导出、`settings.yaml` 容错与首次选库** | `src/features/settings/` (`SettingsView.vue`, `store.ts`, 各子面板), `src/lib/ipc/commands.ts` (`pickBackupSavePath` / `pickBackupOpenPath`), `src/features/onboarding/` | `domain/settings.rs`, `domain/ui_state.rs`, `application/persistence.rs`, `infra/settings_watch.rs`, `commands/settings.rs`, `commands/library.rs`, `commands/window.rs` (`window_close_settings`) | `src/features/settings/__tests__/*`, `src-tauri/tests/persistence_test.rs`, `m4_export_and_auxiliary.rs` |
| **SQLite FTS5 混合分词索引、只读对账与性能压测** | — | `domain/tokenizer.rs`, `infra/sqlite.rs`, `src-tauri/migrations/0001_init.sql`, `0002_directory_workspace_cache.sql`, `application/reconciler.rs`, `application/index.rs` | `src-tauri/tests/domain_note_and_tokenizer_test.rs`, `sqlite_migration_test.rs`, `m4_performance_budget.rs` |
| **设计 Token 与基础通用组件** | `src/styles/tokens.css`, `src/components/Qn*.vue` | — | `scripts/__tests__/check-architecture.test.ts` |

---

## 4. 高频改动操作菜谱（最少文件集）

1. **新增 / 修改 IPC 命令**：
   1. `src-tauri/src/domain/<mod>.rs` 定义结构体（`Serialize, Deserialize, Type` + `#[serde(rename_all = "camelCase")]`）
   2. `src-tauri/src/bindings.rs` 注册新类型 → 运行 `pnpm gen:bindings` 更新 `src/bindings/types.ts`
   3. `src-tauri/src/application/` + `src-tauri/src/commands/` 实现并在 `src-tauri/src/lib.rs` `invoke_handler` 注册
   4. `src/lib/ipc/commands.ts`（或 `events.ts`）添加前端强类型封装（若为 `ui_state_set` 新增键，需先在 `domain/ui_state.rs` 的 `UiStatePatchKey` 登记并同步 `bindings.rs`）
2. **新增前端动作 / 快捷键 / 原生菜单项**：
   1. `src/lib/actions/catalog.ts` 注册 `ActionDef`（明确 `scope`、`when`、`swallowWhenDisabled`）
   2. 若需挂入 macOS 顶部菜单：在 `src-tauri/src/lib.rs` `build_app_menu` 添加同 `id` 菜单项并在 `setup_tray_and_menu` 放行转发至 `menu:action`；若支持改键同步菜单，加入 `src/lib/actions/registry.ts::NATIVE_MENU_ACTION_IDS`
   3. 在对应宿主组件（如 `MainWorkbenchShell.vue` 或 `NotesSidebar.vue`）通过 `actionRegistry.bindHandler(id, ...)` 绑定处理器并在卸载时释放
3. **修改 UI 样式**：
   - 仅使用 `src/styles/tokens.css` 已定义的 `--qn-*` CSS Token（禁硬编码 hex/rgba 色值与魔法 `z-index`）；新增 Token 必须同时补齐浅色、`:root[data-theme='dark']` 和 `@media (prefers-color-scheme: dark)` 三处。

---

## 5. 已验证实机暗坑与不可破红线（动手前必看）

1. **macOS `WKWebView` 剪贴板粘贴气泡**：调用 `navigator.clipboard.readText()` 会在鼠标处弹出系统原生 `Paste` 确认气泡。终端粘贴严禁拦截 `⌘V` 调 `readText()`，必须放行给浏览器原生 `paste` 事件流向 xterm textarea。
2. **`@xterm/xterm` v6 自绘滚动条**：使用 VS Code `DomScrollableElement` 自绘 `.xterm-scrollable-element > .scrollbar`（非原生 `::-webkit-scrollbar`），隐藏滚动条须同时覆盖该 DOM 选择器。
3. **原生 `NSPanel` 零私有 API**：Quick Bar 与 Toast 严禁开 Tauri `.transparent(true)`；`collectionBehavior` 保持 `256`（`fullScreenAuxiliary`，不含 `canJoinAllSpaces`），不跨 Space、切 Space 自动复位、默认失焦关闭（固定态例外）。
4. **全屏 ↔ 45% 侧栏辅助窗异步竞态**：原生全屏退全屏是异步动画，`bridge.m` 仅在非全屏态保存标准 frame，且通过一次性 `NSWindowDidExitFullScreenNotification` 观察者等待动画完成后再贴靠右侧 45%；全屏 Space 悬浮叠加行为由 macOS 原生层配置，托盘与 Quick Bar 均提供切换入口。
5. **macOS `Option` 组合键**：`⌥⌘C` / `⌥⌘1` 等组合键会让 `KeyboardEvent.key` 变特殊字符，快捷键匹配与录制一律优先基于 `event.code` 归一化。
6. **外链与文件拖放**：`WKWebView` 拦截 `window.open`，外链必须走参数化 Rust `app_open_external_url`；Finder/截图拖入走 `onWindowFileDrop`。
7. **`meta + body` 分离与坐标系**：编辑器内所有行号（大纲 / 收件箱提取 / 命令书 `line`）均为剥离 frontmatter 后的 `NoteDoc.body` 1-based 行号；`note_write` 绝不改写 YAML（缺失 ULID 补写为唯一例外），且扫描对账过程只读不改写用户文件。
8. **Rust 与安全底线**：`src-tauri/src/**` 零 `.unwrap()`/`.expect()`；所有测试落位 `src-tauri/tests/**`；外部命令严禁 shell 拼接；日志严禁打印笔记正文、剪贴板、终端字节流或 PDF HTML；`git commit` / `git merge` 必须带 `--no-gpg-sign`；收工必过 `pnpm check:all`。
9. **编辑器输入法组合态防抖**：编辑器自动保存在输入法组合态（IME composition）期间不触发，组合结束后再补排保存。
10. **切库必切 DB + 启动必增量对账（静态门禁强约束）**：`application/library.rs::set_library_root` 必须调用 `pm.switch_vault_db` 切至 `cache/vaults/<hash>.db`（并同步在 `cache/index.db` 留存 `library_root` 供损坏配置冷启动回退）；`lib.rs` 启动 `setup` 必须执行一次只读增量对账 `index_rebuild`/`reconcile_vault`，防止首次选库重启后连向空库导致目录全空与标签全失联。
11. **外部文件/目录监听全链路对齐（静态门禁强约束）**：`VaultWatcher` 回调必须走 `application/watch.rs::handle_vault_watch_path`（覆盖 `.md`、`.quicknote.yaml` 与目录移动/删除增量对账）；`notes/store.ts` 必须同时订阅 `index:status` 与 `fs:external-change` 实时刷新侧边栏；`SelfWriteGuard` 按路径保留窗口内多次自写历史，避免连续自动保存回声误判冲突。
12. **`EditorStatePool` 防旧状态复活 & 冲突/退出保存护栏（静态门禁强约束）**：`MarkdownEditor.vue` 复用池化 `EditorState` 前必须校验 `pooled.state.doc.toString() === tab.draftBody`；无 `id` 文档首次保存补写 ULID 升级 `tab.key` 时原地继承当前 `view.state` 绝不读回旧快照；冲突暂缓态须保留「自动保存已暂停」提示且手动 `⌘S` 须重新展开冲突横幅；窗口失焦/隐藏/卸载前必须触发 `flushAllDirtyTabs()`。
