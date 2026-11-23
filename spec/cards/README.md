# M0 阶段任务卡调度总览

> 阶段目标：搭建单向分层脚手架、自动化门禁与 `specta` 契约流水线，打通持久化与 SQLite 基座、macOS 原生 `NSPanel` / 动态激活 / 全局热键桥接，以及 `--qn-*` 双主题视觉基线与首次选库引导，为后续 M1–M4 业务里程碑提供稳定底座。

## 1. 卡片清单与串行顺序

M0 阶段共拆分为 4 张严格串行推进的任务卡（`M0-01 → M0-02 → M0-03 → M0-04`）：

| 序号 | 卡号与文件 | 核心主题 | 覆盖契约与 IPC 命令 | 人工点验 |
| --- | --- | --- | --- | --- |
| 1 | [M0-01-bootstrap-and-gates.md](M0-01-bootstrap-and-gates.md) | 工程脚手架、单向分层、`specta` 流水线与全量门禁 | `C1`–`C3`、`C9.1`、`C10`、`D-0009`、`D-0010`；`app_get_info`（基础） | 无 |
| 2 | [M0-02-persistence-and-sqlite.md](M0-02-persistence-and-sqlite.md) | 应用数据落盘、`settings.yaml` 容错、`ui-state.json` 与 SQLite 迁移 | `C3.4`、`C4.2`–`C4.3`、`C5`、`C6.1`–`C6.3`、`C9.2`、`D-0008`；`settings_get`、`ui_state_get`、`ui_state_set`、`library_get_info`、`library_set_root` | 无 |
| 3 | [M0-03-macos-native-bridge.md](M0-03-macos-native-bridge.md) | macOS 原生窗口桥接：`NSPanel`、非激活 Toast、动态激活与热键 | `C2.4`、`C3.4`、`C4.2`–`C4.3`、`C9.3`、`D-0001`–`D-0004`；`window_show_main`、`window_hide_main`、`window_show_quick_bar`、`window_hide_quick_bar`、`window_open_settings`、`hotkey_update` | **关键实机点验** |
| 4 | [M0-04-tokens-shells-and-onboarding.md](M0-04-tokens-shells-and-onboarding.md) | `--qn-*` 双主题视觉基线、多窗口 Vue 壳与首次选库引导 | `C1.2`、`C4.2`–`C4.3`、`C7.1`、`C8.1`（S1）、`C9.1`、`D-0004`、`D-0008`；消费 M0 全部命令与事件 | **视觉与引导点验** |

## 2. 单卡施工规程（严格执行 `C10`）

1. **一卡一干净会话**：每张卡必须在全新 AI 会话中开始，开场先读 [AGENTS.md](../../AGENTS.md)、[engineering-contract.md](../engineering-contract.md)、[DECISIONS.md](../../DECISIONS.md) 与对应任务卡。
2. **分支隔离**：从 `dev` 拉取对应功能分支（如 `feat/M0-01-bootstrap-and-gates`），严守卡片白名单。
3. **收工门禁**：必须运行 `pnpm check:all`（`C9.1`）全部通过并按 `C10.2` 中文规范提交后，合并回 `dev` 并结束当前会话。
4. **遇阻塞即停**：若涉及未定义的技术选型（如原生桥接 crate 路线），按 `C11` 先记入 `DECISIONS.md` 再编码；涉及产品取舍先询问用户。

## 3. M0 收官检查点

4 张卡片全部合入 `dev` 后，先执行以下两步再拆分 M1 任务卡：

1. **独立只读审查（`C10.3`）**：新开只读审查会话，对照 `quicknote-design-v4.md`、`engineering-contract.md`、`DECISIONS.md` 检查 M0 全量实现有无隐性偏差或吞错。
2. **实机点验与基线打标**：由用户在真实 macOS 桌面完成 `C9.3` 涉及原生窗口/动态激活/热键/引导的实机点验，确认无误后在 Git 打出 M0 里程碑标签。
