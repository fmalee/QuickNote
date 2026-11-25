# M5-02c：修复编辑器改标签后列表不更新（R21）

- 所属里程碑：M5
- 关联方案与契约：`v5` §2；C2.2、C4.2、C9.1；D-0006、D-0007
- 前置依赖：无
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：定位并修复「在编辑器里修改标签后，左侧列表里的标签显示没有同步」。
- 不做：不改标签批量重命名功能；不改 `note_update_meta` 的 YAML 最小补丁逻辑。**根因尚未定位，禁止在没复现的情况下直接改代码。**

## 2. 允许修改文件白名单

- `src-tauri/tests/note_crud_and_index_test.rs`
- `src/features/notes/__tests__/notes-tree-list-and-tabs.test.ts`（或同目录新建测试文件）
- 视定位结果而定，仅限：`src/features/notes/store.ts`、`src/features/editor/store.ts`

若定位结果指向这两个文件之外，停下并报告，不要扩范围。

## 3. 具体实现要求（按顺序执行，逐步排除）

### 3.1 已排除的路径（主控已核对，不必重查）

- 后端索引是同步的：`note_update_meta`（`application/notes.rs` 约 717–799）在返回前调用 `index_single_note`，事务内替换 `note_tags`。
- 前端通知链路存在：`updateActiveMeta`（`editor/store.ts` 约 1533）调用 `notifyNotesRefresh()` → `notesStore.refreshAll()` → `loadTreeAndTags()` + `loadNotesList()`。

### 3.2 第一步：后端集成测试

在 `note_crud_and_index_test.rs` 新增用例：

1. 建库，建一篇笔记，`note_update_meta` 把 `tags` 改为 `["a", "b"]`。
2. **立即**调用 `notes_list`（`tab: "tags"`, `tags: ["a"]`）与 `notes_get_tree_and_tags`。
3. 断言列表包含该笔记且 `tags` 为 `["a", "b"]`，标签树含 `a`、`b`。

若失败：根因在后端，按失败处最小修复（白名单不含后端，需停下报告）。

### 3.3 第二步：前端复现

在 notes 测试里新增用例：mock `noteUpdateMeta` 返回新 `meta`，调用 `editorTabsStore.updateActiveMeta({ tags: ['x'] })` 后，断言 `notesStore.notes[0].tags` 与 `notesStore.navTree.tags` 已更新。

若通过，继续排查最可疑的点：

- `loadNotesList`（`notes/store.ts` 约 154–174）用请求序号守卫丢弃旧响应。检查是否存在「刷新被更早发出的请求抢先占用序号」或「`refreshAll` 与 `index:status` 订阅回调并发」导致新鲜响应被丢。
- 列表卡片用 `:key="note.key"`。检查是否因重复 `key` 导致 DOM 未更新（两篇笔记共用同一 ULID 时会出现）。
- 「标签」标签页下带筛选时，被改标签的笔记进出筛选结果属正常行为，不是缺陷。

### 3.4 第三步：修复并补回归测试

只修定位到的点，并保留第 3.2、3.3 步的测试作为回归保护。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- 提交说明里必须写明：定位到的根因、证据（哪条测试在修复前失败）。

## 5. 人工点验清单

- [ ] 顶栏增删标签 chip 后，左侧列表卡片上的标签立即更新。
- [ ] 「标签」标签页的标签树数量立即更新。
- [ ] `[属性]` 面板修改 `project` / `pinned` 后列表立即更新。

## 上下文提示

- 若三步排查后仍无法复现，保持代码不动，把排查记录写进提交说明并结束会话，由用户在实机补充复现步骤。
