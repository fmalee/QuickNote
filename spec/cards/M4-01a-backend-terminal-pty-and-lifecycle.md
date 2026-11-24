# M4-01a：后端 M4 契约补全（上）——内嵌终端 PTY 引擎、折叠保活与 30 分钟空闲回收

- 所属里程碑：M4
- 关联方案与契约：`v4` §5.6.7、§5.12、§5.14、§6 M4；`C1.2`、`C1.3`、`C1.4`、`C2.1`、`C2.3`、`C3.1`–`C3.4`、`C4.2`、`C4.3`、`C6.2`、`C8.7`、`C9.1`、`D-0005`、`D-0009`、`D-0010`
- 前置依赖：`m3-baseline`（`dev` 当前基线）
- 建议分支：`feat/M4-01a-backend-terminal-pty`
- 会话策略：独立新会话执行（零大文档读取，所有 Rust 结构体、状态机、缓冲/背压阈值、进程回收时序与 IPC/Channel 签名已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **新增终端依赖（`C1.3` 串行声明）**：
   - 在 `src-tauri/Cargo.toml` 添加 `portable-pty = "0.8"` 与 `libc = "0.2"`（base64 编解码使用纯 Rust RFC 4648 实现，无需引入额外 crate）。
2. **新增终端领域模型与纯状态机（`src-tauri/src/domain/terminal.rs`，零 IO / 零 `tauri` 依赖）**：
   - 按 `C3.4` 与 `C4.2`/`C4.3` 精确定义全部 IPC 结构体与枚举（全部派生 `Serialize, Deserialize, Type` 并标注 `#[serde(rename_all = "camelCase")]`，下划线变体按契约精确 `#[serde(rename = "...")]`）：
     ```rust
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
     pub enum TerminalLifecycleState {
         #[serde(rename = "active")]
         Active,
         #[serde(rename = "hidden_alive")]
         HiddenAlive,
         #[serde(rename = "recycled")]
         Recycled,
     }

     #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
     pub enum TerminalExitReason {
         #[serde(rename = "process_exit")]
         ProcessExit,
         #[serde(rename = "manual_close")]
         ManualClose,
         #[serde(rename = "idle_recycled")]
         IdleRecycled,
         #[serde(rename = "app_exit")]
         AppExit,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TerminalSpawnArgs {
         pub workspace_key: String,
         pub cwd: String,
         pub cols: u16,
         pub rows: u16,
         pub shell: Option<String>,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TerminalWriteArgs {
         pub session_id: String,
         pub data_base64: String,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TerminalResizeArgs {
         pub session_id: String,
         pub cols: u16,
         pub rows: u16,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TerminalVisibilityArgs {
         pub session_id: String,
         pub visible: bool,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TerminalKillArgs {
         pub session_id: String,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TerminalSessionInfo {
         pub session_id: String,
         pub workspace_key: String,
         pub cwd: String,
         pub has_active_foreground_child: bool,
         pub output_paused: bool,
         pub state: TerminalLifecycleState,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TerminalData {
         pub session_id: String,
         pub data_base64: String,
     }

     #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
     #[serde(rename_all = "camelCase")]
     pub struct TerminalExitPayload {
         pub session_id: String,
         pub exit_code: Option<i32>,
         pub reason: TerminalExitReason,
     }
     ```
   - 提供纯算法常量与组件（全部在 `domain/terminal.rs`）：
     - `pub const TERMINAL_RING_BUFFER_MAX_BYTES: usize = 1024 * 1024;`（1MB 折叠环形缓冲）
     - `pub const TERMINAL_BACKPRESSURE_LIMIT_BYTES: usize = 10 * 1024 * 1024;`（10MB 背压暂停阈值）
     - `pub const TERMINAL_FLUSH_INTERVAL_MS: u64 = 16;` 与 `pub const TERMINAL_FLUSH_BATCH_BYTES: usize = 64 * 1024;`
     - `pub const TERMINAL_IDLE_SWEEP_INTERVAL_SECS: u64 = 60;`
     - `TerminalRingBuffer`：固定上限 1MB 的字节环形缓冲（`push(&mut self, bytes: &[u8])` 超限时丢弃最旧字节保留最新 1MB；`drain_all(&mut self) -> Vec<u8>`；`clear(&mut self)`；`len(&self) -> usize`）。
     - `should_idle_recycle(state: TerminalLifecycleState, idle_recycle_minutes: u32, has_active_foreground_child: bool, last_io_at_ms: i64, now_ms: i64) -> bool`：仅当 `state == HiddenAlive && idle_recycle_minutes > 0 && !has_active_foreground_child && now_ms.saturating_sub(last_io_at_ms) >= (idle_recycle_minutes as i64) * 60_000` 返回 `true`。
     - RFC 4648 标准 `encode_base64(bytes: &[u8]) -> String` 与 `decode_base64(raw: &str) -> Result<Vec<u8>, AppError>`（严格校验非法字符/长度，返回 `ValidationError`）。
