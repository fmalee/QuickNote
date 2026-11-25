# M5-06f：清理废弃的全局工作区模型（可选）

- 所属里程碑：M5
- 关联方案与契约：`v5` §4；C3.4、C4.2、C6.2、C6.3、C8.1、C11.1、C11.2；D-0012
- 前置依赖：M5-06d
- 会话策略：独立新会话执行；完成后提交并结束会话

> **可选卡**，用户已同意「不会引起后续任务错乱就一并删除」。这是全包里契约牵连最广的一张（触及 `C8.1` 目标真值表），建议由能力较强的模型执行，并且必须放在最后。
> 难度偏高。执行中若发现工作量明显超出本卡范围，或会破坏 Quick Bar 捕捉的现有测试，**停下并报告**，不要强行推进。

## 1. 目标范围

- 要做：删除 D-0012 已废止的「脱离笔记库的独立全局工作区」模型在代码与契约中的残留：`settings.yaml` 的 `workspaces`、`NavTreeSnapshot.workspaces`、`NotesFilterArgs.workspaceKey`、`ui-state.json` 的 `selectedWorkspaceKey` / `workspaceModes` / `lastActiveDoc.workspaceKey`、`TargetResolveInput.selectedWorkspaceKey` / `isInboxView`，以及 `C8.1` 里依赖它们的场景。
- 不做：不改目录级 `.quicknote.yaml` 三级级联（`domain/workspace_context.rs`）；不改终端的会话分组键 `workspaceKey` 的现有语义（它是一个自由字符串，继续用于「每个上下文至多 1 个会话」）。

## 2. 允许修改文件白名单（每一步单独提交，范围不足时停下报告，不要擅自扩）

契约提交：`spec/engineering-contract.md`

Rust：`src-tauri/src/domain/settings.rs`、`src-tauri/src/domain/capture.rs`、`src-tauri/src/domain/note.rs`、`src-tauri/src/domain/ui_state.rs`、`src-tauri/src/application/capture.rs`、`src-tauri/src/application/notes.rs`、`src-tauri/src/application/external.rs`、`src-tauri/src/bindings.rs`

前端：`src/features/notes/store.ts`、`src/features/quickbar/store.ts`、`src/features/quickbar/QuickBarTargetPicker.vue`、`src/features/editor/DocPropertiesPopover.vue`、`src/features/terminal/store.ts`、`src/features/commands/api.ts`

文档：`DECISIONS.md`（在 `D-0012` 的「否决 / 废止的旧方案」下补一行「模型残留已于 M5-06f 清理」）、`spec/CODEBASE_MAP.md`、`spec/PROJECT_STATE.md`

测试文件（不计入上限）：`src-tauri/tests/capture_truth_table_test.rs`、`src-tauri/tests/persistence_test.rs`、`src-tauri/tests/note_crud_and_index_test.rs`、`src-tauri/tests/m3_commands_related_tags.rs`、`src/features/notes/__tests__/*`、`src/features/quickbar/__tests__/*`、`src/features/terminal/__tests__/*`

## 3. 具体实现要求（严格按顺序，三次提交）

### 3.1 第零步：盘点（只读，不提交）

执行并把结果整理成清单写进提交说明：

```sh
grep -rn 'workspaces\|WorkspaceConfig\b\|WorkspaceSummary\|selected_workspace_key\|workspace_key\|workspace_modes\|is_inbox_view\|isInboxView\|default_entry_doc' src-tauri/src --include=*.rs | grep -v DirectoryWorkspace
grep -rn 'workspaces\|selectedWorkspace\|selectedWorkspaceKey\|workspaceModes\|workspaceKey\|isInboxView' src --include=*.ts --include=*.vue | grep -v __tests__
```

把命中逐条标成「全局工作区模型（要删）」或「目录级 `.quicknote.yaml` / 终端会话键 / 笔记 `project` 字段（保留）」。若有命中落在白名单之外，停下报告。

### 3.2 第一步：契约先行（单独一次 `contract/` 提交）

`spec/engineering-contract.md`：

- C3.4：
  - `TargetResolveInput` 改为 `{ mainVisible: bool, activeDocKey: Option<String> }`。
  - `ResolvedTarget` 去掉 `workspaceKey`、`workspaceName`、`isCrossWorkspaceFallback`。
  - `NotesFilterArgs` 改为 `{ tab: String, dirRelPath: Option<String>, tags: Vec<String>, filterText: Option<String> }`。
  - `NavTreeSnapshot` 改为 `{ dirs: Vec<DirNode>, tags: Vec<TagNode> }`，删除 `WorkspaceSummary` 定义。
  - `AppendNoteArgs` 去掉 `workspaceKey`。
- C4.2：`ui_state_set` 键白名单去掉 `selectedWorkspace`、`workspaceModes`。
- C6.2：YAML 示例删除 `workspaces:` 段（若存在），并注明该键仅为兼容旧文件而保留读取。
- C6.3：`UiState` 删除 `selectedWorkspaceKey` 与 `workspaceModes`；`lastActiveDoc` 改为 `{ relPath: String, touchedAtMs: number }`。
- C8.1：真值表重写为下面的新表（原文替换表格）：

