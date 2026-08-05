//! 默认库路径。
//!
//! 数据根是 [`lya_base::data_root`]，不在这里重算——`lya-config` 也要用同一个答案，
//! 各写一份的结果就是两处实现逐字相同、改一处忘一处。

use std::path::PathBuf;

use crate::error::DbError;

/// 数据根目录：`$HOME/.lya`。
pub fn data_root() -> Result<PathBuf, DbError> {
    lya_base::data_root().map_err(|err| DbError::Path(err.to_string()))
}

/// 默认库文件：`$HOME/.lya/lya.db`。
pub fn default_db_path() -> Result<PathBuf, DbError> {
    Ok(data_root()?.join("lya.db"))
}
