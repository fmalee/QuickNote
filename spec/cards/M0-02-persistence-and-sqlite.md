# M0-02：应用数据落盘、`settings.yaml` 容错、`ui-state.json` 与 SQLite 迁移基座

- 所属里程碑：M0
- 关联方案与契约：[quicknote-design-v4.md](../quicknote-design-v4.md) §4.1、§4.7、§5.3、§5.9、§6 M0；`C3.4`、`C4.2`、`C4.3`、`C5.1`、`C5.2`、`C6.1`–`C6.3`、`C9.2`、`D-0008`、`D-0010`
- 前置依赖：`M0-01`
- 会话策略：独立新会话执行（从 `dev` 拉出 `feat/M0-02-persistence-and-sqlite`）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

- **要做**：
  - 实现 `$APP_DATA_DIR` 初始化管理器，自动创建 `recovery/`、`backups/` 目录，并实现 `backups/` 滚动清理（保留最近 30 天内且总数不超过 50 份，按修改时间淘汰最旧）。
  - 实现单一配置文件 `settings.yaml`（`C6.2` 完整 Schema + `#[serde(deny_unknown_fields)]`）的容错解析与热重载：首次缺失写默认配置；手改出现 YAML 语法错误、未知字段或类型错误时**绝不覆写磁盘原文件**，回退上次有效/默认配置，提取精确 `errorLine` / `errorColumn` / `errorMessage`，并向前端推送 `settings:status` 事件；实现 `settings_get`。
  - 实现 `ui-state.json` 完整 Schema（`C6.3`）原子读写，落地 `ui_state_get` 与键白名单校验的 `ui_state_set`（非白名单键返回 `ErrorCode::ValidationError`）。
  - 引入 `rusqlite`（bundled FTS5）与迁移框架，编写 `src-tauri/migrations/0001_init.sql`（严格对齐 `C5.2` 全部 6 张表及索引，开启 `WAL` 与 `foreign_keys = ON`，支持删库无损重建）。
  - 实现 `library_get_info` 与 `library_set_root`（支持未选库状态 `root = None`；`createIfMissing = true` 时初始化库根与默认 `inbox.md`、`_templates/{note,doc,command-book}.md`、`assets/` 骨架；同步更新 `app_get_info` 中的 `libraryRoot` 字段）。
  - 创建共享静态测试样本 `src-tauri/tests/fixtures/vault/settings-invalid.yaml` 及配套 Rust 集成测试。
- **明确不做**：
  - 不实现库内文档全量扫描、FTS 索引入库与 `notes_list`（归 `M1`）。
  - 不实现设置页 UI 写回 `settings_update`（归 `M2`）。
  - 不实现 macOS 原生 `NSPanel` 桥接与动态激活策略（归 `M0-03`）。

## 2. 允许修改文件白名单

- 共享串行文件（卡内显式声明）：`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/migrations/0001_init.sql`、`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（自动生成）、`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`
- Rust 模块与测试：`src-tauri/src/domain/settings*`、`src-tauri/src/domain/ui_state*`、`src-tauri/src/domain/backup*`、`src-tauri/src/application/persistence*`、`src-tauri/src/application/library*`、`src-tauri/src/infra/fs*`、`src-tauri/src/infra/sqlite*`、`src-tauri/src/infra/settings_watch*`、`src-tauri/src/commands/settings*`、`src-tauri/src/commands/library*`、`src-tauri/src/commands/app*`、`src-tauri/src/lib.rs`、`src-tauri/tests/**`

## 3. 具体实现要求

1. **严格单一格式（`D-0008`）**：配置文件仅读写 `settings.yaml`，不得创建或回退读取 JSON 配置文件。监听磁盘 `settings.yaml` 变更时须做防抖与只读保护：解析失败立即保持原文件字节零变动、切回内存中最后有效配置并 emit `settings:status`。
2. **`ui_state_set` 键白名单（`C4.2`）**：仅允许更新 `windows`、`tabs`、`panelGroup`、`filters`、`selectedWorkspace`、`hotkey`、`lastActiveDoc`、`workspaceModes` 8 个合法键；非法键或非法值类型必须返回 `ErrorCode::ValidationError`，不可静默丢弃。
3. **原子落盘策略**：`ui-state.json`、`settings.yaml`（首次生成时）及 `backups/` 写入必须先写同目录临时文件再原子重命名，且对权限失败等错误通过 `?` 映射为 `AppError`（严禁 `.unwrap()`）。
4. **`0001_init.sql` 精确对齐 `C5.2`**：逐字段核对 `library_meta`、`notes`、`note_tags`、`note_links`、`commands`、`notes_fts` 命名与索引，确认 FTS5 使用 `unicode61` tokenizer。
5. **首次未选库状态**：当 `settings.yaml` 中 `library_root` 为空或未配置且 `library_meta` 无记录时，`library_get_info` 返回 `root: None`，不得擅自强制在用户 `~/Documents` 创建未授权目录。

## 4. 自动化验收标准（必须全绿）

- **新增测试与 Fixture**：
  - `src-tauri/tests/fixtures/vault/settings-invalid.yaml`：包含未知字段与 YAML 缩进错误样本。
  - `src-tauri/tests/persistence_test.rs`：
    1. 校验读取 `settings-invalid.yaml` 后磁盘原文件字节 SHA-256 零变动、返回 `SettingsLoadStatus.valid == false` 且含有效行列号、运行时仍能使用上次有效配置或默认配置。
    2. 校验 `ui_state_set` 合法键持久化读回一致、非法键返回 `ErrorCode::ValidationError`。
    3. 校验 `backups/` 滚动清理纯函数与真实文件测试：写入 55 份跨 40 天时间戳文件后准确保留最新 50 份且淘汰超过 30 天与超限旧文件。
  - `src-tauri/tests/sqlite_migration_test.rs`：校验 `0001_init.sql` 迁移创建全部 6 张表及 FTS5 虚表、支持删库后重新执行迁移完全幂等、`library_set_root(createIfMissing=true)` 生成预期目录树且同步反映到 `app_get_info`。
- **必过命令**：`pnpm check:all`（含 `pnpm gen:bindings` 无 diff 与全部 Rust/TS 单测全绿）。

## 5. 人工点验清单（如涉及 macOS 原生行为）

- 本卡聚焦持久化与 SQLite 基座，100% 通过 Rust 集成测试与 `pnpm check:all` 机测验证，无独立原生点验项。
