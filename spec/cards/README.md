# 分阶段任务卡调度总览（M0–M1）

> 本目录存放按里程碑动态滚动拆分的施工任务卡（严格遵循 `C12` 标准模板）。每张卡片对应一个独立新 AI 会话，严格串行推进并以 `pnpm check:all` 全绿收工。

---

## 1. M1 阶段任务卡（捕捉与笔记库核心闭环 · 当前阶段）

> 阶段目标：完成库扫描、CJK+ASCII 混合分词 FTS5、`meta + body` 分离读写与 YAML 最小补丁、Quick Bar `S1–S8` 目标真值表与分区追加/5 秒撤销（收口 `ToastAppendUndoPayload` 挂起债务）、`notify` 自身写入防回环与三态外部变更处理、两层主界面（导航树/标签树/收件箱 500 行提醒/多标签）+ CodeMirror 6 基础编辑器 + 右侧大纲面板，以及 Quick Bar 记录模式端到端闭环。

M1 阶段共拆分为 7 张轻量、自包含、严格串行推进的任务卡（`M1-01a → M1-01b → M1-02 → M1-03 → M1-04a → M1-04b → M1-05`）：

| 序号 | 卡号与文件 | 核心主题 | 覆盖契约、IPC 命令与事件 | 状态 / 人工点验 |
| --- | --- | --- | --- | --- |
| 1 | [M1-01a-vault-fixtures-and-domain-note-fts.md](M1-01a-vault-fixtures-and-domain-note-fts.md) | 共享静态 Fixture 库、`domain` 纯算法（混合分词 / YAML 最小补丁 / `meta + body`）与绑定导出 | `C3.1`–`C3.4`、`C5.3`、`C6.4`、`C9.2`、`D-0006`、`D-0007`、`D-0009` | **已完成（`9dbf2d3`）** |
| 2 | [M1-01b-sqlite-fts-index-and-note-crud-commands.md](M1-01b-sqlite-fts-index-and-note-crud-commands.md) | SQLite FTS5 派生索引入库、笔记 CRUD 用例与 11 条 IPC 命令封装 | `C4.2`–`C4.3`、`C5.1`–`C5.3`、`C6.4`、`D-0006`、`D-0007`；`index_get_status`、`index_rebuild`、`notes_list`、`notes_get_tree_and_tags`、`note_read`、`note_create`、`note_write`、`note_update_meta`、`note_rename`、`note_delete`、`search_notes_and_files`；事件 `index:status` | **已完成（`2ba211d`）** |
| 3 | [M1-02-capture-truth-table-append-and-undo.md](M1-02-capture-truth-table-append-and-undo.md) | Quick Bar `S1–S8` 目标真值表、分区追加、5 秒撤销与 `ToastAppendUndoPayload` 收口 | `C3.4`、`C4.2`–`C4.3`、`C8.1`–`C8.3`、`D-0001`、`D-0006`、`D-0009`；`capture_resolve_target`、`note_append`、`note_undo_last_append`；事件 `toast:append-undo`（收口 `AppendResult` 绑定） | **已完成（`b8d6018`）** |
| 4 | [M1-03-notify-loop-guard-and-recovery.md](M1-03-notify-loop-guard-and-recovery.md) | `notify` 自身写入防回环、外部变更状态机与持久化辅助（`recovery/` & `quickbar-draft.json`） | `C4.3`、`C6.1`、`C8.4`、`C8.5`、`C9.2`、`D-0006`；事件 `fs:external-change`（`ExternalChangeEvent`）、中文 Slug 算法 | **已完成（`3d297b7`）** |
| 5 | [M1-04a-main-navigation-list-and-tabs.md](M1-04a-main-navigation-list-and-tabs.md) | 主界面第一层（导航树/标签树/列表/重命名/收件箱 500 行膨胀提醒）与多标签状态栏 | `C1.2`、`C2.2`、`C6.2`、`C6.3`、`C7.1`、`D-0006`、`D-0007`；消费 `notes_get_tree_and_tags`、`notes_list`、`note_create`、`note_rename`、`note_delete` 与 `ui-state.json` 恢复 | **已完成（`7185486`）** |
| 6 | [M1-04b-cm6-editor-autosave-conflict-and-outline.md](M1-04b-cm6-editor-autosave-conflict-and-outline.md) | CodeMirror 6 单实例多状态编辑器、600ms/2s 自动保存、三态外部变更横幅与右侧大纲面板 | `C1.2`、`C2.2`、`C7.1`、`C7.4`、`C8.4`、`C8.5`、`D-0006`；消费 `note_read`、`note_write` 与 `fs:external-change` 三态 UI | **已完成（`66bb613`）** |
| 7 | [M1-05-quickbar-capture-mode-closed-loop.md](M1-05-quickbar-capture-mode-closed-loop.md) | Quick Bar 记录模式完整闭环（`S1–S8` 顶部 Chip、`⌘T` 选目标、相关笔记改投、`⌘↵`/`⇧⌘↵` 与草稿恢复） | `C1.2`、`C2.2`、`C6.1`、`C6.3`、`C7.1`、`C7.2`、`C8.1`–`C8.3`、`C9.3`、`D-0001`–`D-0003`；消费 `capture_resolve_target`、`note_append`、`note_undo_last_append`、`search_notes_and_files` | **M1 端到端实机点验** |

---

## 2. M0 阶段任务卡（已收官 · 基线标签 `m0-baseline`）

| 序号 | 卡号与文件 | 核心主题 | 状态 |
| --- | --- | --- | --- |
| 1 | [M0-01-bootstrap-and-gates.md](M0-01-bootstrap-and-gates.md) | 工程脚手架、单向分层、`specta` 流水线与全量门禁 | 已完成 |
| 2 | [M0-02-persistence-and-sqlite.md](M0-02-persistence-and-sqlite.md) | 应用数据落盘、`settings.yaml` 容错、`ui-state.json` 与 SQLite 迁移 | 已完成 |
| 3 | [M0-03-macos-native-bridge.md](M0-03-macos-native-bridge.md) | macOS 原生窗口桥接：`NSPanel`、非激活 Toast、动态激活与热键 | 已完成 |
| 4 | [M0-04-tokens-shells-and-onboarding.md](M0-04-tokens-shells-and-onboarding.md) | `--qn-*` 双主题视觉基线、多窗口 Vue 壳与首次选库引导 | 已完成 |

---

## 3. 单卡施工规程（严格执行 `C10` 与省流规矩）

1. **一卡一干净会话（零大文档整篇读取）**：每张卡在独立会话中执行；开场**只读** [AGENTS.md](../../AGENTS.md) 与当期任务卡，任务卡已内联所需契约签名与边界，**除非遇到未定义冲突，禁止整篇读取 1150 行的 `quicknote-design-v4.md` 与 745 行的 `engineering-contract.md`**。
2. **分支隔离与白名单**：从 `dev` 拉取对应功能分支，严守卡片文件白名单与 `C1.3` 共享串行文件声明。
3. **收工门禁**：必须运行 `pnpm check:all`（`C9.1`）全部通过并按 `C10.2` 中文规范提交后，合并回 `dev` 并结束当前会话。
