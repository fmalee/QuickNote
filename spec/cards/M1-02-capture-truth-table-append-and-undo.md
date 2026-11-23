# M1-02：Quick Bar `S1–S8` 目标真值表、分区追加、5 秒撤销与 `ToastAppendUndoPayload` 收口

- 所属里程碑：M1
- 关联方案与契约：`v4` §1.2–§1.5、§3.5.1、§4.4、§4.11、§6 M1；`C1.2`、`C1.3`、`C2.1`、`C3.1`–`C3.4`、`C4.2`、`C4.3`、`C6.1`、`C8.1`–`C8.3`、`C9.1`、`C9.2`、`D-0001`、`D-0006`、`D-0009`、`D-0010`
- 前置依赖：`M1-01b`
- 会话策略：独立新会话执行（零大文档读取，所有结构体、真值表与算法已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 在 `src-tauri/src/domain/capture.rs` 定义 IPC 结构体（均加 `#[derive(..., Serialize, Deserialize, Type)]` 与 `#[serde(rename_all = "camelCase")]`；注意 `CaptureMode` 已在 `domain::ui_state` 定义，直接复用）**：
    - `TargetKind`: 枚举 `Unconfigured | Note | Inbox`（序列化为 `"unconfigured" | "note" | "inbox"`）
    - `TargetResolveInput { pub main_visible: bool, pub active_doc_key: Option<String>, pub selected_workspace_key: Option<String>, pub is_inbox_view: bool }`
    - `ResolvedTarget { pub scenario: String, pub kind: TargetKind, pub target_rel_path: Option<String>, pub target_title: String, pub section_hint: Option<String>, pub workspace_key: Option<String>, pub workspace_name: Option<String>, pub is_cross_workspace_fallback: bool, pub recommended_mode: CaptureMode, pub chip_label: String }`
    - `AppendNoteArgs { pub target_rel_path: String, pub content: String, pub mode: CaptureMode, pub workspace_key: Option<String> }`
    - `AppendResult { pub rel_path: String, pub title: String, pub added_lines: u32, pub section_heading: Option<String>, pub body_hash: String, #[specta(type = specta_typescript::Number)] pub mtime_ms: i64, pub undo_available: bool }`
    - `UndoAppendResult { pub rel_path: String, pub restored_body_hash: String, #[specta(type = specta_typescript::Number)] pub mtime_ms: i64 }`
  - **2. 在 `src-tauri/src/domain/capture.rs` 实现 `C8.1` Quick Bar `S1–S8` 目标解析无副作用纯函数 `resolve_target`**：
    - **全量真值表（8 场景，不得偏离）**：
      - **S1**：`library_root == None` → `scenario = "S1"`, `kind = Unconfigured`, `target_rel_path = None`, `target_title = "未选库"`, `is_cross_workspace_fallback = false`, `recommended_mode = List`, `chip_label = "请先选择或创建笔记库"`。
      - **S2**：`main_visible && active_doc != None`（且在库中能解析到该文档） → `scenario = "S2"`, 目标取 `active_doc`（若为 `inbox.md` 则 `kind = Inbox`，否则 `kind = Note`；工作区随 `active_doc.project`）。
      - **S3**：`main_visible && active_doc == None && selected_workspace == Some(W) && last_active_doc (24h TTL 内且文件存在) && last_active_doc.workspace == Some(W)` → `scenario = "S3"`, 目标取 `last_active_doc`（同工作区沿用）。
      - **S4**：`main_visible && active_doc == None && selected_workspace == Some(W)` 且 `last_active_doc` 缺失/过期 或 属于异区 `W'`（`!= Some(W)`） → `scenario = "S4"`：**跨工作区/空档保护**：若 `W.default_entry_doc` 配置且存在则目标取 `default_entry_doc`（`kind = Note`），否则目标取 `"inbox.md"`（`kind = Inbox`）；**当因为 `last_active_doc` 存在（未过期）但属于异区 `W'` 触发回退至 `inbox.md` 时，`is_cross_workspace_fallback = true`，`chip_label = "收件箱 · 跨工作区"`**。
      - **S5**：`main_visible && active_doc == None && selected_workspace == None` → `scenario = "S5"`：若 `is_inbox_view == true` 则目标取 `"inbox.md"`；否则若 24h 内 `last_active_doc` 存在则取 `last_active_doc`，无则取 `"inbox.md"`。
      - **S6**：`!main_visible && selected_workspace == Some(W) && last_active_doc (24h 内) && last_active_doc.workspace == Some(W)` → `scenario = "S6"`：**忽略隐藏态 `active_doc`**，目标取 `last_active_doc`。
      - **S7**：`!main_visible && selected_workspace == Some(W)` 且 `last_active_doc` 缺失/过期 或 属于异区 `W'` → `scenario = "S7"`：有 `W.default_entry_doc` 则取之，否则取 `"inbox.md"`；若因 `last_active_doc` 属于异区 `W'` 回退至 `inbox.md`，标记 `is_cross_workspace_fallback = true`，`chip_label = "收件箱 · 跨工作区"`。
      - **S8**：`!main_visible && selected_workspace == None` → `scenario = "S8"`：24h 内 `last_active_doc` 存在则取之（工作区随该文档），否则取 `"inbox.md"`。
    - **推荐模式 `recommended_mode` 与 `section_hint` / `chip_label` 规则**：
      - 当目标为 `"inbox.md"`（`kind == Inbox`）时：`recommended_mode` 固定为 `CaptureMode::List`；`section_hint = Some(today_date)`（如 `"2026-11-24"`）；若 `is_cross_workspace_fallback == true` 则 `chip_label = "收件箱 · 跨工作区"`，否则 `chip_label = "inbox.md › 今日"`（或 `"收件箱 › YYYY-MM-DD"`）。
      - 当目标为普通文档（`kind == Note`）时：按所属工作区 `workspace_key` 从 `ui_state.workspace_modes` 查找记忆模式（若无工作区或未记忆，默认 `CaptureMode::List`；若记忆为 `Todo` 或文档含 `## 待办` 分区可沿用记忆模式）；`section_hint` 在 `List` 下为 `"想法"`（若文档缺 `## 想法` 且 `!auto_create_missing_section` 则为 `"将追加到文末"`）、`Todo` 下为 `"待办"`（缺 `## 待办` 且不自动创建时为 `"将追加到文末"`）、`Prose` 下为 `None`；`chip_label` 格式：有工作区名为 `"{target_title} › {section} · {workspace_name}"`，无工作区为 `"{target_title} › {section}"`。
  - **3. 在 `src-tauri/src/domain/capture.rs` 实现 `C8.2` 分区扫描、精确插入与前缀去重纯函数 `append_to_body`**：
    - **前缀去重规则**（对 `content` 按行处理，保留行内文本）：
      - `CaptureMode::List`：每个非空行若已以 `- `、`* `、`+ ` 或有序列表 `^\d+\.\s` 开头，不再重复加 `- `；**当目标为 `inbox.md` 时**，列表符后必须带 `HH:mm ` 时间戳（如 `- 14:05 内容`；若用户已输入 `- 内容` 但无 `^\d{2}:\d{2}\s` 前缀，则在列表符后补入 `{hh_mm} `；若已带 `- 14:05 内容` 则原样保留）。
      - `CaptureMode::Todo`：每个非空行若已以 `- [ ] ` 或 `- [x] ` / `- [X] ` 开头则保留原样；若以普通列表符 `- `、`* `、`+ ` 开头则将该列表符替换为 `- [ ] `；其余非空行前置 `- [ ] `。
      - `CaptureMode::Prose`：原样保留段落与换行，不加列表前缀。
    - **分区扫描与插入位置**：
      - 按行扫描 `body` 并追踪围栏代码块（`` ``` `` 或 `~~~`），**严禁匹配代码块内部的假标题**（验证样本：`code-fence-heading.md`）。
      - 目标二级标题：`inbox.md` 匹配 `## {YYYY-MM-DD}`；普通文档在 `List` 下匹配 `## 想法`，`Todo` 下匹配 `## 待办`，`Prose` 不找特定二级标题直接追加到文末。若有多个同名二级标题，取代码块外的**第一个**。
      - 命中标题后：从标题下一行向下扫描，直到遇到代码块外的下一个一级或二级标题（`# ` 或 `## `）或 EOF；回退跳过区间末尾的连续空行，紧接该分区最后一个非空内容行之后插入新行（与后方下一个标题之间保持一个空行分隔）。
      - 未命中标题时：
        - 若目标是 `inbox.md`：在文末自动新建 `## {YYYY-MM-DD}` 标题（前留一个空行）并追加格式化后的条目，`section_heading = Some("## YYYY-MM-DD")`。
        - 若目标是普通文档：当 `auto_create_missing_section == true` 且模式为 `List`/`Todo` 时在文末新建 `## 想法` 或 `## 待办` 并追加；当 `auto_create_missing_section == false`（默认）时直接追加到文末，`section_heading = None`。
  - **4. 在 `src-tauri/src/application/capture.rs` 与 `src-tauri/src/commands/capture.rs` 实现 `C8.3` 5 秒单槽撤销快照与 3 条 IPC 命令**：
    - `CaptureManager`（由 Tauri `app.manage(Arc<CaptureManager>)` 托管）：内部持有一个 `Mutex<Option<LastAppendUndo>>`，其中 `LastAppendUndo { rel_path: String, after_body_hash: String, before_body: String, created_at_ms: i64 }`，单篇原正文上限 `MAX_UNDO_BODY_BYTES = 5 * 1024 * 1024`（5MB）。
    - `capture_resolve_target(args: TargetResolveInput)`：读取当前 `PersistenceManager` 的 `settings`、`ui_state` 与 SQLite 索引文档列表，调用 `domain::capture::resolve_target` 返回 `ResolvedTarget`。
    - `note_append(args: AppendNoteArgs)`：
      - 调用时读取当前系统本地时间 `Local::now()` 生成 `YYYY-MM-DD` 与 `HH:mm`。
      - 若目标为 `"inbox.md"` 且磁盘尚不存在，自动初始化最小 `inbox.md`。
      - 读取磁盘原文件并 `parse_note` 拆出 `block`（原始 YAML frontmatter）与 `before_body`；**严守 `D-0006`：`note_append` 绝不修改、绝不补写 YAML frontmatter（哪怕是 `missing-id.md` 也必须 100% 保持原 frontmatter 字节不变）**！
      - 调用 `append_to_body` 生成 `after_body` 与 `after_body_hash`；若 `before_body.len() <= 5MB`，存入单槽快照 `LastAppendUndo`（`undo_available = true`），若 `> 5MB` 则清空快照且 `undo_available = false`。
      - 使用 `compose_note(parsed.block.as_deref(), &after_body)` 原子写盘，调用 `index_single_note` 刷新 SQLite 索引，并更新 `ui-state.json` 的 `last_active_doc`（`rel_path`, `workspace_key`, `touched_at_ms = now_ms`）与 `workspace_modes`（非 `inbox.md` 且有 `workspace_key` 时记住 `args.mode`）。
      - 若在 Tauri 运行时上下文（`AppHandle` 存在），调用 `WindowManager::show_append_toast(app, &append_result)` 显示 5 秒非激活 Toast 浮窗并 emit `"toast:append-undo"` 事件（载荷即 `AppendResult`）。
    - `note_undo_last_append()`：
      - 取出单槽快照（无快照则返回 `ErrorCode::UndoUnavailable`）。
      - 读取磁盘当前目标文件并 `parse_note` 计算 `current_body_hash`；仅当 `current_body_hash == snapshot.after_body_hash` 时，**保留磁盘当前的 `parsed.block`（即若期间用户通过 `note_update_meta` 改了 frontmatter，最新 `meta` 原样保留）**，与 `snapshot.before_body` 拼接原子写回磁盘并调用 `index_single_note` 刷新索引，返回 `UndoAppendResult { rel_path, restored_body_hash, mtime_ms }`；若 `current_body_hash != snapshot.after_body_hash`，拒绝回滚并返回 `ErrorCode::UndoUnavailable`。
  - **5. 收口 M0 挂起债务 `ToastAppendUndoPayload`**：
    - 在 `src-tauri/src/bindings.rs` 注册 `TargetKind`、`TargetResolveInput`、`ResolvedTarget`、`AppendNoteArgs`、`AppendResult`、`UndoAppendResult`，运行 `pnpm gen:bindings` 更新 `src/bindings/types.ts`。
    - 在 `src/lib/ipc/commands.ts` 新增 `captureResolveTarget`、`noteAppend`、`noteUndoLastAppend`；在 `src/lib/ipc/events.ts` 新增 `onToastAppendUndo(handler: (payload: AppendResult) => void)`。
    - 修改 `src/features/toast/api.ts`、`src/features/toast/store.ts`、`src/features/toast/ToastView.vue`、`src/features/toast/__tests__/toast-view.test.ts`：彻底移除手写的 `ToastAppendUndoPayload`，统一使用 `@/bindings/types` 生成的 `AppendResult`（或将其作为 `AppendResult` 的类型导出别名），并在 `useToastStore.triggerUndo()` 中调用 `noteUndoLastAppend()`。
