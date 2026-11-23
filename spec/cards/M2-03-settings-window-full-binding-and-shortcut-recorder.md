# M2-03：设置窗口完整双向绑定（`settings_update`）、快捷键录制/冲突检测与库/收件箱/外观配置

- 所属里程碑：M2
- 关联方案与契约：`v4` §3.8、§4.10、§4.11、§5.11、§6 M2；`C1.2`、`C2.2`、`C4.2`、`C6.2`、`C7.1`、`C7.2`、`C7.3`、`C9.1`、`D-0004`、`D-0008`
- 前置依赖：`M2-01`、`M2-02`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、表单项与状态流已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 扩展 `src/features/settings/api.ts` 与 `src/features/settings/store.ts`**：
    - `api.ts` 新增：
      - `saveSettingsPatch(patch: SettingsPatch): Promise<SettingsSnapshot>`（调用 `settingsUpdate`）；
      - `changeLibraryRoot(args: SetRootArgs): Promise<LibraryInfo>`（调用 `librarySetRoot`）；
      - `listTemplateNotes(): Promise<NoteSummary[]>`（调用 `notesList({ tab: 'templates', dirRelPath: null, tags: [], workspaceKey: null, filterText: null })`）；
      - `createTemplateNote(title: string): Promise<NoteDoc>`（调用 `noteCreate({ title, dirRelPath: '_templates', templateName: 'note', body: null })`）。
    - `store.ts` 扩展：
      - `SettingsSection = 'general' | 'appearance' | 'library' | 'shortcuts' | 'baseline'`；
      - 新增状态：`saving: Ref<boolean>`、`saveError: Ref<string | null>`、`lastSavedAt: Ref<number | null>`、`templates: Ref<NoteSummary[]>`、`loadingTemplates: Ref<boolean>`；
      - `patchSettings(patch: SettingsPatch): Promise<boolean>`：
        - 若 `isSettingsInvalid.value === true`，直接拒绝写入并设置 `saveError = 'settings.yaml 当前存在语法错误，已只读保护，请先修复原文件后再保存'`（`D-0008`）；
        - 正常时调用 `saveSettingsPatch(patch)`，用返回的 `SettingsSnapshot` 更新 `snapshot.value` 并同步 `setTheme(res.settings.editor.theme)`（如改了主题），设置 `lastSavedAt = Date.now()`；
      - `setTheme(theme: EditorTheme, options?: { persist?: boolean })`：默认保持立即可见切换 `applyThemeToDocument(theme)`；当用户点击主题卡片传 `{ persist: true }` 时同时调用 `patchSettings({ editor: { theme } })` 原子落盘！
      - `loadTemplates()` 与 `updateLibraryRoot(root: string, createIfMissing: boolean)`（切库成功后刷新 `snapshot` 与 `templates`）。
  - **2. 拆分设置子面板组件（零裸色值，全部 `--qn-*` Token，保持组件简洁）**：
    - `src/features/settings/SettingsAppearancePanel.vue`（“外观与编辑器”分类）：
      - 主题卡片（`system` / `light` / `dark`，点击持久化写入 `settings.yaml`）；
      - 编辑器字号（`10`–`32` px，默认 `14`）、行高（`1.0`–`3.0`，步进 `0.1`，默认 `1.6`）、最大阅读宽度（px，默认 `900`，`0` 表示不限制宽度）；
      - 缩进配置：正文软空格宽度 `tabSizeProse`（`1`–`8`，默认 `2`）、代码块缩进宽度 `tabSizeCode`（`1`–`8`，默认 `4`）；
      - 自动保存间隔：防抖 `autoSaveDebounceMs`（默认 `600`ms）、最大等待 `autoSaveMaxWaitMs`（默认 `2000`ms，校验 `>= debounce`）；
      - 链接交互开关：`singleClickLink`（默认 `false` = `⌘` + 单击打开，开启后支持普通单击打开）；
      - 所有控件在 `store.isSettingsInvalid` 时处于禁用态（只读保护），修改失焦/提交触发 `store.patchSettings({ editor: ... })` 并展示“已保存”状态提示。
    - `src/features/settings/SettingsLibraryPanel.vue`（“库与收件箱”分类）：
      - 笔记库根路径显示与编辑输入框 + “目录不存在时自动创建”勾选 + “应用库路径”按钮（调用 `store.updateLibraryRoot`）；
      - 收件箱膨胀提醒阈值 `inbox.bloatThresholdLines`（默认 `500` 行，最小值 `10`，绑定 `settings_update`）；
      - Quick Bar 缺失分区自动创建开关 `quickBar.autoCreateMissingSection`（默认 `false`，绑定 `settings_update`）；
      - Quick Bar 活跃文档有效期 `quickBar.lastActiveTtlHours`（默认 `24` 小时）与隐藏重建延迟 `quickBar.destroyAfterHiddenMinutes`（默认 `10` 分钟）；
      - 模板管理卡片：展示 `_templates/` 下全部模板列表（调用 `store.loadTemplates()`，默认含 `note` / `doc` / `command-book`），提供“新建自定义模板”快捷输入（输入名称后创建 `_templates/<name>.md` 并刷新列表）。
    - `src/features/settings/SettingsShortcutsPanel.vue`（“快捷键”分类）：
      - 接入 `@/lib/actions`（`actionRegistry`、`keyboardEventToCombo`、`formatKeyComboDisplay`）；
      - 顶部搜索框（按动作名称 / ID / 作用域 / 快捷键实时过滤）+ 分类筛选 + “全部恢复默认”按钮 + “导出配置”/“导入配置”按钮（使用 `QnModalShell` 展示/粘贴 JSON，调用 `exportOverridesJson` / `importOverridesJson` 并展示错误/冲突反馈）；
      - 动作列表按分类/作用域展示：每行显示标题、ID、作用域徽章（`QnBadge`）、当前快捷键（Mac 符号）、“录制”按钮与“恢复默认”按钮（存在覆写时显示）；
      - **交互式快捷键录制器**：
        - 点击某行“录制”进入录制态（高亮提示“请按下组合键，Esc 取消，Backspace 清除”）；
        - 捕获 `keydown` → `Esc` 退出录制、`Backspace` 清除键位（`null`）、有效组合键经 `keyboardEventToCombo` 规范化后调用 `actionRegistry.setKeyOverride(id, combo)`；
        - 若返回 `{ ok: false, conflicts }`：**阻止静默覆盖**，在界面显眼处（`data-testid="shortcut-conflict-banner"`）高亮冲突动作名称与作用域，保持原键位不变！
  - **3. 装配 `src/features/settings/SettingsView.vue`**：
    - 侧栏导航完整呈现：`通用与热键`（`general`）、`外观与编辑器`（`appearance`）、`库与收件箱`（`library`）、`快捷键`（`shortcuts`）、`视觉基线预览`（`baseline`）；
    - 顶部保留 `SettingsErrorBanner`（`D-0008`）与保存状态微提示；
    - 完整保留既有 M0 `data-testid`（`hotkey-status-card`、`hotkey-conflict-banner`、`switch-backup-hotkey-btn`、`switch-primary-hotkey-btn`）确保 M0 测试零回归。
