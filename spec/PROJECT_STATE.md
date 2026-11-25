# PROJECT_STATE · QuickNote 总控状态板

> 主控会话接续唯一动态真源。每次里程碑收官或启动新主控会话时同步更新，不重复 `AGENTS.md` / `engineering-contract.md` 静态正文。

## 1. 当前精确进度

- **当前里程碑**：**`v5 Architecture Refactor（Task 00–12 已勾选，12/22）`**
- **前置基线**：`v1.0.0`（`m4-baseline`）已合入 `master`；`spec/quicknote-design-v4.md` 已冻结，`AGENTS.md`、`spec/engineering-contract.md`、`DECISIONS.md`（`D-0012`）已写入 v5 架构覆写约束。
- **阶段状态**：**Task 00–12 已完成**（Phase A + 后端外部文件/书签/目录工作区 IPC 契约、绑定导出与窗口命令注册已交付，并同步登记至 `spec/CODEBASE_MAP.md`）。
- **下一入口**：**`Task 13`**（按序推进 v5 重构 Task 13–22）。

## 2. v5 Architecture Refactor 任务清单（Task 00–12 已完成，下一入口 Task 13）

| 编号 | 状态 | 任务项 | 编号 | 状态 | 任务项 |
| --- | --- | --- | --- | --- | --- |
| Task 00 | [x] | v5 重构基线与 Phase A 前置准备 | Task 12 | [x] | 目录级 `.quicknote.yaml` 解析与原子自写守卫 |
| Task 01 | [x] | 架构覆写与状态板初始化 | Task 13 | [ ] | 外部动作与终端 cwd 三级级联解析 |
| Task 02 | [x] | 纯缓存 SQLite 路径隔离（按 Vault 哈希） | Task 14 | [ ] | 纯 Rust 外部文件系统探测与直扫引擎 |
| Task 03 | [x] | 启动全量增量对账与只读扫描 | Task 15 | [ ] | 外部书签持久化至配置目录 |
| Task 04 | [x] | 无 `id` 文档基于 `rel_path` 确定性兜底键 | Task 16 | [ ] | Settings/Toast 按需创建与闲置销毁 |
| Task 05 | [x] | 运行期文件监听单向收敛至 SQLite | Task 17 | [ ] | 主编辑器单 `EditorView` 与 CM6 `EditorState` LRU 池 |
| Task 06 | [x] | 随时删库全量重建机制与验证 | Task 18 | [ ] | 45% 侧栏辅助模式（macOS 全屏 Space 悬浮叠加） |
| Task 07 | [x] | 笔记列表与 FTS 排除 `.quicknote.yaml` | Task 19 | [ ] | 托盘与 QuickBar 侧栏模式切换入口 |
| Task 08 | [x] | 废弃 Vault 根目录旧库迁移/清理 | Task 20 | [ ] | IPC 绑定导出与前端状态同步适配 |
| Task 09 | [x] | 废弃独立全局工作区模型与清理 | Task 21 | [ ] | v5 集成测试与删库重建/级联回归 |
| Task 10 | [x] | 废弃 Spotlight `search_roots` 主检索引擎 | Task 22 | [ ] | 全量门禁 `pnpm check:all` 与实机验收收官 |
| Task 11 | [x] | 配置目录配置文件与只读容错校验适配 | | | |

## 3. 已验证实机约定（摘要）

- **辅助窗口 URL 路由**：`index.html?window=quickbar|toast|settings`，主窗口为无参 `index.html`。
- **原生 Toast 零私有 API**：严禁开启 `.transparent(true)`，采用 `bridge.m` `NSPanel` + `clearColor` + `CALayer` 圆角裁剪。
- **macOS `Option` 快捷键与外链/拖放**：快捷键优先基于 `event.code` 归一化；外链走 `app_open_external_url`，文件拖放走 `onDragDropEvent`。
- **行号坐标系**：编辑器内全部行号统一为剥离 frontmatter 后的 `NoteDoc.body` 1-based 行号（`D-0006`）。
