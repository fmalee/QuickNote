# M5-04c：左侧栏整体改造

- 所属里程碑：M5
- 关联方案与契约：`v5` §2-1、§6；C3.4、C6.3、C7.1、C8.2；D-0012
- 前置依赖：M5-04b
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做（左侧栏导航最终形态：`目录｜标签｜最近｜相关文档｜大纲`）：
  - R10：删除「收件箱」标签页及其徽标、相关无效代码。**`inbox.md` 的追加能力必须保留。**
  - R12：「项目」改称「目录」。
  - R11：`inbox.md` 默认置顶显示在「目录」列表里。
  - R13：未选子目录时只显示笔记库根目录下的文档，不再显示全库文档。
  - R05：列表摘要只显示一行（左侧列表一处；另一处「同级文件」列表由 M5-04e 处理）。
  - R06：卡片的「重命名」「删除」按钮移入右键菜单，所有文档卡片支持右键。
  - R08：搜索框与「+ 新建」移到标签标题下方。
  - R09：收起按钮位于导航标签行最右。
  - R14：标签树改为标签云。
  - R15：左侧栏宽度可拖动并记住。
  - 同时删除 V4 遗留的「工作区」标签页。
- 不做：不改 Quick Bar 的目标解析；不改 `.quicknote.yaml` 目录绑定徽标与目录右键菜单；「相关文档」「大纲」视图不动。

## 2. 允许修改文件白名单

- `src/features/notes/NotesSidebar.vue`
- `src/features/notes/store.ts`
- `src-tauri/src/application/notes.rs`
- `src-tauri/src/domain/note.rs`
- `src-tauri/src/domain/ui_state.rs`

测试文件（不计入上限）：`src/features/notes/__tests__/notes-tree-list-and-tabs.test.ts`、`src-tauri/tests/note_crud_and_index_test.rs`、`src-tauri/tests/persistence_test.rs`

## 3. 具体实现要求

### 3.1 后端：列表语义（`application/notes.rs`）

`notes_list`（基线约 147–245）：

- `"projects"` 分支（约 175–183）改为：

```rust
"projects" => {
    if is_template {
        false
    } else if let Some(ref prefix) = dir_prefix {
        row.rel_path.starts_with(prefix)
    } else {
        !row.rel_path.contains('/')      // 未选目录：仅根目录文档
    }
}
```

- 排序（约 240–242）由「仅 `recent` 按 mtime 排序」改为：先按 `tab` 做稳定预处理，再统一排序：

```rust
if args.tab == "recent" {
    out.sort_by_key(|a| std::cmp::Reverse(a.mtime_ms));
} else if args.tab == "projects" && dir_prefix.is_none() {
    // inbox.md 恒置顶，其余保持 mtime 倒序
    let mut inbox: Vec<NoteSummary> = Vec::new();
    let mut rest: Vec<NoteSummary> = Vec::new();
    for item in out {
        if item.is_inbox { inbox.push(item); } else { rest.push(item); }
    }
    rest.sort_by_key(|a| std::cmp::Reverse(a.mtime_ms));
    inbox.extend(rest);
    out = inbox;
}
```

`notes_get_tree_and_tags`（约 248–315）：删除 `inbox_count` 的计算（约 252–260），`NavTreeSnapshot` 构造去掉该字段。

`domain/note.rs`：`NavTreeSnapshot` 删除 `inbox_count` 字段。随后 `pnpm gen:bindings`。

`domain/ui_state.rs`：`NavTab` 删除 `Inbox` 变体；`Projects` 同时标注 `#[default]` 与 `#[serde(other)]`（旧 `ui-state.json` 里的 `"inbox"` 自动落到 `Projects`）。`FiltersUiState` 的 `Default` 里 `active_tab` 改为 `NavTab::Projects`。

### 3.2 前端：`notes/store.ts`

- `NavTab` 类型改为 `'projects' | 'tags' | 'recent' | 'workspaces'`（`workspaces` 暂留以兼容持久化值，M5-06f 再删）；`normalizeNavTab`：`'inbox'` → `'projects'`，未知值 → `'projects'`，`'workspaces'` → `'projects'`。
- `activeTab` 初值 `'projects'`。
- `navTree` 初值去掉 `inboxCount`。

### 3.3 前端：`NotesSidebar.vue`

