# PROJECT_STATE · QuickNote 总控状态板

> 主控会话接续唯一动态真源（控制 ≤ 80 行）。每次里程碑收官或启动新主控会话时同步更新，不重复 `AGENTS.md` / `engineering-contract.md` 静态正文。

## 1. 当前精确进度

- **当前阶段**：M0 已正式收官（`m0-baseline`）；正推进 **M1（捕捉与笔记库核心闭环，共 7 张轻量卡）**。
- **已完成**：
  - M0 全阶段（`m0-baseline`）。
  - **`M1-01a`**（`9dbf2d3`：共享 Fixture 库、`domain/{tokenizer,frontmatter,note}.rs` 纯算法与 TS 绑定导出）。
  - **`M1-01b`**（`2ba211d`：SQLite FTS5 派生索引入库、笔记 CRUD 用例与 11 条 IPC 命令封装）。
- **待执行**：**`M1-02`**（当前待施工） → `M1-03` → `M1-04a` → `M1-04b` → `M1-05` → M1 增量只读审查。

## 2. 跨里程碑挂起债务（拆卡必查）

| 挂起项 | 来源 | 目标里程碑 | 处理要求 |
| --- | --- | --- | --- |
| `ToastAppendUndoPayload` 手写类型收口 | M0 审查 🟡#4 | **M1（已排入 `M1-02`）** | 随 `AppendResult` 落地收归 Rust 生成并接入 `src/lib/ipc/events.ts`，移除 `src/features/toast/api.ts` 手写接口 |
| 默认模板（`note/doc/command-book`）格式对齐 | M0 审查 🟢#4 | **M2** | `src-tauri/src/infra/fs.rs` 与 `v4` 附录 B.3 标题/字段完全对齐 |
| `SettingsErrorBanner` “打开 `settings.yaml` 修复”按钮 | M0 审查 🟢#4 | **M3** | 接入 `external_action_run`（`D-0008` / `v4` §4.7） |
| `QuickBarView` 与 `bridge.m` 幂等双 `Esc` | M0 审查 🟢#4 | **保留现状** | 保持不动，避免破坏已实机通过的 Quick Bar 原生焦点与全屏行为 |

## 3. 已验证实机约定与踩坑备忘

- **辅助窗口 URL 路由**：`src-tauri/src/lib.rs` 已固定为 `index.html?window=quickbar|toast|settings`，主窗口为无参 `index.html`。
- **原生 Toast 零私有 API**：严禁开启 Tauri `.transparent(true)`（依赖 `macos-private-api`）；采用 `bridge.m` `NSPanel` `opaque = NO` + `clearColor` + `CALayer` 圆角裁剪（`cornerRadius = 10`）。
- **目录选择方式**：M0 暂不引入 `tauri-plugin-dialog`，选库使用路径输入框（默认 `~/Documents/QuickNote`）+ 自动创建开关。
- **`serde_yaml 0.9` 报错特性**：语法缩进错误能精准返回行列号；`deny_unknown_fields` 未知字段错误不带行列号，报错文案与测试断言须明确命中字段名而非强求行列号。
- **macOS 原生桥接选型**：已在 `DECISIONS.md` `D-0011` 锁定 `cc` 编译 `src-tauri/src/infra/macos/bridge.m`（ObjC 静态桥接）。

## 4. 协作与防膨胀规矩（含 Token 省流铁律，已写入 `AGENTS.md`）

- **施工会话零大文档读取**：任务卡与施工提示词内联所需函数签名、结构体定义、SQL 列名与边界要求；**施工会话禁止读取 `spec/quicknote-design-v4.md`（1150 行）、`spec/engineering-contract.md`（745 行）、`DECISIONS.md` 及 `fixtures/vault/` 大样本文件**，只读 `AGENTS.md` + 当期任务卡 + 白名单目标源码。
- **单卡体量控制**：单卡新增/修改生产代码控制在 ≤ 800 行，超过立即拆为 `a/b` 子卡（如 `M1-01a/b`、`M1-04a/b`）。
- **增量审查防膨胀**：M1 起每张里程碑审查只看 `git diff <prev-tag>..dev` 增量与当期契约。

## 5. 下一步动作

1. 在独立新会话中从 `dev` 拉出 `feat/M1-02-capture-truth-table-append-and-undo` 执行 `M1-02`。
2. 完成后回到主控会话获取 `M1-03` 极简提示词。
