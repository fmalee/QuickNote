# PROJECT_STATE · QuickNote 总控状态板

> 主控会话接续唯一动态真源（控制 ≤ 80 行）。每次里程碑收官或启动新主控会话时同步更新，不重复 `AGENTS.md` / `engineering-contract.md` 静态正文。

## 1. 当前精确进度

- **当前阶段**：M0 已正式收官（基线标签 `m0-baseline`）；**M1（捕捉与笔记库核心闭环）** 全部 5 张任务卡（`M1-01`–`M1-05`）已完成拆分并落盘，准备进入 `M1-01` 施工。
- **已完成**：
  - M0 全阶段：`M0-01`–`M0-04`、M0 独立只读审查（`C10.3`）及 `fix/M0-audit`（`m0-baseline`）。
  - M1 阶段拆卡：`M1-01`（Fixture 库 / `meta + body` / YAML 最小补丁 / 混合 FTS5）、`M1-02`（`S1–S8` 真值表 / 分区追加 / 5 秒撤销 / `ToastAppendUndoPayload` 收口）、`M1-03`（`notify` 防回环 / 外部变更状态机 / 中文 Slug / `recovery/` & `quickbar-draft.json`）、`M1-04`（两层主界面 / 500 行膨胀提醒 / 多标签 / CM6 编辑器 / 右侧大纲面板）、`M1-05`（Quick Bar 记录模式完整闭环）。
- **待执行**：`M1-01` → `M1-02` → `M1-03` → `M1-04` → `M1-05` → M1 增量只读审查与实机点验。

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

## 4. 协作与防膨胀规矩

- **定向按需读文档**：施工卡仅通读 `AGENTS.md` + 当期任务卡，其余按卡内 `Cx.y` / `D-xxxx` / `§x.y` 定向查节。
- **增量审查防膨胀**：M1 起每张里程碑审查只看 `git diff <prev-tag>..dev` 增量与当期契约；代码量大时拆分后端 `src-tauri/` 与前端 `src/` 双视角审查。
- **动态卡滚动拆分**：仅在当前里程碑收官并打标后拆分下一里程碑卡片，不提前写死远期白名单。

## 5. 下一步动作

1. 在独立新会话中从 `dev` 拉出 `feat/M1-01-vault-fixtures-meta-body-and-fts` 执行 `M1-01`。
2. 按 `M1-01 → M1-05` 顺序逐卡推进并在 M1 全卡合入后执行增量只读审查。
