# M5-01a：契约变更包

- 所属里程碑：M5
- 关联方案与契约：`v5` §5、§6；C2.4、C3.4、C4.2、C6.2、C6.3、C7.2、C7.4、C8.5、C9.3、C11.1、C11.3、C12；D-0002
- 前置依赖：无
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：只改文档。把 M5 全部需求涉及的契约口径一次定稿，后续代码卡照此施工。
- 不做：不改任何源码；不改 `AGENTS.md`「已定形态」（由 M5-06a2 在实施时同步）。

## 2. 允许修改文件白名单

- `DECISIONS.md`
- `spec/engineering-contract.md`

## 3. 具体实现要求

### 3.1 `DECISIONS.md`：在文件末尾追加（原文照抄）

```markdown
## D-0013 Quick Bar 默认失焦关闭，「固定」态例外（取代 D-0002 的「点击面板外部不关闭」）

- 日期：2026-11-26
- 状态：proposed（C11.3）
- 关联：`C2.4`、`C3.4`、`C4.2`、`C4.3`、`C6.2`、`C9.3`、`D-0001`、`D-0002`
- 背景：`D-0002` 以「对照其他窗口看内容时极易误收起面板」为由否决了「失焦即关闭」。实际使用中 Quick Bar 是闪念捕捉面板，呼出后用户点击别处即代表本次捕捉结束，常驻不关反而需要再按一次 Esc 或热键，与「呼出—输入—离开」的使用节奏不符。用户已明确要求默认失焦关闭，并以「固定」开关覆盖原决策担心的对照场景。
- 决策：
  1. Quick Bar 面板失去键窗口焦点（`NSWindowDidResignKeyNotification`）时立即隐藏，并上报 `HideReason = "focus_lost"`；草稿照常保留。
  2. 新增「固定 Quick Bar」开关：状态持久化于 `settings.yaml` 的 `quick_bar.pinned`（默认 `false`）；为 `true` 时失焦不隐藏。开关在 Quick Bar 标题栏（「取消」左侧图标）与主窗口「窗口」菜单（`固定 Quick Bar`，勾选项）两处提供，状态保持一致。
  3. `D-0002` 中「点击面板外部不关闭；`esc`、`hotkey`、`space_changed` 是仅有的三种关闭入口」自此被本决策取代；`D-0002` 其余内容（`collectionBehavior = 256`、不跨 Space、切 Space 自动复位）继续有效。
- 后果：`collectionBehavior` 与非激活面板机制不变；`hidesOnDeactivate` 仍为 `NO`，由应用自己决定失焦后是否隐藏。`C9.3` 第 1 条点验口径同步修订。
- 否决替代：维持点外不关（与用户明确诉求冲突）；用 `hidesOnDeactivate = YES` 实现（无法被「固定」开关豁免）。
```

`D-0002` 的状态行本卡**不动**（M5-06a2 实施后再标注 `superseded by D-0013`）。

### 3.2 `spec/engineering-contract.md`：逐条修改

先用 `grep -n '^### C' spec/engineering-contract.md` 定位条款，再只读对应段落。

| 条款 | 改成 |
| --- | --- |
| C2.4 | `event` 取值增加 `"focus_lost"`、`"vault_change_echo_ignored"`；`hideReason` 取值增加 `"focus_lost"`（把 `"esc" \| "hotkey" \| "space_changed" \| null` 改为含 `"focus_lost"`） |
| C3.4 第 2 点 | `NavTreeSnapshot` 定义去掉 `inboxCount`，改为 `{ dirs: Vec<DirNode>, tags: Vec<TagNode>, workspaces: Vec<WorkspaceSummary> }` |
| C3.4 第 5 点 | `HideReason = "esc" \| "hotkey" \| "space_changed" \| "focus_lost"` |
| C4.2 | `note_delete` 行的返回值说明后追加备注「将文件移动到系统废纸篓（M5）」；`ui_state_set` 行下方「说明」中的键白名单改为：`windows`、`tabs`、`leftSidebar`、`terminalPanel`、`filters`、`backupPaths`、`selectedWorkspace`、`hotkey`、`lastActiveDoc`、`workspaceModes` |
| C4.2 窗口与热键分组 | 新增一行：`window_set_quick_bar_pinned \| SetQuickBarPinnedArgs { pinned: bool } \| bool \| M5`，放在 `hotkey_update` 之后 |
| C4.2 搜索与命令库分组末尾 | 新增一行：`search_spotlight \| SpotlightArgs \| Vec<SearchResultItem> \| deprecated（Spotlight 已废止，M5-06e 删除）` |
| C6.2 YAML | `quick_bar:` 下增加 `pinned: false`（注释：固定后失焦不关闭）；`editor:` 下增加 `show_tab_close_button: false`；`panels.terminal:` 下增加 `height: 240`（注释：底部终端面板高度 px） |
| C6.3 | 见下方 3.3，整段替换 `UiState` 接口 |
| C7.2 命名空间 | `quickbar.*` 增加 `quickbar.mode.capture`（`⌘N`）、`quickbar.togglePin`；`editor.*` 增加 `editor.find`（`⌘F`）、`editor.replace`（`⌥⌘F`）；`panel.*` 改为 `panel.toggle.outline`（`⌃⌘O`，左侧栏「大纲」视图）、`panel.toggle.related`（`⌃⌘R`，左侧栏「相关文档」视图）、`panel.toggle.terminal`（`⌃⌘T`，编辑器底部终端）；删除 `panel.toggle.recent` 与 `panel.collapseGroup`；`window.*` 行改为「`window.nextTab`（`⌃⇥`）、`window.prevTab`（`⇧⌃⇥`）、`window.pinQuickBar`、`window.toggleAuxiliary`」 |
| C7.4 | 见下方 3.4，标题与正文整体替换 |
| C8.5 | 见下方 3.5，前三条整体替换 |
| C9.3 第 1 条 | 改为「全屏 App 下 `⌃⌥Space` 呼出 Quick Bar 覆盖当前空间、不切 Space；未固定时点击面板外立即关闭且草稿保留，固定后点击外侧不关；切 Space 自动复位且留存草稿」 |
| C12 | 模板里 `` `v4` §x.y `` 改为 `` `v5` §x.y ``；「所属里程碑：M0 / M1 / M2 / M3 / M4」追加 ` / M5` |

