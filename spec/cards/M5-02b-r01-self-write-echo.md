# M5-02b：修复「检测到外部修改冲突」假弹窗（R01）

- 所属里程碑：M5
- 关联方案与契约：`v5` §2-2；C2.3、C2.4、C8.5、C9.1、C9.3；D-0006
- 前置依赖：M5-01a（C8.5 新口径已写入契约）
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：消除应用在编辑过程中把自己的保存回声误判为「外部修改」而弹出的冲突横幅。
- 不做：不改 `SelfWriteGuard` 的纯状态机语义（`domain/watch.rs` 不动）；不改真实外部修改的冲突横幅、合并视图行为；不改自动保存的防抖时间。

## 2. 允许修改文件白名单

Rust：

- `src-tauri/src/application/notes.rs`
- `src-tauri/src/application/capture.rs`
- `src-tauri/src/application/watch.rs`
- `src-tauri/tests/self_write_guard_and_watch_test.rs`

前端：

- `src/features/editor/store.ts`
- `src/features/editor/MarkdownEditor.vue`
- `src/features/editor/__tests__/editor-autosave-and-conflict.test.ts`

## 3. 具体实现要求

### 3.1 根因（三处，缺一不可）

1. **登记晚于写盘**：`note_write` 的顺序是 写盘 → 更新索引 → 才登记自写哈希。文件监听线程有 100ms 防抖，与主线程真并发，可能在登记之前就检查到「无记录」而放行，把自己的写入当成外部修改发出去。`note_update_meta`、`note_rename`、`tags_batch_rename`、`note_create`、撤销追加是同样的顺序。
2. **前端不认回声**：`handleExternalChange` 收到 `modified` 事件后，脏态分支不比对哈希，直接弹冲突。而事件里的内容其实与标签自己刚保存的一致。
3. **输入法中途保存**：`MarkdownEditor` 在中文输入法组合态每次变化都触发自动保存计时，拼音字母会被存盘；持续输入让标签长时间处于脏态，上面两条的回声就一定落在脏态上。

### 3.2 Rust：先登记再写盘

模式（以 `note_write` 为例，基线约 668–702 行）：`new_content` 已在写盘前算好，把登记提前到 `atomic_write_str` 之前，并在失败时清除。

```rust
let new_body_hash = compute_body_hash(&parse_note(&new_content, stem).body);
pm.record_self_write(&abs_path, &new_body_hash);
if let Err(err) = atomic_write_str(&abs_path, &new_content) {
    pm.clear_self_write(&abs_path);
    return Err(err);
}
```

后面原有的 `let new_body_hash = compute_body_hash(&new_parsed.body); pm.record_self_write(...)` 改为直接使用上面的 `new_body_hash`，不再重复登记。

逐处处理：

| 位置 | 预登记的哈希来源 |
| --- | --- |
| `note_write` 非 Markdown 分支（约 618–621） | `compute_body_hash(&args.body)` |
| `note_write` Markdown 分支（约 693–702） | `parse_note(&new_content).body` |
| `note_create`（约 535） | 创建内容的 body |
| `note_update_meta`（约 779–791） | `parse_note(&new_content).body` |
| `note_rename`（约 902–916） | 重命名前读旧文件内容解析出的 body |
| `tags_batch_rename`（约 1222） | 每个文件写盘前的 `new_content` |
| `capture.rs` 撤销追加（约 303–313） | `compute_body_hash(&snapshot.before_body)`（变量 `restored_body_hash` 已存在，把登记提到 `atomic_write_str` 前） |

规则：预登记一律紧贴写操作之前；写操作返回 `Err` 时立即 `pm.clear_self_write(...)`。`fs::rename` 同理。

`commands/external.rs` 里删除 `.quicknote.yaml` 的登记顺序不在本卡范围（M5-03a 会重写那段），不要动。

### 3.3 Rust：诊断日志

`application/watch.rs` 的 `process_external_fs_path` 里，在 `should_suppress_external_modify` 返回 `true` 的分支加一行：

```rust
eprintln!("[vault-watch] event=vault_change_suppressed");
```

不得输出路径、文件名或任何内容（C2.3）。

### 3.4 Rust 测试

在 `self_write_guard_and_watch_test.rs` 新增一个用例，模拟「监听先到、登记后到」的旧竞态已不存在：

1. 用文件里已有的 `setup_pm_with_library` 建库并建一篇笔记。
2. 直接读取笔记文件字节，调用 `note_write` 写入新正文。
3. 写入返回后立即调用 `process_external_fs_path(&pm, Path::new(&doc.abs_path))`，断言返回 `None`。
4. 再写入一次不同正文，重复第 3 步，仍断言 `None`。

