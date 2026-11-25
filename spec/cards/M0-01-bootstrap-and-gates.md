# M0-01：工程脚手架、单向分层、`specta` 流水线与全量门禁

- 所属里程碑：M0
- 关联方案与契约：[quicknote-design-v4.md](../quicknote-design-v4.md) §5.1、§5.2、§6 M0；`C1.1`–`C1.4`、`C2.1`–`C2.3`、`C3.1`–`C3.3`、`C4.1`、`C9.1`、`C10.1`、`C10.2`、`D-0009`、`D-0010`
- 前置依赖：无（若当前工作区尚未初始化 Git 仓库，先初始化提交基线文档至 `master`，再创建 `dev` 与分支 `feat/M0-01-bootstrap-and-gates`）
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）

- **要做**：
  - 按 `C1.1` 搭建 Tauri 2 + Vue 3 + TypeScript + Vite + Pinia + ESLint + Prettier + Vitest + Rust workspace 完整工程骨架与分层目录（前端 `src/{app,styles,lib/ipc,lib/utils,bindings,components,features}`；后端 `src-tauri/src/{commands,application,domain,infra,bin/export_bindings.rs}` 及 `src-tauri/tests/`）。
  - 在 Rust `domain` / `commands` 中落地统一错误模型 `AppError` + 全部 `ErrorCode` 枚举（`C3.3`），以及首个冒烟 IPC 类型与命令 `app_get_info`（`AppInfo` 基础实现，默认返回版本、平台与占位字段）。
  - 打通 `src-tauri/src/bindings.rs` 与 `src-tauri/src/bin/export_bindings.rs`，实现 `pnpm gen:bindings` 生成唯一文件 `src/bindings/types.ts`（首行精确包含 `/* GENERATED — DO NOT EDIT. */`）。
  - 建立前端唯一手写 IPC 层 `src/lib/ipc/commands.ts` 与 `src/lib/ipc/events.ts`（仅导入 `@/bindings/types` 与 `@tauri-apps/api`）。
  - 编写静态架构防漂移检查脚本 `scripts/check-architecture.mjs` 与总门禁脚本 `scripts/check-all.sh`，接通 `package.json` 的 `pnpm check:all`（严格执行 `C9.1` 全 9 项校验）。
- **明确不做**：
  - 不编写 SQLite DDL、迁移文件或 `rusqlite` 连接逻辑（归 `M0-02`）。
  - 不编写 `settings.yaml`、`ui-state.json`、`library_set_root` 持久化逻辑（归 `M0-02`）。
  - 不编写 macOS 原生 `NSPanel`、动态激活策略与全局热键桥接（归 `M0-03`）。
  - 不编写 `--qn-*` 主题视觉页与业务前端 UI（归 `M0-04`）。

## 2. 允许修改文件白名单

- 根配置与脚本：`package.json`、`pnpm-lock.yaml`、`pnpm-workspace.yaml`、`tsconfig.json`、`vite.config.ts`、`eslint.config.js`、`.prettierrc`、`.prettierignore`、`.gitignore`、`index.html`、`scripts/check-architecture.mjs`、`scripts/check-all.sh`、`scripts/__tests__/**`
- 前端骨架：`src/app/**`、`src/styles/tokens.css`（空/最小占位）、`src/lib/ipc/**`、`src/lib/utils/**`、`src/bindings/types.ts`（生成物）、`src/components/.gitkeep`、`src/features/.gitkeep`
- 后端骨架：`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/build.rs`、`src-tauri/tauri.conf.json`、`src-tauri/capabilities/**`、`src-tauri/icons/icon.png`、`src-tauri/src/**`、`src-tauri/tests/**`

## 3. 具体实现要求

1. **Git 初始化与分支（若尚未初始化）**：初始化仓库并将既有文档（`AGENTS.md`、`DECISIONS.md`、`spec/`）作为基线提交至 `master`，派生 `dev` 分支，再拉出 `feat/M0-01-bootstrap-and-gates` 进行本卡开发。
2. **TypeScript 与 ESLint 严格模式**：`tsconfig.json` 必须启用 `"strict": true`、`"noUncheckedIndexedAccess": true`；ESLint 禁止 `any`（`@typescript-eslint/no-explicit-any: "error"`），配置 Prettier 一致性检查。
3. **Rust 分层与零 Panic**：`src-tauri/src/lib.rs` 导出供 `src-tauri/tests/**` 调用的公共模块；`domain` 模块保持零 `tauri` / `std::fs` / `rusqlite` 依赖；生产代码零 `.unwrap()` / `.expect()`。
4. **`scripts/check-architecture.mjs` 静态扫描规则（必须全覆盖）**：
   - `src-tauri/src/domain/**` 禁止出现 `tauri`、`std::fs`、`std::process`、`rusqlite`、`notify`、`portable_pty`。
   - `src-tauri/src/**` 生产源码禁止出现 `.unwrap(` 与 `.expect(`。
   - `src/**` 中除 `src/lib/ipc/**` 外，禁止出现 `@tauri-apps/api`。
   - `src/features/**` 禁止跨 feature 深层导入（禁止出现跨模块的内部路径引用）。
   - `src/bindings/` 目录下只允许存在 `types.ts`，且首行必须为 `/* GENERATED — DO NOT EDIT.`。
   - `src/**/*.vue` 与 `src/**/*.css`（除 `src/styles/tokens.css` 与白名单定义文件外）禁止裸十六进制色值（`#[0-9a-fA-F]{3,8}`）与硬编码 `z-index: <数字>`。
5. **`pnpm check:all` 全链路接通**：按 `C9.1` 顺序串接 9 步检查并保证在干净工作树下一次跑通退出码为 `0`。

## 4. 自动化验收标准（必须全绿）

- **新增测试**：
  - `src-tauri/tests/error_and_bindings_test.rs`：校验 `AppError` / `ErrorCode` 序列化形状与 `app_get_info` 基础领域构造，验证 `specta` 导出内容包含完整 `AppInfo` 与 `ErrorCode`。
  - `src/lib/ipc/__tests__/smoke.test.ts` & `scripts/__tests__/check-architecture.test.ts`（Vitest）：验证 `check-architecture.mjs` 在模拟违例（如组件直连 `@tauri-apps/api`、`domain` 引入 `std::fs`、Vue 含裸 `#fff`、生产源码含 `.unwrap()`）时必定非零退出拦截。
- **必过命令**：`pnpm check:all`（覆盖 `C9.1` 全部 9 项，返回码 `0`）。

## 5. 人工点验清单（如涉及 macOS 原生行为）

- 本卡无 macOS 原生窗口与交互改动，跳过实机点验；只需确认 `pnpm check:all` 全绿即可合并。
