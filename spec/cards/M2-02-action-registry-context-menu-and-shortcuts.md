# M2-02：前端统一动作注册表（`C7.2`/`C7.3`）、作用域分发引擎与同源右键/系统菜单桥接

- 所属里程碑：M2
- 关联方案与契约：`v4` §3.7.1、§3.8、§5.6.3、§6 M2、附录 A；`C1.1`、`C1.2`、`C2.2`、`C7.1`、`C7.2`、`C7.3`、`C9.1`
- 前置依赖：`M2-01`
- 会话策略：独立新会话执行（零大文档读取，所有接口签名、优先级规则、冲突矩阵与动作目录已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 在 `src/lib/actions/` 实现零框架依赖的统一动作注册表引擎（`C1.2`、`C7.2`、`C7.3`）**：
    - `src/lib/actions/types.ts`：
      - 严格实现 `C7.2` 契约类型：
        ```ts
        export type ActionCategory = 'global' | 'quickbar' | 'window' | 'editor' | 'external' | 'panel';
        export type ActionScope = 'global' | 'modal' | 'quickbar' | 'terminal' | 'editor' | 'list' | 'main-window';
        export type ActionMenuGroup = 'format' | 'view' | 'tools' | 'editor-context';

        export interface ActionContext {
          activeScope: ActionScope;
          activeScopes?: ActionScope[];
          hasSelection?: boolean;
          isInbox?: boolean;
          isMarkdown?: boolean;
          hasActiveDoc?: boolean;
          hasTerminalSelection?: boolean;
          linkAtCursor?: string | null;
          payload?: Record<string, unknown>;
        }

        export interface ActionDef {
          id: string;
          title: string;
          category: ActionCategory;
          defaultKeys: string | null;
          scope: ActionScope;
          when?: (ctx: ActionContext) => boolean;
          swallowWhenDisabled?: boolean;
          menuGroup?: ActionMenuGroup;
          run: (ctx: ActionContext) => void | Promise<void>;
        }

        export type KeybindingOverrideMap = Record<string, string | null>;

        export interface ShortcutConflict {
          actionId: string;
          title: string;
          scope: ActionScope;
          keys: string;
        }

        export interface ResolvedMenuItem {
          id: string;
          title: string;
          shortcutLabel: string;
          disabled: boolean;
          menuGroup: ActionMenuGroup;
        }
        ```
    - `src/lib/actions/keymap.ts`：
      - `normalizeKeyCombo(raw: string | null): string | null`：规范化组合键为固定修饰顺序 `ctrl+alt+shift+cmd+<key>`（小写），别名映射 `meta`/`command`/`mod` → `cmd`、`option` → `alt`、`control` → `ctrl`、`return` → `enter`、`esc` → `escape`、`spacebar`/` ` → `space`。
      - `keyboardEventToCombo(event: KeyboardEvent): string | null`：
        - **macOS `Option` 键关键修复**：macOS 下按 `⌥⌘C` / `⌥⌘G` / `⌥⌘1` 时 `event.key` 会变成 `ç` / `©` / `¡`！**必须优先通过 `event.code` 映射主键**（`KeyA`..`KeyZ` → `a`..`z`，`Digit0`..`Digit9` → `0`..`9`，`Space` → `space`，`Enter` → `enter`，`Escape` → `escape`，`Tab` → `tab`，`ArrowUp`/`ArrowDown`/`ArrowLeft`/`ArrowRight` → `arrowup` 等，`Comma` → `,`，`Backspace` → `backspace`），仅当 `event.code` 缺失（如单元测试手动构造只有 `key` 的事件）时回退 `event.key`。
        - 单独按下修饰键（`Meta`/`Control`/`Alt`/`Shift`）返回 `null`。
      - `formatKeyComboDisplay(combo: string | null): string`：输出标准 macOS 符号（如 `alt+cmd+c` → `⌥⌘C`、`shift+tab` → `⇧Tab`、`cmd+enter` → `⌘↵`、`ctrl+cmd+o` → `⌃⌘O`）。
    - `src/lib/actions/registry.ts`：
      - 作用域优先级常量 `SCOPE_PRIORITY: readonly ActionScope[] = ['modal', 'quickbar', 'terminal', 'editor', 'list', 'main-window', 'global']`（`C7.3`）。
      - `createActionRegistry(options?: { storageKey?: string; storage?: Storage | null })` 工厂与默认单例 `actionRegistry`：
        - `register(def: ActionDef): () => void`、`registerMany(defs: ActionDef[]): () => void`、`bindHandler(id: string, handler: ActionDef['run'], options?: { when?: ActionDef['when']; swallowWhenDisabled?: boolean }): () => void`。
        - `getAll(): ActionDef[]`、`getById(id: string): ActionDef | undefined`、`getEffectiveKeys(id: string): string | null`（覆写优先于 `defaultKeys`）。
        - **同作用域冲突检测**：`detectConflicts(actionId: string, candidateKeys: string | null): ShortcutConflict[]`：检查同 `scope` 下是否存在其他已注册动作的有效键位与规范化后的 `candidateKeys` 相同；`setKeyOverride(actionId: string, keys: string | null): { ok: true } | { ok: false; conflicts: ShortcutConflict[] }`（存在同作用域冲突时**拒绝静默覆盖**并返回冲突列表！）。
        - `resetKeyOverride(actionId: string)`、`resetAllOverrides()`、`exportOverridesJson(): string`、`importOverridesJson(rawJson: string): { ok: true; applied: number } | { ok: false; error: string; conflicts?: ShortcutConflict[] }`（支持合法性校验与同作用域冲突校验）。
        - 持久化键 `quicknote:action-keybindings-v1`（通过 `localStorage` 读写，并支持 `syncFromStorage()` 供多窗口 `storage` 事件实时同步）。
        - **精确快捷键分发**：`dispatchKeyEvent(event: KeyboardEvent, ctx: ActionContext): { handled: boolean; swallowed: boolean; actionId: string | null }`：
          1. 计算 `combo = keyboardEventToCombo(event)`；若为 `null` 返回 `{ handled: false, swallowed: false, actionId: null }`；
          2. 确定当前激活作用域链路：取 `ctx.activeScopes ?? [ctx.activeScope, 'main-window', 'global']` 并按 `SCOPE_PRIORITY` 从高到低排序去重；
          3. 逐层作用域匹配 `getEffectiveKeys(action.id) === combo`：
             - 若命中且 `action.when === undefined || action.when(ctx) === true`：调用 `event.preventDefault()`、`event.stopPropagation()` 并执行 `void action.run(ctx)`，返回 `{ handled: true, swallowed: false, actionId: action.id }`；
             - 若命中但 `action.when(ctx) === false`：
               - 若 `action.swallowWhenDisabled === true`：**显式吞键**（`event.preventDefault()` + `event.stopPropagation()`），阻断向下层作用域穿透，返回 `{ handled: false, swallowed: true, actionId: action.id }`；
               - 否则：**`when` 未命中放行**，继续检查下层作用域或放行给浏览器/系统默认行为！
        - `executeById(id: string, ctx: ActionContext): Promise<boolean>`：供右键菜单与原生系统菜单（`onMenuAction`）触发，校验 `when(ctx)` 后执行。
        - `getMenuItems(menuGroup: ActionMenuGroup, ctx: ActionContext): ResolvedMenuItem[]`：筛选同源菜单项并返回 `{ id, title, shortcutLabel, disabled: when ? !when(ctx) : false, menuGroup }`。
    - `src/lib/actions/catalog.ts`：
      - 预注册 `C7.2` 全部核心命名空间元数据（确保 Settings 窗口独立打开时即拥有完整动作表用于检索、录制与冲突检测）：
        - `global.*`：`global.toggleQuickBar`（`ctrl+alt+space`）、`global.toggleMainWindow`（`cmd+shift+space`）
        - `quickbar.*`：`quickbar.mode.capture`（`null`）、`quickbar.mode.open`（`cmd+k`）、`quickbar.mode.command`（`cmd+shift+k`）、`quickbar.captureMode.list`（`cmd+1`）、`quickbar.captureMode.todo`（`cmd+2`）、`quickbar.captureMode.prose`（`cmd+3`）、`quickbar.selectTarget`（`cmd+t`）、`quickbar.submitAppend`（`cmd+enter`）、`quickbar.submitOpenMain`（`shift+cmd+enter`）、`quickbar.saveAsNote`（`alt+cmd+enter`）、`quickbar.openCommandSource`（`cmd+o`）
        - `window.*`：`window.newNote`（`cmd+n`）、`window.save`（`cmd+s`）、`window.closeTab`（`cmd+w`）、`window.reopenTab`（`shift+cmd+t`）
        - `editor.*`（全部带 `menuGroup: 'format'` 或 `'editor-context'`，常规格式动作不依赖选区）：
          - `editor.bold`（`cmd+b`）、`editor.italic`（`cmd+i`）、`editor.codeBlock`（`alt+cmd+c`）、`editor.table`（`alt+cmd+g`）、`editor.toggleTodo`（`alt+cmd+x`）、`editor.link`（`alt+cmd+k`）、`editor.image`（`alt+cmd+i`）、`editor.bulletList`（`alt+cmd+l`）、`editor.blockquote`（`alt+cmd+q`）、`editor.heading1`..`6`（`alt+cmd+1`..`6`）、`editor.indent`（`tab`）、`editor.outdent`（`shift+tab`）、`editor.moveSelectionToNewNote`（`defaultKeys: null`, `menuGroup: 'editor-context'`, `when: (ctx) => Boolean(ctx.isInbox && ctx.hasSelection)`）
        - **`terminal` 高优先级显式吞键保护（`C7.3`）**：
          - `terminal.swallowCodeBlock`（`scope: 'terminal'`, `defaultKeys: 'alt+cmd+c'`, `when: () => false`, `swallowWhenDisabled: true`）—— 确保焦点位于 `terminal` 时按 `⌥⌘C` 绝不穿透至 `editor.codeBlock`！
          - `terminal.copySelection`（`scope: 'terminal'`, `defaultKeys: 'cmd+c'`, `when: (ctx) => Boolean(ctx.hasTerminalSelection)`, `swallowWhenDisabled: false`）—— 仅在终端有选区时拦截复制，无选区放行给 PTY。
        - `panel.*`（`menuGroup: 'view'`）：`panel.toggle.outline`（`ctrl+cmd+o`）、`panel.toggle.terminal`（`ctrl+cmd+t`）、`panel.toggle.related`（`ctrl+cmd+r`）、`panel.toggle.recent`（`ctrl+cmd+e`）、`panel.collapseGroup`（`ctrl+cmd+0`）
        - `external.*`：`external.reveal`（`cmd+shift+o`）、`external.terminal`（`alt+cmd+t`）、`external.vscode`、`external.sublime`、`external.workspace`、`external.default`、`external.copyPath`、`external.pin`
      - 导出 `registerDefaultActionCatalog(registry)` 并在默认单例初始化时自动注册。
    - `src/lib/actions/index.ts`：统一导出全部类型、工具函数与 `actionRegistry`。
  - **2. 新增纯展示右键菜单组件 `src/components/QnContextMenu.vue`（`C1.2`、`C7.1`）**：
    - Props：`open: boolean`、`x: number`、`y: number`、`items: ContextMenuEntry[]`（支持普通项 `{ type: 'item', id, title, shortcutLabel?, disabled?, danger? }` 与分隔线 `{ type: 'separator', id }`）。
    - Emits：`select(id: string)`、`close()`。
    - 行为：自动夹取视口边界防溢出；点击外部或按 `Esc` 触发 `close`；严格零 IPC、零裸色值、层级使用 `z-index: var(--qn-z-popover)`。
    - 在 `src/components/index.ts` 导出 `QnContextMenu` 及 `ContextMenuEntry` 类型。
  - **3. 在 `MainWorkbenchShell.vue` 装配系统菜单同源桥接与主窗口快捷键分发**：
    - 订阅 `onMenuAction((actionId) => { void actionRegistry.executeById(actionId, buildActionContext()); })`；
    - 监听 `window` 的 `storage` 事件调用 `actionRegistry.syncFromStorage()`（与设置窗口改键跨窗同步）；
    - 绑定主窗口 `window.save`（`editorTabsStore.flushSave()`）、`window.closeTab`（关闭当前标签）、`window.reopenTab`（`editorTabsStore.reopenClosedTab()`）、`panel.toggle.outline`（`panelsStore.togglePanel('outline')`）、`panel.collapseGroup`（`panelsStore.toggleCollapseGroup()`），并在主窗口顶层 `keydown` 根据当前焦点 DOM 自动推导作用域链路（如 Modal 打开时加 `'modal'`、焦点在编辑器时加 `'editor'`、在侧栏列表时加 `'list'`）交由 `actionRegistry.dispatchKeyEvent` 统一分发。
