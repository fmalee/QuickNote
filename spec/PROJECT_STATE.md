# PROJECT_STATE · QuickNote 总控状态板

> 主控会话接续唯一动态真源。每次里程碑收官或启动新主控会话时同步更新，不重复 `AGENTS.md` / `engineering-contract.md` 静态正文。

## 1. 当前精确进度

- **当前状态**：**`v5 Architecture Ready`（Task 00–22 全部勾选，23/23）**
- **架构定稿**：`spec/quicknote-design-v5.md` 已生成定稿；`spec/engineering-contract.md` 已将原临时 `Active Architecture Overrides` 正式合并进正文并清理废弃 Spotlight 与独立全局工作区约束；`AGENTS.md` 已清理过渡期提示并切换默认设计指针至 `spec/quicknote-design-v5.md`；`spec/CODEBASE_MAP.md` 已与最终前后端代码 100% 对齐；`DECISIONS.md` 已归档 `D-0001`–`D-0012`。
- **阶段结论**：v5 架构重构全部开发、目录级 `.quicknote.yaml` 绑定与右键动作集成、文档正式化收口与前后端全量回归验证均已完成，进入可发布/实机验收态。

## 2. v5 Architecture Refactor 任务清单（Task 00–22 全部完成）

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

## 3. 最终回归验证结果（2026-11-25）

| 检查项 | 命令 | 结果 |
| --- | --- | --- |
| Rust → TS 绑定零漂移 | `pnpm gen:bindings && git diff --exit-code src/bindings` | 通过（0 diff） |
| 静态架构分层与红线校验 | `pnpm check:arch`（`node scripts/check-architecture.mjs`） | 通过（0 violations） |
| 前端代码规范与静态类型 | `pnpm lint && pnpm typecheck` | 通过（0 errors） |
| 前端全量单元测试 | `pnpm test:unit --run` | 通过（34/34 测试文件，186/186 用例） |
| Rust 全量单元与集成测试 | `cargo test --manifest-path src-tauri/Cargo.toml --all-features` | 核心 94 条全部通过（含 10k Vault 性能压测、0002 迁移、只读对账、三级级联、目录工作区原子写、外部直扫与书签、PTY 生命周期与辅助窗口策略） |

> 备注（已知非阻断遗留项）：
> - `prettier --check .` 有 10 个历史文件待执行 `pnpm format` 统一格式化；
> - 已废弃 Spotlight 历史兼容用例 `m3_spotlight_path_external::spotlight_concurrent_request_cancels_previous_process` 在并发执行下触发 10s 超时（Spotlight 主检索已于 v5 废止，后续清理 `infra/spotlight.rs` 时一并移除）。`M5-01b` 起已标 `#[ignore]`，待 `M5-06e1` 删除。

## 4. 已验证实机约定（摘要）

- **辅助窗口 URL 路由**：`index.html?window=quickbar|toast|settings`，主窗口为无参 `index.html`（45% 侧栏辅助模式复用主窗口 WebView）。
- **原生 Toast 零私有 API**：严禁开启 `.transparent(true)`，采用 `bridge.m` `NSPanel` + `clearColor` + `CALayer` 圆角裁剪。
- **macOS `Option` 快捷键与外链/拖放**：快捷键优先基于 `event.code` 归一化；外链走 `app_open_external_url`，文件拖放走 `onDragDropEvent`。
- **行号坐标系**：编辑器内全部行号统一为剥离 frontmatter 后的 `NoteDoc.body` 1-based 行号（`D-0006`）。