- 不做：
  - 不实现工作区 CRUD 表单、外部程序 `bundle_id` 探测、Spotlight `search_roots` 配置（归 `M3`）或 `settings_export`/`settings_import` 磁盘文件对话框（归 `M4`；本卡仅支持快捷键覆写 JSON 导入导出）。
  - 不在主窗口添加设置入口按钮（遵守 `v4` §5.11：“主窗口绝不放设置按钮”，通过 `⌘,` 或菜单/托盘打开设置窗口）。

## 2. 允许修改文件白名单
- 共享串行文件：无
- 前端业务模块：`src/features/settings/**`（新增 `SettingsAppearancePanel.vue`、`SettingsLibraryPanel.vue`、`SettingsShortcutsPanel.vue`；更新 `api.ts`、`store.ts`、`SettingsView.vue`、`index.ts`）
- 前端单元测试：`src/features/settings/__tests__/settings-update-and-shortcuts.test.ts`（新增）；可只读参考既有 `settings-status-and-hotkey.test.ts` 确保零破坏

## 3. 具体实现要求
- **严格分层与 Token 规范（`C1.2`、`C7.1`）**：
  - 所有 IPC 调用仅经 `src/features/settings/api.ts` → `@/lib/ipc/commands`；
  - 零硬编码色值与魔法 `z-index`，全用 `--qn-*`；
  - 快捷键录制器必须监听 `event.code`（经 `@/lib/actions` `keyboardEventToCombo`），确保 macOS 下录制 `⌥⌘C`、`⌥⌘1` 等不因 Option 特殊字符失灵。

## 4. 自动化验收标准（必须全绿）
- 新增测试文件：`src/features/settings/__tests__/settings-update-and-shortcuts.test.ts`，覆盖：
  1. `settings_update` 双向绑定：修改 `editor.fontSize`、`editor.maxWidth`、`editor.singleClickLink`、`inbox.bloatThresholdLines`、`quickBar.autoCreateMissingSection` 时正确调用 `settingsUpdate` 并更新 Store `snapshot`；切换主题时同时更新 DOM `data-theme` 与持久化。
  2. 只读保护（`D-0008`）：当 `snapshot.status.valid === false` 时，调用 `patchSettings` 被拦截返回 `false`，不调用 `settingsUpdate` IPC，并给出明确提示。
  3. 快捷键录制与冲突检测：验证录制合法组合键成功更新覆写并持久化；录制同作用域已占用组合键（如将 `editor.italic` 设为 `cmd+b`）被拦截、展示冲突列表且原键位不变；验证单项重置、全部重置与 JSON 导入导出。
  4. 模板列表加载：验证 `loadTemplates()` 调用 `notesList({ tab: 'templates', ... })` 并返回模板摘要。
- 既有 `src/features/settings/__tests__/settings-status-and-hotkey.test.ts` 保持 100% 通过。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] 通过菜单栏或 `⌘,` 打开设置窗口，切换主题、调整编辑器最大宽度（如 `720px`）与收件箱阈值（如 `300` 行）：重启应用后配置保持生效，且 `$APP_DATA_DIR/settings.yaml` 内容准确同步。
- [ ] 在“快捷键”页录制冲突按键（如把“斜体”录为 `⌘B`）：界面明确高亮与“加粗”冲突并拒绝覆盖；录制无冲突按键后可生效并支持“恢复默认”。
