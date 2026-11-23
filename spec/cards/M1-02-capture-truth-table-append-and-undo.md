# M1-02：Quick Bar `S1–S8` 目标真值表、分区追加、5 秒撤销与 `ToastAppendUndoPayload` 收口

- 所属里程碑：M1
- 关联方案与契约：`v4` §1.2–§1.5、§3.5.1、§4.4、§4.11、§6 M1；`C1.2`、`C1.3`、`C2.1`、`C3.1`–`C3.4`、`C4.2`、`C4.3`、`C6.1`、`C8.1`–`C8.3`、`C9.1`、`C9.2`、`D-0001`、`D-0006`、`D-0009`、`D-0010`
- 前置依赖：`M1-01b`
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - 在 `domain::capture` 实现 `C8.1` Quick Bar `S1–S8` 目标解析无副作用纯函数 `resolve_target`，将“当前工作区（`selectedWorkspace`）”与“激活文档（`activeDoc`）”彻底解耦，完整支持 24h `lastActiveDoc`、工作区 `defaultEntryDoc`、跨工作区回退保护（`isCrossWorkspaceFallback = true`，`chipLabel = "收件箱 · 跨工作区"`）与按工作区记忆 `recommendedMode`（`inbox.md` 固定默认 `list`）。
  - 在 `domain::capture` 实现 `C8.2` 分区扫描、精确插入与前缀去重纯函数：
    - 运行时本地时区 `Local::now()` 生成 `YYYY-MM-DD` 与 `HH:mm`。
    - 前缀去重：`list` 模式识别已有 `- `、`* `、`+ `、`^\d+\.\s` 不重复叠加（`inbox.md` 缺失 `HH:mm ` 时在列表符后补时间戳）；`todo` 模式识别 `- [ ] ` / `- [x] ` 保留、`- ` / `* ` 替换为 `- [ ] `；`prose` 模式原样保留。
    - 分区定位：追踪 `` ``` `` / `~~~` 围栏代码块，仅在代码块外匹配首个精确二级标题（`inbox.md` 匹配 `## YYYY-MM-DD`，缺失则在文末新建当日日期标题；普通文档 `list` 匹配 `## 想法`、`todo` 匹配 `## 待办`，缺失时按 `settings.quick_bar.auto_create_missing_section` 决定新建或追加文末），跳过区间尾随空行后精确插入。
  - 实现 `C8.3` 追加 5 秒单槽内存快照撤销（`LastAppendUndo { relPath, afterBodyHash, beforeBody, createdAt }`，原正文 `<= 5MB` 时保存）及 IPC 命令：`capture_resolve_target`、`note_append`（**严守 `D-0006` 绝不修改 YAML frontmatter**，追加成功后更新 SQLite 索引与 `lastActiveDoc`，并触发 5 秒非激活 Toast 浮窗推送 `toast:append-undo` 事件）、`note_undo_last_append`（校验磁盘当前 `bodyHash == afterBodyHash` 时原样保留当前 `meta` 回滚 `beforeBody`，否则返回 `ErrorCode::UndoUnavailable`）。
  - **收口 M0 挂起债务**：在 `src-tauri/src/bindings.rs` 导出 `AppendResult` 等捕捉结构体，在 `src/lib/ipc/events.ts` 新增 `onToastAppendUndo(handler: (payload: AppendResult) => void)`，删除 `src/features/toast/api.ts` 中手写的 `ToastAppendUndoPayload` 接口，改为直接引用 `@/bindings/types` 生成的 `AppendResult`，并在 `useToastStore.triggerUndo()` 中真实调用 `noteUndoLastAppend()`。
- 不做：
  - 不实现 `inbox_extract_to_note`（选中移到新笔记归 `M2`）或 `⌥⌘↵` 模板另存新笔记（归 `M2`）。
  - 不实现 `notify` 文件外部变更监听与编辑器 `recovery/` 崩溃恢复（归 `M1-03`）。
  - 不在本卡改写 Quick Bar 记录模式的完整前端 UI（归 `M1-05`，本卡仅完成后端命令、IPC 封装与 Toast 撤销闭环收口）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（自动生成）、`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`
