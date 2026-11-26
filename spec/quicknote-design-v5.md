# QuickNote 架构与产品设计定稿（v5）

> 本文档为 QuickNote v5 架构的精简结构化总览，聚焦核心架构边界、真相源与缓存机制、外部文件直开与书签体系、目录级工作区继承、内存生命周期与 macOS 原生侧栏形态。机器级静态接口与 DDL 详见 [engineering-contract.md](engineering-contract.md)，决策背景详见 [DECISIONS.md](../DECISIONS.md)（`D-0001`–`D-0012`），代码反查详见 [CODEBASE_MAP.md](CODEBASE_MAP.md)。

---

## 1. 四层职责边界与单向分层架构

QuickNote 严格执行前后端四层单向依赖，并通过 `scripts/check-architecture.mjs` 在每次门禁中静态校验（`C1.2`、`D-0009`、`D-0010`）。

### 1.1 Rust 后端四层（`commands → application → domain`、`application / infra → domain`）

| 层级 | 目录 | 核心职责 | 硬边界 |
| --- | --- | --- | --- |
| **IPC 接入层** | `src-tauri/src/commands/` | 薄 Tauri 命令入口、状态注入与参数转发 | 不含业务逻辑；全量注册于 `src-tauri/src/lib.rs` |
| **应用编排层** | `src-tauri/src/application/` | 业务用例编排、启动只读对账（`reconciler`）、配置/缓存隔离持久化（`persistence`）、写入自写守卫、撤销快照与终端/窗口状态机 | 通过 trait/接口调用 `infra`，结果统一返回 `Result<T, AppError>` |
| **领域核心层** | `src-tauri/src/domain/` | 纯内存数据结构与确定性算法（目标真值表、分区追加、YAML 最小补丁、Slug、混合分词、书签规范化、目录工作区三级级联） | **严禁依赖** `tauri`、`std::fs`、`std::process`、`rusqlite`、`notify`、`portable_pty`、`libc` |
| **基础设施层** | `src-tauri/src/infra/` | 原子文件读写、SQLite FTS5、纯 Rust 受控目录直扫（`fs_browser`）、`notify` 监听、`portable-pty`、macOS ObjC/Carbon 原生桥接（`bridge.m`） | 外部命令严格参数化数组传参，零 shell 拼接；生产代码零 `.unwrap()`/`.expect()` |

### 1.2 前端四层（`lib → components → features → app`）

| 层级 | 目录 | 核心职责 | 硬边界 |
| --- | --- | --- | --- |
| **基础能力层** | `src/lib/` + `src/bindings/` | `src/bindings/types.ts`（Rust `export_bindings.rs` 自动生成唯一契约）、`src/lib/ipc/`（唯一允许调用 `@tauri-apps/api/*` 的手写层）、`src/lib/actions/`（统一动作注册表与快捷键作用域分发）、纯工具函数 | 禁止手改 `src/bindings/types.ts`；不反向依赖 `features/*` |
| **纯 UI 组件层** | `src/components/` + `src/styles/tokens.css` | 通用原子/容器组件（`QnBadge`、`QnBanner`、`QnButton`、`QnContextMenu`、`QnInput`、`QnModalShell`、`QnPanelCard`）与浅/深双主题 `--qn-*` CSS Token | 禁止直连 IPC 或跨 feature 状态；禁止硬编码色值与魔法 `z-index` |
| **业务特性层** | `src/features/*` | 按垂直域拆分的 Store、API 封装与视图（`commands`、`editor`、`notes`、`onboarding`、`panels`、`quickbar`、`quickopen`、`settings`、`terminal`、`toast`） | 跨 feature 仅允许经 `@/features/<name>` 顶层 `index.ts` 导入，禁止深层导入 |
| **应用装配层** | `src/app/` | 多窗口路由入口（`App.vue`）、主工作台壳（`MainWorkbenchShell.vue`）、启动引导与全局动作绑定 | 协调多窗口与全局状态，不承载底层细节算法 |

---

## 2. 磁盘唯一事实源与纯缓存 SQLite 对账机制

依据 `D-0006`、`D-0008`、`D-0012` 与 `C5.1`、`C6.1`，存储架构将**用户事实源**、**应用配置**与**派生索引缓存**彻底解耦：

