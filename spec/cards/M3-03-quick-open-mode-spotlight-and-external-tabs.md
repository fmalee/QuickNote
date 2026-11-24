# M3-03：Quick Bar 打开模式、主窗口大尺寸 `⌘K`、Spotlight 异步合并与外部文件标签页

- 所属里程碑：M3
- 关联方案与契约：`v4` §3.5.2、§3.5.4、§3.6、§5.3、§5.4、§6 M3；`C1.2`、`C2.2`、`C4.2`、`C7.1`–`C7.3`、`C9.1`、`D-0001`、`D-0002`
- 前置依赖：`M3-02` 已合并至 `dev`
- 建议分支：`feat/M3-03-quick-open-spotlight-and-external-tabs`
- 会话策略：独立新会话执行（零大文档读取，所有 IPC 签名、前缀模态规则与跨窗口约定已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **抽离共享快速打开核心引擎与 UI 组件（供 Quick Bar 打开模式与主窗口大尺寸 `⌘K` 共用，但状态完全独立，`v4` §3.5.4 / §3.6）**：
   - 在 `src/features/quickopen/`（新增 feature 模块，仅依赖 `@/lib/*` 与 `@/components`）构建：
     - `src/features/quickopen/api.ts`：封装 `searchNotesAndFiles`、`searchSpotlight`、`pathComplete`、`notesGetTreeAndTags`、`notesList`、`externalActionRun`、`windowShowMain`、`windowHideQuickBar`。
     - `src/features/quickopen/queryParser.ts`（纯函数）：解析输入前缀模态：
       - `/` 或 `~/` 开头 → `mode: 'path'`（路径补全模式，`prefix` 为完整输入）
       - `#` 开头 → `mode: 'fulltext'`（库内全文检索，`term = input.slice(1).trim()`）
       - `@` 开头 → `mode: 'tag'`（标签过滤与带标签笔记检索，`term = input.slice(1).trim()`）
       - `>` 开头 → `mode: 'action'`（前端 `actionRegistry` 已注册动作面板，`term = input.slice(1).trim()`）
       - 其余 → `mode: 'all'`（混合检索：库内笔记 + 已固定/最近外部文件 + 异步 Spotlight `search_spotlight`）
     - `src/features/quickopen/useQuickOpen.ts`（Composable，支持多实例隔离——Quick Bar 与主窗口各持独立实例）：
       - **80ms 防抖 + 单调递增 `requestId` 丢弃过期响应（`C2.2`、`v4` §5.4）**；
       - `mode: 'all'` 时先同步展示本地 `search_notes_and_files` 结果（含置顶常用固定文件），当查询长度 `>= 2` 时并发触发 `search_spotlight` 并将结果按 `absPath` 去重后增量合入列表（保留 `source` 徽标：`库` / `最近` / `Spotlight` / `路径`）；
       - `mode: 'path'` 时调用 `path_complete`，支持 **`Tab` 补全**（单匹配直接补全并若为目录追加 `/`；多匹配补全最长公共前缀并刷新列表）与 `↵`（选中目录则进入该目录继续补全，选中文件则触发打开）；
       - `mode: 'tag'` 时优先按标签树匹配候选标签，选中标签或直接输入完整标签后展示对应笔记；
       - `mode: 'action'` 时从传入的 `actionItems`（来自 `actionRegistry.listActions()`）按标题/ID/快捷键模糊过滤并支持直接回车执行；
       - 键盘导航：`↑` / `↓` 循环移动选中项；`↵` 触发 `onSelect(item, 'open')`；`⌘↵` 触发 `onSelect(item, 'newTab')`；`⌥↵` 触发 `external.reveal` 在 Finder 显示（仅对本地文件/目录项有效）。
     - `src/features/quickopen/QuickOpenPalette.vue`（共享展示组件，支持 `variant: 'compact' | 'large'`）：
       - `compact` 供 Quick Bar 浮层内打开模式使用；
       - `large` 供主窗口大尺寸 `⌘K` 使用（展示完整文件路径、来源徽标、更宽敞的结果行与右侧/底部预览摘要条，含快捷键提示脚栏：`↵ 主窗口打开 · ⌘↵ 新标签 · ⌥↵ Finder 显示 · / 路径 · # 全文 · @ 标签 · > 动作`）。

