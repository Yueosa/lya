//! 扫描 `data_root()` 体积分项，供存储页只读展示。
//!
//! # 为什么每个节点都带一整组数字
//!
//! 「这个目录占多少」不是一个数。本地媒体缓存优先用硬链接，指向用户原来的文件——
//! 那些条目 `ls -l` 看着有 86 MB，删掉释放 0 字节。只报一个数就必然在骗人：报逐
//! 文件之和会让人以为清一下能腾出 169 MB，报 inode 去重后的值又看不出「有东西
//! 在别处共用」。所以每个节点都给出 [`DiskUsage`] 一整组，让界面能说清哪部分是
//! 真占盘、哪部分只是别人的影子。
//!
//! 树是**齐整**的：每个节点都是「一组数字 + 可选子节点」，没有「缓存节点额外挂
//! 两个特殊字段」这种例外。之前那种例外让前端没法统一处理折叠，Local/Web 两行
//! 永远强制展开。

#![deny(missing_docs)]

use std::collections::HashMap;
use std::fs::Metadata;
use std::path::Path;

use lya_base::data_root;
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

/// 一批文件的占用情况。
#[derive(Debug, Clone, Default, Serialize)]
pub struct DiskUsage {
    /// 逐文件 size 之和。硬链接进来的文件在这里按全尺寸算。
    pub logical_bytes: u64,
    /// 按 inode 去重后的占用，也就是这批文件在磁盘上实际压了多少。
    pub physical_bytes: u64,
    /// 删掉这批文件真能腾出来的字节：所有硬链接都在扫描范围内的那部分。
    pub reclaimable_bytes: u64,
    /// 与扫描范围之外共用 inode 的字节。删了不会腾出空间，原文件还在。
    pub shared_bytes: u64,
    /// 文件数。
    pub file_count: u64,
    /// 其中 `nlink > 1` 的文件数。
    pub linked_file_count: u64,
}

/// 树形占用节点。
#[derive(Debug, Clone, Serialize)]
pub struct UsageSection {
    /// 节点 id，前端拿它记折叠状态。
    pub id: String,
    /// 展示名。
    pub label: String,
    /// 该节点合计。
    pub usage: DiskUsage,
    /// 子节点。叶子为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<UsageSection>>,
}

/// 占用汇总。
#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    /// 数据根目录。
    pub root: String,
    /// 整个数据目录的合计。分类之和加上「其它」等于它。
    pub usage: DiskUsage,
    /// 顶层分类树。
    pub sections: Vec<UsageSection>,
}

const CACHE_DIRS: [(&str, &str, &str); 3] = [
    ("cache.image", "图片", "img_cache"),
    ("cache.video", "视频", "vdo_cache"),
    ("cache.audio", "音频", "ado_cache"),
];

/// `~/.lya/theme/{主题}/` 下的分类目录，和主题素材 API 的 `kind` 对齐。
const THEME_KINDS: [(&str, &str); 2] = [("home", "加载图"), ("cg", "记忆大厅")];

/// 扫描 `~/.lya`（或 `data_root()`）占用。
pub fn scan_usage() -> Result<UsageReport, StorageError> {
    let root = data_root().map_err(|err| StorageError::Invalid(err.to_string()))?;
    scan_usage_at(&root)
}

fn scan_usage_at(root: &Path) -> Result<UsageReport, StorageError> {
    let mut whole = Scan::default();
    whole.scan_dir(root);
    let usage = whole.finish();

    let (database, config) = scan_root_files(root);
    let cache = scan_cache(&root.join("sessions"));
    let theme = scan_theme(&root.join("theme"));

    let mut sections = vec![database, config, cache, theme];

    // 「其它」是兜底项：分类规则漏掉的文件不能凭空消失，否则分类之和对不上总数，
    // 而对不上的时候没人分得清是漏扫了还是算错了。
    let counted = sum_usage(sections.iter().map(|section| &section.usage));
    if usage.physical_bytes > counted.physical_bytes {
        let physical = usage.physical_bytes - counted.physical_bytes;
        sections.push(UsageSection {
            id: "other".into(),
            label: "其它".into(),
            usage: DiskUsage {
                logical_bytes: usage.logical_bytes.saturating_sub(counted.logical_bytes),
                physical_bytes: physical,
                // 只有本地媒体缓存会硬链接，剩下的散落文件都是独立占盘的
                reclaimable_bytes: physical,
                shared_bytes: 0,
                file_count: usage.file_count.saturating_sub(counted.file_count),
                linked_file_count: 0,
            },
            children: None,
        });
    }

    Ok(UsageReport {
        root: root.to_string_lossy().into_owned(),
        usage,
        sections,
    })
}

