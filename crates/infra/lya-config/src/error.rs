//! 配置错误。

use std::io;
use std::path::PathBuf;

/// `lya-config` 错误。
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// 读写配置文件失败。
    #[error("io error at {path}: {source}")]
    Io {
        /// 出错的文件。
        path: PathBuf,
        /// 底层 IO 错误。
        #[source]
        source: io::Error,
    },

    /// TOML 解析失败（含字段类型不符、未知字段等）。
    #[error("failed to parse {path}: {source}")]
    Parse {
        /// 出错的文件。
        path: PathBuf,
        /// 解析错误。
        #[source]
        source: toml::de::Error,
    },

    /// 配置目录无法确定。
    #[error("config path error: {0}")]
    Path(String),

    /// 配置内容自相矛盾（如默认模型不在清单里）。
    #[error("{0}")]
    Invalid(String),

    /// 还没准备好投入使用（如 api_key 仍是模板占位符）。
    #[error("{0}")]
    NotReady(String),
}
