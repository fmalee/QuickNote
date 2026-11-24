# M3-02：设置窗口工作区 CRUD、外部程序与搜索配置、`SettingsErrorBanner` 债务收口与主窗口外部动作集成

- 所属里程碑：M3
- 关联方案与契约：`v4` §3.7.1、§4.7、§5.10、§5.11、§6 M3；`C1.2`、`C2.2`、`C4.2`、`C6.2`、`C7.1`–`C7.3`、`C9.1`、`D-0008`
- 前置依赖：`M3-01b` 已合并至 `dev`
- 建议分支：`feat/M3-02-settings-workspaces-and-external-actions`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、组件接口与边界规则已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **收口 M0→M3 跨里程碑挂起债务：`SettingsErrorBanner` 接入 `external_action_run`（`D-0008` / `v4` §4.7）**：
   - 在 `src/features/settings/api.ts` 封装 `openSettingsYamlForRepair()` 与 `revealSettingsYamlInFinder()`，内部调用 `externalActionRun`：
     - 打开修复：`externalActionRun({ id: 'external.default', absPath: '$SETTINGS_YAML', dirPath: null })`
     - 在 Finder 显示：`externalActionRun({ id: 'external.reveal', absPath: '$SETTINGS_YAML', dirPath: null })`
   - 更新 `src/features/settings/SettingsErrorBanner.vue`：
     - 在错误横幅的 `#actions` 插槽（或右侧操作区）新增两个按钮：
       - `data-testid="open-settings-yaml-btn"`：“打开 settings.yaml 修复”（点击调用 `openSettingsYamlForRepair()`）；
       - `data-testid="reveal-settings-yaml-btn"`：“在 Finder 中显示”（点击调用 `revealSettingsYamlInFinder()`）；
     - 按钮操作失败时在横幅下方以清晰内联提示展示错误信息，绝不吞错或崩溃。
     - 确认主窗口（`MainWorkbenchShell.vue`）与设置窗口（`SettingsView.vue`）均复用同一 `SettingsErrorBanner` 组件，因此两处自动同时生效。

2. **设置窗口新增「工作区」与「外部程序与搜索」配置页（`v4` §4.7、§5.11、`C6.2`）**：
   - 扩展 `src/features/settings/store.ts` 的 `SettingsSection`：
     - `'general' | 'appearance' | 'library' | 'workspaces' | 'external' | 'shortcuts' | 'baseline'`
   - 新增 `src/features/settings/SettingsWorkspacesPanel.vue`（工作区 CRUD）：
     - 展示当前 `settings.workspaces` 列表（`Record<string, WorkspaceConfig>`），支持**新增、编辑、删除**工作区。
     - 覆盖 `WorkspaceConfig` 全部 8 个字段：
       - `key: string`（唯一键，供 frontmatter `project` 引用，仅允许非空且编辑新键时检查不与已有 key 冲突）
       - `name: string`（显示名称，必填）
       - `root: string`（项目根目录，必填）
       - `openDir: string`（VS Code / Sublime 打开目录，默认自动填 `root`）
       - `codeWorkspace: string | null`（`.code-workspace` 路径，可选）
       - `terminalDir: string | null`（终端初始目录覆盖，可选）
       - `defaultEditor: string`（下拉：`vscode` / `sublime`）
       - `defaultEntryDoc: string | null`（Quick Bar 默认入口库内文档相对路径，可选）
     - 每次保存/删除调用 `store.patchSettings({ workspaces: nextWorkspaces })`（遵循 `D-0008`：当 `isSettingsInvalid` 时表单只读禁用并提示先修复 `settings.yaml`）。
   - 新增 `src/features/settings/SettingsExternalPanel.vue`（外部程序与 Spotlight 搜索根目录配置）：
     - **外部终端配置**（`settings.terminal: { name, bundleId }`）：提供常用终端快捷预设按钮（Terminal.app `com.apple.Terminal`、iTerm2 `com.googlecode.iterm2`、Warp `dev.warp.Warp-Stable`、Ghostty `com.mitchellh.ghostty`）+ 自定义名称与 `bundleId` 输入框，变更后调用 `store.patchSettings({ terminal })`。
     - **外部编辑器配置**（`settings.editors.vscode` 与 `settings.editors.sublime`）：支持编辑 `name` 与 `bundleId`（附恢复默认按钮），调用 `store.patchSettings({ editors })`。
     - **Spotlight 搜索根目录配置**（`settings.searchRoots: string[]`）：展示已配置目录列表，支持新增路径输入框（校验非空去重）、一键移除某项，调用 `store.patchSettings({ searchRoots })`。
   - 更新 `src/features/settings/SettingsView.vue`：在左侧导航挂载 `工作区`（`workspaces`）与 `外部与搜索`（`external`）两个页签并渲染对应面板组件。