- 不做：
  - 不实现 `inbox_extract_to_note` 或 `⌥⌘↵` 模板另存新笔记（归 `M2`）。
  - 不实现 `notify` 文件外部变更监听与防回环（归 `M1-03`）。
  - 不改写 Quick Bar 记录模式的完整前端 UI（归 `M1-05`）。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（自动生成）、`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`
- Rust 领域、应用与命令层：`src-tauri/src/domain/mod.rs`、`src-tauri/src/domain/capture.rs`、`src-tauri/src/application/mod.rs`、`src-tauri/src/application/capture.rs`、`src-tauri/src/application/window.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/commands/capture.rs`、`src-tauri/src/lib.rs`
- 前端 Toast 债务收口：`src/features/toast/api.ts`、`src/features/toast/store.ts`、`src/features/toast/ToastView.vue`、`src/features/toast/__tests__/toast-view.test.ts`
- Rust 集成测试：`src-tauri/tests/capture_truth_table_test.rs`、`src-tauri/tests/append_and_undo_test.rs`

## 3. 具体实现要求
- **严守单向分层与零 Panic（`C1.2`、`C2.1`）**：`src-tauri/src/domain/capture.rs` 严禁依赖 `tauri`、`std::fs`、`rusqlite`；生产代码零 `.unwrap()` / `.expect()` / `panic!()`。
- **Tauri 命令参数约定**：`capture_resolve_target` 与 `note_append` 统一接收 `args: TargetResolveInput` / `args: AppendNoteArgs`（前端 `invoke('note_append', { args })`）；`note_undo_last_append` 无入参。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src-tauri/tests/capture_truth_table_test.rs`、`src-tauri/tests/append_and_undo_test.rs`、`src/features/toast/__tests__/toast-view.test.ts`
  - `capture_truth_table_test.rs`：表驱动覆盖 `S1–S8` 全 8 个场景（含 24h TTL 过期边界、同工作区沿用、异工作区回退 `defaultEntryDoc` 与回退 `inbox.md` 时 `isCrossWorkspaceFallback == true` 及 `chipLabel == "收件箱 · 跨工作区"`、工作区模式记忆与 `inbox.md` 强制默认 `list`）。
  - `append_and_undo_test.rs`：使用 `inbox.md`、`missing-id.md`、`code-fence-heading.md` 与 `>5MB` 动态大文本验证：跨午夜本地日期标题创建与追加、`list`/`todo`/`prose` 前缀去重、跳过代码块内假标题、`missing-id.md` 追加后 frontmatter 零改动、5 秒快照一键回滚（连同顺带新建的当日日期标题一并回滚）、正文被改后拒撤返回 `ErrorCode::UndoUnavailable`、仅通过 `note_update_meta` 改 `meta` 后撤销依然成功且保留最新 `meta`、`>5MB` 文档 `undo_available == false`。
  - `toast-view.test.ts`：验证基于生成的 `AppendResult` 渲染 `toast:append-undo` 并在点击 `[撤销]` 时触发 `note_undo_last_append` IPC 调用。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡主要为 Rust 纯算法、追加/撤销命令与 Toast 类型收口，实机端到端捕捉点验在 `M1-05` 统一完成；本卡以 `pnpm check:all` 全绿为准。