- 不做：
  - 不修改 `src/features/settings/` UI（归 `M2-03`）。
  - 不实现 CM6 具体格式命令实现体（归 `M2-05a`，本卡仅注册动作目录与默认占位 handler）。

## 2. 允许修改文件白名单
- 共享串行文件：无
- 前端基础库：`src/lib/actions/**`（新增：`types.ts`、`keymap.ts`、`registry.ts`、`catalog.ts`、`index.ts`、`__tests__/action-registry.test.ts`）
- 纯展示组件：`src/components/QnContextMenu.vue`（新增）、`src/components/index.ts`
- 应用装配壳：`src/app/MainWorkbenchShell.vue`

## 3. 具体实现要求
- **严格分层（`C1.2`）**：`src/lib/actions/**` 严禁导入 `src/components`、`src/features`、`src/app` 或 `@tauri-apps/api`；`src/components/QnContextMenu.vue` 严禁导入 `src/lib/ipc`、`src/features` 或 `src/app`。
- **样式红线（`C7.1`）**：`QnContextMenu.vue` 严禁出现 `#xxx` 裸色值与数字 `z-index`，仅使用 `--qn-*` 与 `var(--qn-z-popover)`。
- **TypeScript 严格模式（`C2.2`）**：零 `any`，遵循 `noUncheckedIndexedAccess`。

