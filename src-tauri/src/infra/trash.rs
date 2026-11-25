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
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to move file to Trash",
                e.to_string(),
            )
        })
    }
}

/// 真正删除文件用的默认删除器。
pub fn default_file_remover() -> Arc<dyn FileRemover> {
    Arc::new(TrashFileRemover)
}
