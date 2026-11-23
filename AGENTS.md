# AGENTS.md · QuickNote

每个新 AI 会话（含独立审查会话）动手前必读本文件。本文件只写硬规则与指针，契约正文不重复复制。

## 动手前

1. 读机器契约 [spec/engineering-contract.md](spec/engineering-contract.md)，条目按 `Cx.y` 引用。
2. 读架构决策 [DECISIONS.md](DECISIONS.md)（`D-xxxx`）；按 `C11` 处理契约未覆盖项与冲突。
3. 查产品规格 [spec/quicknote-design-v4.md](spec/quicknote-design-v4.md) 确认交互与边界。
4. 按 `spec/cards/` 中单张任务卡（模板 `C12`）施工；未定义的技术选择先记 `DECISIONS.md` 再写码，涉及产品行为取舍先询问用户。

## 硬规则

- **分支与提交**：仅在 `dev` 派生的功能分支开发（`C10.1`），提交信息用中文写清「为什么」并附 `Contract:` 与 `Card:`（`C10.2`），禁止 `--no-verify`。
- **单向分层**：Rust 严格遵循 `commands → application → domain`、`application / infra → domain`（`domain` 严禁依赖 `tauri`、`std::fs`、`rusqlite`）；前端严格遵循 `lib → components → features → app`，禁止跨 feature 深层导入，Vue 组件严禁直连 `@tauri-apps/api`（`C1.2`）。
- **IPC 契约单一源**：所有 IPC 结构体仅在 Rust 定义并由 `export_bindings.rs` 导出至 `src/bindings/types.ts`（首行 `/* GENERATED — DO NOT EDIT. */`），禁止手改生成物；手写封装仅放 `src/lib/ipc/`（`C1.4`、`C3.1`、`C3.2`）。
- **样式与质量**：UI 颜色、间距、圆角、字体与层级仅使用 `--qn-*` CSS Token，禁止硬编码色值与魔法 `z-index`（`C7.1`）；Rust 生产代码零 `unwrap()` / `expect()`，测试统一落位 `src-tauri/tests/**`（`C2.1`）。
- **串行与范围**：严守当前任务卡的文件白名单（`C12`），不顺手重构或扩范围；修改共享串行文件（`C1.3`）前须确认独占。
- **数据与隐私**：SQLite 仅派生索引且迁移只追加不改历史（`C5.1`）；日志严禁打印笔记正文、剪贴板内容、终端输出或本机敏感密钥（`C2.3`）。
- **收工门禁**：提交前必须跑通 `pnpm check:all` 全套检查（`C9.1`），不通过禁止收工。

## 已定形态（勿重新论证）

- Quick Bar 与全局 Toast 采用原生非激活 `NSPanel`，`collectionBehavior = fullScreenAuxiliary`（256，不含 `canJoinAllSpaces`），不跨 Space、切 Space 自动复位、点击外部不关闭（`D-0001`、`D-0002`）。
- macOS 激活策略动态切换：主窗口与设置窗隐藏时为 `.accessory`，显示期间为 `.regular`（`D-0003`）。
- 默认全局热键为 `⌃⌥Space`（备用 `⌃⌥⌘Space`），必须包含首启自检与设置页热重注册通道（`D-0004`）。
- 内嵌终端使用 `portable-pty` + `ipc::Channel` + `xterm.js`，折叠保活且在隐藏无前台子进程闲置 30 分钟后自动回收（`D-0005`）。
- 文档采用 `meta + body` 分离（正文写回仅缺失 ULID 时补写 `id`，追加绝不改 frontmatter）；配置单一 `settings.yaml` 带只读容错回退（`D-0006`、`D-0007`、`D-0008`）。

## 文档与目录指针

| 内容 | 位置 |
| --- | --- |
| 机器级工程契约（`C1`–`C12`） | `spec/engineering-contract.md` |
| 架构决策账本（`D-xxxx`；变更 `C11`） | `DECISIONS.md` |
| 产品与实现定稿方案 | `spec/quicknote-design-v4.md` |
| 分阶段施工任务卡 | `spec/cards/` |
| 共享静态测试 Fixture 库 | `src-tauri/tests/fixtures/vault/` |
