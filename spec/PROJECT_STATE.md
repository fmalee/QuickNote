# PROJECT_STATE · QuickNote 总控状态板

> 主控会话接续唯一动态真源。每次里程碑收官或启动新主控会话时同步更新，不重复 `AGENTS.md` / `engineering-contract.md` 静态正文。

## 1. 当前精确进度

- **当前状态**：**`M5 Completed`（M5 迭代全量收官，含 R35 与可选卡 `M5-06f`）**
- **本轮概述**：完成 M5 全量缺陷修复与体验升级——修复启动标签恢复、自写回声假冲突、标签列表同步、滚动橡皮筋、45% 侧栏全屏 Space 顶边贴靠与退出回桌面、以及终端字距；接入系统废纸篓删除与备份原生文件选择框；重构主窗口单栏左侧栏（目录/标签/最近/相关/大纲）+ 底部终端布局与状态栏；补齐编辑器查找/替换与标签页交互；实现 Quick Bar 失焦关闭、图钉固定、`⌘N` 切换与混合打开引擎复用；彻底移除 Spotlight 死代码并完成记录类文档同步。
- **阶段结论**：M5 全部卡片（含 R35 与可选卡 `M5-06f`）均已完成并过 `pnpm check:all` 全量门禁。

## 2. M5 迭代任务清单（2026-11-26）

| 顺序 | 卡片文件 | 主题 | 状态 |
| --- | --- | --- | --- |
| 1 | `M5-01a-contract-baseline.md` | 契约变更包（只改文档） | [x] |
| 2 | `M5-01b-gate-baseline.md` | 门禁基线转绿 + 版本号 | [x] |
| 3 | `M5-02a-r03-restore-reactivity.md` | 启动后当前标签不加载 | [x] |
| 4 | `M5-02b-r01-self-write-echo.md` | 「检测到外部修改冲突」假弹窗 | [x] |
| 5 | `M5-02c-r21-tag-sync.md` | 改标签后列表不同步 | [x] |
| 6 | `M5-02d-r36-overscroll.md` | 滚动橡皮筋 | [x] |
| 7 | `M5-02e-r35-aux-frame.md` | 45% 侧栏全屏 Space 顶部留白（R35） | [x] |
| 8 | `M5-02f-r41-terminal-font.md` | 终端字距过大 | [x] |
| 9 | `M5-03a-r26-trash-delete.md` | 删除到系统废纸篓 | [x] |
| 10 | `M5-03b1-r28-dialog-backend.md` | 文件选择窗口（依赖与后端） | [x] |
| 11 | `M5-03b2-r28-dialog-frontend.md` | 文件选择窗口（设置页） | [x] |
| 12 | `M5-03c-r37-r40-build.md` | 图标与打包配置 | [x] |
| 13 | `M5-04a-layout-state-model.md` | 布局新状态模型（与旧状态并存） | [x] |
| 14 | `M5-04b-layout-shell.md` | 壳布局：删右侧栏、底部终端、删顶部横栏 | [x] |
| 15 | `M5-04c-sidebar-redesign.md` | 左侧栏整体改造 | [x] |
| 16 | `M5-04d-layout-legacy-cleanup.md` | 清理旧右侧栏状态与死组件 | [x] |
| 17 | `M5-04e-r07-context-actions.md` | 外部动作进右键菜单 | [x] |
| 18 | `M5-05a-r02-find-replace.md` | 查找与替换 | [x] |
| 19 | `M5-05b-tabs-settings-actions.md` | 关闭按钮开关、上一个 / 下一个标签 | [x] |
| 20 | `M5-05c-tabs-interactions.md` | 标签拖拽排序、删除子菜单、去「关闭其他」 | [x] |
| 21 | `M5-05d-r29-statusbar.md` | 状态栏重排 | [x] |
| 22 | `M5-06a1-r30-quickbar-blur.md` | 失焦关闭：原生层与窗口状态机 | [x] |
| 23 | `M5-06a2-r31-quickbar-pin.md` | 固定 Quick Bar：后端、菜单、ADR 流转 | [x] |
| 24 | `M5-06a3-r31-quickbar-pin-ui.md` | 固定 Quick Bar：界面图标与动作 | [x] |
| 25 | `M5-06b-r32-quickopen-engine.md` | 混合打开移植到 Quick Bar | [x] |
| 26 | `M5-06c-r33-r34-quickbar-keys.md` | 取消键直接隐藏、⌘N | [x] |
| 27 | `M5-06e1-spotlight-removal.md` | Spotlight 死代码：命令与 Runner | [x] |
| 28 | `M5-06e2-search-source-cleanup.md` | Spotlight 残留的来源变体 | [x] |
| 29 | `M5-06d-docs-cleanup.md` | 文档记录类收尾 | [x] |
| 30 | `M5-06f-workspaces-removal.md` | 废弃的全局工作区模型清理 | [x] |

