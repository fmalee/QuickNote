# PROJECT_STATE · QuickNote 总控状态板

> 主控会话接续唯一动态真源（控制 ≤ 80 行）。每次里程碑收官或启动新主控会话时同步更新，不重复 `AGENTS.md` / `engineering-contract.md` 静态正文。

## 1. 当前精确进度

- **当前阶段**：**M0–M4（1.0 正式版）全部收官**！`m4-baseline` 与 `v1.0.0` 已同步落地并合入 `master`，后续按需规划 **M5（生态扩展：同步兼容 / 移动端 / AI）**。
- **已完成**：
  - M0–M3 全阶段（`m0-baseline` / `m1-baseline` / `m2-baseline` / `m3-baseline`）。
  - M4 全阶段（`m4-baseline` / `v1.0.0`：`M4-01a`–`M4-06` + M4 独立只读审查 + `fix/M4-audit` `6cf8114` + `fix/M4-qa` `375d4d8` + `fix/M4-qa-scrollbar` `bfd693b` 实机点验收官；万篇 FTS5 2.7–6.1ms 达标、M2/M3 挂起债务全部清零）。
- **待执行**：按需启动 M5 规划，或直接从 `v1.0.0` 执行 macOS 正式应用构建打包（`pnpm tauri build`）。

## 2. 跨里程碑挂起债务（拆卡必查）

| 挂起项 | 来源 | 目标里程碑 | 处理要求 |
| --- | --- | --- | --- |
| `ToastAppendUndoPayload` 手写类型收口 | M0 审查 🟡#4 | **M1（已在 `M1-02` `b8d6018` 收口）** | 已随 `AppendResult` 收归 Rust 生成并接入 `src/lib/ipc/events.ts` |
| 默认模板（`note/doc/command-book`）格式对齐 | M0 审查 🟢#4 | **M2（已在 `M2-01` `87f1a44` 收口）** | 已在 `domain::template` 单源定义并与 `v4` 附录 B.3 逐字对齐 |
| `SettingsErrorBanner` “打开 `settings.yaml` 修复”按钮 | M0 审查 🟢#4 | **M3（已在 `M3-02` `455c2ee` 收口）** | 已在 `M3-01b` 落地 `$SETTINGS_YAML` 路由、`M3-02` 接入 `external_action_run`（`D-0008` / `v4` §4.7） |
| `QuickBarView` 与 `bridge.m` 幂等双 `Esc` | M0 审查 🟢#4 | **保留现状** | 保持不动，避免破坏已实机通过的 Quick Bar 原生焦点与全屏行为 |
| `collectTodoDecorations` 超长文档单遍扫描优化 | M2 审查 🟢#2 | **M4（已在 `M4-06` `67dd3a9` 收口）** | 已改为 $O(N)$ 单遍围栏扫描并补充 10,000 行大文档基准测试 |
| PDF 导出库内相对路径图片解析（`WKWebView` `baseURL` 沙箱评估） | M4 审查 🟢#1 | **M5（1.0 之后）** | 保持 1.0 离屏 `WKWebView` 最小沙箱面，M5 评估安全白名单内联 |
| `check-architecture.mjs` 增加全仓 CSS Token 定义校验 | M4 审查 🟢#3 | **M5（1.0 之后）** | 随 M5 统一清理历史未定义 Token（M4 增量在 `fix/M4-audit` 清零） |

## 3. 已验证实机约定与踩坑备忘

- **辅助窗口 URL 路由**：`src-tauri/src/lib.rs` 已固定为 `index.html?window=quickbar|toast|settings`，主窗口为无参 `index.html`。
- **原生 Toast 零私有 API**：严禁开启 Tauri `.transparent(true)`（依赖 `macos-private-api`）；采用 `bridge.m` `NSPanel` `opaque = NO` + `clearColor` + `CALayer` 圆角裁剪（`cornerRadius = 10`）。
- **目录选择方式**：暂不引入 `tauri-plugin-dialog`，选库使用路径输入框（默认 `~/Documents/QuickNote`）+ 自动创建开关。
- **`serde_yaml 0.9` 报错特性**：语法缩进错误能精准返回行列号；`deny_unknown_fields` 未知字段错误不带行列号，报错文案与测试断言须明确命中字段名而非强求行列号。
- **macOS `Option` 快捷键识别**：macOS 按 `⌥⌘C`/`⌥⌘1` 等组合键时 `KeyboardEvent.key` 会变成特殊字符，前端统一动作注册表（`M2-02`）与录制器（`M2-03`）须优先基于 `event.code` 归一化。
- **模板文件 YAML 占位符**：`v4` B.3 模板含裸 `id: {{id}}` / `title: {{title}}`，索引 `_templates/` 时强制按 `NoteType::Template` 归类并不计入 `meta_error` / 重复 `id`（`M2-01`）。
- **macOS Tauri 2 外链、文件拖放与菜单快捷键**：`WKWebView` 静默拦截 `window.open`，外链（`http/https/mailto`）走参数化 Rust `app_open_external_url`（`open`）；Finder/截图拖入文件走 `getCurrentWebview().onDragDropEvent`（`onWindowFileDrop`）；原生菜单快捷键通过 `app_set_menu_accelerators` 与 `actionRegistry` 实时同步且在快捷键录制态临时挂起防抢键。
- **M3 跨窗口打开与坐标系约定**：Quick Bar → 主窗口打开文件经 `PENDING_OPEN_FILE_STORAGE_KEY` 跨窗传递，`onWindowState(visible)` 命中待开文件时跳过 `handleMainWindowShown()` 防抢回旧焦点；编辑器内全部行号（大纲 / 收件箱提取 / `CommandEntry.line`）统一为剥离 frontmatter 后的 `NoteDoc.body` 1-based 行号（`D-0006`）；库外常用固定文件独立落盘 `<app_data_dir>/pinned-files.json`（不改动 `C6.3` `UiState`）。

## 4. 协作与防膨胀规矩（含 Token 省流铁律，已写入 `AGENTS.md`）

- **施工会话零大文档读取**：任务卡与施工提示词内联所需函数签名、结构体定义、SQL 列名与边界要求；**施工会话禁止读取 `spec/quicknote-design-v4.md`（1150 行）、`spec/engineering-contract.md`（745 行）、`DECISIONS.md` 及 `fixtures/vault/` 大样本文件**，只读 `AGENTS.md` + 当期任务卡 + 白名单目标源码。
- **单卡体量控制**：单卡新增/修改生产代码控制在 ≤ 800 行，超过立即拆为 `a/b` 子卡（如 `M1-01a/b`、`M1-04a/b`、`M2-05a/b`）。
- **增量审查防膨胀**：里程碑审查只看 `git diff <prev-tag>..dev` 增量与当期契约。

## 5. 下一步动作

1. QuickNote **1.0.0** 已合入 `master`（标签 `m4-baseline` / `v1.0.0`）；如需导出本机安装包，在 `master` / `dev` 上按需配置签名并运行 `pnpm tauri build`。
2. 如需继续演进 M5（外部同步冲突兼容、移动端、AI 能力及 §2 中 2 项 M5 优化），可随时新开主控会话基于 `m4-baseline` 拆卡启动。
