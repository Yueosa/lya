//! # lya-config
//!
//! 分层配置，全部落在 `~/.lya/` 下。
//!
//! | 文件 | 层级 | 何时生效 |
//! |------|------|----------|
//! | `core.toml` | 进程级：端口、日志、库路径、HTTP 超时 | 改了要重启 |
//! | `runtime.toml` | 各模块默认值：轮数上限、默认模式/模型、索引体积 | 重新加载即可 |
//! | `models.toml` | 模型清单（含密钥，权限 0600） | 重新加载即可 |
//! | `persona.toml` | 全局人设 | 重新加载即可 |
//!
//! **没有「会话级配置文件」**。会话自己设过的工作模式、启用工具、人设都存在
//! `sessions` 表里，本 crate 只回答「会话没设时用什么」。
//!
//! ## 依赖取向
//!
//! 只解析成朴素数据，不产出别的 crate 的配置类型——否则本 crate 会因为要
//! 构造 `HttpConfig` 而拖进 reqwest、因为要构造 `IndexBudget` 而拖进 rusqlite，
//! 而它偏偏是被最多人依赖的那个。映射由装配方（agent / core）负责，也就那
//! 几行。唯一的例外是 [`lya_mode::Mode`]：让 `default_work_mode = "asdf"`
//! 在启动时就报错，比运行时才发现值得。
//!
//! ## 「可热改」的现有含义
//!
//! 指「重新 [`Config::load_from`] 一次就生效」，不含文件监听。runtime 那几个
//! 值不影响已建立的连接与监听端口，所以够用了；真正的 watcher 等 HTTP 层再说。

#![deny(missing_docs)]

pub mod core;
pub mod error;
pub mod models;
pub mod runtime;
pub mod write;

pub use core::{CoreConfig, DbConfig, HttpSettings, LogConfig, LogLevel, ServerConfig};
pub use error::ConfigError;
pub use write::{edit_file, merge_table, write_persona, write_runtime};
pub use models::{ModelCatalog, ModelEntry};
pub use models::{CAPABILITY_TEXT, CAPABILITY_VISION};
pub use runtime::{
    AgentSettings, MemorySettings, RuntimeConfig, ShellConfirm, ShellSettings, ToolSettings,
};

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// `core.toml` 文件名。
pub const CORE_FILE: &str = "core.toml";
/// `runtime.toml` 文件名。
pub const RUNTIME_FILE: &str = "runtime.toml";
/// `models.toml` 文件名。
pub const MODELS_FILE: &str = "models.toml";
/// `persona.toml` 文件名。
pub const PERSONA_FILE: &str = "persona.toml";

const CORE_TEMPLATE: &str = include_str!("../templates/core.toml");
const RUNTIME_TEMPLATE: &str = include_str!("../templates/runtime.toml");
const MODELS_TEMPLATE: &str = include_str!("../templates/models.toml");
const PERSONA_TEMPLATE: &str = include_str!("../templates/persona.toml");

/// 配置根目录：`$HOME/.lya`。
///
/// 与 `lya_db::data_root` 是同一个位置。这里重写一遍五行逻辑，是为了不让
/// 配置层依赖数据库层（那会拖进 rusqlite）。
pub fn data_root() -> Result<PathBuf, ConfigError> {
    let home = env::var_os("HOME").ok_or_else(|| ConfigError::Path("HOME is not set".into()))?;
    if home.is_empty() {
        return Err(ConfigError::Path("HOME is empty".into()));
    }
    Ok(PathBuf::from(home).join(".lya"))
}

/// `persona.toml` 的结构。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PersonaFile {
    /// 人设正文。
    text: String,
}

/// 合并后的完整配置。
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// 进程级。
    pub core: CoreConfig,
    /// 运行时默认值。
    pub runtime: RuntimeConfig,
    /// 模型清单。
    pub models: ModelCatalog,
    /// 全局人设；未配置或为空时是 `None`，由 `lya-prompt` 回退到内置默认。
    pub persona: Option<String>,
    /// 本次读取的目录。
    pub dir: PathBuf,
}

