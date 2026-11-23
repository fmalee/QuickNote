# 分阶段任务卡调度总览（M0–M2）

> 本目录存放按里程碑动态滚动拆分的施工任务卡（严格遵循 `C12` 标准模板）。每张卡片对应一个独立新 AI 会话，严格串行推进并以 `pnpm check:all` 全绿收工。

---

## 1. M2 阶段任务卡（Markdown 完整编辑、模板与文档属性 · 当前阶段）

> 阶段目标：补全 M2 后端 3 条 IPC 命令（`settings_update`、`note_save_image`、`inbox_extract_to_note`）并收口 `fs.rs` 默认模板与 `v4` 附录 B.3 对齐债务；落地前端统一动作注册表（`C7.2`/`C7.3` 作用域优先级、`when` 放行、终端 `⌥⌘C` 显式吞键）、右键菜单与原生系统菜单同源桥接；升级设置窗口双向绑定与快捷键录制/冲突检测；实现文档头内联编辑 + `[属性]` Popover（含外部无 frontmatter 二次确认）、模板系统（`_templates/` + `⌘N` + Quick Bar `⌥⌘↵`）与收件箱“移到新笔记…”；以及 CM6 全套 Markdown 增强（上下文 Tab 缩进、行首 `Decoration.widget` 待办复选框、全套格式动作、语法树链接与 `C8.4` 中文 Slug 锚点跳转、图片粘贴/拖入落盘 `assets/YYYY-MM/`）。

M2 阶段共拆分为 6 张轻量、自包含、严格串行推进的任务卡（`M2-01 → M2-02 → M2-03 → M2-04 → M2-05a → M2-05b`）：

| 序号 | 卡号与文件 | 核心主题 | 覆盖契约、IPC 命令与事件 | 状态 / 人工点验 |
| --- | --- | --- | --- | --- |
| 1 | [M2-01-backend-settings-templates-image-and-inbox-extract.md](M2-01-backend-settings-templates-image-and-inbox-extract.md) | 后端 M2 契约补全：`settings_update`、B.3 模板对齐与渲染、`note_save_image`、`inbox_extract_to_note` 与菜单桥接 | `C1.3`、`C1.4`、`C3.1`–`C3.4`、`C4.2`、`C6.2`、`C6.4`、`C8.5`、`D-0006`、`D-0008`；`settings_update`、`note_save_image`、`inbox_extract_to_note`；收口 M0→M2 默认模板债务；`menu:action` 菜单事件转发 | **已完成（`87f1a44`）** |
| 2 | [M2-02-action-registry-context-menu-and-shortcuts.md](M2-02-action-registry-context-menu-and-shortcuts.md) | 前端统一动作注册表、作用域分发（`when` 放行 / 终端显式吞键）、`QnContextMenu` 与系统菜单桥接 | `C1.2`、`C2.2`、`C7.1`、`C7.2`、`C7.3`、`v4` §3.8 / 附录 A；`src/lib/actions/**` 与 `QnContextMenu` | **待执行（当前卡）** |
| 3 | [M2-03-settings-window-full-binding-and-shortcut-recorder.md](M2-03-settings-window-full-binding-and-shortcut-recorder.md) | 设置窗口完整双向绑定（`settings_update`）、快捷键录制/冲突检测与外观/编辑器/库/收件箱配置 | `C4.2`、`C6.2`、`C7.1`–`C7.3`、`D-0004`、`D-0008`、`v4` §3.8 / §5.11；消费 `settings_update`、`library_set_root`、`notes_list(templates)` | 待执行 |
| 4 | [M2-04-doc-header-properties-templates-and-inbox-extract.md](M2-04-doc-header-properties-templates-and-inbox-extract.md) | 文档头元数据内联编辑、`[属性]` Popover（含外部无 frontmatter 确认）、模板新建（`⌘N` / `⌥⌘↵`）与收件箱提取 | `C3.4`、`C4.2`、`C6.4`、`C7.1`、`D-0006`、`v4` §3.4 / §3.5.1 / §4.9–§4.11；消费 `note_update_meta`、`note_create`、`inbox_extract_to_note` | 待执行 |
| 5 | [M2-05a-cm6-indent-todos-and-format-actions.md](M2-05a-cm6-indent-todos-and-format-actions.md) | CM6 Markdown 增强（上）：上下文 Tab/⇧Tab 缩进、行首 `Decoration.widget` 待办复选框与全套格式动作/右键菜单 | `C1.2`、`C7.1`–`C7.3`、`v4` §5.6.1 / §5.6.3 / §5.6.5；`⌘B`/`⌘I`/`⌥⌘C`/`⌥⌘G`/`⌥⌘X`/`⌥⌘K`/`⌥⌘I`/`⌥⌘L`/`⌥⌘Q`/`⌥⌘1..6` 与 `Tab`/`⇧Tab` | 待执行 |
| 6 | [M2-05b-cm6-links-anchors-and-image-assets.md](M2-05b-cm6-links-anchors-and-image-assets.md) | CM6 Markdown 增强（下）：语法树链接与 `C8.4` 中文 Slug 锚点跳转、图片粘贴/拖入落盘 `assets/` 与排版设置热响应 | `C1.2`、`C2.3`、`C4.2`、`C7.1`、`C8.4`、`v4` §5.6.3 / §5.6.4；消费 `note_save_image`、本地绝对路径二次确认与 CM6 `Compartment` 热响应 | 待执行 |