**导航行**（基线约 628–656）

- `navItems` 改为四项：`目录`（`projects`）、`标签`（`tags`）、`最近`（`recent`）、其后是上一张卡加的 `相关文档`、`大纲`。删除 `收件箱` 与 `工作区`。
- 删除收件箱徽标 `QnBadge`（`inbox-count-badge`）。
- `handleSelectTab`：删除 `if (tab === 'inbox') { openNote('inbox.md') }` 分支。
- 收起按钮保持在 `v-for` 之后（即整行最右），位置不变。
- 删除 `v-else-if="notesStore.activeTab === 'workspaces'"` 整块子面板（约 813–846）。
- 「项目目录」子面板标题改为「目录」；「全部项目」链接文案改为「← 根目录」。

**标签标题下方的工具条**（R08）

- 把现有的 `div.qn-notes-sidebar__toolbar`（约 848–867）从列表上方移到导航行正下方，只在 `activeView === 'workspace'` 且 `activeTab` 为 `projects` / `tags` / `recent` 时渲染。
- 内容不变（过滤输入框 + 「+ 新建」），保留 `data-testid="notes-filter-input"` 与 `notes-create-btn"`。占位文案按标签变化：目录「过滤目录文档…」、标签「过滤标签文档…」、最近「过滤最近文档…」。

**标签云**（R14）

- 删除 `flatTags` 展开树渲染（约 745–800 的树与展开按钮），删除 `expandedTags` / `toggleTagExpand`。
- 新增：

```ts
interface TagCloudItem { tag: string; count: number }
const tagCloudItems = computed<TagCloudItem[]>(() =>
  flattenTagNodes(notesStore.navTree.tags)   // 递归展开所有层级的 { tag, count }
    .sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag))
    .slice(0, 80),
);
const tagCloudTier = (count: number): 'sm' | 'md' | 'lg' => { /* 按数量在云内的分位：前 1/3 lg、中 1/3 md、其余 sm */ };
```

- 模板：

```html
<div class="qn-notes-sidebar__tag-cloud" data-testid="tag-cloud">
  <button v-for="item in tagCloudItems" :key="item.tag" type="button"
    :class="['qn-notes-sidebar__tag-cloud-item', `qn-notes-sidebar__tag-cloud-item--${tagCloudTier(item.count)}`,
             { 'qn-notes-sidebar__tag-cloud-item--active': notesStore.selectedTags.includes(item.tag) }]"
    :title="`${item.tag} · ${item.count}`" :data-testid="`tag-cloud-item-${item.tag}`"
    @click="notesStore.toggleTag(item.tag)" @contextmenu.prevent="openTagContextMenu($event, item.tag)">
    #{{ item.tag }}
  </button>
</div>
```

- 三档字号只用 Token：`--qn-text-sm`、`--qn-text-base`、`--qn-text-lg`。
- 保留「清除筛选」与已选标签 chips。标签右键菜单（重命名 / 合并）保留。

**列表卡片**（R05、R06）

- 摘要 `<p class="qn-notes-sidebar__card-excerpt">` 改为单行：`white-space: nowrap; overflow: hidden; text-overflow: ellipsis;`，并加 `:title="note.excerpt"`。
- 删除卡片内的「重命名」「删除」两个按钮及其 `template v-if="canMutateNote(note)"`，删除 `.qn-notes-sidebar__card-action` 相关样式。
- 卡片加 `@contextmenu.prevent="openNoteContextMenu($event, note)"`。
- 新增笔记右键菜单（`QnContextMenu`），条目：

| id | 标题 | 说明 |
| --- | --- | --- |
| `note.open` | 打开 | 预览打开 |
| `note.openPinned` | 在新标签打开 | 固定打开 |
| `note.reveal` | 在访达中显示 | 调 `notesStore.revealExternalFile(note.absPath)` |
| 分隔线 | | |
| `note.rename` | 重命名… | 进入现有的内联重命名输入框；仅库内文档且非 `inbox.md` 时显示 |
| `note.delete` | 删除（移到废纸篓） | `danger: true`；调用 `notesStore.deleteNoteItem`；仅库内文档且非 `inbox.md` 时显示 |

- 重命名输入框（`note-rename-input`）与 `commitRename` 逻辑保留。
- 常用外部文件条目沿用现有的外部文件右键菜单，不并入。

