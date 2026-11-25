# M5-04b：壳布局——删右侧栏、底部终端、删顶部横栏

- 所属里程碑：M5
- 关联方案与契约：`v5` §6；C2.3、C4.2、C7.1、C7.2、C7.3、C7.4、C7.5；D-0003、D-0005
- 前置依赖：M5-04a
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - R20：从界面上移除整个右侧栏（`RightPanelGroup`）。
  - R16 / R17：终端移到编辑器正下方的底部面板，可拖动调整高度，保留 `⌃⌘T`。
  - R18 / R19：「大纲」「相关文档」改为左侧栏的两个顶层视图，排在现有导航标签之后。
  - R04：删除主窗口顶部的横栏。
- 不做：不改左侧栏的列表内容、搜索框、标签树（M5-04c 负责）；不清理 `panels` store 里的旧状态（M5-04d 负责）；不改终端会话生命周期。

## 2. 允许修改文件白名单

- `src/app/MainWorkbenchShell.vue`
- `src/features/notes/NotesSidebar.vue`
- `src/features/panels/RightPanelGroup.vue`（删除）
- `src/features/panels/OutlinePanel.vue`（仅当它内部引用了旧的 `panelGroup` 状态时做适配）
- `src/lib/actions/catalog.ts`
- `src-tauri/src/lib.rs`

测试文件（不计入上限）：`src/app/__tests__/auxiliary-window-mode.test.ts`、`src/features/panels/__tests__/right-panel-group.test.ts`

## 3. 具体实现要求

### 3.1 `MainWorkbenchShell.vue`：模板

现结构（基线约 658–775 行）：`header.qn-main-shell__titlebar` → `div.qn-main-shell__body`（`NotesSidebar` + `section.qn-main-shell__editor-col`），其中 `div.qn-main-shell__workspace` 是「编辑区 + `RightPanelGroup`」横向排列。

改成：

```html
<div class="qn-main-shell__body">
  <!-- 原有左侧抽屉遮罩与 <NotesSidebar> 保持不变 -->
  <section class="qn-main-shell__editor-col">
    <EditorTabBar />
    <InboxBloatBanner />
    <EditorConflictBanner />

    <!-- 45% 侧栏辅助模式专用的两个悬浮按钮（标准模式下不渲染） -->
    <div
      v-if="mainWindowMode === 'auxiliary'"
      class="qn-main-shell__aux-controls"
      data-testid="aux-controls"
    >
      <QnButton variant="ghost" size="sm" data-testid="auxiliary-nav-drawer-btn"
        @click="panelsStore.toggleLeftSidebar()">
        {{ panelsStore.leftSidebarCollapsed ? '拉出导航' : '收起导航' }}
      </QnButton>
      <QnButton variant="secondary" size="sm" data-testid="exit-auxiliary-mode-btn"
        @click="exitAuxiliaryMode">退出侧栏辅助</QnButton>
    </div>

    <div class="qn-main-shell__workspace">
      <div class="qn-main-shell__editor-stage">
        <!-- 这一块原有内容（失联横幅 / 文档舞台 / 占位）原样保留 -->
      </div>

      <!-- 底部终端：有存活会话时折叠也保持挂载（D-0005 折叠保活） -->
      <aside
        v-show="panelsStore.terminalPanel.visible"
        class="qn-terminal-dock"
        :class="{ 'qn-terminal-dock--collapsed': !panelsStore.isTerminalPanelShown }"
        data-testid="terminal-dock"
      >
        <div class="qn-terminal-dock__header">
          <button type="button" class="qn-terminal-dock__toggle"
            :title="panelsStore.isTerminalPanelShown ? '折叠终端' : '展开终端'"
            data-testid="terminal-dock-toggle"
            @click="panelsStore.toggleTerminalCollapse()">
            {{ panelsStore.isTerminalPanelShown ? '▾' : '▴' }} 终端
          </button>
          <button type="button" class="qn-terminal-dock__close" title="隐藏终端（会话保留）"
            data-testid="terminal-dock-close" @click="panelsStore.toggleTerminalPanel()">×</button>
        </div>
        <div v-show="panelsStore.isTerminalPanelShown" class="qn-terminal-dock__body"
          :style="{ height: `${panelsStore.terminalPanel.height}px` }">
          <div class="qn-terminal-dock__resizer" role="separator" aria-orientation="horizontal"
            aria-label="调整终端高度" data-testid="terminal-dock-resizer" @pointerdown="startTerminalResize" />
          <TerminalPanel v-if="panelsStore.terminalPanel.visible || terminalStore.hasAnySession" />
        </div>
      </aside>
    </div>
  </section>
</div>
```

