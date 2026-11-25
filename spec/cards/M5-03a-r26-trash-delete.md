# M5-03a：删除到系统废纸篓（R26）

- 所属里程碑：M5
- 关联方案与契约：`v5` §1.1；C1.2、C2.1、C4.2、C11.1；D-0011
- 前置依赖：M5-01a（契约已登记 `trash` 依赖与 `note_delete` 新语义）、M5-02b
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：用户文档的删除一律移到系统废纸篓，可从废纸篓还原。覆盖两处：`note_delete`（笔记文件）与解除目录工作区绑定（删除 `.quicknote.yaml`）。
- 不做：不改 `backups/` 滚动清理与 `recovery/` 快照清理（它们是应用自己的状态文件，继续直接删除）；`inbox.md` 仍不可删除。

## 2. 允许修改文件白名单

- `src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`
- `src-tauri/src/infra/trash.rs`（新建）
- `src-tauri/src/infra/mod.rs`
- `src-tauri/src/application/notes.rs`
- `src-tauri/src/application/external.rs`
- `src-tauri/src/commands/external.rs`

测试文件（不计入上限）：`src-tauri/tests/note_crud_and_index_test.rs`，以及任何因注入删除器而需要适配的既有测试。

## 3. 具体实现要求

### 3.1 依赖

`Cargo.toml` 的 `[dependencies]` 增加 `trash = "5"`。首次 `cargo check` 会更新 `Cargo.lock`。

### 3.2 `infra/trash.rs`（新建）

```rust
use std::path::Path;
use std::sync::Arc;

use crate::domain::error::{AppError, ErrorCode};

/// 把文件移到系统废纸篓的删除器（C4.2 `note_delete`）。可注入以便测试不污染用户废纸篓。
pub trait FileRemover: Send + Sync {
    fn remove(&self, path: &Path) -> Result<(), AppError>;
}

pub struct TrashFileRemover;

impl FileRemover for TrashFileRemover {
    fn remove(&self, path: &Path) -> Result<(), AppError> {
        trash::delete(path).map_err(|e| {
            AppError::with_detail(ErrorCode::InternalError, "Failed to move file to Trash", e.to_string())
        })
    }
}

/// 真正删除文件用的默认删除器。
pub fn default_file_remover() -> Arc<dyn FileRemover> {
    Arc::new(TrashFileRemover)
}
```

`infra/mod.rs` 增加 `pub mod trash;`。`trash` crate 的具体函数名以其文档为准（当前为 `trash::delete`）；若编译不过，改用该版本提供的等价 API，不要换别的实现方式。

### 3.3 `note_delete` 改造（`application/notes.rs` 约 926–950）

拆成两个函数：

```rust
pub fn note_delete(pm: &PersistenceManager, args: RelPathArgs) -> Result<(), AppError> {
    note_delete_with(pm, args, default_file_remover().as_ref())
}

pub fn note_delete_with(
    pm: &PersistenceManager,
    args: RelPathArgs,
    remover: &dyn FileRemover,
) -> Result<(), AppError> {
    // 原有的 root / rel 校验与「inbox.md 不可删除」保持不变
    ...
    remover.remove(&abs_path)?;                 // 替换原来的 fs::remove_file
    pm.with_db(|conn| delete_note_index_by_rel_path(conn, &rel))?;
    pm.clear_self_write(&abs_path);
    Ok(())
}
```

### 3.4 解除目录工作区绑定（commands 层直接删文件的旧代码下沉）

`commands/external.rs` 的 `set_directory_workspace_config`（基线约 249–296）的 `None =>` 分支现在直接在 commands 层操作文件系统，与 v5 §1.1「接入层不含业务逻辑」不符。改为：

- 在 `application/external.rs` 新增：

```rust
pub fn directory_workspace_config_delete_uc(
    pm: &PersistenceManager,
    vault_rel_dir: &str,
) -> Result<(), AppError> {
    directory_workspace_config_delete_uc_with(pm, vault_rel_dir, default_file_remover().as_ref())
}

pub fn directory_workspace_config_delete_uc_with(
    pm: &PersistenceManager,
    vault_rel_dir: &str,
    remover: &dyn FileRemover,
) -> Result<(), AppError>
```

函数体就是原 `None =>` 分支的内容（校验目录 → 拼 `rel_path` → 存在则登记自写并删除 → `reconcile_path`），只把 `std::fs::remove_file` 换成 `remover.remove(&target_path)?`。`normalize_vault_rel_dir_checked` 若是 commands 层的私有函数，把它一并搬到 application 层并改为 `pub`。

- `commands/external.rs` 的 `None =>` 分支改为 `directory_workspace_config_delete_uc(&pm, &vault_rel_dir)`。commands 层不得再出现 `std::fs`。

### 3.5 测试

- `note_crud_and_index_test.rs`：把现有删除用例改为调用 `note_delete_with(pm, args, &fake)`，`fake` 是测试内定义的 `FileRemover` 实现，行为是记录被删路径并执行 `std::fs::remove_file`。新增断言：传入的路径等于笔记绝对路径。
- 任何调用 `set_directory_workspace_config` 传 `None` 或调用删除相关用例的既有测试，同样改用 `_with` 版本 + fake。
- **禁止**在测试里把任何文件真的移进用户废纸篓。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（clippy `-D warnings` 不得有新警告）。
- `grep -rn 'std::fs' src-tauri/src/commands/` 只剩 `commands/external.rs` 中读写 `.quicknote.yaml` 的 `Some(cfg)` 分支若存在的话；删除分支不再直接操作文件系统。

## 5. 人工点验清单

- [ ] 左侧列表删除一篇笔记：文件从笔记库消失，废纸篓里可见同名文件，可「放回原处」。
- [ ] 对目录右键解除工作区绑定：`.quicknote.yaml` 进废纸篓；左栏徽标消失；不触发误报「外部修改」。
- [ ] 尝试删除 `inbox.md`：仍被拒绝。

## 上下文提示

- 只读 `notes.rs` 约 926–950、`commands/external.rs` 约 200–299、`application/external.rs` 顶部 `use` 区与 `directory_workspace_config_write_uc` 附近。
- 忽略：其余笔记读写逻辑。