<details>
<summary>历史阶段：v5 Architecture Refactor 任务清单（Task 00–22 全部完成）</summary>

| 编号 | 状态 | 任务项 | 编号 | 状态 | 任务项 |
| --- | --- | --- | --- | --- | --- |
| Task 00 | [x] | v5 重构基线与 Phase A 前置准备 | Task 12 | [x] | 目录级 `.quicknote.yaml` 解析与原子自写守卫 |
| Task 01 | [x] | 架构覆写与状态板初始化 | Task 13 | [x] | 外部动作与终端 cwd 三级级联解析 |
| Task 02 | [x] | 纯缓存 SQLite 路径隔离（按 Vault 哈希） | Task 14 | [x] | 纯 Rust 外部文件系统探测与直扫引擎 |
| Task 03 | [x] | 启动全量增量对账与只读扫描 | Task 15 | [x] | 外部书签持久化至配置目录 |
| Task 04 | [x] | 无 `id` 文档基于 `rel_path` 确定性兜底键 | Task 16 | [x] | Settings/Toast 按需创建与闲置销毁 |
| Task 05 | [x] | 运行期文件监听单向收敛至 SQLite | Task 17 | [x] | 主编辑器单 `EditorView` 与 CM6 `EditorState` LRU 池 |
| Task 06 | [x] | 随时删库全量重建机制与验证 | Task 18 | [x] | 45% 侧栏辅助模式（macOS 全屏 Space 悬浮叠加） |
| Task 07 | [x] | 笔记列表与 FTS 排除 `.quicknote.yaml` | Task 19 | [x] | 托盘与 QuickBar 侧栏模式切换入口 |
| Task 08 | [x] | 废弃 Vault 根目录旧库迁移/清理 | Task 20 | [x] | IPC 绑定导出与前端状态同步适配 |
| Task 09 | [x] | 废弃独立全局工作区模型与清理 | Task 21 | [x] | v5 集成测试与删库重建/级联回归 |
| Task 10 | [x] | 废弃 Spotlight `search_roots` 主检索引擎 | Task 22 | [x] | 全量门禁回归验证与 v5 文档正式收口 |
| Task 11 | [x] | 配置目录配置文件与只读容错校验适配 | | | |

</details>

## 3. 最终回归验证结果（2026-11-26）

| 检查项 | 命令 | 结果 |
| --- | --- | --- |
| Rust → TS 绑定零漂移 | `pnpm gen:bindings && git diff --exit-code src/bindings` | 通过（0 diff） |
| 静态架构分层与红线校验 | `pnpm check:arch`（`node scripts/check-architecture.mjs`） | 通过（0 violations） |
| Prettier 格式检查 | `pnpm exec prettier --check .` | 通过（0 warnings） |
| 前端代码规范与静态类型 | `pnpm lint && pnpm typecheck` | 通过（0 errors） |
| 前端全量单元测试 | `pnpm test:unit --run` | 通过（40/40 测试文件，221/221 用例） |
| Rust 全量单元与集成测试 | `cargo test --manifest-path src-tauri/Cargo.toml --all-features` | 通过（102 条通过，0 失败；1 条 10k 压测默认 `#[ignore]`，可 `--ignored` 单独运行） |

> **实机专项约定（R35）**：
> - **45% 侧栏辅助模式**：进入时临时去掉 `NSWindowStyleMaskTitled` 并放行 `canBecomeKeyWindow`，通过 `CGWindowListCopyWindowInfo` 区分桌面与全屏 Space，并监听 `NSWorkspaceActiveSpaceDidChangeNotification` 自动重算高度；全屏 Space 下退出侧栏先 `orderOut` 保留 `CanJoinAllSpaces` 切回桌面 Space 上屏后再剥离 `CanJoinAllSpaces`；原生全屏主窗口异步退全屏进入侧栏期间不覆盖 `Auxiliary` 状态并在退全屏完成后补存标准窗口状态。

## 4. 已验证实机约定（摘要）

- **辅助窗口 URL 路由**：`index.html?window=quickbar|toast|settings`，主窗口为无参 `index.html`（45% 侧栏辅助模式复用主窗口 WebView）。
- **原生 Toast 零私有 API**：严禁开启 `.transparent(true)`，采用 `bridge.m` `NSPanel` + `clearColor` + `CALayer` 圆角裁剪。
- **macOS `Option` 快捷键与外链/拖放**：快捷键优先基于 `event.code` 归一化；外链走 `app_open_external_url`，文件拖放走 `onDragDropEvent`。
- **行号坐标系**：编辑器内全部行号统一为剥离 frontmatter 后的 `NoteDoc.body` 1-based 行号（`D-0006`）。
- **持久化向后兼容**：`ui-state.json` 旧键 `panelGroup` 读取时忽略；`settings.yaml` 新字段均有默认值，旧配置可直接加载。