1. **磁盘文档为唯一真相源**：
   - 笔记库（Vault）内的 `.md` 笔记与目录级 `.quicknote.yaml` 是唯一真相源。
   - 正文与元数据遵循 `meta + body` 分离（`D-0006`）：编辑器只接触 `body`，`note_write` 保留磁盘原 YAML（仅缺失 ULID 时补写 `id`），`note_append` 绝不改写 frontmatter，元数据编辑走 `note_update_meta` 最小补丁（`D-0007`）。
2. **启动只读对账 + 运行期监听单向收敛**：
   - 启动或切库时，由 `application/reconciler.rs` 执行只读全量/增量对账，将磁盘 `.md` 与 `.quicknote.yaml` 单向同步至 SQLite。
   - 无 Frontmatter `id` 的 `.md` 以确定性兜底键 `path:<rel_path>` 收录，**扫描与对账过程严格只读，绝不改写用户文件**。
   - 运行期由 `infra/vault_watch.rs` 与 `application/watch.rs` 监听外部变更并经自写防回环缓存剔除内部写入回声，单向更新派生索引与前端状态。应用内写盘前先预登记目标路径与预期内容哈希，前端同步层对同哈希外部变更通知自动忽略，抑制自写回声（`C8.5`）。
3. **纯缓存 SQLite 物理隔离与无损重建**：
   - SQLite 数据库文件（含 `-wal` / `-shm`）强制落盘于应用缓存目录 `$APP_CACHE_DIR/<vault_hash>.db`（按 Vault 路径哈希隔离），**严禁放入笔记库根目录**，避免污染 Git / 云盘同步目录。
   - SQLite 仅存放可重建的派生元数据、FTS5 混合分词索引（`0001_init.sql`）与目录级工作区缓存表 `directory_workspace_configs`（`0002_directory_workspace_cache.sql`），迁移只追加不改历史，随时删除缓存库后启动可无损全量重建。
4. **用户配置目录独立持久化**：
   - `$APP_CONFIG_DIR/` 存放 `settings.yaml`（唯一主配置，手改语法错误时只读不动并回退有效配置，`D-0008`）、`ui-state.json`、`bookmarks.json`、`quickbar-draft.json`、`recovery/` 与滚动清理的 `backups/`，不以 SQLite 为唯一存储。

---

## 3. 外部文件直开、受控直扫与书签体系

依据 `D-0012`、`C2.1`、`C3.4`、`C4.2`，v5 全面废止 Spotlight（`mdfind`）作为主检索引擎，构建纯 Rust 受控外部文件能力：

- **纯 Rust 受控探测与目录直扫（`infra/fs_browser.rs`）**：
  - 统一提供路径探测（`probe_external_path`）与目录直扫（`list_external_directory`），原生支持 `.xxx` 隐藏目录（如 `~/.config`）及 `/usr/local/etc`、`/opt/homebrew/etc` 等系统路径。
  - 内置深度与条目数熔断保护，防止超大目录或符号链接环阻塞主线程。
- **同级文件快速切换（`SiblingFilesPopover.vue`）**：
  - 经 `list_current_doc_siblings` 读取当前文档（含库内笔记与外部文件）所在目录的同级文件列表，支持在文档顶栏一键切换打开。
- **外部文件直开与非 Markdown 编辑**：
  - Quick Open（`⌘K`）与路径直扫命中的外部文件可直接作为标签页在主编辑器打开（key 为 `abs:<absPath>`），并支持多语言代码高亮与诊断；Quick Bar 的「打开」复用同一引擎。
- **外部书签体系（`domain/bookmark.rs`）**：
  - 通过 `upsert_external_bookmark` / `remove_external_bookmark` 管理常用外部文件与目录书签，纯内存执行路径规范化、去重与排序，持久化至配置目录配置文件（`bookmarks.json` / `settings.yaml`）。

---

## 4. 目录级 `.quicknote.yaml` 三级工作区继承

依据 `D-0012`、`C6.1`、`C7.2`、`C8.5`，v5 废弃脱离笔记库的独立全局工作区，改为紧贴项目笔记目录的声明式配置：

1. **目录绑定模型（`domain/workspace_context.rs`）**：
   - 在笔记库任意子目录下放置 `.quicknote.yaml`，可声明 `vscodeWorkspacePath`、`projectRoot`、`defaultTerminalCwd`、`externalAppOverrides`。
   - 左侧目录树经统一动作注册表提供 `folder.bindWorkspaceConfig`（弹窗编辑/清除 `.quicknote.yaml`）与 `folder.openWorkspaceInVscode`（直接以绑定的工作区或项目根目录在 VS Code 中打开），并对已绑定目录渲染轻量工作区徽标与路径摘要提示。
