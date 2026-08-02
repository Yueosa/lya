//! 扫描 `data_root()` 体积分项，供配置页只读展示。

#![deny(missing_docs)]

use std::path::Path;

use lya_config::data_root;
use serde::Serialize;
use thiserror::Error;

/// 扫描失败。
#[derive(Debug, Error)]
pub enum StorageError {
    /// 数据目录不可用。
    #[error("{0}")]
    Invalid(String),

    /// IO 错误。
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// 一项占用分类。
#[derive(Debug, Clone, Serialize)]
pub struct CategoryUsage {
    /// 分类 id，如 `img_cache_web`。
    pub id: String,
    /// 展示名。
    pub label: String,
    /// 字节数。
    pub bytes: u64,
}

/// 占用汇总。
#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    /// 数据根目录。
    pub root: String,
    /// 合计字节。
    pub total_bytes: u64,
    /// 各分类。
    pub categories: Vec<CategoryUsage>,
}

/// 扫描 `~/.lya`（或 `data_root()`）占用。
pub fn scan_usage() -> Result<UsageReport, StorageError> {
    let root = data_root().map_err(|err| StorageError::Invalid(err.to_string()))?;
    let mut categories = Vec::new();

    categories.push(CategoryUsage {
        id: "database".into(),
        label: "数据库".into(),
        bytes: file_size(&root.join("lya.db")),
    });

    let (img_local, img_web) = scan_session_img_cache(&root.join("sessions"));
    categories.push(CategoryUsage {
        id: "img_cache_local".into(),
        label: "图片缓存（本地）".into(),
        bytes: img_local,
    });
    categories.push(CategoryUsage {
        id: "img_cache_web".into(),
        label: "图片缓存（远程）".into(),
        bytes: img_web,
    });

    let mut config_bytes = 0u64;
    for name in ["runtime.toml", "models.toml", "persona.toml", "core.toml"] {
        config_bytes += file_size(&root.join(name));
    }
    categories.push(CategoryUsage {
        id: "config".into(),
        label: "配置文件".into(),
        bytes: config_bytes,
    });

    let counted: u64 = categories.iter().map(|c| c.bytes).sum();
    let total_bytes = walk_dir(&root);
    if total_bytes > counted {
        categories.push(CategoryUsage {
            id: "other".into(),
            label: "其它".into(),
            bytes: total_bytes - counted,
        });
    }

    Ok(UsageReport {
        root: root.to_string_lossy().into_owned(),
        total_bytes,
        categories,
    })
}

fn scan_session_img_cache(sessions: &Path) -> (u64, u64) {
    let mut local = 0u64;
    let mut web = 0u64;
    let Ok(read) = std::fs::read_dir(sessions) else {
        return (0, 0);
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        local += walk_dir(&path.join("img_cache/local"));
        web += walk_dir_shallow(&path.join("img_cache/web"));
    }
    (local, web)
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn walk_dir(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() {
        return file_size(path);
    }
    let mut total = 0u64;
    let Ok(read) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_file() {
            total += file_size(&path);
        } else if path.is_dir() {
            total += walk_dir(&path);
        }
    }
    total
}

/// 只统计目录下直接文件，不含子目录（持久 web 缓存是扁平的）。
fn walk_dir_shallow(path: &Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    let Ok(read) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_file() {
            total += file_size(&path);
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_missing_dir_is_zero() {
        assert_eq!(walk_dir(Path::new("/nonexistent/lya-test-dir")), 0);
    }
}