### 3.3 C6.3 新的 `UiState` 接口（替换代码块里的 interface，保留注释风格）

```typescript
interface UiState {
  windows: { main: { x: number; y: number; width: number; height: number; mode: "standard" | "fullscreen" | "auxiliary"; isOpen: boolean } };
  tabs: { /* 保持原文不变 */ };
  leftSidebar: {
    width: number;                                   // px，默认 300
    activeView: "workspace" | "related" | "outline";  // 左侧栏顶层视图
    collapsed: boolean;
  };
  terminalPanel: { visible: boolean; height: number; collapsed: boolean }; // 编辑器底部终端
  filters: {
    activeTab: "projects" | "tags" | "recent";       // 旧值 "inbox" 读取时映射为 "projects"
    selectedDir: string | null;
    selectedTags: string[];
    listScrollTop: number;
  };
  backupPaths: { lastExportPath: string | null; lastImportPath: string | null };
  selectedWorkspaceKey: string | null; // 已废弃，M5-06f 移除
  lastActiveDoc: { relPath: string; workspaceKey: string | null; touchedAtMs: number } | null;
  workspaceModes: Record<string, "list" | "todo" | "prose">;
  hotkey: { combo: string; backupCombo: string; selfCheck: "pending" | "ok" | "conflict" };
}
```

接口上方补一句说明：`leftSidebar`、`terminalPanel`、`backupPaths` 在旧版 `ui-state.json` 缺失时取默认值；旧键 `panelGroup` 读取时忽略。

### 3.4 C7.4 新正文（标题改为「左侧栏视图注册表与底部终端面板」）

```markdown
- 左侧栏顶层视图顺序：`workspace`（笔记导航：目录 / 标签 / 最近）、`related`（相关文档）、`outline`（大纲）。
- 左侧栏宽度：默认 `300px`，最小 `220px`，可拖动并持久化于 `ui-state.json` 的 `leftSidebar.width`；收起为 `36px` 图标条。
- 底部终端面板：默认隐藏，高度默认 `240px`，范围 `120–600px`，可拖动并持久化于 `terminalPanel.height`；折叠时终端会话保活。
- 响应式断点：`< 1000px` 左侧栏收为图标条；`< 900px`（含 45% 侧栏辅助窄窗口）左侧栏以覆盖式抽屉浮于编辑区之上，编辑区最小宽度 `480px`。
```

### 3.5 C8.5 前三条新正文

```markdown
- 应用每次写入前（`note_write` / `note_append` / `note_update_meta` / `note_rename` / `tags_batch_rename` / 撤销追加 / `set_directory_workspace_config` 的写入与删除）先以「预计落盘 body_hash」在内存写入缓存表登记 `(canonical_abs_path, body_hash, expires_at = now + 1500ms)`；写入失败必须立即清除该登记。`.quicknote.yaml` 的应用内写入 / 清除同样走原子写 + 自写守卫。
- `notify` 收到磁盘修改事件后先重读正文计算 `disk_body_hash`：若与写入缓存中的近期哈希相同（或处于抑制窗口且内容未变），仅更新索引 `mtime` 并静默丢弃事件；若哈希确实不同，发送 `fs:external-change(modified)`。
- 前端收到 `modified` 事件时，若 `newBodyHash` 等于该标签的 `baseBodyHash` 且 `newMeta` 与标签当前 `meta` 一致，视为应用自身写入的回声：记诊断事件 `vault_change_echo_ignored` 并忽略，不重置编辑器状态、不弹冲突横幅。其余情况按原规则：干净态静默重载，脏态激活冲突横幅。
```

其后「删除事件」一条保持原文。

### 3.6 文末新增依赖登记

在 C11.1 小节末尾追加一行：
「M5 登记的新依赖：Rust `trash`（删除到废纸篓）、`tauri-plugin-dialog` 与 npm `@tauri-apps/plugin-dialog`（系统文件选择窗口）、npm `@codemirror/search`（查找替换）。」

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（本卡只改文档，`spec/` 与 `DECISIONS.md` 在 `.prettierignore` 中，不会触发格式检查）。
- `grep -n 'v4' spec/engineering-contract.md` 只剩 `D-0012` / v4 FROZEN 相关的历史说明，C12 模板内不再出现 `v4`。
- `grep -c 'D-0013' DECISIONS.md` 为 1，且该条状态为 `proposed`。

## 5. 人工点验清单

- 无原生行为改动。由用户通读两份文档的 diff 确认口径。

## 上下文提示

- 只需关注 `DECISIONS.md` 与契约文档中上表列出的条款。
- 忽略：`spec/quicknote-design-v4.md`（FROZEN，禁读禁改）、任何源码。
- 不要顺手修订表外的其他条款。
