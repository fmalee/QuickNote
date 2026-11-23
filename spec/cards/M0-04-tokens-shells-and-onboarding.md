# M0-04：`--qn-*` 双主题视觉基线、多窗口 Vue 壳与首次选库 Onboarding

- 所属里程碑：M0
- 关联方案与契约：[quicknote-design-v4.md](../quicknote-design-v4.md) §1.2（S1）、§3.1、§3.2、§5.8、§6 M0；`C1.2`、`C4.2`、`C4.3`、`C7.1`、`C8.1`（S1）、`C9.1`、`D-0004`、`D-0008`
- 前置依赖：`M0-03`
- 会话策略：独立新会话执行（从 `dev` 拉出 `feat/M0-04-tokens-shells-and-onboarding`）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

- **要做**：
  - 在 `src/styles/tokens.css` 完整定义 `C7.1` 规范的浅色（默认）与深色（`[data-theme="dark"]` 及系统自适应）全套 `--qn-*` CSS Token（色彩、排版、间距、圆角、阅读最大宽度与层级）。
  - 在 `src/components/` 构建无业务逻辑、零直连 IPC 的基础原子 UI 组件库（Button、Input、Badge、Banner、PanelCard、ModalShell）。
  - 搭建四个窗口的前端挂载入口与骨架视图：`main`（两层主工作台骨架 + 顶部状态横幅）、`quickbar`（`640×420` 浮层骨架）、`toast`（5 秒非激活撤销浮窗骨架）、`settings`（macOS 侧栏分类设置窗骨架）。
  - 落地首次启动选库引导（`LibraryInfo.root == null`）：主窗口锁定工作台，展示选库/建库引导（默认建议 `~/Documents/QuickNote`，支持选择文件夹并调用 `library_set_root`）；Quick Bar 在 `root == null` 时严格呈现 **S1 未选库引导态**（禁用输入提交，显示一键打开主窗口初始化按钮）。
  - 落地 `settings.yaml` 异常容错横幅：监听 `settings:status`，当 `valid == false` 时主窗口与设置窗口顶部展示清晰错误横幅（精确列出 `errorLine`、`errorColumn` 与 `errorMessage`）。
  - 在设置窗口“通用”分类落地全局热键状态卡：展示当前 `HotkeyStatus`（`combo`、`backupCombo`、`selfCheck`）、热键冲突提示与一键切至备用热键 `⌃⌥⌘Space`（调用 `hotkey_update`）。
  - 提供一页式**视觉基线预览页**（可在设置窗“外观”页或开发调试入口打开），集中展示浅/深两套主题全部 `--qn-*` 组件与排版密度供用户确认。
- **明确不做**：
  - 不改动任何 `src-tauri/` Rust 后端代码或增改 IPC 契约。
  - 不实现笔记目录树/标签列表真实数据加载与 CM6 编辑器实例（归 `M1`）。
  - 不实现 Quick Bar 搜索/目标选择器/追加提交业务（归 `M1`–`M3`）。

## 2. 允许修改文件白名单

- 共享串行文件（卡内显式声明）：`src/styles/tokens.css`
- 前端目录：`index.html`（及多窗口 HTML 入口）、`src/app/**`、`src/components/**`、`src/features/onboarding/**`、`src/features/settings/**`、`src/features/quickbar/**`、`src/features/toast/**`、`src/lib/utils/**`

## 3. 具体实现要求

1. **严守前端单向分层与零裸值（`C1.2`、`C7.1`）**：`src/components/**` 严禁直接导入 `@tauri-apps/api` 或 `src/lib/ipc/`；所有业务组件只经所属 `features/*/api.ts` 或 `src/lib/ipc/` 调用后端；全部 CSS 严格使用 `var(--qn-*)`，零硬编码色值或魔法 `z-index`。
2. **完整覆盖 S1 引导态**：Quick Bar 打开时实时读取 `library_get_info` 与 `settings_get`；当 `root == null` 时，必须禁止任何提交行为并直接显示一键唤起主窗口选库引导操作。
3. **四窗口路由与按需渲染**：`main`、`quickbar`、`toast`、`settings` 共享同一 `tokens.css` 视觉语言；`toast` 窗体透明无框、内容严格居中紧凑、自动倒计时关闭动画平滑。
4. **`settings:status` 横幅不可吞错**：当收到 `valid == false` 时，即使主窗口后续重启才打开，冷启动也必须通过 `settings_get` 读到异常状态并立即渲染横幅。
5. **严谨处理 TS `strict` 与 `noUncheckedIndexedAccess`**：绝不使用 `any` 或非空断言跳过类型安全。

## 4. 自动化验收标准（必须全绿）

- **新增测试（Vitest）**：
  - `src/features/onboarding/__tests__/onboarding-state.test.ts`：Mock `src/lib/ipc/commands`，验证 `libraryRoot == null` 时主窗口进入引导锁定态、Quick Bar 渲染 S1 禁用提示并支持点击调用 `window_show_main`；验证调用 `library_set_root` 后状态解除锁定。
  - `src/features/settings/__tests__/settings-status-and-hotkey.test.ts`：验证 `SettingsLoadStatus { valid: false, errorLine: 14, errorColumn: 3, ... }` 时错误横幅正确渲染精确行列号；验证热键状态为 `"conflict"` 时触发告警并可一键调用 `hotkey_update` 切至备用键 `ctrl+alt+cmd+space`。
  - `src/features/toast/__tests__/toast-view.test.ts`：验证收到 `toast:append-undo` 载荷时显示目标文档名与 `[撤销]` 按钮。
- **必过命令**：`pnpm check:all`（含 `scripts/check-architecture.mjs` 扫描零裸色值、零魔法 `z-index`、零违规导入，返回码 `0`）。

## 5. 人工点验清单（M0 视觉与端到端闭环点验）

- [ ] 清空测试数据目录启动应用，确认主窗口与按 `⌃⌥Space` 呼出的 Quick Bar 均正确进入 S1 未选库引导；在主窗口完成初始化建库后，两处同步恢复可用态且重启保持记住库根。
- [ ] 手动往 `$APP_DATA_DIR/settings.yaml` 写入错误字段并保存，确认磁盘原文件未被覆盖、主窗口顶部横幅立刻展示精确错误行号与列号。
- [ ] 在设置窗口切换浅色 / 深色主题，核对视觉基线预览页的对比度、字号、圆角、间距密度与 Toast 浮窗样式达到预期（确认后作为 M1–M4 全部 UI 的固定视觉标准）。