| 场景 | 条件 | 输出目标与说明 |
| --- | --- | --- |
| S1 | `libraryRoot == None` | `kind = unconfigured`（禁用提交） |
| S2 | `mainVisible && activeDoc != None` | `kind = note/inbox`，`target = activeDoc` |
| S3 | `mainVisible && activeDoc == None` | `lastActiveDoc in 24h` 存在则取之，否则 `inbox.md` |
| S4 | `!mainVisible` | `lastActiveDoc in 24h` 存在则取之，否则 `inbox.md` |

  原 S3 / S4 / S6 / S7 中依赖「选中工作区」与 `W.defaultEntryDoc` 的分支随模型废止而消失。

### 3.3 第二步：Rust（单独一次提交）

按盘点清单执行，要点：

- `domain/settings.rs`：`WorkspaceConfig`、`Settings.workspaces`、`SettingsPatch.workspaces` 及 YAML 转换里的对应字段全部删除。**兼容性**：老 `settings.yaml` 里若残留 `workspaces:` 键，不得导致解析失败——确认 YAML 结构体是否带 `deny_unknown_fields`；若是，则为该结构体保留一个 `#[serde(default)] workspaces: Option<serde_yaml::Value>`（只读入、不使用、不写出），并在字段上加注释说明是兼容占位。
- `domain/capture.rs`：`resolve_target` 按新真值表实现，签名与上下文结构体去掉 `workspaces` 引用。
- `domain/note.rs` / `application/notes.rs`：`NotesFilterArgs` 去 `workspace_key`，`notes_list` 删除 `"workspaces"` 分支，`NavTreeSnapshot` 去 `workspaces`。
- `domain/ui_state.rs`：`UiState` 删除 `selected_workspace_key`、`workspace_modes`，`LastActiveDocState` 删除 `workspace_key`；`UiStatePatchKey` 删除 `SelectedWorkspace`、`WorkspaceModes`。旧文件里的这些键读取时自动忽略。
- `application/capture.rs`：构造 `TargetResolveInput` 的地方同步；追加目标文档的「想法 / 待办 / 正文」模式若原先按 `workspace_modes` 记忆，改为按文档记忆或固定默认值——先读清楚现有行为再决定，并在提交说明写明取舍。
- `application/external.rs`：约 519、561–562 行对 `workspace_key` / `selected_workspace_key` 的回退删除。
- `bindings.rs`：去掉 `WorkspaceSummary`、`WorkspaceConfig` 的引用。
- `pnpm gen:bindings`。

**注意**：本步提交后前端会类型报错，`pnpm check:all` 不能全绿。因此第二、三步必须在**同一个会话内连续完成**后一起跑门禁，但分成两次提交。若无法在一个会话内完成，不要提交第二步。

### 3.4 第三步：前端（单独一次提交）

- `notes/store.ts`：删除 `selectedWorkspace`、`selectWorkspace`、`'workspaces'` 相关；`NavTab` 变为 `'projects' | 'tags' | 'recent'`；`buildFilterArgs` 去掉 `workspaceKey`。
- `quickbar/store.ts` 与 `QuickBarTargetPicker.vue`：删除 `selectedWorkspaceKey` 及按工作区的筛选 chips；其余目标选择逻辑保留。
- `editor/DocPropertiesPopover.vue`：`workspaceSuggestions` 改为从笔记的 `project` 字段去重得到（`notesStore.notes` 与 `navTree` 里出现的全部 `project`），或直接去掉该联想；按现有 UI 语义选择改动最小的一种并注明。
- `terminal/store.ts`：`selectedWorkspaceKey` 入参与 `matchedWs` 分支删除，保留按文档目录 / `.quicknote.yaml` 解析出的会话键。
- `commands/api.ts`：`workspaceKey` 字段若仅用于展示则删除。

### 3.5 第四步：文档

`DECISIONS.md` 在 `D-0012` 末尾补一行；`spec/CODEBASE_MAP.md` 目录树与反查表里去掉相关描述；`spec/PROJECT_STATE.md` 的 M5 清单把本卡标为 `[x]`。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- 第 3.1 步的两条 `grep` 只剩目录级 `.quicknote.yaml`、终端会话键与兼容占位字段。
- `capture_truth_table_test.rs` 按新真值表重写并通过：四个场景各至少一条用例。
- `persistence_test.rs` 新增用例：手写一份含 `workspaces:`、`selectedWorkspaceKey`、`workspaceModes`、`filters.activeTab: "inbox"`、`panelGroup` 的旧版配置目录，`PersistenceManager::open_with_dirs` 能打开，`settings_get` 的 `status.valid == true`，`ui_state_get` 成功。

## 5. 人工点验清单

- [ ] 用 M5 之前的旧 `settings.yaml` 与 `ui-state.json` 启动应用：配置正常加载、不进入只读容错横幅。
- [ ] 主窗口有激活文档时 `⌃⌥Space` 呼出并 `⌘↵`：追加到激活文档（S2）。
- [ ] 主窗口无激活文档：24 小时内最近用过的文档优先，否则 `inbox.md`（S3）。
- [ ] 主窗口隐藏时：同上规则（S4）。
- [ ] 未配置笔记库时：禁止提交并提示去初始化（S1）。
- [ ] Quick Bar 目标选择器、左侧目录、终端打开当前文档目录均正常。

## 上下文提示

- 本卡是整包的最后一张。前面所有卡片都已合入，不再有后续任务依赖被删的字段。
- 忽略：`domain/workspace_context.rs`、`.quicknote.yaml` 相关的一切。