**左栏可拖宽**（R15）

- 根元素 `<aside class="qn-notes-sidebar">` 加 `:style="{ width: `${panelsStore.leftSidebarWidth}px` }"`；样式里的 `width: 300px` 改为 `flex: 0 0 auto; min-width: 0;`。
- 右边缘加手柄（抽屉模式与折叠态不渲染）：

```html
<div class="qn-notes-sidebar__resizer" role="separator" aria-orientation="vertical"
  aria-label="调整左侧栏宽度" data-testid="left-sidebar-resizer" @pointerdown="startSidebarResize" />
```

- `startSidebarResize`：记录起始 `clientX` 与起始宽度（`panelsStore.leftSidebar.width`），`pointermove` 时 `setLeftSidebarWidth(起始宽度 + (当前 X − 起始 X))`，`pointerup` 时 `commitLeftSidebarWidth()` 并解绑；卸载时解绑。写法参考仓库里其他拖拽手柄的 `pointer` 事件处理，不要用 `mousemove`。

**其他清理**

- 删除 `navTree.workspaces` 相关的渲染代码；`notesStore.selectedWorkspace`、`selectWorkspace` 本卡不动。
- 全文件搜索 `inbox-count`、`nav-tab-inbox`、`工作区` 字样，确认无残留（「目录工作区配置」是指 `.quicknote.yaml` 绑定，保留）。

### 3.4 测试

- `notes-tree-list-and-tabs.test.ts`：删除 / 改写依赖收件箱标签页、收件箱徽标、卡片内重命名与删除按钮的用例；新增用例：
  1. 默认 `activeTab` 为 `'projects'`；持久化值 `'inbox'` 经 `normalizeNavTab` 后为 `'projects'`。
  2. 渲染含 `nav-tab-projects`、`nav-tab-tags`、`nav-tab-recent`，不含 `nav-tab-inbox` 与 `nav-tab-workspaces`；「项目」文案不再出现。
  3. 标签云渲染 `tag-cloud`，点击某个标签后 `notesList` 的调用参数里 `tags` 含该标签。
- `note_crud_and_index_test.rs`：新增用例「未选目录时 `notes_list(tab=projects)` 只返回根目录文档，且 `inbox.md` 排第一；选了子目录时返回该子目录文档」。
- `persistence_test.rs`：新增用例「`ui-state.json` 的 `filters.activeTab` 为 `"inbox"` 时可正常加载，读回为 `Projects`」。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- `grep -rn 'inboxCount\|nav-tab-inbox\|收件箱' src/features/notes/ src/features/editor/store.ts | grep -v __tests__` 只剩与 `inbox.md` 文件本身相关的逻辑（如 `isInboxBloated`、「收件箱清理」提示）。

## 5. 人工点验清单

- [ ] 左侧导航为 `目录｜标签｜最近｜相关文档｜大纲`；无「收件箱」「工作区」。
- [ ] 「目录」默认只列根目录文档，`inbox.md` 在最上方；点进子目录只列该目录文档；「← 根目录」回到根。
- [ ] 搜索框与「+ 新建」在标签行正下方；切到「相关文档」「大纲」时不显示。
- [ ] 标签以云的形式展示，字号随数量变化；点击筛选、再次点击取消；右键仍可重命名 / 合并。
- [ ] 卡片摘要单行省略；卡片上不再有重命名 / 删除按钮；右键菜单在任意卡片上可用，`inbox.md` 的右键菜单没有重命名 / 删除。
- [ ] 右键「删除（移到废纸篓）」：文件进废纸篓，对应标签显示「已删除」。
- [ ] 拖动左栏右边缘改变宽度；重启后宽度保留；窄于 900px 时为抽屉且不能拖宽。
- [ ] Quick Bar 仍可追加到 `inbox.md`（`⌘↵` 提交后内容出现在当日分区）。

## 上下文提示

- 只读 `NotesSidebar.vue` 全文（必须通读，约 1660 行，是本卡主战场）、`notes/store.ts` 约 25–130、`application/notes.rs` 约 147–315、`domain/note.rs` 中 `NavTreeSnapshot` 定义、`ui_state.rs` 开头的 `NavTab` 与 `FiltersUiState`。
- 忽略：Quick Bar、设置页、右侧面板 store。