2. **Quick Bar 接入打开模式（`v4` §3.5.2、§3.5.4）**：
   - 更新 `src/features/quickbar/store.ts` 与 `src/features/quickbar/QuickBarView.vue`：
     - 支持 `activeMode: QuickBarMode`（`'capture' | 'open' | 'command'`，本卡落地 `'capture'` ↔ `'open'`，为 `M3-04` 预留 `'command'` 槽位）；
     - 响应 `window:state` 事件中的 `mode` 字段切换初始模式；
     - 浮层内按 **`⌘K`** 切到打开模式；在打开模式按 **`Esc`** 先退回记录模式（不立即隐藏浮层），在记录模式按 `Esc` 维持原有隐藏并保留草稿行为（保留 `M0` 实机通过的双 `Esc` 幂等约定）；
     - 打开模式选中笔记/外部文件按 `↵` 或 `⌘↵` 后：
       - 先通过 `ui_state_set` 或广播机制通知主窗口打开该文件（具体实现：若调用 `windowShowMain()` 激活主窗口，同时将待打开文件写入 `localStorage` 约定键 `quicknote:pending-open-file-v1` `{ absPath, newTab, line?: number, ts }` 并触发 `storage` 事件，主窗口收到后立即调用 `editorTabsStore.openNoteByAbsPath(absPath, { preview: !newTab })` 并清理；该方案零新增表外 IPC 命令且跨 Tauri 同源 Webview 即时生效），然后隐藏 Quick Bar 浮层；
       - `⌥↵` 直接调用 `external_action_run(external.reveal)` 在 Finder 显示。

3. **主窗口大尺寸 `⌘K` 快速打开（`v4` §3.6）**：
   - 在 `src/lib/actions/catalog.ts` 注册（若尚未注册）或确认存在 `window.quickOpen`（`defaultKeys: 'cmd+k', scope: 'main-window'`）与 `window.commandBook`（`defaultKeys: 'cmd+shift+k', scope: 'main-window'`）。
   - 在 `src/app/MainWorkbenchShell.vue` 挂载大尺寸 `QuickOpenPalette`（包裹于 `QnModalShell`，`variant="large"`）：
     - `⌘K` 或系统菜单“文件 > 快速打开…”（`menu:action`）唤起；
     - `↵` 在当前预览/常规标签打开、`⌘↵` 强制新固定标签打开、`⌥↵` 在 Finder 显示、`>` 执行主窗口动作后自动关闭弹窗。
     - 监听 `window` `storage` 事件消费来自 Quick Bar 的 `quicknote:pending-open-file-v1`，在主窗口即时打开目标文件（并支持后续 `M3-04` 的 `line` 跳转定位）。

4. **外部文件标签页与常用文件固定闭环**：
   - 完善 `src/features/editor/store.ts` 的 `openNoteByAbsPath(absPath, options)`：
     - 确保打开任意库外 Markdown 或非 Markdown 文本文件（如 `nginx.conf`、`.json`、`.yaml`）均能以独立标签页打开、编辑并自动保存；
     - 在 `EditorTabBar.vue` 与 `DocHeaderBar.vue` 为库外文件标签渲染醒目“外部”徽标（`QnBadge`）与快捷固定按钮（点击触发 `external.pin` 切换固定状态）。
   - 在 `src/features/notes/NotesSidebar.vue`（“最近”或“工作区”顶部）新增紧凑「常用外部文件」折叠区：
     - 消费 `search_notes_and_files({ query: '', limit: 50, requestId })` 中 `source === 'recent'` 的已固定外部文件项展示；
     - 点击直接以标签页打开，支持一键取消固定或右键调用外部动作。

### 明确不做什么
- 本卡**不做** Quick Bar 命令模式与主窗口大尺寸命令库（归 `M3-04`）。
- 本卡**不做**非 Markdown 语法高亮与诊断（归 `M3-05b`，本卡打开外部文件后先以正常文本在编辑器可读可编辑保存即可）。
- 本卡**不修改**任何 Rust 代码或 `C1.3` 共享文件。

## 2. 允许修改文件白名单

- 新增快速打开 feature（`src/features/quickopen/`）：
  - `src/features/quickopen/api.ts`（新增）
  - `src/features/quickopen/queryParser.ts`（新增）
  - `src/features/quickopen/useQuickOpen.ts`（新增）
  - `src/features/quickopen/QuickOpenPalette.vue`（新增）
  - `src/features/quickopen/index.ts`（新增）