3. **PTY 底层基础设施（`src-tauri/src/infra/pty.rs`）**：
   - 使用 `portable-pty::native_pty_system()` 打开 PTY，以登录 shell 启动（`-l`，尊重 `args.shell` > `settings.panels.terminal.shell` > `$SHELL` > `/bin/zsh`，设置 `TERM=xterm-256color`、`COLORTERM=truecolor`、`LANG=zh_CN.UTF-8` 或继承环境 UTF-8），工作目录为展开 `~` 后存在且为目录的 `cwd`（不存在返回 `WorkspaceDirMissing`）。
   - 前台子进程检测（Unix/macOS）：从 PTY master 获取原始 fd 与子 shell `pgid`（`pid`），通过 `libc::tcgetpgrp(master_fd)` 对比 `shell_pgid`：`fg_pgid > 0 && fg_pgid != shell_pgid` 判定 `has_active_foreground_child = true`。
   - 零僵尸进程优雅终止协议（`C8.7` / `D-0005`）：
     - 向整个进程组发送 `libc::kill(-(pgid as i32), libc::SIGHUP)`；
     - 轮询 `try_wait()` 最长 `500ms`；
     - 若仍存活，等待 `200ms` 后发送 `libc::kill(-(pgid as i32), libc::SIGKILL)` 并强制 `wait()` 回收，杜绝僵尸进程。
   - 定义 `PtyBackend` / `PtySessionHandle` 抽象 trait（真实实现 `PortablePtyBackend` + 供无头测试/模拟时间/高吞吐背压测试的注入能力），使自动化集成测试既可跑真实 `/bin/sh` / `/bin/zsh` PTY，也可快速验证 10MB 背压与 30 分钟空闲回收状态机。
