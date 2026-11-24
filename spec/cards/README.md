# 分阶段任务卡调度总览（M0–M3）

> 本目录存放按里程碑动态滚动拆分的施工任务卡（严格遵循 `C12` 标准模板）。每张卡片对应一个独立新 AI 会话，严格串行推进并以 `pnpm check:all` 全绿收工。

---

## 1. M3 阶段任务卡（进行中 · 快速打开、外部协作、完整命令库与面板组）

> 阶段目标：100% 补齐 `C4.2` 全部 7 条 M3 IPC 命令（`commands_search`、`notes_get_related`、`tags_batch_rename`、`search_spotlight`、`path_complete`、`external_actions_resolve`、`external_action_run`）；落地命令书全形态解析与 `note_links` 索引、可取消 `mdfind` Spotlight（黑名单/50 条硬上限）、统一外部动作（库内无配置严禁回退）并收口 `SettingsErrorBanner` “打开 `settings.yaml` 修复”债务（`D-0008`）；交付 Quick Bar 打开/命令双模式与主窗口大尺寸 `⌘K`/`⌘⇧K`、外部文件标签页与常用文件固定、完整右侧面板组（大纲/终端占位/相关文档前 20/最近编辑前 20 + 宽度持久化 + `<1200`/`<1000`/`<900` 响应式抽屉）、非 Markdown 15 语言高亮与轻量诊断（>2MB 关诊断/>5MB 退纯文本）及全库标签重命名/批量合并。

M3 阶段共拆分为 7 张轻量、自包含、严格串行推进的任务卡（`M3-01a → M3-01b → M3-02 → M3-03 → M3-04 → M3-05a → M3-05b`）：

| 序号 | 卡号与文件 | 核心主题 | 覆盖契约、IPC 命令与事件 | 状态 / 人工点验 |
| --- | --- | --- | --- | --- |
| 1 | [M3-01a-backend-command-book-links-related-and-tag-rename.md](M3-01a-backend-command-book-links-related-and-tag-rename.md) | 后端 M3（上）：命令书解析、`note_links` 索引、`C8.6` 关联打分、标签批量重命名与非 Markdown 安全读写 | `C1.3`、`C1.4`、`C3.4`、`C4.2`、`C5.2`、`C6.4`、`C8.6`、`C9.2`、`v4` §4.8；`commands_search`、`notes_get_related`、`tags_batch_rename` | **已完成（`5e450c7`）** |
| 2 | [M3-01b-backend-spotlight-path-external-actions.md](M3-01b-backend-spotlight-path-external-actions.md) | 后端 M3（下）：可取消 `mdfind` Spotlight、路径补全、统一外部动作（含 `$SETTINGS_YAML` / `pinned-files.json`）与菜单桥接 | `C1.3`、`C1.4`、`C2.1`、`C3.4`、`C4.2`、`D-0008`、`v4` §3.7.1 / §5.3 / §5.10；`search_spotlight`、`path_complete`、`external_actions_resolve`、`external_action_run` | **已完成（`b82f490`）** |
| 3 | [M3-02-settings-workspaces-external-apps-and-banner-fix.md](M3-02-settings-workspaces-external-apps-and-banner-fix.md) | 设置窗口工作区 CRUD、外部程序/`search_roots` 配置、`SettingsErrorBanner` 债务收口与主窗口外部动作集成 | `C1.2`、`C2.2`、`C4.2`、`C6.2`、`C7.1`–`C7.3`、`D-0008`、`v4` §3.7.1 / §4.7 / §5.11；收口 M0→M3 `SettingsErrorBanner` 债务 | **已完成（`455c2ee`）** |
| 4 | [M3-03-quick-open-mode-spotlight-and-external-tabs.md](M3-03-quick-open-mode-spotlight-and-external-tabs.md) | Quick Bar 打开模式（`/` `#` `@` `>` + `Tab` 补全 + Spotlight）、主窗口大尺寸 `⌘K` 与外部文件标签页/常用文件固定 | `C1.2`、`C2.2`、`C4.2`、`C7.1`–`C7.3`、`D-0001`、`v4` §3.5.2 / §3.5.4 / §3.6 / §5.4；消费 `search_spotlight`、`path_complete`、`external.pin` | **已完成（`6da5535`）** |
| 5 | [M3-04-command-book-quickbar-and-main-modal.md](M3-04-command-book-quickbar-and-main-modal.md) | Quick Bar 命令模式（`↵` 复制、`⌘↵` 开终端不粘贴、`⌘O` 跳源文件、`<!-- danger -->` 警示）与主窗口大尺寸命令库 | `C1.2`、`C2.2`、`C4.2`、`C7.1`–`C7.3`、`v4` §3.5.3 / §3.5.4 / §3.6 / §4.8；消费 `commands_search`、`external_actions_resolve`、`external_action_run` | **待执行（当前卡）** |
| 6 | [M3-05a-right-panel-group-and-responsive-breakpoints.md](M3-05a-right-panel-group-and-responsive-breakpoints.md) | 完整右侧面板组（大纲 / 终端占位 / 相关文档前 20 / 最近编辑前 20）、宽度持久化与 `<1200`/`<1000`/`<900` 响应式抽屉 | `C1.2`、`C2.2`、`C4.2`、`C6.3`、`C7.1`–`C7.4`、`C8.6`、`v4` §3.2 / §5.6.6；消费 `notes_get_related`、`ui_state_set(panelGroup)` | 待执行（需实机点验） |
| 7 | [M3-05b-non-markdown-highlight-diagnostics-and-tag-merge.md](M3-05b-non-markdown-highlight-diagnostics-and-tag-merge.md) | 非 Markdown 15 语言语法高亮与自动缩进、轻量诊断（>2MB 关诊断 / >5MB 退纯文本）与侧栏标签重命名/批量合并 | `C1.2`、`C2.2`、`C4.2`、`C7.1`、`v4` §5.5.1 / §5.6.1；零新增 npm 依赖；消费 `tags_batch_rename` | 待执行（需实机点验） |

