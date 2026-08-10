//! 进程级配置（`core.toml`）：改了要重启才生效的那些。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// `core.toml` 的内容。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoreConfig {
    /// HTTP 服务监听设置。
    pub server: ServerConfig,
    /// 日志设置。
    pub log: LogConfig,
    /// 数据库位置。
    pub db: DbConfig,
    /// 出站 HTTP 客户端设置。
    pub http: HttpSettings,
}

/// 监听设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// 监听地址；本地应用默认只听回环。
    pub host: String,
    /// 首选端口。
    pub port: u16,
    /// 端口被占用时向后依次尝试的最大偏移。
    pub port_backoff_max: u16,
    /// 经 Caddy 等反代用域名访问时，浏览器 `Origin` 的主机名白名单（不含 scheme 与端口）。
    ///
    /// 例：`["lya.lian.love"]` 允许 `http://lya.lian.love` 与 `https://lya.lian.love`。
    #[serde(default)]
    pub trusted_hosts: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 51616,
            port_backoff_max: 50,
            trusted_hosts: Vec::new(),
        }
    }
}

impl ServerConfig {
    /// 依次要尝试的端口（含首选端口本身）。
    pub fn candidate_ports(&self) -> impl Iterator<Item = u16> + '_ {
        (0..=self.port_backoff_max).filter_map(|offset| self.port.checked_add(offset))
    }
}

/// 日志级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// 只记错误。
    Error,
    /// 记警告及以上。
    Warn,
    /// 默认级别。
    #[default]
    Info,
    /// 调试。
    Debug,
    /// 全部。
    Trace,
}

impl LogLevel {
    /// 转成日志库常用的字符串。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

/// 日志设置。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LogConfig {
    /// 级别。
    pub level: LogLevel,
}

/// 数据库位置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DbConfig {
    /// 库文件路径；相对路径按数据根解析。
    pub path: String,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            path: "lya.db".into(),
        }
    }
}

impl DbConfig {
    /// 解析成绝对路径：相对路径接在 `data_root` 后面。
    pub fn resolve(&self, data_root: &Path) -> PathBuf {
        let path = Path::new(&self.path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            data_root.join(path)
        }
    }
}

/// 出站 HTTP 客户端设置。
///
/// 这里刻意存成朴素数值而不是 `lya_http::HttpConfig`——本 crate 不依赖
/// `lya-http`（那会连带拖进 reqwest），由装配方把这些值映射过去。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpSettings {
    /// 单次请求总超时（秒）。
    pub timeout_secs: u64,
    /// 建连超时（秒）。
    pub connect_timeout_secs: u64,
    /// 空闲连接保留时长（秒）。
    pub pool_idle_timeout_secs: u64,
    /// 每主机最大空闲连接数。
    pub pool_max_idle_per_host: usize,
    /// User-Agent。
    pub user_agent: String,
}

impl Default for HttpSettings {
    fn default() -> Self {
        Self {
            timeout_secs: 120,
            connect_timeout_secs: 10,
            pool_idle_timeout_secs: 90,
            pool_max_idle_per_host: 4,
            user_agent: "lya/0.1".into(),
        }
    }
}