既有用例（`self_write_guard_pure_state_machine` 等）不应需要修改；若因签名变化编译失败，只做最小适配。

### 3.5 前端：忽略自身回声

`handleExternalChange`（基线约 910–970 行）的 `event.kind === 'modified'` 分支，在「`tab.evicted && !tab.isDirty`」判断之后、`if (!tab.isDirty)` 之前插入：

```ts
const isEcho =
  Boolean(event.newBodyHash) &&
  event.newBodyHash === tab.baseBodyHash &&
  (!event.newMeta || isSameNoteMeta(event.newMeta, tab.doc?.meta));
if (isEcho) {
  continue;
}
```

在文件内新增模块级纯函数 `isSameNoteMeta(a: NoteMeta, b: NoteMeta | undefined): boolean`，逐字段比较 `title`、`docType`、`tags`（数组逐项）、`project`、`openDir`、`codeWorkspace`、`terminalDir`、`pinned`、`archived`；`b` 为 `undefined` 时返回 `false`。

注意位置：必须在干净态分支之前，这样干净态标签也不会被回声重置编辑器状态（重置会打断正在进行的输入法组合）。

### 3.6 前端：输入法组合态不触发自动保存

`store.ts`：

- `updateTabBody(key, newBody, cursorOffset?, line?, col?, composing = false)` 增加第 6 个可选参数。
- 末尾 `if (tab.conflictState === null && !tab.isMissing)` 里改为：`tab.saveStatus = 'dirty';` 始终执行；`scheduleAutoSave(tab.key)` 仅在 `composing` 为 `false` 时执行。
- 新增并导出 `scheduleAutoSaveIfDirty(key: string): void`：标签存在、`isDirty`、`conflictState === null`、`!isMissing`、`saveStatus !== 'saving'` 时调用 `scheduleAutoSave(key)`。

`MarkdownEditor.vue`：

- `updateListener` 的 `if (update.docChanged)` 分支里，调用改为 `editorTabsStore.updateTabBody(key, nextBody, head, line, col, update.view.composing)`。
- 在已有的 `EditorView.domEventHandlers({ ... })` 里增加：

```ts
compositionend: (_event, targetView) => {
  if (currentTabKey) {
    editorTabsStore.scheduleAutoSaveIfDirty(currentTabKey);
  }
  return false;
},
```

### 3.7 前端测试

在 `editor-autosave-and-conflict.test.ts` 新增三个用例：

1. **回声被忽略**：标签处于脏态时，`handleExternalChange({ kind: 'modified', absPath, newBody: <与 baseBodyHash 对应>, newBodyHash: <等于 tab.baseBodyHash>, newMeta: <与 tab.doc.meta 相同> })`，断言 `conflictState` 为 `null`、`saveStatus` 不是 `'conflict'`。
2. **真实外部修改仍冲突**：同上但 `newBodyHash` 不同，断言 `conflictState?.kind === 'modified'`。
3. **组合态不排保存**：`updateTabBody(key, '新正文', 0, 1, 1, true)` 后用假定时器推进超过防抖时间，`noteWrite` 未被调用；随后调用 `scheduleAutoSaveIfDirty(key)` 并推进定时器，`noteWrite` 被调用一次。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（含 gen:bindings 零 diff、Rust fmt / clippy / tests）。

## 5. 人工点验清单

- [ ] C9.3 第 5 条：中文拼音输入法在编辑器连续输入 60 秒以上，全程不弹「检测到外部修改冲突」。
- [ ] 英文快速连续输入 30 秒，不弹冲突。
- [ ] 在编辑过程中持续输入，编辑器光标与滚动位置不发生跳变。
- [ ] 用外部编辑器（如 VS Code）真实修改正在编辑的文件：脏态弹冲突横幅、干净态静默重载，行为与之前一致。
- [ ] 组合态中途停顿 3 秒再选词：上屏后文件正常保存；选词前磁盘文件里不应出现未上屏的拼音。

## 上下文提示

- Rust 只读 `notes.rs` 中表格列出的六段、`capture.rs` 约 286–326 行、`watch.rs` 全文（143 行）。
- 前端只读 `store.ts` 的 `updateTabBody`、`scheduleAutoSave`、`handleExternalChange` 三段，`MarkdownEditor.vue` 约 642–720 行。
- 忽略：`EditorConflictBanner.vue`、`EditorMergeView.vue`、`domain/watch.rs`。