4. **终端会话管理器（`src-tauri/src/application/terminal.rs`）**：
   - `TerminalManager`（以 `Arc<TerminalManager>` 注入 Tauri `State`）：
     - **并发约束（`C8.7`）**：
       - 每个 `workspace_key` 最多 1 个活跃会话：若该工作区已有未退出会话，拒绝重复创建并返回 `AppError::new(ErrorCode::TerminalLimitReached, ...)`（或前端先查 `terminal_list_sessions` 复用）；
       - 全局并发会话数达到 `settings.panels.terminal.max_concurrent_sessions.max(1)`（默认 3）时拒绝新建并返回 `ErrorCode::TerminalLimitReached`。
     - **流式通道批处理（`C4.3`）**：
       - 后台读线程持续读取 PTY 输出，按 **16ms 或 64KB** 批量打包为 `TerminalData { session_id, data_base64 }` 发送至 `on_data` 回调（封装 `tauri::ipc::Channel<TerminalData>`）。
     - **折叠保活与 1MB 环形缓冲（`C8.7`）**：
       - `set_visibility(session_id, false)`：切换为 `HiddenAlive`，停止向前端 `Channel` 推送，新输出写入该会话的 1MB `TerminalRingBuffer` 并持续更新 `last_io_at_ms`；
       - `set_visibility(session_id, true)`：切换为 `Active`，将 `output_paused` 复位为 `false`（兼作暂停后的清空/恢复机制：若此前因 10MB 暂停，清空溢出标记并推送最新 `terminal:session-state`），将 1MB 环形缓冲中积压数据按 64KB 分片一次性吐回前端 `Channel` 后恢复流式推送。
     - **10MB 背压暂停保护（`v4` §5.6.7）**：
       - 维护当前流式窗口累计推送字节（或连续 1 秒内未节制刷屏 / 持续未消费输出累计达 `10MB`）；一旦触发，将 `output_paused = true`、停止向前端 `Channel` 继续轰炸、推送 `terminal:session-state` 事件；同时继续读取 PTY fd 丢弃旧溢出字节（避免阻塞子进程死锁），并在用户恢复（`set_visibility(id, true)` 重新激活/清空）时清零计数器并恢复正常显示。
     - **前台子进程巡检与 30 分钟空闲自动回收（`C8.7`）**：
       - 后台巡检线程每 `60s`（测试可配置更短周期）检查所有会话：
         1. 刷新 `has_active_foreground_child`；若较上次值发生变化，立即推送 `terminal:session-state`；
         2. 对 `HiddenAlive` 会话调用 `should_idle_recycle(...)`；命中时执行 `SIGHUP -> 500ms -> 200ms SIGKILL` 回收、将状态置为 `Recycled`、从活跃表移除并推送 `terminal:session-state` 与 `terminal:exit(reason = IdleRecycled)`。
       - 为实时指示点 `●` 灵敏度：可见（`Active`）会话或写输入后可按较短间隔（如 `1s–2s`）同步检测 `tcgetpgrp` 变化并推送 `terminal:session-state`；空闲回收仍严格遵循 `idle_recycle_minutes`（且支持测试直接调用 `sweep_idle_sessions(now_ms)` 进行确定性毫秒级验证）。
     - **手动关闭与应用退出零僵尸**：
       - `kill_session(session_id)` 按 `ManualClose` 执行 `SIGHUP -> SIGKILL` 回收并发送 `terminal:exit(reason = ManualClose)`；
       - PTY 子进程自然退出时，读线程收尾并发送 `terminal:exit(reason = ProcessExit, exit_code)`；
       - `shutdown_all()` 对全部存活会话执行 `AppExit` 回收，并在 `src-tauri/src/lib.rs` 的 `Builder::build(...).run(|app, event| { if let RunEvent::Exit = event { ... } })` 中兜底调用（注意将 `lib.rs` 末尾 `.run(...)` 升级为 `.build(...)?.run(...)` 模式）。
5. **IPC 命令、绑定导出与前端 IPC 封装**：
   - 新增 `src-tauri/src/commands/terminal.rs` 注册全部 6 条 `M4` 终端命令：
     - `terminal_spawn(app, state, pm, args: TerminalSpawnArgs, on_data: Channel<TerminalData>) -> Result<TerminalSessionInfo, AppError>`
     - `terminal_write(state, args: TerminalWriteArgs) -> Result<(), AppError>`
     - `terminal_resize(state, args: TerminalResizeArgs) -> Result<(), AppError>`
     - `terminal_set_visibility(app, state, args: TerminalVisibilityArgs) -> Result<(), AppError>`
     - `terminal_kill(app, state, args: TerminalKillArgs) -> Result<(), AppError>`
     - `terminal_list_sessions(state) -> Result<Vec<TerminalSessionInfo>, AppError>`
   - 在 `src-tauri/src/bindings.rs` 注册全部 10 个新增类型并运行 `pnpm gen:bindings` 更新 `src/bindings/types.ts`。
   - 在 `src/lib/ipc/commands.ts` 添加上述 6 条命令封装（`terminalSpawn(args, onData)` 使用 `@tauri-apps/api/core` 的 `Channel<TerminalData>`），在 `src/lib/ipc/events.ts` 添加 `onTerminalSessionState` 与 `onTerminalExit`。