3. **主窗口统一外部动作注册表与右键菜单集成（`v4` §3.7.1、`C7.2`）**：
   - 在 `src/features/editor/api.ts` 封装：
     - `resolveExternalActions(args: ResolveExternalArgs): Promise<ExternalActionState[]>`
     - `runExternalAction(args: RunExternalArgs): Promise<void>`
   - 在 `src/app/MainWorkbenchShell.vue`（或抽取到 `src/features/editor/` 的轻量 composable `useExternalActions.ts`，严禁跨 feature 深层导入）：
     - 当主窗口当前激活文档切换、元数据 `openDir`/`project`/`terminalDir`/`codeWorkspace` 更新或设置重载时，异步调用 `externalActionsResolve({ absPath: activeDoc.absPath, workspaceKey: activeDoc.meta.project, commandId: null })` 缓存当前文档的 8 个外部动作实时可用态（附递增请求序号防异步乱序，`C2.2`）。
     - 将 `actionRegistry` 中全部 8 个 `external.*` 动作（`external.reveal`、`external.terminal`、`external.vscode`、`external.sublime`、`external.workspace`、`external.default`、`external.copyPath`、`external.pin`）绑定真实执行逻辑与动态 `when`：
       - 执行时使用 `resolvedPath`（文件类动作传 `absPath`，目录类动作传 `dirPath`，`.code-workspace` 传 `absPath`）；
       - 若动作被禁用（例如库内文档未配置 `open_dir` 或工作区），点击/菜单项显示为禁用态并明确展示后端返回的 `disabledReason`（绝不触发回退）；
       - 执行失败或成功（如“已复制路径”、“已固定到常用文件 / 已取消固定”）在主窗口顶部/状态提示轻反馈。
   - 在文档头 `DocHeaderBar.vue` 增加轻量「外部动作 ▾」按钮（点击弹出 `QnContextMenu` 展示这 8 个外部动作及其启用/禁用理由）以及标签栏 `EditorTabBar.vue` 标签右键菜单接入相同 8 个外部动作项（同源驱动，零重复硬编码规则）。

### 明确不做什么
- 本卡**不做** Quick Bar 打开模式、主窗口 `⌘K` 与外部文件标签页搜索（归 `M3-03`）。
- 本卡**不做** Quick Bar 命令模式与主窗口命令库（归 `M3-04`）。
- 本卡**不改动**任何 Rust 代码或 `C1.3` 共享串行文件。

## 2. 允许修改文件白名单

- 设置模块（`src/features/settings/`）：
  - `src/features/settings/api.ts`
  - `src/features/settings/store.ts`
  - `src/features/settings/SettingsErrorBanner.vue`
  - `src/features/settings/SettingsWorkspacesPanel.vue`（新增）
  - `src/features/settings/SettingsExternalPanel.vue`（新增）
  - `src/features/settings/SettingsView.vue`
  - `src/features/settings/index.ts`
- 编辑器与主工作台（外部动作绑定与菜单）：
  - `src/features/editor/api.ts`
  - `src/features/editor/useExternalActions.ts`（新增）
  - `src/features/editor/DocHeaderBar.vue`
  - `src/features/editor/EditorTabBar.vue`
  - `src/features/editor/index.ts`
  - `src/app/MainWorkbenchShell.vue`
- 组件微调（仅若 `QnContextMenu` 需支持禁用项 `disabledReason` tooltip/副标题展示）：
  - `src/components/QnContextMenu.vue`
