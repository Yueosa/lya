//! 扫描 `data_root()` 体积分项，供配置页只读展示。

#![deny(missing_docs)]

use std::collections::HashMap;
use std::path::Path;

use lya_config::data_root;
use serde::Serialize;
use std::os::unix::fs::MetadataExt;
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

/// Local 缓存占用（含硬链接去重）。
#[derive(Debug, Clone, Serialize)]
pub struct LocalCacheStats {
    /// 逐文件 size 之和。
    pub logical_bytes: u64,
    /// 按 inode 去重后的磁盘占用。
    pub physical_bytes: u64,
    /// 硬链接共用的字节（logical − physical）。
    pub shared_bytes: u64,
    /// 文件数。
    pub file_count: u64,
    /// `nlink > 1` 的文件数。
    pub linked_file_count: u64,
}

/// Web 缓存占用。
#[derive(Debug, Clone, Serialize)]
pub struct WebCacheStats {
    /// 字节数。
    pub bytes: u64,
    /// 文件数。
    pub file_count: u64,
}

/// 树形占用节点。
#[derive(Debug, Clone, Serialize)]
pub struct UsageSection {
    /// 节点 id。
    pub id: String,
    /// 展示名。
    pub label: String,
    /// 该节点合计字节。
    pub bytes: u64,
    /// 子节点（数据库/配置文件按文件列出；缓存下为媒体类型）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<UsageSection>>,
    /// 缓存叶子：Local 统计。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<LocalCacheStats>,
    /// 缓存叶子：Web 统计。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web: Option<WebCacheStats>,
}

/// 占用汇总。
#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    /// 数据根目录。
    pub root: String,
    /// 合计字节。
    pub total_bytes: u64,
    /// 顶层分类树。
    pub sections: Vec<UsageSection>,
}

const CACHE_DIRS: [(&str, &str, &str); 3] = [
    ("cache.image", "图片", "img_cache"),
    ("cache.video", "视频", "vdo_cache"),
    ("cache.audio", "音频", "ado_cache"),
];

/// 扫描 `~/.lya`（或 `data_root()`）占用。
pub fn scan_usage() -> Result<UsageReport, StorageError> {
    let root = data_root().map_err(|err| StorageError::Invalid(err.to_string()))?;
    scan_usage_at(&root)
}

fn scan_usage_at(root: &Path) -> Result<UsageReport, StorageError> {
    let total_bytes = walk_dir(root);

    let (database, config) = scan_root_files(root);
    let cache = scan_cache(&root.join("sessions"));

    let mut sections = vec![database, config, cache];
    let counted: u64 = sections.iter().map(|section| section.bytes).sum();
    if total_bytes > counted {
        sections.push(UsageSection {
            id: "other".into(),
            label: "其它".into(),
            bytes: total_bytes - counted,
            children: None,
            local: None,
            web: None,
        });
    }

    Ok(UsageReport {
        root: root.to_string_lossy().into_owned(),
        total_bytes,
        sections,
    })
}

fn scan_root_files(root: &Path) -> (UsageSection, UsageSection) {
    let mut db_files: Vec<(String, u64)> = Vec::new();
    let mut cfg_files: Vec<(String, u64)> = Vec::new();

    if let Ok(read) = std::fs::read_dir(root) {
        for entry in read.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let bytes = file_size(&path);
            if is_database_file(name) {
                db_files.push((name.to_string(), bytes));
            } else if name.ends_with(".toml") {
                cfg_files.push((name.to_string(), bytes));
            }
        }
    }

    db_files.sort_by(|a, b| a.0.cmp(&b.0));
    cfg_files.sort_by(|a, b| a.0.cmp(&b.0));

    (
        section_from_files("database", "数据库", &db_files),
        section_from_files("config", "配置文件", &cfg_files),
    )
}

fn section_from_files(id: &str, label: &str, files: &[(String, u64)]) -> UsageSection {
    let bytes: u64 = files.iter().map(|(_, size)| size).sum();
    let children = if files.is_empty() {
        None
    } else {
        Some(
            files
                .iter()
                .map(|(name, size)| UsageSection {
                    id: format!("{id}.{name}"),
                    label: name.clone(),
                    bytes: *size,
                    children: None,
                    local: None,
                    web: None,
                })
                .collect(),
        )
    };
    UsageSection {
        id: id.into(),
        label: label.into(),
        bytes,
        children,
        local: None,
        web: None,
    }
}

fn is_database_file(name: &str) -> bool {
    name.ends_with(".db-wal") || name.ends_with(".db-shm") || name.ends_with(".db")
}

fn scan_cache(sessions: &Path) -> UsageSection {
    let mut children = Vec::new();
    for (id, label, dir_name) in CACHE_DIRS {
        let mut local = LocalScan::default();
        let mut web = WebScan::default();
        scan_session_cache(sessions, dir_name, &mut local, &mut web);
        let bytes = local.logical_bytes + web.bytes;
        children.push(UsageSection {
            id: id.into(),
            label: label.into(),
            bytes,
            children: None,
            local: Some(local.into_stats()),
            web: Some(web.into_stats()),
        });
    }
    let bytes: u64 = children.iter().map(|child| child.bytes).sum();
    UsageSection {
        id: "cache".into(),
        label: "缓存".into(),
        bytes,
        children: Some(children),
        local: None,
        web: None,
    }
}

