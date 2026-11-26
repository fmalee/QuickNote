# AGENTS.md · QuickNote

每个新 AI 会话（含独立审查会话）动手前必读本文件。本文件只写硬规则与指针，契约正文不重复复制。

## 动手前与上下文省流铁律（必遵）

1. **只读自包含任务卡**：完整阅读 `spec/cards/` 中的当前任务卡（模板 `C12`），锁定目标范围与文件白名单。**每张任务卡已内联当期所需的全部函数签名、结构体定义、SQL 列名与边界规则**。
2. **严禁整篇读取大文档与大样本**：
   - **施工会话默认禁止读取** [spec/quicknote-design-v5.md](spec/quicknote-design-v5.md)、[spec/engineering-contract.md](spec/engineering-contract.md)、[DECISIONS.md](DECISIONS.md) 以及 `src-tauri/tests/fixtures/vault/**` 下的大样本文件（仅主控拆卡、变更契约或里程碑只读审查会话可按需读取）。
   - 若施工中极个别细节确需核对原契约，必须先用 `grep` / `ripgrep` 定位精确行号，再用 `read_file` 读取 **≤ 60 行**的局部片段，绝不整篇加载。
3. **只读白名单相关的最小代码集**：仅读取当前任务卡「允许修改文件白名单」中将被修改的源码或其直接调用的接口文件（优先只读顶部 struct / `pub fn` 签名，不读无关模块与历史测试文件）。日常点状维护/小改需求可先读 [spec/CODEBASE_MAP.md](spec/CODEBASE_MAP.md) 3 秒定位目标文件，避免全仓盲搜。
4. 遇契约未定义的技术选型按 `C11` 先记 `DECISIONS.md` 再写码；涉及产品行为取舍先询问用户。

## 硬规则

- **分支与提交**：仅在 `dev` 派生的功能分支开发（`C10.1`），提交信息用中文写清「为什么」并附 `Contract:` 与 `Card:`（`C10.2`），禁止 `--no-verify`；**沙盒环境无 `.gnupg` 私钥访问权限，执行 `git commit` 或 `git merge` 时必须显式添加 `--no-gpg-sign` 参数（例如：`git commit --no-gpg-sign -m "feat: ..."`），严禁不带该参数直接提交**。
- **单向分层**：Rust 严格遵循 `commands → application → domain`、`application / infra → domain`（`domain` 严禁依赖 `tauri`、`std::fs`、`rusqlite`）；前端严格遵循 `lib → components → features → app`，禁止跨 feature 深层导入，Vue 组件严禁直连 `@tauri-apps/api`（`C1.2`）。
- **IPC 契约单一源**：所有 IPC 结构体仅在 Rust 定义并由 `export_bindings.rs` 导出至 `src/bindings/types.ts`（首行 `/* GENERATED — DO NOT EDIT. */`），禁止手改生成物；手写封装仅放 `src/lib/ipc/`（`C1.4`、`C3.1`、`C3.2`）。
- **唯一真相源与缓存库隔离**：磁盘文档（`.md` 与目录级 `.quicknote.yaml`）是唯一真相源，启动全量/增量只读对账与运行期监听单向收敛至 SQLite；无 Frontmatter `id` 的 `.md` 以 `path:<rel_path>` 确定性兜底键收录，扫描过程只读不改写用户文件。SQLite（含 WAL/SHM）强制存于应用缓存目录（按 Vault 路径哈希隔离），严禁放入笔记库根目录，仅作派生索引且支持随时删库全量重建，迁移只追加不改历史（`C5.1`、`C6.1`）。用户配置与外部书签持久化于配置目录配置文件，不以 SQLite 为唯一存储。
- **工作区模型与 `.quicknote.yaml` 隔离**：笔记库子目录通过 `.quicknote.yaml` 绑定工作区配置，外部动作与终端 `cwd` 严格按 `文档 Frontmatter > 最近祖先目录 .quicknote.yaml > 全局默认` 三级级联解析；`.quicknote.yaml` 必须排除出笔记列表与 FTS，应用内写入必须走原子写 + 自写守卫（`C6.1`、`C8.5`）。
- **外部文件引擎**：外部文件检索统一使用纯 Rust 受控文件系统探测与目录直扫（支持 `.xxx` 隐藏目录与 `/usr/local/etc` 等系统路径），禁止以 Spotlight（`mdfind`）作为主检索引擎（`C2.1`）。
- **样式与质量**：UI 颜色、间距、圆角、字体与层级仅使用 `--qn-*` CSS Token，禁止硬编码色值与魔法 `z-index`（`C7.1`）；Rust 生产代码零 `unwrap()` / `expect()`，测试统一落位 `src-tauri/tests/**`（`C2.1`）。
- **串行与范围**：严守当前任务卡的文件白名单（`C12`），不顺手重构或扩范围；修改共享串行文件（`C1.3`）前须确认独占。
- **隐私脱敏**：日志严禁打印笔记正文、剪贴板内容、终端输出或本机敏感密钥（`C2.3`）。
- **收工门禁**：提交前必须跑通 `pnpm check:all` 全套检查（`C9.1`），不通过禁止收工。

## 已定形态（勿重新论证）

- Quick Bar 与全局 Toast 采用原生非激活 `NSPanel`，`collectionBehavior = fullScreenAuxiliary`（256，不含 `canJoinAllSpaces`），不跨 Space、切 Space 自动复位、默认失焦即关闭，「固定」态例外（`D-0001`、`D-0002`、`D-0013`）。
- 常驻内存与渲染模型：主窗口与 Quick Bar 常驻，Settings / Toast 等低频副窗口按需创建并闲置销毁；主编辑器复用单一 CM6 `EditorView`，配合可配置上限的 `EditorState` LRU 池（绝不淘汰激活态或 `isDirty` 标签）（`C7.5`）。
- 45% 侧栏辅助模式：复用主工作台 WebView，macOS 原生层配置全屏 Space 悬浮叠加行为，托盘与 Quick Bar 均提供切换入口（`C7.5`）。
- macOS 激活策略动态切换：主窗口与设置窗隐藏时为 `.accessory`，显示期间为 `.regular`（`D-0003`）。
- 默认全局热键为 `⌃⌥Space`（备用 `⌃⌥⌘Space`），必须包含首启自检与设置页热重注册通道（`D-0004`）。
- 内嵌终端使用 `portable-pty` + `ipc::Channel` + `xterm.js`，折叠保活且在隐藏无前台子进程闲置 30 分钟后自动回收（`D-0005`）。
- 文档采用 `meta + body` 分离（正文写回仅缺失 ULID 时补写 `id`，追加绝不改 frontmatter）；配置单一 `settings.yaml` 带只读容错回退（`D-0006`、`D-0007`、`D-0008`）。

## 文档与目录指针

| 内容 | 位置 |
| --- | --- |
| 产品使用指南与本地构建速查 | `README.md` |
| AI 省流代码地图（目录树 / 数据流 / 功能反查 / 改法菜谱 / 实机暗坑） | `spec/CODEBASE_MAP.md` |
| 机器级工程契约（`C1`–`C12`） | `spec/engineering-contract.md` |
| 架构决策账本（`D-xxxx`；变更 `C11`） | `DECISIONS.md` |
| 产品与实现定稿方案 | `spec/quicknote-design-v5.md` |
| 分阶段施工任务卡 | `spec/cards/` |
| 共享静态测试 Fixture 库 | `src-tauri/tests/fixtures/vault/` |
