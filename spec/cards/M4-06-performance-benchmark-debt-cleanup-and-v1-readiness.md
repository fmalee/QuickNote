# M4-06：M4 性能压测专项、历史债务收口与 1.0 发布前核验

- 所属里程碑：M4
- 关联方案与契约：`v4` §5.5.1、§5.12、§5.13、§5.14、§6 M4；`C1.3`、`C2.1`、`C2.3`、`C5.3`、`C9.1`、`C9.2`、`C9.3`；收口 `PROJECT_STATE.md` §2 全部 M4 挂起债务（M2 审查 🟢#2、M3 审查 🟢#5）
- 前置依赖：`M4-05` 已合并至 `dev`
- 建议分支：`feat/M4-06-perf-debt-and-v1-readiness`
- 会话策略：独立新会话执行（零大文档读取，所有性能预算指标、债务重构目标与发布前核验清单已 100% 内联于本卡）；完成后跑通 `pnpm check:all`、release 压测与 `pnpm build`，用 `git commit --no-gpg-sign` 提交、合并回 `dev` 并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

### 要做
1. **收口 M2 债务（🟢#2）：`collectTodoDecorations` 超长文档单遍扫描优化**：
   - 位置：`src/features/editor/extensions/markdownTodo.ts`（与关联 `markdownIndent.ts` 中的围栏识别辅助函数）。
   - 问题根因：当前 `collectTodoDecorations(doc, ranges)` 在可视行循环内逐行调用 `isLineInFencedCodeBlock(doc, ln)`，每次从第 1 行重扫至第 `ln` 行，大文档（如 10,000 行）退化为 $O(N^2)$。
   - 重构要求：
     - 将围栏状态跟踪改为**单遍顺序扫描（$O(N)$）**：合并并排序 `scanRanges` 得到最大目标行 `maxEndLine`，从第 1 行顺序推进至 `maxEndLine` 仅维护一次当前围栏状态（`fenceChar: '`' | '~' | null`、`fenceLen: number`），仅对落入 `scanRanges` 且不在围栏内的行执行 `TODO_LINE_RE` 匹配并生成 `TodoDecorationEntry`；
     - 保持围栏匹配语义与既有测试 100% 一致（支持 3 个及以上 `` ` `` 或 `~`、闭合围栏须同类字符且长度 `>=` 开围栏、行首允许最多 3 空格缩进）；
     - 补充 10,000 行大文档基准单元测试（含多段围栏与数百条待办项），验证结果精确且耗时稳定在 `< 100ms` 级别。
2. **收口 M3 债务（🟢#5）：非 Markdown 轻量诊断多行启发式边界微调**：
   - 位置：`src/features/editor/extensions/codeDiagnostics.ts`（不引入任何重型解析器，保持轻量零依赖）。
   - 微调 1 — **Nginx 多行指令（`diagnoseNginx`）**：
     - 现有逻辑对每行末尾非 `{`、`}`、`;`、`\` 立即报“可能缺少分号”，导致合法跨行指令（如 `log_format main '...' \n '...';` 或多行 `proxy_set_header` / `add_header`）误报；
     - 改为跟踪轻量跨行指令状态 `pendingDirective: { line, col, offset, length } | null`：
       - 若某行打开跨行引号或以参数续行方式延续至下一行以 `;` 结束，清除 `pendingDirective` 不误报；
       - 仅当存在未以 `;` 结束的 `pendingDirective` 且后续遇到块边界 `{` / `}` 或文件结束（EOF）或明显新指令关键字行时，再报告缺少分号 warning（同时保留花括号配对与多余 `}` 报错）。
   - 微调 2 — **YAML 多行块标量与跨行引号（`diagnoseYaml`）**：
     - 识别块标量头行（例如 `key: |`、`key: >`、`key: |-`、`key: >-`、`key: |+`、`key: >+`、`- |`），记录块标量基准缩进 `blockScalarBaseIndent`：在后续缩进严格大于 `blockScalarBaseIndent` 的行（或纯空行）属于块标量正文，跳过“缺少冒号”与“未闭合引号”检查；当遇到缩进 `<= blockScalarBaseIndent` 的非空行时自动退出块标量态；
     - 支持常规跨行双引号/单引号值（跟踪 `openQuote` 状态跨行延续，仅当到达文件末尾仍未闭合时报错），消除多行字符串误报，同时继续精准检出 Tab 缩进、顶层缺冒号、真正未闭合引号与括号失配。
3. **万篇文档 FTS5 与大文档性能压测套件（`src-tauri/tests/m4_performance_budget.rs`）**：
   - **严禁将 10,000 个静态文件提交到 git 仓库！**全部压测数据在 `tempdir` 中由 Rust 测试确定性动态生成（约 10,000 篇混合中英文笔记，含 frontmatter、标签、项目、中文段落、技术英文关键词与命令块），测试结束自动清理。
   - 覆盖并校验 `v4` §5.13 全部核心性能预算：
     1. **万篇文档全量索引构建**：验证 10,000 篇全部成功入库 SQLite FTS5，无 `fts_degraded` 降级；
     2. **万篇文档 FTS5 全文检索（`v4` §5.13 预算 `< 50ms`）**：分别测纯中文词、纯英文词、中英混合词查询延迟（预热 1 次后取中位数/多次均值）；
     3. **本地索引快速打开查询（`search_notes_and_files`，`v4` §5.13 预算 `< 30ms`）**；
     4. **普通笔记原子保存（`note_write`，`v4` §5.13 预算 `< 50ms`）**；
     5. **6MB 超大文档边界验收**：动态生成 6MB Markdown 文档，验证 `note_read` / `note_write` 成功且超 5MB 追加时 `note_append` 的撤销快照正确标记 `undo_available = false` 不爆内存。
   - **debug vs release 阈值策略**：
     - 测试打印结构化耗时摘要（`[perf] fts5_10k_cn_ms=... fts5_10k_en_ms=... search_local_ms=... note_write_ms=...`）；
     - 使用 `#[cfg(not(debug_assertions))]` 在 release 模式下强制断言硬预算（FTS5 `< 50ms`、本地搜索 `< 30ms`、原子保存 `< 50ms`），在 debug 模式下使用宽松上限（如 4× 容差，防 CI/无优化编译抖动），并在收工前**必须实跑 release 压测**确认所有硬预算 100% 达标！