要点：

- 整个 `header.qn-main-shell__titlebar` 删除，连同它的样式。「视觉基线预览」按钮与 `<VisualBaselinePreview>` 弹层一并从壳里删除（设置页已有入口）；若删除后 `VisualBaselinePreview` 组件在壳里不再被引用，去掉对应 `import` 与 `baselinePreviewOpen` 状态。
- 「⌃⌥Space 捕捉」徽标随横栏一起删除。
- `toggleAuxiliaryMode` / `exitAuxiliaryMode` 函数保留（`window.toggleAuxiliary` 动作绑定还要用）。
- `.qn-main-shell__workspace` 改为纵向弹性布局：编辑区 `flex: 1; min-height: 0;`，终端面板在下方。
- `.qn-main-shell__editor-col` 设 `position: relative`；`.qn-main-shell__aux-controls` 绝对定位到右上角，`z-index: var(--qn-z-drawer)`。

### 3.2 拖动调整终端高度

仿照被删除的 `RightPanelGroup.vue` 里 `startDragging` 的写法（该文件基线约 40–60 行，删除前先读）在壳里实现 `startTerminalResize(event: PointerEvent)`：

- 记录起始 `clientY` 与起始高度。
- `pointermove` 时新高度 = 起始高度 + (起始 Y − 当前 Y)（向上拖变高），调用 `panelsStore.setTerminalPanelHeight(next)`。
- `pointerup` 时调用 `panelsStore.commitTerminalPanelHeight()` 并解绑监听。
- 组件卸载时确保解绑。

### 3.3 终端可见性联动

终端折叠或隐藏时，存活会话必须收到「不可见」通知以进入保活（Rust 侧蓄水）。先读 `src/features/terminal/TerminalPanel.vue` 约 200–270 行与 `src/features/terminal/store.ts`，找到现有的可见性通知入口（`terminalSetVisibility` / 会话 `visible` 状态）。原来由 `RightPanelGroup` 用 `v-show` 控制 `TerminalPanel`；现在改为：在壳里加一个

```ts
watch(() => panelsStore.isTerminalPanelShown, (shown) => { /* 调用终端 store 的批量可见性入口 */ });
```

若终端 store 没有批量入口，则在壳里对每个会话调用现有的单项入口。保持原有行为：隐藏时不杀会话。

### 3.4 `NotesSidebar.vue`：追加两个顶层视图（最小改动）

- `navItems`（基线约 438–444）保持现有五项不动，在数组末尾追加 `{ id: 'related', label: '相关文档' }` 与 `{ id: 'outline', label: '大纲' }`。把 `navItems` 的类型放宽为 `{ id: string; label: string }[]`。
- 通过 `@/features/panels` 引入 `usePanelsStore`、`OutlinePanel`、`RelatedNotesPanel`。
- 标签点击：`id` 为 `'related'` / `'outline'` 时调用 `panelsStore.toggleLeftView(id)`；其余标签保持原来的 `handleSelectTab`，并在其内部开头加 `panelsStore.setLeftView('workspace')`。
- 选中态：`'related'` / `'outline'` 标签的激活条件是 `panelsStore.leftSidebar.activeView === item.id`。
- 现有的各子面板、工具条、常用外部文件、笔记列表外层包一个 `<div v-show="panelsStore.leftSidebar.activeView === 'workspace'">`（需要 `v-if` / `v-else-if` 的子面板结构整体放进这个 `div` 里）。
- 在其下方追加：