fn scan_root_files(root: &Path) -> (UsageSection, UsageSection) {
    let mut db_files: Vec<(String, DiskUsage)> = Vec::new();
    let mut cfg_files: Vec<(String, DiskUsage)> = Vec::new();

    if let Ok(read) = std::fs::read_dir(root) {
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let mut scan = Scan::default();
            scan.add(&meta);
            if is_database_file(name) {
                db_files.push((name.to_string(), scan.finish()));
            } else if name.ends_with(".toml") {
                cfg_files.push((name.to_string(), scan.finish()));
            }
        }
    }

    db_files.sort_by(|a, b| a.0.cmp(&b.0));
    cfg_files.sort_by(|a, b| a.0.cmp(&b.0));

    (
        group_databases(&db_files),
        section_from_files("config", "配置文件", &cfg_files),
    )
}

/// 一个 `.db` 与它的 `-wal` / `-shm` 是同一个库的三个文件，列成三行只是噪音。
fn group_databases(files: &[(String, DiskUsage)]) -> UsageSection {
    let mut groups: Vec<(String, Vec<(String, DiskUsage)>)> = Vec::new();
    for (name, usage) in files {
        let key = database_group(name);
        match groups.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, members)) => members.push((name.clone(), usage.clone())),
            None => groups.push((key, vec![(name.clone(), usage.clone())])),
        }
    }

    let children: Vec<UsageSection> = groups
        .into_iter()
        .map(|(key, members)| {
            // 只有一个文件时不必再套一层，套了反而多一次点击
            if members.len() == 1 {
                let (name, usage) = members.into_iter().next().expect("len == 1");
                return UsageSection {
                    id: format!("database.{name}"),
                    label: name,
                    usage,
                    children: None,
                };
            }
            section_from_files(&format!("database.{key}"), &key, &members)
        })
        .collect();

    UsageSection {
        usage: sum_usage(children.iter().map(|child| &child.usage)),
        id: "database".into(),
        label: "数据库".into(),
        children: (!children.is_empty()).then_some(children),
    }
}

/// `lya.db-wal` → `lya.db`；`lya.db.bak-…` 是另一个库，保持独立。
fn database_group(name: &str) -> String {
    for sidecar in ["-wal", "-shm", "-journal"] {
        if let Some(base) = name.strip_suffix(sidecar) {
            if base.ends_with(".db") {
                return base.to_string();
            }
        }
    }
    name.to_string()
}

fn section_from_files(id: &str, label: &str, files: &[(String, DiskUsage)]) -> UsageSection {
    let children: Vec<UsageSection> = files
        .iter()
        .map(|(name, usage)| UsageSection {
            id: format!("{id}.{name}"),
            label: name.clone(),
            usage: usage.clone(),
            children: None,
        })
        .collect();
    UsageSection {
        usage: sum_usage(children.iter().map(|child| &child.usage)),
        id: id.into(),
        label: label.into(),
        children: (!children.is_empty()).then_some(children),
    }
}

fn is_database_file(name: &str) -> bool {
    // `.db.bak-before-lianclaw-migrate` 这类备份也是数据库，以前被归进「其它」，
    // 存储页上就成了一坨没人认得的匿名占用
    name.ends_with(".db") || name.contains(".db-") || name.contains(".db.")
}

fn scan_cache(sessions: &Path) -> UsageSection {
    let children: Vec<UsageSection> = CACHE_DIRS
        .iter()
        .map(|(id, label, dir_name)| {
            let mut local = Scan::default();
            let mut web = Scan::default();
            scan_session_cache(sessions, dir_name, &mut local, &mut web);
            let local = UsageSection {
                id: format!("{id}.local"),
                label: "本地文件".into(),
                usage: local.finish(),
                children: None,
            };
            let web = UsageSection {
                id: format!("{id}.web"),
                label: "网络下载".into(),
                usage: web.finish(),
                children: None,
            };
            UsageSection {
                usage: sum_usage([&local.usage, &web.usage]),
                id: (*id).into(),
                label: (*label).into(),
                children: Some(vec![local, web]),
            }
        })
        .collect();

    UsageSection {
        usage: sum_usage(children.iter().map(|child| &child.usage)),
        id: "cache".into(),
        label: "缓存".into(),
        children: Some(children),
    }
}