fn scan_session_cache(sessions: &Path, cache_dir: &str, local: &mut LocalScan, web: &mut WebScan) {
    let Ok(read) = std::fs::read_dir(sessions) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        local.scan_dir(&path.join(cache_dir).join("local"));
        web.scan_dir(&path.join(cache_dir).join("web"));
    }
}

#[derive(Default)]
struct LocalScan {
    logical_bytes: u64,
    file_count: u64,
    linked_file_count: u64,
    inodes: HashMap<(u64, u64), u64>,
}

impl LocalScan {
    fn scan_dir(&mut self, path: &Path) {
        if !path.exists() {
            return;
        }
        if path.is_file() {
            self.scan_file(path);
            return;
        }
        let Ok(read) = std::fs::read_dir(path) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_file() {
                self.scan_file(&path);
            } else if path.is_dir() {
                self.scan_dir(&path);
            }
        }
    }

    fn scan_file(&mut self, path: &Path) {
        let Ok(meta) = std::fs::metadata(path) else {
            return;
        };
        if !meta.is_file() {
            return;
        }
        let size = meta.len();
        self.logical_bytes += size;
        self.file_count += 1;
        if meta.nlink() > 1 {
            self.linked_file_count += 1;
        }
        self.inodes
            .entry((meta.dev(), meta.ino()))
            .or_insert(size);
    }

    fn into_stats(self) -> LocalCacheStats {
        let physical_bytes: u64 = self.inodes.values().copied().sum();
        LocalCacheStats {
            logical_bytes: self.logical_bytes,
            physical_bytes,
            shared_bytes: self.logical_bytes.saturating_sub(physical_bytes),
            file_count: self.file_count,
            linked_file_count: self.linked_file_count,
        }
    }
}

#[derive(Default)]
struct WebScan {
    bytes: u64,
    file_count: u64,
}

impl WebScan {
    /// 只统计目录下直接文件（持久 web 缓存是扁平的）。
    fn scan_dir(&mut self, path: &Path) {
        if !path.is_dir() {
            return;
        }
        let Ok(read) = std::fs::read_dir(path) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_file() {
                self.bytes += file_size(&path);
                self.file_count += 1;
            }
        }
    }

    fn into_stats(self) -> WebCacheStats {
        WebCacheStats {
            bytes: self.bytes,
            file_count: self.file_count,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write_file(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = fs::File::create(path).unwrap();
        file.write_all(bytes).unwrap();
    }

    #[test]
    fn walk_missing_dir_is_zero() {
        assert_eq!(walk_dir(Path::new("/nonexistent/lya-test-dir")), 0);
    }

    #[test]
    fn is_database_file_by_suffix() {
        assert!(is_database_file("lya.db"));
        assert!(is_database_file("lya.db-wal"));
        assert!(is_database_file("lya.db-shm"));
        assert!(is_database_file("backup.db"));
        assert!(!is_database_file("runtime.toml"));
    }

    #[test]
    fn local_scan_deduplicates_hard_links() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.bin");
        let link = dir.path().join("link.bin");
        write_file(&source, &[1, 2, 3, 4]);
        fs::hard_link(&source, &link).unwrap();

        let mut scan = LocalScan::default();
        scan.scan_dir(dir.path());
        let stats = scan.into_stats();

        assert_eq!(stats.file_count, 2);
        assert_eq!(stats.linked_file_count, 2);
        assert_eq!(stats.logical_bytes, 8);
        assert_eq!(stats.physical_bytes, 4);
        assert_eq!(stats.shared_bytes, 4);
    }

    #[test]
    fn scan_usage_tree_in_temp_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        write_file(&root.join("lya.db"), &[0; 100]);
        write_file(&root.join("lya.db-wal"), &[0; 50]);
        write_file(&root.join("core.toml"), b"");
        write_file(
            &root.join("sessions/s1/img_cache/local/a.png"),
            &[0; 10],
        );
        write_file(&root.join("sessions/s1/img_cache/web/b.png"), &[0; 20]);
        write_file(
            &root.join("sessions/s1/vdo_cache/web/c.mp4"),
            &[0; 30],
        );

        let report = scan_usage_at(root).unwrap();

        assert_eq!(report.total_bytes, 100 + 50 + 20 + 10 + 30);
        assert_eq!(report.sections.len(), 3);
        assert_eq!(report.sections[0].id, "database");
        assert_eq!(report.sections[0].bytes, 150);
        assert_eq!(report.sections[1].id, "config");
        assert_eq!(report.sections[2].id, "cache");
        let cache_children = report.sections[2].children.as_ref().unwrap();
        assert_eq!(cache_children[0].local.as_ref().unwrap().logical_bytes, 10);
        assert_eq!(cache_children[0].web.as_ref().unwrap().bytes, 20);
        assert_eq!(cache_children[1].web.as_ref().unwrap().bytes, 30);
    }
}
