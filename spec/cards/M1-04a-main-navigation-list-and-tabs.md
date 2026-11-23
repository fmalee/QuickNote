# M1-04a：主界面第一层（导航树 / 层级标签树 / 笔记列表 / 重命名 / 收件箱 500 行膨胀提醒）与多标签状态栏

- 所属里程碑：M1
- 关联方案与契约：`v4` §3.2–§3.4、§3.9（F/H）、§4.6、§4.11、§5.9、§6 M1；`C1.2`、`C2.2`、`C4.2`、`C4.3`、`C6.2`、`C6.3`、`C7.1`、`C9.1`、`D-0006`、`D-0007`、`D-0009`
- 前置依赖：`M1-03`
- 会话策略：独立新会话执行（零大文档读取，所有契约已内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - 在 `src/features/notes/` 落地主界面**第一层（导航 + 列表）**：
    - 顶部 5 个导航分段切换：`inbox`（收件箱，展示 `NavTreeSnapshot.inboxCount`）、`projects`（库内目录树 `DirNode` 递归展开与选中）、`tags`（派生层级标签树 `TagNode` 展开，支持点击切换多标签 AND 筛选，`dev` 匹配 `dev/*`）、`recent`（最近修改列表）、`workspaces`（配置的工作区列表 `WorkspaceSummary`，选中时同步更新 `ui_state_set({ key: 'selectedWorkspace', value })`）。
    - 下方笔记列表：调用 `notesList(NotesFilterArgs)` 渲染 `NoteSummary`（展示标题、摘要 `excerpt`、标签 chips、置顶标记 `pinned`、相对时间、`metaError` 警告徽标）；列表顶部提供搜索过滤框（**`80ms` 防抖 + 递增校验丢弃过期响应**；列表聚焦时按 `⌘F` 聚焦该过滤框）。
    - 列表操作：顶部 `[+ 新建]` 按钮调用 `noteCreate` 创建笔记并打开；列表项支持双击标题或操作按钮内联重命名（调用 `noteRename`，并同步通知已打开标签更新 `relPath`/`absPath`）与删除笔记（调用 `noteDelete`，`inbox.md` 禁用删除）。
    - **收件箱 500 行被动膨胀提醒**：当当前打开文档为 `inbox.md` 且其正文总行数超过 `settingsStore.settings.inbox.bloatThresholdLines`（默认 `500` 行）时，在主界面顶部展示温和非阻塞 `QnBanner`：“收件箱已超过 500 行，建议整理”。
  - 在 `src/features/editor/`（或 `src/features/notes/`）落地**多标签页状态栏（Tabs Store + TabBar 组件）与 `ui-state.json` 恢复**：
    - 预览标签 vs 固定标签状态机：全局最多存在 1 个预览标签（`isPreview: true`，标题斜体渲染）；**单击**列表项打开或替换当前预览标签；**双击**列表项、双击标签头或后续产生编辑时晋升为固定标签（`isPreview: false, isPinned: true`）。
    - 标签操作：点击 `×` 或中键关闭标签（关闭后推入 `closedStack`）、关闭其他标签、`⌘1…9` 切换第 1~9 个标签、`⌘⇧T` 从 `closedStack` 弹出并重开最近关闭的标签。
    - 状态持久化与搬库重定位：标签列表、`activeKey`、`filters` 变更时防抖调用 `uiStateSet` 持久化至 `ui-state.json`；启动或 `librarySetRoot` 搬库后恢复标签（优先按 `relPath` 定位，找不到则用 `key`/`id` 在 `notesList` 中全库搜寻重定位，彻底缺失则保留为失联占位提示标签）。
- 不做：
  - 不在本卡安装或挂载 CodeMirror 6 编辑器实例、自动保存、外部变更冲突横幅或右侧大纲面板（全部归 `M1-04b`；本卡在右侧编辑区仅渲染当前激活标签的 `NoteDoc` 基础文档头与正文只读/占位舞台，为 `M1-04b` 准备好 `useEditorTabsStore` 数据源）。

## 2. 允许修改文件白名单
- 前端工具与组件：`src/lib/utils/**`、`src/components/**`
- 前端业务模块：`src/features/notes/**`、`src/features/editor/**`、`src/app/MainWorkbenchShell.vue`、`src/app/bootstrapStore.ts`
- 前端单元测试：`src/features/notes/__tests__/**`

## 3. 具体实现要求
- **严格遵守前端分层与 Token 规范（`C1.2`、`C7.1`）**：任何 `.vue` 或 `src/components/**` 严禁直接导入 `@tauri-apps/api`；跨 feature 仅通过 `src/features/<name>/index.ts` 公共导出导入；全部 CSS 仅使用 `var(--qn-*)` 变量，零裸十六进制色值、零魔法数字 `z-index`。
- **TypeScript 严格类型**：遵守 `"strict": true` 与 `"noUncheckedIndexedAccess": true`，禁止 `any`。

## 4. 自动化验收标准（必须全绿）
- 新增/修改测试文件：`src/features/notes/__tests__/notes-tree-list-and-tabs.test.ts`
  - 验证目录树/标签树切换、多标签 AND 筛选与 `80ms` 防抖列表过滤。
  - 验证单击列表打开/替换斜体预览标签、双击晋升固定标签、关闭标签推入 `closedStack` 与 `⌘⇧T` 恢复最近关闭标签。
  - 验证 `noteRename` 成功后已打开标签页路径同步更新，以及 `ui-state.json` 启动恢复时按 `relPath` → `id` 兜底重定位与失联占位标签逻辑。
  - 验证 `inbox.md` 行数 `> 500` 时显示膨胀提醒横幅、`<= 500` 时隐藏。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 本卡完成后可在主窗口查看左侧导航树、标签树与多标签切换；完整编辑器与大纲实机点验在 `M1-04b` 一并执行。