fn scan_session_cache(sessions: &Path, cache_dir: &str, local: &mut Scan, web: &mut Scan) {
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

/// `~/.lya/theme/`：按主题 id 分，再拆加载图 / 记忆大厅。
///
/// BA 的 CG 动辄几十 MB，以前全掉进「其它」，存储页上看不出是主题素材。
fn scan_theme(theme_root: &Path) -> UsageSection {
    let mut packs: Vec<(String, std::path::PathBuf)> = Vec::new();
    if let Ok(read) = std::fs::read_dir(theme_root) {
        for entry in read.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            packs.push((name.to_string(), path));
        }
    }
    packs.sort_by(|a, b| a.0.cmp(&b.0));

    let children: Vec<UsageSection> = packs
        .into_iter()
        .map(|(name, path)| scan_theme_pack(&path, &name))
        .collect();

    UsageSection {
        usage: sum_usage(children.iter().map(|child| &child.usage)),
        id: "theme".into(),
        label: "主题资源".into(),
        children: (!children.is_empty()).then_some(children),
    }
}

fn scan_theme_pack(dir: &Path, name: &str) -> UsageSection {
    let mut children: Vec<UsageSection> = THEME_KINDS
        .iter()
        .map(|(kind, label)| {
            let mut scan = Scan::default();
            scan.scan_dir(&dir.join(kind));
            UsageSection {
                id: format!("theme.{name}.{kind}"),
                label: (*label).into(),
                usage: scan.finish(),
                children: None,
            }
        })
        .collect();

    // 主题目录里若还有不在 home/cg 下的散文件，不能让它们滑回顶层「其它」
    let mut whole = Scan::default();
    whole.scan_dir(dir);
    let pack = whole.finish();
    let counted = sum_usage(children.iter().map(|child| &child.usage));
    if pack.physical_bytes > counted.physical_bytes {
        children.push(UsageSection {
            id: format!("theme.{name}.other"),
            label: "其它".into(),
            usage: DiskUsage {
                logical_bytes: pack.logical_bytes.saturating_sub(counted.logical_bytes),
                physical_bytes: pack.physical_bytes - counted.physical_bytes,
                reclaimable_bytes: pack
                    .reclaimable_bytes
                    .saturating_sub(counted.reclaimable_bytes),
                shared_bytes: pack.shared_bytes.saturating_sub(counted.shared_bytes),
                file_count: pack.file_count.saturating_sub(counted.file_count),
                linked_file_count: pack
                    .linked_file_count
                    .saturating_sub(counted.linked_file_count),
            },
            children: None,
        });
    }

    UsageSection {
        usage: pack,
        id: format!("theme.{name}"),
        label: name.into(),
        children: Some(children),
    }
}

fn sum_usage<'a>(parts: impl IntoIterator<Item = &'a DiskUsage>) -> DiskUsage {
    let mut total = DiskUsage::default();
    for part in parts {
        total.logical_bytes += part.logical_bytes;
        total.physical_bytes += part.physical_bytes;
        total.reclaimable_bytes += part.reclaimable_bytes;
        total.shared_bytes += part.shared_bytes;
        total.file_count += part.file_count;
        total.linked_file_count += part.linked_file_count;
    }
    total
}

/// inode 去重的体积累加器。
#[derive(Default)]
struct Scan {
    logical_bytes: u64,
    file_count: u64,
    linked_file_count: u64,
    inodes: HashMap<(u64, u64), Inode>,
}

struct Inode {
    size: u64,
    /// 这个 inode 在本次扫描里出现了几次。
    seen: u64,
    /// 系统里一共有几个硬链接指向它。
    nlink: u64,
}