## 4. 自动化验收标准（必须全绿）
- 新增测试文件：`src/lib/actions/__tests__/action-registry.test.ts`，覆盖完整冲突矩阵与作用域分发：
  1. **优先级排序**：同时在 `modal`、`terminal`、`editor`、`main-window` 注册同一快捷键时，严格按 `modal > quickbar > terminal > editor > list > main-window > global` 命中最高优先级动作。
  2. **`when` 未命中放行**：`editor.moveSelectionToNewNote` 在 `hasSelection: false` 时放行不调用 `preventDefault()`；`terminal.copySelection`（`cmd+c`）在 `hasTerminalSelection: false` 时放行（供 PTY 处理），在 `hasTerminalSelection: true` 时命中执行。
  3. **高优先级显式吞键**：当 `activeScope: 'terminal'` 且链路含 `['terminal', 'editor', 'main-window']` 时，按下 `⌥⌘C`（含 macOS `event.code = 'KeyC', event.key = 'ç'` 场景）被 `terminal.swallowCodeBlock` 显式吞掉（`swallowed: true`, `preventDefault` 被调用），绝不触发下层 `editor.codeBlock`；按下 `⌃C`（未注册）放行直传 PTY。
  4. **同作用域冲突检测与跨作用域隔离**：将 `editor.italic` 改为 `cmd+b` 返回冲突 `editor.bold` 且不覆盖原键位；将 `quickbar` 动作设为 `cmd+b`（不同作用域）允许成功。
  5. **覆写持久化与导入导出**：验证 `setKeyOverride`、`resetKeyOverride`、`resetAllOverrides`、`exportOverridesJson`、`importOverridesJson`（非法 JSON 或冲突配置被拒绝）。
  6. **同源菜单生成**：验证 `getMenuItems('format', ctx)` 与 `getMenuItems('editor-context', ctx)` 返回最新自定义快捷键标签与正确的 `disabled` 态。
- 必过命令：`pnpm check:all`

## 5. 人工点验清单（如涉及 macOS 原生行为）
- 无新增原生桥接（macOS 菜单转发验证将在 `M2-05a` 接入编辑器格式命令后一并点验）。
