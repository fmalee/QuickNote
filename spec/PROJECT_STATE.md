# PROJECT_STATE · QuickNote 总控状态板

> 主控会话接续唯一动态真源（控制 ≤ 80 行）。每次里程碑收官或启动新主控会话时同步更新，不重复 `AGENTS.md` / `engineering-contract.md` 静态正文。

## 1. 当前精确进度

- **当前阶段**：M0（`m0-baseline`）、M1（`m1-baseline`）与 M2（`m2-baseline`）均已正式收官；**M3（快速打开、外部协作、完整命令库与面板组）** 已完成 7 张自包含轻量任务卡拆分（`M3-01a → M3-01b → M3-02 → M3-03 → M3-04 → M3-05a → M3-05b`），准备按卡逐一施工。
- **已完成**：
  - M0 全阶段（`m0-baseline`）。
  - M1 全阶段（`m1-baseline`：`M1-01a`–`M1-05` + M1 独立只读审查与 `fix/M1-audit` 实机点验收官）。
  - M2 全阶段（`m2-baseline`：`M2-01`–`M2-05b` + M2 独立只读审查 + `fix/M2-audit` `be79e07` + `fix/M2-qa` `4d33f0d` 实机点验收官）。
- **待执行**：M3 后端契约已 100% 落盘（`M3-01a` `5e450c7`、`M3-01b` `b82f490`），按 `spec/cards/README.md` 顺序串行推进前端 5 卡（`M3-02` 至 `M3-05b`），随后进行 M3 独立只读审查与 macOS 实机点验。

## 2. 跨里程碑挂起债务（拆卡必查）

| 挂起项 | 来源 | 目标里程碑 | 处理要求 |
| --- | --- | --- | --- |
| `ToastAppendUndoPayload` 手写类型收口 | M0 审查 🟡#4 | **M1（已在 `M1-02` `b8d6018` 收口）** | 已随 `AppendResult` 收归 Rust 生成并接入 `src/lib/ipc/events.ts` |
| 默认模板（`note/doc/command-book`）格式对齐 | M0 审查 🟢#4 | **M2（已在 `M2-01` `87f1a44` 收口）** | 已在 `domain::template` 单源定义并与 `v4` 附录 B.3 逐字对齐 |
| `SettingsErrorBanner` “打开 `settings.yaml` 修复”按钮 | M0 审查 🟢#4 | **M3（已排入 `M3-01b` + `M3-02`）** | `M3-01b` 落地 `$SETTINGS_YAML` 路由、`M3-02` 按钮接入 `external_action_run`（`D-0008` / `v4` §4.7） |
| `QuickBarView` 与 `bridge.m` 幂等双 `Esc` | M0 审查 🟢#4 | **保留现状** | 保持不动，避免破坏已实机通过的 Quick Bar 原生焦点与全屏行为 |
| `collectTodoDecorations` 超长文档单遍扫描优化 | M2 审查 🟢#2 | **M4** | 随 M4 性能压测专项（大文档/万篇压测）统一优化 |

## 3. 已验证实机约定与踩坑备忘

- **辅助窗口 URL 路由**：`src-tauri/src/lib.rs` 已固定为 `index.html?window=quickbar|toast|settings`，主窗口为无参 `index.html`。
- **原生 Toast 零私有 API**：严禁开启 Tauri `.transparent(true)`（依赖 `macos-private-api`）；采用 `bridge.m` `NSPanel` `opaque = NO` + `clearColor` + `CALayer` 圆角裁剪（`cornerRadius = 10`）。
- **目录选择方式**：暂不引入 `tauri-plugin-dialog`，选库使用路径输入框（默认 `~/Documents/QuickNote`）+ 自动创建开关。
- **`serde_yaml 0.9` 报错特性**：语法缩进错误能精准返回行列号；`deny_unknown_fields` 未知字段错误不带行列号，报错文案与测试断言须明确命中字段名而非强求行列号。
- **macOS `Option` 快捷键识别**：macOS 按 `⌥⌘C`/`⌥⌘1` 等组合键时 `KeyboardEvent.key` 会变成特殊字符，前端统一动作注册表（`M2-02`）与录制器（`M2-03`）须优先基于 `event.code` 归一化。
- **模板文件 YAML 占位符**：`v4` B.3 模板含裸 `id: {{id}}` / `title: {{title}}`，索引 `_templates/` 时强制按 `NoteType::Template` 归类并不计入 `meta_error` / 重复 `id`（`M2-01`）。
- **macOS Tauri 2 外链、文件拖放与菜单快捷键**：`WKWebView` 静默拦截 `window.open`，外链（`http/https/mailto`）走参数化 Rust `app_open_external_url`（`open`）；Finder/截图拖入文件走 `getCurrentWebview().onDragDropEvent`（`onWindowFileDrop`）；原生菜单快捷键通过 `app_set_menu_accelerators` 与 `actionRegistry` 实时同步且在快捷键录制态临时挂起防抢键。

## 4. 协作与防膨胀规矩（含 Token 省流铁律，已写入 `AGENTS.md`）

- **施工会话零大文档读取**：任务卡与施工提示词内联所需函数签名、结构体定义、SQL 列名与边界要求；**施工会话禁止读取 `spec/quicknote-design-v4.md`（1150 行）、`spec/engineering-contract.md`（745 行）、`DECISIONS.md` 及 `fixtures/vault/` 大样本文件**，只读 `AGENTS.md` + 当期任务卡 + 白名单目标源码。
- **单卡体量控制**：单卡新增/修改生产代码控制在 ≤ 800 行，超过立即拆为 `a/b` 子卡（如 `M1-01a/b`、`M1-04a/b`、`M2-05a/b`）。
- **增量审查防膨胀**：里程碑审查只看 `git diff <prev-tag>..dev` 增量与当期契约。

## 5. 下一步动作

1. 新开干净施工会话从 `dev` 拉取 `feat/M3-02-settings-workspaces-and-external-actions` 执行 `M3-02`（设置窗口工作区 CRUD、外部程序/`search_roots` 配置、`SettingsErrorBanner` 债务收口与主窗口外部动作集成）。
2. 逐卡串行推进至 `M3-05b` 全部合入 `dev` 后，执行 M3 增量独立只读审查与 macOS 实机点验并打 `m3-baseline` 标签。