impl Scan {
    fn scan_dir(&mut self, path: &Path) {
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            return;
        };
        if meta.is_file() {
            self.add(&meta);
            return;
        }
        // 不跟符号链接走，否则一个指回上层的链接就能让扫描转圈
        if !meta.is_dir() {
            return;
        }
        let Ok(read) = std::fs::read_dir(path) else {
            return;
        };
        for entry in read.flatten() {
            self.scan_dir(&entry.path());
        }
    }

    fn add(&mut self, meta: &Metadata) {
        let size = meta.len();
        self.logical_bytes += size;
        self.file_count += 1;
        if meta.nlink() > 1 {
            self.linked_file_count += 1;
        }
        self.inodes
            .entry((meta.dev(), meta.ino()))
            .and_modify(|inode| inode.seen += 1)
            .or_insert(Inode {
                size,
                seen: 1,
                nlink: meta.nlink(),
            });
    }

    fn finish(self) -> DiskUsage {
        let mut physical_bytes = 0;
        let mut reclaimable_bytes = 0;
        for inode in self.inodes.values() {
            physical_bytes += inode.size;
            // 扫描范围内看到的链接数 < 系统里的总链接数，说明外面还有人指着它：
            // 删掉我们这份，磁盘上的数据依然被原文件占着
            if inode.seen >= inode.nlink {
                reclaimable_bytes += inode.size;
            }
        }
        DiskUsage {
            logical_bytes: self.logical_bytes,
            physical_bytes,
            reclaimable_bytes,
            shared_bytes: physical_bytes - reclaimable_bytes,
            file_count: self.file_count,
            linked_file_count: self.linked_file_count,
        }
    }
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

    fn scan_of(path: &Path) -> DiskUsage {
        let mut scan = Scan::default();
        scan.scan_dir(path);
        scan.finish()
    }

    #[test]
    fn scan_missing_dir_is_zero() {
        assert_eq!(
            scan_of(Path::new("/nonexistent/lya-test-dir")).physical_bytes,
            0
        );
    }

    #[test]
    fn is_database_file_by_suffix() {
        assert!(is_database_file("lya.db"));
        assert!(is_database_file("lya.db-wal"));
        assert!(is_database_file("lya.db-shm"));
        assert!(is_database_file("backup.db"));
        assert!(is_database_file("lya.db.bak-before-lianclaw-migrate"));
        assert!(!is_database_file("runtime.toml"));
    }

    #[test]
    fn sidecars_group_with_their_database_backups_do_not() {
        assert_eq!(database_group("lya.db-wal"), "lya.db");
        assert_eq!(database_group("lya.db-shm"), "lya.db");
        assert_eq!(database_group("lya.db"), "lya.db");
        assert_eq!(
            database_group("lya.db.bak-before-lianclaw-migrate"),
            "lya.db.bak-before-lianclaw-migrate"
        );
    }

    #[test]
    fn links_inside_the_scan_are_reclaimable() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.bin");
        let link = dir.path().join("link.bin");
        write_file(&source, &[1, 2, 3, 4]);
        fs::hard_link(&source, &link).unwrap();

        let usage = scan_of(dir.path());

        assert_eq!(usage.file_count, 2);
        assert_eq!(usage.linked_file_count, 2);
        assert_eq!(usage.logical_bytes, 8);
        assert_eq!(usage.physical_bytes, 4);
        // 两个链接都在范围内，删掉整个目录这 4 字节就真回来了
        assert_eq!(usage.reclaimable_bytes, 4);
        assert_eq!(usage.shared_bytes, 0);
    }

    #[test]
    fn links_reaching_outside_the_scan_are_not_reclaimable() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("original.bin");
        let cache = dir.path().join("cache");
        write_file(&outside, &[0; 64]);
        fs::create_dir_all(&cache).unwrap();
        fs::hard_link(&outside, cache.join("linked.bin")).unwrap();

        // 只扫 cache/，原文件在范围外——正是本地媒体硬链接缓存的形状
        let usage = scan_of(&cache);

        assert_eq!(usage.logical_bytes, 64);
        assert_eq!(usage.physical_bytes, 64);
        assert_eq!(usage.reclaimable_bytes, 0);
        assert_eq!(usage.shared_bytes, 64);
    }

    #[test]
    fn scan_usage_tree_in_temp_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        write_file(&root.join("lya.db"), &[0; 100]);
        write_file(&root.join("lya.db-wal"), &[0; 50]);
        write_file(&root.join("lya.db.bak-old"), &[0; 7]);
        write_file(&root.join("core.toml"), b"");
        write_file(&root.join("sessions/s1/img_cache/local/a.png"), &[0; 10]);
        write_file(&root.join("sessions/s1/img_cache/web/b.png"), &[0; 20]);
        write_file(&root.join("sessions/s1/vdo_cache/web/c.mp4"), &[0; 30]);

        let report = scan_usage_at(root).unwrap();

        assert_eq!(report.usage.physical_bytes, 100 + 50 + 7 + 20 + 10 + 30);
        // 数据库 / 配置 / 缓存 / 主题资源（空也占一位，免得前端要特判缺项）
        assert_eq!(report.sections.len(), 4);

        let database = &report.sections[0];
        assert_eq!(database.id, "database");
        assert_eq!(database.usage.physical_bytes, 157);
        let db_groups = database.children.as_ref().unwrap();
        // lya.db 与它的 -wal 合成一个节点，备份自己一行
        assert_eq!(db_groups.len(), 2);
        assert_eq!(db_groups[0].label, "lya.db");
        assert_eq!(db_groups[0].usage.physical_bytes, 150);
        assert_eq!(db_groups[1].label, "lya.db.bak-old");

        assert_eq!(report.sections[1].id, "config");
        let cache = &report.sections[2];
        assert_eq!(cache.id, "cache");
        assert_eq!(cache.usage.physical_bytes, 60);
        assert_eq!(report.sections[3].id, "theme");
        assert_eq!(report.sections[3].usage.physical_bytes, 0);

        // 父节点等于子节点之和，一层层都成立
        let image = &cache.children.as_ref().unwrap()[0];
        assert_eq!(image.usage.physical_bytes, 30);
        let image_children = image.children.as_ref().unwrap();
        assert_eq!(image_children[0].usage.physical_bytes, 10);
        assert_eq!(image_children[1].usage.physical_bytes, 20);
    }

    #[test]
    fn theme_assets_are_their_own_section_not_other() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(&root.join("theme/ba/home/boot.png"), &[0; 11]);
        write_file(&root.join("theme/ba/cg/hall.mp4"), &[0; 77]);
        write_file(&root.join("theme/ba/readme.txt"), b"extra");
        write_file(&root.join("theme/mtf/home/a.jpg"), &[0; 5]);

        let report = scan_usage_at(root).unwrap();

        let theme = report
            .sections
            .iter()
            .find(|section| section.id == "theme")
            .expect("主题资源该单独成项");
        assert_eq!(theme.label, "主题资源");
        assert_eq!(theme.usage.physical_bytes, 11 + 77 + 5 + 5);
        assert!(report.sections.iter().all(|section| section.id != "other"));

        let packs = theme.children.as_ref().unwrap();
        assert_eq!(packs.len(), 2);
        assert_eq!(packs[0].id, "theme.ba");
        assert_eq!(packs[0].usage.physical_bytes, 11 + 77 + 5);
        let ba_kinds = packs[0].children.as_ref().unwrap();
        assert_eq!(ba_kinds[0].label, "加载图");
        assert_eq!(ba_kinds[0].usage.physical_bytes, 11);
        assert_eq!(ba_kinds[1].label, "记忆大厅");
        assert_eq!(ba_kinds[1].usage.physical_bytes, 77);
        assert_eq!(ba_kinds[2].label, "其它");
        assert_eq!(ba_kinds[2].usage.physical_bytes, 5);

        assert_eq!(packs[1].id, "theme.mtf");
        assert_eq!(packs[1].usage.physical_bytes, 5);
    }

    #[test]
    fn nested_web_dirs_are_counted_not_dumped_into_other() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // 扫描只看目录下的直接文件的话，任何一层嵌套里的字节都会落进「其它」，
        // 报出来就是「有 N MB 不知道是什么」
        write_file(&root.join("sessions/s1/img_cache/web/nested/x.png"), &[0; 42]);

        let report = scan_usage_at(root).unwrap();

        let cache = report
            .sections
            .iter()
            .find(|section| section.id == "cache")
            .unwrap();
        assert_eq!(cache.usage.physical_bytes, 42);
        assert!(report.sections.iter().all(|section| section.id != "other"));
    }
}
