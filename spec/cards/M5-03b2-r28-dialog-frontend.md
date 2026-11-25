# M5-03b2：系统文件选择窗口——设置页改造（R28 后半）

- 所属里程碑：M5
- 关联方案与契约：`v5` §1.2；C1.2、C2.2、C4.2、C6.3；D-0008
- 前置依赖：M5-03b1
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：「配置备份与恢复」去掉两个路径文本框，改为系统文件选择窗口；选中后在界面上只读显示路径并记住。
- 不做：不改 `settings_export` / `settings_import` 的 Rust 实现；不改编码格式。

## 2. 允许修改文件白名单

- `src/lib/ipc/commands.ts`（C1.3 共享文件，仅追加函数）
- `src/features/settings/SettingsView.vue`
- `src/features/settings/store.ts`
- `src/features/settings/api.ts`

测试文件（不计入上限）：`src/features/settings/__tests__/` 下相关测试。

## 3. 具体实现要求

### 3.1 IPC 封装（唯一允许直连 Tauri 的位置）

`src/lib/ipc/commands.ts` 追加：

```ts
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

const BACKUP_YAML_FILTER = [{ name: 'YAML', extensions: ['yaml', 'yml'] }];

/** 弹出系统保存窗口，返回所选路径；用户取消时返回 null。 */
export async function pickBackupSavePath(defaultPath?: string | null): Promise<string | null> {
  const picked = await saveDialog({
    defaultPath: defaultPath ?? undefined,
    filters: BACKUP_YAML_FILTER,
  });
  return typeof picked === 'string' && picked.length > 0 ? picked : null;
}

/** 弹出系统打开窗口，返回所选文件路径；用户取消时返回 null。 */
export async function pickBackupOpenPath(defaultPath?: string | null): Promise<string | null> {
  const picked = await openDialog({
    multiple: false,
    directory: false,
    defaultPath: defaultPath ?? undefined,
    filters: BACKUP_YAML_FILTER,
  });
  return typeof picked === 'string' && picked.length > 0 ? picked : null;
}
```

若 `src/lib/ipc/__tests__/smoke.test.ts` 对 `commands.ts` 的导出做快照或枚举断言，同步更新。

### 3.2 设置 store

`settings/store.ts`：

1. 新增状态 `backupPaths = ref<BackupPathsUiState>({ lastExportPath: null, lastImportPath: null })`，在 `initialize()` / `refreshUiState()` 里从 `uiState.backupPaths` 读取。
2. `exportSettings(destPath)` 成功后把 `backupPaths.lastExportPath` 设为 `destPath` 并 `patchUiState('backupPaths', backupPaths.value)`。
3. `importSettings(srcPath)` 成功后同样记录 `lastImportPath`。
4. 新增 `exportBackupPathDraft = ref('')`、`importBackupPathDraft = ref('')`，由文件选择结果填充，界面只读展示。

`api.ts`：若 `patchUiState` 尚未从该文件导出给 store 使用，补一个 `patchBackupPaths(value: BackupPathsUiState)`。

### 3.3 设置页

`SettingsView.vue` 的「配置备份与恢复」卡片（基线约 332–390）：

- 删除「导出备份路径」「导入备份路径」两个 `QnInput`。
- 导出区：一个「选择位置并导出…」按钮 + 一行只读文本显示 `exportBackupPathDraft`（为空时显示「尚未导出」）。
- 导入区：一个「选择文件并导入…」按钮 + 一行只读文本显示 `importBackupPathDraft`（为空时显示「尚未导入」）。
- 保留原有的成功 / 失败提示与「导入前将自动备份当前配置至 `backups/`」说明。
- 新增的 `data-testid`：`backup-export-pick-btn`、`backup-export-path`、`backup-import-pick-btn`、`backup-import-path`。

点击流程：调用 3.1 的选择函数（`defaultPath` 传入上次路径）→ 结果为 `null` 则什么都不做 → 否则执行原有导出 / 导入逻辑。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。
- 更新或新增单元测试：mock `@tauri-apps/plugin-dialog` 的 `save` / `open`，覆盖「选中路径后调用导出并记住路径」「取消时什么都不做」两种情况。vitest 环境为 `node`，mock 需用 `vi.mock('@tauri-apps/plugin-dialog', ...)`。

## 5. 人工点验清单

- [ ] 点「选择位置并导出…」弹出系统保存窗口，默认定位到上次导出的目录；保存后界面显示完整路径。
- [ ] 点「选择文件并导入…」弹出系统打开窗口，只能选 `.yaml` / `.yml`；导入成功后界面显示路径，其余设置立即生效。
- [ ] 两个窗口里按 Esc 取消：界面无变化、无报错。
- [ ] 重启应用后再次打开面板：上次路径仍显示，选择窗口默认定位到上次的目录。

## 上下文提示

- 只读 `SettingsView.vue` 约 40–70 与 325–390、`settings/store.ts` 约 195–315。
- 忽略：设置页其他分区。
