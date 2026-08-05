//! 数据根目录。
//!
//! `lya-db` 要在这里放库文件，`lya-config` 要在这里放四份 TOML，`lya-media` 和
//! `lya-storage` 要在这里找会话目录。四个 crate 想知道同一件事，所以答案在这里只有
//! 一份——之前 `lya-db` 和 `lya-config` 各写了一份逐字相同的实现。

use std::env;
use std::path::PathBuf;

use crate::error::BaseError;

/// 数据目录在家目录下的名字。
pub const DATA_DIR_NAME: &str = ".lya";

/// 数据根目录：`$HOME/.lya`。
///
/// 不检查它存不存在——建目录是各自写入时的事，读的一方遇到目录缺失应当回退到默认值
/// 而不是报错。
pub fn data_root() -> Result<PathBuf, BaseError> {
    let home = env::var_os("HOME").ok_or_else(|| BaseError::Path("HOME 未设置".into()))?;
    if home.is_empty() {
        return Err(BaseError::Path("HOME 是空的".into()));
    }
    Ok(PathBuf::from(home).join(DATA_DIR_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_root_sits_under_home() {
        // 这里不改进程环境变量（会影响并行跑的其它测试），只核对形状
        let root = data_root().expect("测试环境应当有 HOME");
        assert!(root.ends_with(DATA_DIR_NAME));
        assert!(root.is_absolute());
    }
}