- 测试文件：
  - `src/features/settings/__tests__/settings-workspaces-and-external.test.ts`（新增）
  - `src/features/settings/__tests__/settings-status-and-hotkey.test.ts`（如需补充 `SettingsErrorBanner` 按钮断言）
  - `src/features/editor/__tests__/external-actions-integration.test.ts`（新增）

## 3. 具体实现要求

1. **已内联的 IPC 签名（全部位于 `src/lib/ipc/commands.ts` 与 `src/bindings/types.ts`）**：
   - `externalActionsResolve(args: ResolveExternalArgs): Promise<ExternalActionState[]>`
     - `ResolveExternalArgs = { absPath: string | null; workspaceKey: string | null; commandId: string | null }`
     - `ExternalActionState = { id: ExternalActionId; enabled: boolean; disabledReason: string | null; resolvedPath: string | null }`
   - `externalActionRun(args: RunExternalArgs): Promise<void>`
     - `RunExternalArgs = { id: ExternalActionId; absPath: string | null; dirPath: string | null }`
     - `ExternalActionId = 'external.reveal' | 'external.terminal' | 'external.vscode' | 'external.sublime' | 'external.workspace' | 'external.default' | 'external.copyPath' | 'external.pin'`
   - `settingsUpdate(args: SettingsPatch): Promise<SettingsSnapshot>`（已有）
2. **分层与样式红线（`C1.2`、`C7.1`）**：
   - Vue 组件严禁直接 import `@tauri-apps/api/*`，全部通过各 feature 的 `api.ts` 调用 `src/lib/ipc/commands.ts`。
   - 所有新增 CSS 仅使用 `--qn-*` CSS Token，严禁硬编码十六进制色值或魔法 `z-index`。
3. **竞态与空状态防线（`C2.2`）**：
   - `useExternalActions` 每次异步调用 `externalActionsResolve` 必须携带递增 `seq`，仅最新响应可写入状态，防止快速切标签出现旧文档动作状态覆盖新文档。
   - 当 `settings.yaml` 损坏（`isSettingsInvalid === true`）时，工作区与外部配置表单全部只读不可提交，绝不触发 `settings_update`。

## 4. 自动化验收标准（必须全绿）

1. `src/features/settings/__tests__/settings-workspaces-and-external.test.ts` 覆盖：
   - `SettingsErrorBanner` 在 `valid: false` 时渲染“打开 settings.yaml 修复”与“在 Finder 中显示”按钮，点击分别精确调用 `externalActionRun({ id: 'external.default', absPath: '$SETTINGS_YAML', dirPath: null })` 与 `external.reveal`；
   - 工作区新增、编辑、key 冲突校验、删除触发精确的 `settingsUpdate({ workspaces })` 载荷；
   - 外部终端预设一键切换（如 iTerm2 → Ghostty）、编辑器 `bundleId` 修改与 `searchRoots` 增删去重均正确触发 `settingsUpdate`；
   - `settings.yaml` 损坏态下禁止保存任何工作区/外部程序变更。
2. `src/features/editor/__tests__/external-actions-integration.test.ts` 覆盖：
   - 库内未配置 `open_dir` 与工作区文档时，菜单项精确禁用并展示 `disabledReason`，点击不触发 `externalActionRun`；
   - 库外文件或已关联工作区文档时，`external.reveal` / `external.terminal` / `external.vscode` / `external.pin` 正确启用并传递精确 `absPath` / `dirPath`；
   - 快速切换文档时过期 `externalActionsResolve` 响应被静默丢弃。
3. 运行 `pnpm check:all` 全部 9 项门禁 100% 通过。

## 5. 人工点验清单（可选提前验证，或待 M3-04 后合并点验）

- 手改损坏 `settings.yaml` 后在设置窗口与主窗口均看到错误横幅及“打开 settings.yaml 修复”按钮，点击能直接用系统默认编辑器打开 `settings.yaml`，修复保存后横幅自动消失。
- 在设置窗口新建一个工作区并在一篇文档属性中关联该 `project`，验证文档头外部动作菜单“在终端中打开目录 / 用 VS Code 打开 / 在 Finder 显示”工作正常；切到未配置工作区的库内笔记验证对应动作明确禁用。