impl Config {
    /// 从默认目录 `~/.lya` 加载。
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from(data_root()?)
    }

    /// 从指定目录加载；**缺失的文件一律用内置默认值**，不报错。
    ///
    /// 想让用户有东西可改，先调 [`Config::init_missing`]。
    pub fn load_from(dir: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let dir = dir.as_ref().to_path_buf();
        let runtime_path = dir.join(RUNTIME_FILE);
        // 旧版前端曾把 runtime 写进 [tables.*]，导致整文件无法解析
        repair_runtime_tables(&runtime_path)?;
        let config = Self {
            core: read_toml(&dir.join(CORE_FILE))?.unwrap_or_default(),
            runtime: read_toml(&runtime_path)?.unwrap_or_default(),
            models: read_toml(&dir.join(MODELS_FILE))?.unwrap_or_default(),
            persona: read_toml::<PersonaFile>(&dir.join(PERSONA_FILE))?
                .map(|file| file.text.trim().to_string())
                .filter(|text| !text.is_empty()),
            dir,
        };
        config.validate()?;
        Ok(config)
    }

    /// 在目录下补齐缺失的配置文件（已存在的不动），返回新建了哪些。
    ///
    /// `models.toml` 含密钥，创建时权限设为 `0600`。
    pub fn init_missing(dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, ConfigError> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir).map_err(|source| ConfigError::Io {
            path: dir.to_path_buf(),
            source,
        })?;

        let mut created = Vec::new();
        for (name, template, secret) in [
            (CORE_FILE, CORE_TEMPLATE, false),
            (RUNTIME_FILE, RUNTIME_TEMPLATE, false),
            (MODELS_FILE, MODELS_TEMPLATE, true),
            (PERSONA_FILE, PERSONA_TEMPLATE, false),
        ] {
            let path = dir.join(name);
            if path.exists() {
                continue;
            }
            fs::write(&path, template).map_err(|source| ConfigError::Io {
                path: path.clone(),
                source,
            })?;
            if secret {
                restrict_permissions(&path)?;
            }
            created.push(path);
        }
        Ok(created)
    }

    /// 数据库文件的绝对路径。
    pub fn db_path(&self) -> PathBuf {
        self.core.db.resolve(&self.dir)
    }

    /// 默认模型；未指定 `default_model` 时取清单里的第一条。
    pub fn default_model(&self) -> Option<&ModelEntry> {
        match &self.runtime.agent.default_model {
            Some(id) => self.models.get(id),
            None => self.models.models.first(),
        }
    }

    /// 结构性校验：清单本身合法，且 `default_model` 不悬空。
    fn validate(&self) -> Result<(), ConfigError> {
        self.models.validate()?;
        if let Some(id) = &self.runtime.agent.default_model
            && self.models.get(id).is_none()
        {
            return Err(ConfigError::Invalid(format!(
                "runtime.toml 的 default_model = {id:?} 在 models.toml 里不存在；现有 id：{:?}",
                self.models.ids()
            )));
        }
        Ok(())
    }

    /// 检查是否已经可以真正发请求。
    ///
    /// 与 [`Config::validate`] 分开：结构合法但密钥还是模板占位符时，配置本身
    /// 没错，只是还没填完。测试和界面可以加载配置而不必先有密钥。
    pub fn check_ready(&self) -> Result<(), ConfigError> {
        if self.models.is_empty() {
            return Err(ConfigError::NotReady(format!(
                "{MODELS_FILE} 里没有任何模型，请先添加一条"
            )));
        }
        let placeholders: Vec<&str> = self
            .models
            .models
            .iter()
            .filter(|entry| entry.api_key_is_placeholder())
            .map(|entry| entry.id.as_str())
            .collect();
        if !placeholders.is_empty() {
            return Err(ConfigError::NotReady(format!(
                "请先在 {MODELS_FILE} 填入真实 api_key：{placeholders:?}"
            )));
        }
        Ok(())
    }
}

/// 读取并解析一个 TOML 文件；文件不存在返回 `Ok(None)`。
fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

/// 去掉误写入的 `[tables]` / `[tables.*]` 段（旧版前端 bug 产物）。
fn repair_runtime_tables(path: &Path) -> Result<(), ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if !text.contains("[tables") {
        return Ok(());
    }
    let mut out = Vec::new();
    let mut skip = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[tables]" || trimmed.starts_with("[tables.") {
            skip = true;
            continue;
        }
        if skip && trimmed.starts_with('[') {
            skip = false;
        }
        if !skip {
            out.push(line);
        }
    }
    let repaired = format!("{}\n", out.join("\n").trim_end());
    fs::write(path, &repaired).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// 把文件权限收紧到仅所有者可读写。
#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<(), ConfigError> {
    Ok(())
}