- Rust 领域、应用与命令层：`src-tauri/src/domain/mod.rs`、`src-tauri/src/domain/capture.rs`、`src-tauri/src/application/mod.rs`、`src-tauri/src/application/capture.rs`、`src-tauri/src/application/window.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/commands/capture.rs`、`src-tauri/src/lib.rs`
- 前端 Toast 债务收口：`src/features/toast/api.ts`、`src/features/toast/store.ts`、`src/features/toast/ToastView.vue`、`src/features/toast/__tests__/toast-view.test.ts`
- Rust 集成测试：`src-tauri/tests/capture_truth_table_test.rs`、`src-tauri/tests/append_and_undo_test.rs`

## 3. 具体实现要求
- **`S1–S8` 真值表零偏差（`C8.1`）**：`domain::capture::resolve_target` 必须逐项覆盖 `S1`（未选库 `unconfigured`）至 `S8`（主窗隐藏且无锁定工作区），主窗口隐藏（`mainVisible == false`）时 `activeDoc` 绝对不参与默认目标决策；跨工作区回退（`S4`、`S7` 中 `lastActiveDoc` 属于异区 `W'`）且无 `defaultEntryDoc` 时，`isCrossWorkspaceFallback` 必须为 `true` 且 `chipLabel` 显示 `收件箱 · 跨工作区`。
- **`note_append` 零 frontmatter 篡改（`D-0006`）**：对 `missing-id.md` 执行 `note_append` 后，其 frontmatter 必须保持无 `id` 原状（ULID 补写仅允许发生在 `note_write`）。
- **围栏代码块防误判（`C8.2`、`C9.2`）**：对 `code-fence-heading.md`（代码块内含假 `## 想法` / `## 待办`）执行追加时，必须跳过围栏代码块内部，精确插入到代码块外的真实二级标题分区末尾。
- **撤销快照哈希与 5MB 边界（`C8.3`）**：当原正文 `> 5MB` 时 `AppendResult.undoAvailable = false` 且不保存快照；当追加后至点击撤销前文件正文被二次修改（`currentBodyHash != afterBodyHash`），`note_undo_last_append` 必须拒绝覆写并返回 `ErrorCode::UndoUnavailable`，但若期间仅通过 `note_update_meta` 修改了 frontmatter（`bodyHash` 未变），撤销仍须成功且保留最新 `meta`。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/capture_truth_table_test.rs`、`src-tauri/tests/append_and_undo_test.rs`、`src/features/toast/__tests__/toast-view.test.ts`
  - `capture_truth_table_test.rs`：表驱动覆盖 `S1–S8` 全 8 个场景（含 24h TTL 过期边界、同工作区沿用、异工作区回退 `defaultEntryDoc` 与回退 `inbox.md`、工作区模式记忆与 `inbox.md` 强制默认 `list`）。
  - `append_and_undo_test.rs`：使用 `inbox.md`、`missing-id.md`、`code-fence-heading.md` 与 `>5MB` 动态大文本验证：跨午夜本地日期标题创建与追加、`list`/`todo`/`prose` 前缀去重、跳过代码块内假标题、`missing-id.md` 追加后 frontmatter 零改动、5 秒快照一键回滚（连同顺带新建的当日日期标题一并回滚）、正文被改后拒撤返回 `UndoUnavailable`、仅改 `meta` 后撤销成功且保留新 `meta`。
  - `toast-view.test.ts`：验证基于生成的 `AppendResult` 渲染 `toast:append-undo` 并在点击 `[撤销]` 时调用 `note_undo_last_append`。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡主要为 Rust 纯算法、追加/撤销命令与 Toast 类型收口，实机端到端捕捉点验在 `M1-05` 统一完成；本卡以 `pnpm check:all` 全绿为准。
