# PROJECT_STATE · QuickNote 总控状态板

> 主控会话接续唯一动态真源（控制 ≤ 80 行）。每次里程碑收官或启动新主控会话时同步更新，不重复 `AGENTS.md` / `engineering-contract.md` 静态正文。

## 1. 当前精确进度

- **当前阶段**：M0 已正式收官（基线标签 `m0-baseline`），准备进入 **M1（捕捉与笔记库核心闭环）**。
- **已完成**：`M0-01`（脚手架与 9 步门禁）、`M0-02`（`settings.yaml` 容错 / `ui-state.json` / SQLite `0001_init.sql`）、`M0-03`（原生 `NSPanel` / 动态 `.accessory ↔ .regular` / `⌃⌥Space`）、`M0-04`（`--qn-*` 双主题 / 四窗口壳 / 首次选库 `S1`）、M0 独立只读审查（`C10.3`），以及 `fix/M0-audit`（`32112f5`，收口热键重启持久化、Toast 圆角无私有 API 透明、`App.vue` 启动报错与架构扫描加固）。

## 2. 跨里程碑挂起债务（拆卡必查）

| 挂起项 | 来源 | 目标里程碑 | 处理要求 |
| --- | --- | --- | --- |
| `ToastAppendUndoPayload` 手写类型收口 | M0 审查 🟡#4 | **M1** | 随 `AppendResult` 落地收归 Rust 生成并接入 `src/lib/ipc/events.ts` |
| 默认模板（`note/doc/command-book`）格式对齐 | M0 审查 🟢#4 | **M2** | `src-tauri/src/infra/fs.rs` 与 `v4` 附录 B.3 标题/字段完全对齐 |
| `SettingsErrorBanner` “打开 `settings.yaml` 修复”按钮 | M0 审查 🟢#4 | **M3** | 接入 `external_action_run`（`D-0008` / `v4` §4.7） |
| `QuickBarView` 与 `bridge.m` 幂等双 `Esc` | M0 审查 🟢#4 | **保留现状** | 保持不动，避免破坏已实机通过的 Quick Bar 原生焦点与全屏行为 |

## 3. 已验证实机约定与踩坑备忘

- **辅助窗口 URL 路由**：`src-tauri/src/lib.rs` 已固定为 `index.html?window=quickbar|toast|settings`，主窗口为无参 `index.html`。
- **原生 Toast 零私有 API**：严禁开启 Tauri `.transparent(true)`（依赖 `macos-private-api`）；采用 `bridge.m` `NSPanel` `opaque = NO` + `clearColor` + `CALayer` 圆角裁剪（`cornerRadius = 10`）。
- **目录选择方式**：M0 暂不引入 `tauri-plugin-dialog`，选库使用路径输入框（默认 `~/Documents/QuickNote`）+ 自动创建开关。
- **`serde_yaml 0.9` 报错特性**：语法缩进错误能精准返回行列号；`deny_unknown_fields` 未知字段错误不带行列号，报错文案与测试断言须明确命中字段名而非强求行列号。
- **macOS 原生桥接选型**：已在 `DECISIONS.md` `D-0011` 锁定 `cc` 编译 `src-tauri/src/infra/macos/bridge.m`（ObjC 静态桥接）。

## 4. 协作与防膨胀规矩

- **定向按需读文档**：施工卡仅通读 `AGENTS.md` + 当期任务卡，其余按卡内 `Cx.y` / `D-xxxx` / `§x.y` 定向查节。
- **增量审查防膨胀**：M1 起每张里程碑审查只看 `git diff <prev-tag>..dev` 增量与当期契约；代码量大时拆分后端 `src-tauri/` 与前端 `src/` 双视角审查。
- **动态卡滚动拆分**：仅在当前里程碑收官并打标后拆分下一里程碑卡片，不提前写死远期白名单。

## 5. 下一步动作

1. 基于 `m0-baseline` 与上述 M1 挂起债务，拆分 **M1（捕捉与笔记库核心闭环）** 阶段任务卡（含共享 Fixture 库）。
2. 从 `dev` 拉出 `M1-01` 分支并依卡逐个推进。