- Quick Bar 集成：
  - `src/features/quickbar/api.ts`
  - `src/features/quickbar/store.ts`
  - `src/features/quickbar/QuickBarView.vue`
- 主窗口、编辑器标签页与侧栏常用文件：
  - `src/lib/actions/catalog.ts`
  - `src/features/editor/store.ts`
  - `src/features/editor/EditorTabBar.vue`
  - `src/features/editor/DocHeaderBar.vue`
  - `src/features/notes/api.ts`
  - `src/features/notes/store.ts`
  - `src/features/notes/NotesSidebar.vue`
  - `src/app/MainWorkbenchShell.vue`
- 测试文件：
  - `src/features/quickopen/__tests__/quick-open-engine.test.ts`（新增）
  - `src/features/quickbar/__tests__/quickbar-open-mode.test.ts`（新增）

## 3. 具体实现要求

1. **已内联的 IPC 签名（全部位于 `src/lib/ipc/commands.ts`）**：
   - `searchNotesAndFiles(args: { query: string; limit: number | null; requestId: number }): Promise<SearchResponse>`
   - `searchSpotlight(args: { query: string; requestId: number }): Promise<SearchResponse>`
   - `pathComplete(args: { prefix: string; requestId: number }): Promise<PathEntry[]>`
   - `externalActionRun(args: { id: ExternalActionId; absPath: string | null; dirPath: string | null }): Promise<void>`
   - `windowShowMain(args?: { mode: WindowMode | null }): Promise<WindowState>`
   - `windowHideQuickBar(args: { reason: HideReason }): Promise<WindowState>`
2. **严格遵守单向分层（`C1.2`）**：
   - `src/features/quickopen/` 不得直接 import `@tauri-apps/api/*`，也不得跨 feature 深层导入其他 feature 内部文件（仅可经各 feature `index.ts` 或由宿主组件通过 props/events 传递回调与动作列表）。
   - `QuickOpenPalette.vue` 仅使用 `--qn-*` CSS Token（`C7.1`）。
3. **竞态与输入法保护（`C2.2`）**：
   - 所有键盘事件（`Enter`、`Tab`、`Escape`、`ArrowUp`/`ArrowDown`）必须检查 `event.isComposing || event.keyCode === 229` 避免中文拼音选词误触。
   - Spotlight 慢响应晚于最新查询到达时，因 `response.requestId !== activeRequestId` 必须静默丢弃。
4. **跨窗口通信键规范**：
   - 统一导出常量 `PENDING_OPEN_FILE_STORAGE_KEY = 'quicknote:pending-open-file-v1'`（含 `absPath: string; newTab: boolean; line?: number | null; ts: number`），供 `M3-04` 命令模式 `⌘O` 跳源文件直接复用！

## 4. 自动化验收标准（必须全绿）

1. `src/features/quickopen/__tests__/quick-open-engine.test.ts` 覆盖：
   - 前缀解析全部 5 模态（默认混合、`/` 与 `~/` 路径、`#` 全文、`@` 标签、`>` 动作）；
   - 80ms 防抖与 `requestId` 过期乱序丢弃（旧 Spotlight 响应晚到不覆盖新结果）；
   - `/` 路径模式 `Tab` 单项自动补全与多项公共前缀补全、`↵` 进目录与打开文件；
   - `>` 动作模式过滤并触发目标动作执行；
   - `⌥↵` 调用 `externalActionRun(external.reveal)`。
2. `src/features/quickbar/__tests__/quickbar-open-mode.test.ts` 覆盖：
   - Quick Bar 内 `⌘K` 切打开模式、`Esc` 先退回记录模式再按隐藏浮层；
   - 打开模式选中项后写入待打开载荷、唤起主窗口并隐藏 Quick Bar；
   - 外部文件标签页打开与固定到常用文件展示。
3. 运行 `pnpm check:all` 全部 9 项门禁 100% 通过。

## 5. 人工点验清单

- 呼出 Quick Bar（`⌃⌥Space`）→ 按 `⌘K` 切打开模式，依次测试 `/` 路径 `Tab` 补全、`#` 全文、`@` 标签、`>` 动作、Spotlight 搜 `search_roots` 下文件、按 `⌥↵` 在 Finder 显示、按 `↵` 在主窗口打开外部文件标签页并固定为常用文件；按 `Esc` 确认先退回记录模式而非直接消失。
- 主窗口按 `⌘K` 验证大尺寸快速打开弹窗正常且独立于 Quick Bar 状态。