2. **三级级联解析规则**：
   - 所有外部动作（Finder / 外部终端 / VS Code / Sublime / 工作区打开）与内嵌终端启动 `cwd`，严格按以下优先级级联解析：
     $$\text{文档 Frontmatter} > \text{最近祖先目录 } \texttt{.quicknote.yaml} > \text{全局默认}$$
3. **隔离与自写守卫**：
   - `.quicknote.yaml` 强制排除出左侧笔记列表、目录计数与 FTS5 索引，绝不作为笔记条目呈现。
   - 应用内通过 `set_directory_workspace_config` 写入或删除 `.quicknote.yaml` 时，强制走原子写 + 自写守卫，防止触发误报外部变更事件。

---

## 5. 常驻窗口销毁策略与 CM6 单视图 LRU 渲染池

依据 `D-0001`–`D-0005`、`D-0012`、`C7.5`，v5 在保持瞬时响应的前提下严控常驻内存：

1. **多窗口分级生命周期**：
   - **常驻窗口**：主工作台窗口（`index.html`）与 Quick Bar 原生非激活 `NSPanel`（`?window=quickbar`，`collectionBehavior = fullScreenAuxiliary` = `256`，不跨 Space、切 Space 自动复位、默认失焦关闭，固定态例外（`D-0013`））。
   - **按需销毁窗口**：Settings（`?window=settings`）与全局 5 秒撤销 Toast（`?window=toast`）等低频副窗口按需创建，在关闭或闲置后经 `window_close_settings` / `window_dismiss_toast` 销毁并释放 WebView 内存。
   - **系统集成策略**：主窗口与设置窗隐藏时动态切为 `.accessory`，显示期间切为 `.regular`（`D-0003`）；默认全局热键 `⌃⌥Space`（备用 `⌃⌥⌘Space`）带首启自检与热重注册（`D-0004`）；内嵌终端折叠保活并在隐藏无前台子进程闲置 30 分钟后自动回收（`D-0005`）。
2. **主编辑器单 `EditorView` + `EditorState` LRU 池**：
   - 主工作台全局仅维护单一 CM6 `EditorView` 实例，避免多标签同时挂起多个完整编辑器 DOM 树。
   - 标签页以轻量 `DocTabStub`（元数据 + 光标/滚动快照）存在；`src/features/editor/core/editorPool.ts` 实现泛型 `EditorStatePool<TState>`，上限由 `settings.maxActiveEditorRenders` 控制。
   - 淘汰策略**严格保护当前激活态标签与所有 `isDirty` 未保存标签**不被逐出；被逐出的干净标签再次切回时按需从磁盘重新水合并恢复滚动与光标位置。

---

## 6. macOS 45% 全屏悬浮侧栏辅助模式

依据 `D-0011`、`D-0012`、`C7.4`、`C7.5`、`C9.3`，45% 侧栏辅助模式让用户在阅读文档、浏览网页或全屏编码时可随时在屏幕右侧贴靠呼出完整工作台：

- **单 WebView 复用**：直接复用主工作台 WebView，不额外拉起独立辅助窗口实例，保持标签页、编辑态与终端会话完全连续。
- **macOS 原生全屏 Space 悬浮叠加**：由 `src-tauri/src/infra/macos/bridge.m` 在原生层配置全屏 Space 悬浮叠加行为与右侧 45% 宽度贴靠，覆盖当前全屏应用右侧而不挤压或改变其他应用尺寸。
- **异步退全屏竞态保护**：仅在非全屏态记录标准窗口 frame；从主窗口原生全屏切换至辅助侧栏时，通过一次性 `NSWindowDidExitFullScreenNotification` 观察者等待退出全屏动画结束后再执行右侧贴靠，避免窗口错位。
- **统一入口与窄屏适配**：系统托盘菜单、Quick Bar 及动作注册表（`window.toggleAuxiliary` → `window_toggle_auxiliary_sidebar`）均提供切换入口；侧栏模式下编辑器区右上角提供「拉出导航 / 退出侧栏辅助」；窗口宽度 `< 900px` 时左侧栏以覆盖式抽屉浮于编辑区之上，编辑区最小宽度不低于 `480px`；内嵌终端位于编辑器下方，可拖动高度并持久化。
- **左侧栏顶层视图**：工作区（目录 / 标签 / 最近）、相关文档、大纲。