```html
<RelatedNotesPanel v-if="panelsStore.leftSidebar.activeView === 'related'" data-testid="left-view-related" />
<OutlinePanel v-else-if="panelsStore.leftSidebar.activeView === 'outline'" data-testid="left-view-outline" />
```

- 折叠态图标条（`navItems` 的首字按钮）同样支持新标签。
- `OutlinePanel.vue` 目前已存在但未被任何界面使用，直接复用；若它内部引用了 `panelsStore.panelGroup`，改为读 `leftSidebar.activeView`。

### 3.5 动作目录与原生菜单

`src/lib/actions/catalog.ts`：

- 删除 `panel.toggle.recent` 与 `panel.collapseGroup` 两个定义。
- `panel.toggle.outline`、`panel.toggle.related` 的 `title` 改为「大纲视图」「相关文档视图」；`panel.toggle.terminal` 的 `title` 改为「终端面板」。

`MainWorkbenchShell.vue` 的 `bindMainWindowActions`（基线约 200–290）里对应处理函数改为：

- `panel.toggle.outline` → `panelsStore.toggleLeftView('outline')`
- `panel.toggle.related` → `panelsStore.toggleLeftView('related')`
- `panel.toggle.terminal` → `panelsStore.toggleTerminalPanel()`

`src-tauri/src/lib.rs`：

- 「视图」菜单（基线约 218–250）删除 `panel.toggle.recent` 与 `panel.collapseGroup` 两项；保留的三项标题改为「大纲视图」「相关文档视图」「终端面板」。
- `on_menu_event` 的转发白名单（基线约 435–440）删除 `panel.toggle.recent`、`panel.collapseGroup`。
- `src/lib/actions/registry.ts` 的 `NATIVE_MENU_ACTION_IDS` 里的这两项本卡不动（多出的 id 无害），在提交说明里注明留给 M5-04d。

### 3.6 测试

- 删除 `right-panel-group.test.ts` 中依赖 `RightPanelGroup` 组件渲染的用例；其中仍然成立的 store 级断言（如断点下左侧栏自动收起、`applyViewportWidth` 行为）迁移到 `src/features/panels/__tests__/left-sidebar-state.test.ts`。迁移不了的直接删除。
- `auxiliary-window-mode.test.ts`：把对 `toggle-auxiliary-mode-btn` 的断言改为 `exit-auxiliary-mode-btn`；新增断言：标准模式下 `aux-controls` 不渲染，45% 模式下渲染且含「退出侧栏辅助」。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- `grep -rn 'RightPanelGroup' src/` 无结果（`panels/index.ts` 的导出由 M5-04d 清理，此处允许只剩那一行）。

## 5. 人工点验清单

- [ ] 主窗口顶部不再有横栏；窗口仍可通过系统标题栏拖动。
- [ ] `⌃⌘T` 在编辑器下方展开终端；再按一次隐藏。展开状态下向上拖动顶部手柄可改变高度；重启应用后高度保留。
- [ ] 终端里运行 `ping localhost`，点「×」隐藏终端面板：进程不被杀；`⌃⌘T` 再展开后输出连续。
- [ ] 折叠（`▾`）后只剩标题条，会话存活。
- [ ] `⌃⌘O` / `⌃⌘R` 在左侧栏切到「大纲」/「相关文档」视图；再按一次回到笔记列表。
- [ ] 原生菜单「视图」下只有三项，快捷键同步正确。
- [ ] 45% 侧栏辅助模式：右上角出现「拉出导航 / 退出侧栏辅助」；两个按钮都可用；标准模式下不出现。
- [ ] 窗口宽度小于 900px：左侧栏为覆盖式抽屉，编辑区不被挤到 480px 以下。

## 上下文提示

- 只读 `MainWorkbenchShell.vue` 全文（约 1050 行，必须通读，它是本卡主战场）、`NotesSidebar.vue` 约 430–660、`RightPanelGroup.vue` 全文（仅参考拖拽与保活写法）、`catalog.ts` 约 395–440、`lib.rs` 约 218–250 与 414–453。
- 忽略：`panels/store.ts`（上一张卡已备好全部状态与动作，本卡不改）、左侧栏列表内容。