---

## 2. M2 阶段任务卡（已收官 · 基线标签 `m2-baseline`）

> 阶段目标：补全 M2 后端 3 条 IPC 命令（`settings_update`、`note_save_image`、`inbox_extract_to_note`）并收口 `fs.rs` 默认模板与 `v4` 附录 B.3 对齐债务；落地前端统一动作注册表（`C7.2`/`C7.3` 作用域优先级、`when` 放行、终端 `⌥⌘C` 显式吞键）、右键菜单与原生系统菜单同源桥接；升级设置窗口双向绑定与快捷键录制/冲突检测；实现文档头内联编辑 + `[属性]` Popover（含外部无 frontmatter 二次确认）、模板系统（`_templates/` + `⌘N` + Quick Bar `⌥⌘↵`）与收件箱“移到新笔记…”；以及 CM6 全套 Markdown 增强（上下文 Tab 缩进、行首 `Decoration.widget` 待办复选框、全套格式动作、语法树链接与 `C8.4` 中文 Slug 锚点跳转、图片粘贴/拖入落盘 `assets/YYYY-MM/`）。全部 6 卡及 `fix/M2-audit`（`be79e07`）、`fix/M2-qa`（`4d33f0d`）实机点验均已收官。

M2 阶段共拆分为 6 张轻量、自包含、严格串行推进的任务卡（`M2-01 → M2-02 → M2-03 → M2-04 → M2-05a → M2-05b`）：