---

## 2. M1 阶段任务卡（已收官 · 基线标签 `m1-baseline`）

| 序号 | 卡号与文件 | 核心主题 | 覆盖契约、IPC 命令与事件 | 状态 |
| --- | --- | --- | --- | --- |
| 1 | [M1-01a-vault-fixtures-and-domain-note-fts.md](M1-01a-vault-fixtures-and-domain-note-fts.md) | 共享静态 Fixture 库、`domain` 纯算法（混合分词 / YAML 最小补丁 / `meta + body`）与绑定导出 | `C3.1`–`C3.4`、`C5.3`、`C6.4`、`C9.2`、`D-0006`、`D-0007`、`D-0009` | 已完成（`9dbf2d3`） |
| 2 | [M1-01b-sqlite-fts-index-and-note-crud-commands.md](M1-01b-sqlite-fts-index-and-note-crud-commands.md) | SQLite FTS5 派生索引入库、笔记 CRUD 用例与 11 条 IPC 命令封装 | `C4.2`–`C4.3`、`C5.1`–`C5.3`、`C6.4`、`D-0006`、`D-0007` | 已完成（`2ba211d`） |
| 3 | [M1-02-capture-truth-table-append-and-undo.md](M1-02-capture-truth-table-append-and-undo.md) | Quick Bar `S1–S8` 目标真值表、分区追加、5 秒撤销与 `ToastAppendUndoPayload` 收口 | `C3.4`、`C4.2`–`C4.3`、`C8.1`–`C8.3`、`D-0001`、`D-0006`、`D-0009` | 已完成（`b8d6018`） |
| 4 | [M1-03-notify-loop-guard-and-recovery.md](M1-03-notify-loop-guard-and-recovery.md) | `notify` 自身写入防回环、外部变更状态机与持久化辅助（`recovery/` & `quickbar-draft.json`） | `C4.3`、`C6.1`、`C8.4`、`C8.5`、`C9.2`、`D-0006` | 已完成（`3d297b7`） |
| 5 | [M1-04a-main-navigation-list-and-tabs.md](M1-04a-main-navigation-list-and-tabs.md) | 主界面第一层（导航树/标签树/列表/重命名/收件箱 500 行膨胀提醒）与多标签状态栏 | `C1.2`、`C2.2`、`C6.2`、`C6.3`、`C7.1`、`D-0006`、`D-0007` | 已完成（`7185486`） |
| 6 | [M1-04b-cm6-editor-autosave-conflict-and-outline.md](M1-04b-cm6-editor-autosave-conflict-and-outline.md) | CodeMirror 6 单实例多状态编辑器、600ms/2s 自动保存、三态外部变更横幅与右侧大纲面板 | `C1.2`、`C2.2`、`C7.1`、`C7.4`、`C8.4`、`C8.5`、`D-0006` | 已完成（`66bb613`） |
| 7 | [M1-05-quickbar-capture-mode-closed-loop.md](M1-05-quickbar-capture-mode-closed-loop.md) | Quick Bar 记录模式完整闭环（`S1–S8` 顶部 Chip、`⌘T` 选目标、相关笔记改投、`⌘↵`/`⇧⌘↵` 与草稿恢复） | `C1.2`、`C2.2`、`C6.1`、`C6.3`、`C7.1`、`C7.2`、`C8.1`–`C8.3`、`C9.3`、`D-0001`–`D-0003` | 已完成（`3fea386`） |

---

## 3. M0 阶段任务卡（已收官 · 基线标签 `m0-baseline`）

| 序号 | 卡号与文件 | 核心主题 | 状态 |
| --- | --- | --- | --- |
| 1 | [M0-01-bootstrap-and-gates.md](M0-01-bootstrap-and-gates.md) | 工程脚手架、单向分层、`specta` 流水线与全量门禁 | 已完成 |
| 2 | [M0-02-persistence-and-sqlite.md](M0-02-persistence-and-sqlite.md) | 应用数据落盘、`settings.yaml` 容错、`ui-state.json` 与 SQLite 迁移 | 已完成 |
| 3 | [M0-03-macos-native-bridge.md](M0-03-macos-native-bridge.md) | macOS 原生窗口桥接：`NSPanel`、非激活 Toast、动态激活与热键 | 已完成 |
| 4 | [M0-04-tokens-shells-and-onboarding.md](M0-04-tokens-shells-and-onboarding.md) | `--qn-*` 双主题视觉基线、多窗口 Vue 壳与首次选库引导 | 已完成 |

---

## 4. 单卡施工规程（严格执行 `C10` 与省流规矩）

1. **一卡一干净会话（零大文档整篇读取）**：每张卡在独立会话中执行；开场**只读** [AGENTS.md](../../AGENTS.md) 与当期任务卡，任务卡已内联所需契约签名与边界，**除非遇到未定义冲突，禁止整篇读取 1150 行的 `quicknote-design-v4.md` 与 745 行的 `engineering-contract.md`**。
2. **分支隔离与白名单**：从 `dev` 拉取对应功能分支，严守卡片文件白名单与 `C1.3` 共享串行文件声明。
3. **收工门禁**：必须运行 `pnpm check:all`（`C9.1`）全部通过并按 `C10.2` 中文规范提交（`git commit --no-gpg-sign`）后，合并回 `dev` 并结束当前会话。