4. **1.0 发布前打包、安全与合规核验**：
   - 核查并整理 `src-tauri/tauri.conf.json`、`package.json`、`src-tauri/Cargo.toml`：
     - 确认 CSP 严格配置、生产构建关闭 DevTools（不启用 `devtools` feature）、无遥测上报；
     - 确认 `src-tauri/capabilities/default.json` 仍维持最小权限集 `["core:default"]`；
     - 确认 `bundle` 配置就绪（保留 `active: false` 或按 `tauri.conf.json` 安全打包元信息核查，**不擅自把版本号改为 `1.0.0`**——版本号变更若需升级由用户在最终发布前拍板，本卡只确保构建与 `tauri.conf.json` 合法零告警）。
   - 执行全仓静态隐私与安全自检（复核 `C2.3` 日志脱敏、无调试残留 `console.log`、无测试遗留临时文件），并顺手在 `src/styles/tokens.css` 补全 `--qn-radius-full: 9999px;`（`M4-02` 终端运行指示点 `●` 使用了 `var(--qn-radius-full)`，补全后确保圆点为正圆），确保 `pnpm build` 与 `pnpm check:all` 全绿。

### 明确不做
- 不擅自改动产品功能交互或版本号（`0.1.0 → 1.0.0` 留待 M4 独立审查通过与实机全量点验后再由用户确认打 `v1.0.0`）。
- 不把动态生成的万篇压测样本提交进仓库。

## 2. 允许修改文件白名单

### 共享串行文件（`C1.3` 显式声明）
- `src/styles/tokens.css`（仅补全 `--qn-radius-full: 9999px;`）

### 业务与测试文件
- `src/features/editor/extensions/markdownTodo.ts`
- `src/features/editor/extensions/codeDiagnostics.ts`
- `src/features/editor/__tests__/cm6-markdown-editing.test.ts`
- `src/features/editor/__tests__/code-languages-and-diagnostics.test.ts`
- `src-tauri/tests/m4_performance_budget.rs`（新增）
- `src-tauri/tauri.conf.json`（仅当核验发现打包/安全元信息确需修复时才改动，否则保持不动）

## 3. 具体实现要求

1. **性能测试真实性**：
   - `m4_performance_budget.rs` 必须直接走真实 `PersistenceManager` + `rebuild_index` + `search_notes_and_files` + `write_note` 链路，严禁绕过 SQLite FTS5 或 mock 磁盘 IO。
   - 若万篇文档实测 FTS5 耗时接近或超过 50ms，需定位 SQL 索引/查询瓶颈（如 `LIMIT`、查询计划或列读取），若确需改 `application/index.rs` 或 `application/notes.rs`，先汇报记录再最小化精准优化（不改 DDL 破坏历史迁移）。
2. **向后兼容**：
   - `collectTodoDecorations` 与 `diagnoseNginx` / `diagnoseYaml` 重构后，所有 M2 / M3 既有编辑器与诊断测试必须 100% 保持通过。

## 4. 自动化验收标准（必须全绿）

- 前端单测：
  - `cm6-markdown-editing.test.ts` 新增 10,000 行大文档待办单遍扫描测试（验证正确性与耗时）；
  - `code-languages-and-diagnostics.test.ts` 新增 Nginx 多行指令与 YAML `|`/`>` 多行块标量及跨行引号回归测试。
- Rust 压测：
  - `cargo test --manifest-path src-tauri/Cargo.toml --release --test m4_performance_budget -- --nocapture` 全绿，且控制台实测报告满足 `v4` §5.13 硬预算（万篇 FTS5 `< 50ms`、本地搜索 `< 30ms`、原子保存 `< 50ms`）。
- 生产前端构建与全套门禁：
  - `pnpm build` 零错误；
  - `pnpm check:all` 全绿。

## 5. 1.0 收官全量人工点验清单（`C9.3` 全 5 项 + M4 专项，供 M4 审查收官后实机执行）

- [ ] `C9.3 #1` 全屏 App（如 Safari / 终端全屏）下 `⌃⌥Space` 呼出 Quick Bar 覆盖当前空间、不切 Space、点击外侧不关、切 Space 自动复位且留存草稿；
- [ ] `C9.3 #2` `⌘↵` 提交后 5 秒非激活 Toast 正常出现于当前空间且点击撤销不抢原 App 焦点；
- [ ] `C9.3 #3` 打开主窗口/关闭主窗口时 `.accessory ↔ .regular` 切换正常、系统顶栏菜单完备、全屏工作台 Mission Control 拖拽排序持久化；
- [ ] `C9.3 #4` 45% 侧栏辅助窗口覆盖全屏右侧不挤压/改变其他 App 尺寸；
- [ ] `C9.3 #5` 中文拼音输入法在 CM6 待办复选框行与内嵌 xterm.js 终端内的组合态选词与上屏、快捷键拦截全部正常；
- [ ] M4 专项：内嵌终端折叠保活 + 前台子进程指示点 `●` + 30 分钟空闲回收 + 退出零僵尸；`MergeView` 并排合并；富文本粘贴与 PDF 导出排版；设置导入导出备份恢复；双 WebView 空闲内存 `< 300MB`（活动监视器复核）。