| 序号 | 卡号与文件 | 核心主题 | 覆盖契约、IPC 命令与事件 | 状态 / 人工点验 |
| --- | --- | --- | --- | --- |
| 1 | [M2-01-backend-settings-templates-image-and-inbox-extract.md](M2-01-backend-settings-templates-image-and-inbox-extract.md) | 后端 M2 契约补全：`settings_update`、B.3 模板对齐与渲染、`note_save_image`、`inbox_extract_to_note` 与菜单桥接 | `C1.3`、`C1.4`、`C3.1`–`C3.4`、`C4.2`、`C6.2`、`C6.4`、`C8.5`、`D-0006`、`D-0008`；`settings_update`、`note_save_image`、`inbox_extract_to_note`；收口 M0→M2 默认模板债务；`menu:action` 菜单事件转发 | **已完成（`87f1a44`）** |
| 2 | [M2-02-action-registry-context-menu-and-shortcuts.md](M2-02-action-registry-context-menu-and-shortcuts.md) | 前端统一动作注册表、作用域分发（`when` 放行 / 终端显式吞键）、`QnContextMenu` 与系统菜单桥接 | `C1.2`、`C2.2`、`C7.1`、`C7.2`、`C7.3`、`v4` §3.8 / 附录 A；`src/lib/actions/**` 与 `QnContextMenu` | **已完成（`74628d1`）** |
| 3 | [M2-03-settings-window-full-binding-and-shortcut-recorder.md](M2-03-settings-window-full-binding-and-shortcut-recorder.md) | 设置窗口完整双向绑定（`settings_update`）、快捷键录制/冲突检测与外观/编辑器/库/收件箱配置 | `C4.2`、`C6.2`、`C7.1`–`C7.3`、`D-0004`、`D-0008`、`v4` §3.8 / §5.11；消费 `settings_update`、`library_set_root`、`notes_list(templates)` | **已完成（`31d445a`）** |
| 4 | [M2-04-doc-header-properties-templates-and-inbox-extract.md](M2-04-doc-header-properties-templates-and-inbox-extract.md) | 文档头元数据内联编辑、`[属性]` Popover（含外部无 frontmatter 确认）、模板新建（`⌘N` / `⌥⌘↵`）与收件箱提取 | `C3.4`、`C4.2`、`C6.4`、`C7.1`、`D-0006`、`v4` §3.4 / §3.5.1 / §4.9–§4.11；消费 `note_update_meta`、`note_create`、`inbox_extract_to_note` | **已完成（`a92c0fd`）** |
| 5 | [M2-05a-cm6-indent-todos-and-format-actions.md](M2-05a-cm6-indent-todos-and-format-actions.md) | CM6 Markdown 增强（上）：上下文 Tab/⇧Tab 缩进、行首 `Decoration.widget` 待办复选框与全套格式动作/右键菜单 | `C1.2`、`C7.1`–`C7.3`、`v4` §5.6.1 / §5.6.3 / §5.6.5；`⌘B`/`⌘I`/`⌥⌘C`/`⌥⌘G`/`⌥⌘X`/`⌥⌘K`/`⌥⌘I`/`⌥⌘L`/`⌥⌘Q`/`⌥⌘1..6` 与 `Tab`/`⇧Tab` | **已完成（`55097f2`）** |
| 6 | [M2-05b-cm6-links-anchors-and-image-assets.md](M2-05b-cm6-links-anchors-and-image-assets.md) | CM6 Markdown 增强（下）：语法树链接与 `C8.4` 中文 Slug 锚点跳转、图片粘贴/拖入落盘 `assets/` 与排版设置热响应 | `C1.2`、`C2.3`、`C4.2`、`C7.1`、`C8.4`、`v4` §5.6.3 / §5.6.4；消费 `note_save_image`、本地绝对路径二次确认与 CM6 `Compartment` 热响应 | **已完成（`89fc2c9`）** |

---

## 3. M1 阶段任务卡（已收官 · 基线标签 `m1-baseline`）

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

## 4. M0 阶段任务卡（已收官 · 基线标签 `m0-baseline`）

| 序号 | 卡号与文件 | 核心主题 | 状态 |
| --- | --- | --- | --- |
| 1 | [M0-01-bootstrap-and-gates.md](M0-01-bootstrap-and-gates.md) | 工程脚手架、单向分层、`specta` 流水线与全量门禁 | 已完成 |
| 2 | [M0-02-persistence-and-sqlite.md](M0-02-persistence-and-sqlite.md) | 应用数据落盘、`settings.yaml` 容错、`ui-state.json` 与 SQLite 迁移 | 已完成 |
| 3 | [M0-03-macos-native-bridge.md](M0-03-macos-native-bridge.md) | macOS 原生窗口桥接：`NSPanel`、非激活 Toast、动态激活与热键 | 已完成 |
| 4 | [M0-04-tokens-shells-and-onboarding.md](M0-04-tokens-shells-and-onboarding.md) | `--qn-*` 双主题视觉基线、多窗口 Vue 壳与首次选库引导 | 已完成 |

---

## 5. 单卡施工规程（严格执行 `C10` 与省流规矩）

1. **一卡一干净会话（零大文档整篇读取）**：每张卡在独立会话中执行；开场**只读** [AGENTS.md](../../AGENTS.md) 与当期任务卡，任务卡已内联所需契约签名与边界，**除非遇到未定义冲突，禁止整篇读取 1150 行的 `quicknote-design-v4.md` 与 745 行的 `engineering-contract.md`**。
2. **分支隔离与白名单**：从 `dev` 拉取对应功能分支，严守卡片文件白名单与 `C1.3` 共享串行文件声明。
3. **收工门禁**：必须运行 `pnpm check:all`（`C9.1`）全部通过并按 `C10.2` 中文规范提交（`git commit --no-gpg-sign`）后，合并回 `dev` 并结束当前会话。
