//! 数据目录与默认库路径。
//!
//! lya 的数据根固定为 `$HOME/.lya`，库文件为 `lya.db`。

use std::env;
use std::path::PathBuf;

use crate::error::DbError;

/// 数据根目录：`$HOME/.lya`。
pub fn data_root() -> Result<PathBuf, DbError> {
    let home = env::var_os("HOME").ok_or_else(|| DbError::Path("HOME is not set".into()))?;
    if home.is_empty() {
        return Err(DbError::Path("HOME is empty".into()));
    }
    Ok(PathBuf::from(home).join(".lya"))
}

/// 默认库文件：`$HOME/.lya/lya.db`。
pub fn default_db_path() -> Result<PathBuf, DbError> {
    Ok(data_root()?.join("lya.db"))
}