### 明确不做
- 不在本卡安装 `xterm.js` 或编写前端终端 Vue 组件（归 `M4-02`）。
- 不在本卡实现 `settings_export`、`settings_import`、`note_export_pdf` 或 45% 侧栏辅助窗口（归 `M4-01b`）。

## 2. 允许修改文件白名单

### 共享串行文件（`C1.3` 显式声明）
- `src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`
- `src-tauri/src/bindings.rs`
- `src/bindings/types.ts`（仅由 `pnpm gen:bindings` 生成，禁止手改）
- `src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`

### 业务与测试文件
- `src-tauri/src/domain/terminal.rs`（新增）
- `src-tauri/src/domain/mod.rs`
- `src-tauri/src/infra/pty.rs`（新增）
- `src-tauri/src/infra/mod.rs`
- `src-tauri/src/application/terminal.rs`（新增）
- `src-tauri/src/application/mod.rs`
- `src-tauri/src/commands/terminal.rs`（新增）
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tests/m4_terminal_lifecycle.rs`（新增）
- `src/lib/ipc/__tests__/smoke.test.ts`

## 3. 具体实现要求

1. **严格遵守 `C2.1` 与 `C2.3`**：
   - `src-tauri/src/**` 生产代码零 `.unwrap()` / `.expect()`；所有锁与 IO 错误均映射为 `AppError`。
   - 严禁在任何 `eprintln!`、`tracing` 或 `AppError.detail` 中记录终端输入/输出字节流或其 base64 文本！仅可记录 `session_id`、字节数与 `ErrorCode`。
2. **严格遵守 `C1.2` 单向分层**：
   - `domain/terminal.rs` 严禁引入 `tauri`、`std::fs`、`portable_pty`、`libc`、`rusqlite`。
3. **退出清理与跨线程安全**：
   - `src-tauri/src/lib.rs` 改用 `.build(tauri::generate_context!())` + `.run(|app_handle, event| { if let tauri::RunEvent::Exit = event { ... } })`，确保退出时回收所有 PTY 会话及子进程组，不破坏既有 `SingleInstance`、托盘、菜单和窗口关闭拦截逻辑。
4. **SessionId 生成**：
   - 使用单调递增或时间戳+随机/原子计数器生成的不可预测短 ID（如 `term-<ts_ms>-<seq>`），无额外 crate 依赖。

## 4. 自动化验收标准（必须全绿）

- 新增集成测试 `src-tauri/tests/m4_terminal_lifecycle.rs`，至少覆盖：
  1. `TerminalRingBuffer` 超过 1MB 时精确保留最新 1MB 字节、`drain_all` 清空复位；
  2. `encode_base64` / `decode_base64` 往返与非法字符报错；
  3. 真实 PTY `spawn` → `write("echo hi\n")` → `on_data` 收到输出 → `resize(100, 30)` → `kill` 回收验证（进程退出且无僵尸进程）；
  4. 同工作区重复创建返回 `TerminalLimitReached`、全局满 3 并发上限返回 `TerminalLimitReached`；
  5. 折叠保活：切换 `visible = false` 期间输出蓄入 1MB 环形缓冲不推送前端，切回 `visible = true` 后一次性吐回缓冲数据；
  6. 10MB 背压暂停：连续注入超 10MB 输出触发 `output_paused = true` 并停止继续推送，调用恢复逻辑后清零恢复；
  7. 30 分钟空闲自动回收：`HiddenAlive` + 无前台子进程 + 超时 → 自动回收并发 `IdleRecycled`；存在前台子进程（`has_active_foreground_child = true`）或 `idle_recycle_minutes = 0` 或 `Active` 态 → 严格豁免不回收；
  8. `shutdown_all()` 一次性回收全部存活会话并触发 `AppExit`。
- 必过命令：`pnpm check:all`（含 `gen:bindings` 零差异、`check:arch`、`clippy -D warnings`、`cargo test`、`vitest`）。

## 5. 人工点验清单

- 本卡为纯后端引擎，无独立人工桌面点验项（将随 `M4-02` 前端终端面板合入后统一执行实机点验）。
