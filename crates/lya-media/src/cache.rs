//! 会话级媒体缓存：`img_cache` / `vdo_cache` / `ado_cache` 下的 `{local,web}/`。
//!
//! 聊天里的媒体走这里，而不是每次读原路径或直连外网——本地文件被移动后仍能播放，
//! 远程资源也只抓一次。

use std::io;
use std::path::{Path, PathBuf};
use lya_base::data_root;
use lya_http::HttpClient;
use lya_tool::tools::web::net::{Reach, classify_literal, classify_resolved, split_host_port};
use sha2::{Digest, Sha256};

/// 媒体大类：决定缓存目录与 MIME 白名单。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCategory {
    /// 图片（`img_cache`）。
    Image,
    /// 视频（`vdo_cache`）。
    Video,
    /// 音频（`ado_cache`）。
    Audio,
}

/// 单类媒体的 serving 与留存策略。
///
/// 「留存」而不是「缓存」：缓存暗示可以随时丢弃的副本，而这两个开关决定的是
/// **要不要自己留一份**——留了，源文件被移走或远程挂掉之后照样能看。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryLimits {
    /// 单文件大小上限（字节）。
    pub max_bytes: u64,
    /// 本地媒体是否在 `{cache}/local` 留一份。关掉就直接从源路径读。
    pub retain_local: bool,
    /// 远程媒体下载后是否留在 `{cache}/web`。关掉就只过一遍内存，不落盘。
    pub retain_web: bool,
}

impl Default for CategoryLimits {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024 * 1024,
            retain_local: true,
            retain_web: true,
        }
    }
}

/// 各媒体类的 serving 限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaLimits {
    /// 图片。
    pub image: CategoryLimits,
    /// 视频。
    pub video: CategoryLimits,
    /// 音频。
    pub audio: CategoryLimits,
}

impl Default for MediaLimits {
    fn default() -> Self {
        Self {
            image: CategoryLimits {
                max_bytes: 32 * 1024 * 1024,
                ..CategoryLimits::default()
            },
            video: CategoryLimits {
                max_bytes: 512 * 1024 * 1024,
                ..CategoryLimits::default()
            },
            audio: CategoryLimits {
                max_bytes: 128 * 1024 * 1024,
                ..CategoryLimits::default()
            },
        }
    }
}

impl MediaLimits {
    /// 按大类取限制。
    pub fn for_category(self, category: MediaCategory) -> CategoryLimits {
        match category {
            MediaCategory::Image => self.image,
            MediaCategory::Video => self.video,
            MediaCategory::Audio => self.audio,
        }
    }
}

/// 缓存或读取失败。
#[derive(Debug, thiserror::Error)]
pub enum MediaCacheError {
    /// 路径/参数不合法。
    #[error("{0}")]
    Invalid(String),

    /// 文件不存在。
    #[error("not found")]
    NotFound,

    /// 不允许访问。
    #[error("forbidden")]
    Forbidden,

    /// 类型不支持。
    #[error("unsupported media type")]
    Unsupported,

    /// 过大。
    #[error("payload too large")]
    TooLarge,

    /// IO 错误。
    #[error(transparent)]
    Io(#[from] io::Error),

    /// 网络错误。
    #[error("{0}")]
    Network(String),
}

/// 留存副本是怎么落盘的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainKind {
    /// 硬链接到源文件：同一份数据两个名字，删掉释放 0 字节。
    Hardlink,
    /// 独立拷贝：删掉能真的腾出空间。
    Copy,
}

impl RetainKind {
    /// 给接口用的稳定标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hardlink => "hardlink",
            Self::Copy => "copy",
        }
    }
}

/// 盘上的留存副本。
#[derive(Debug, Clone)]
pub struct Retained {
    /// 副本位置。
    pub path: PathBuf,
    /// 硬链接还是拷贝。
    pub kind: RetainKind,
}

/// 媒体字节从哪儿取。
#[derive(Debug, Clone)]
pub enum MediaBytes {
    /// 盘上的文件：源文件，或留存副本。
    File(PathBuf),
    /// 只在内存里过一遍，没有落盘。
    Memory(Vec<u8>),
}

/// 可以拿去 serving 的媒体，以及它的来龙去脉。
#[derive(Debug, Clone)]
pub struct CachedMedia {
    /// 字节从哪儿取。
    pub bytes: MediaBytes,
    /// MIME。
    pub mime: &'static str,
    /// 本地媒体的源文件绝对路径；远程为 `None`。
    pub source_path: Option<String>,
    /// 远程媒体的原始 URL；本地为 `None`。
    pub origin_url: Option<String>,
    /// 我们自己留的那一份；没留就是 `None`。
    pub retained: Option<Retained>,
    /// 建议下载文件名。
    pub filename: String,
    /// `local` 或 `web`。
    pub kind: &'static str,
}

fn cache_dir_name(category: MediaCategory) -> &'static str {
    match category {
        MediaCategory::Image => "img_cache",
        MediaCategory::Video => "vdo_cache",
        MediaCategory::Audio => "ado_cache",
    }
}

/// 一个会话在盘上的目录。
pub fn session_dir(session_id: &str) -> Result<PathBuf, MediaCacheError> {
    if session_id.is_empty() || session_id.contains('/') || session_id.contains('\\') {
        return Err(MediaCacheError::Invalid("session id 无效".into()));
    }
    Ok(data_root()
        .map_err(|err| MediaCacheError::Invalid(err.to_string()))?
        .join("sessions")
        .join(session_id))
}

/// 会话某一类媒体的留存根目录。
pub fn cache_root(session_id: &str, category: MediaCategory) -> Result<PathBuf, MediaCacheError> {
    Ok(session_dir(session_id)?.join(cache_dir_name(category)))
}

/// 删掉一个会话在盘上的全部媒体。
///
/// 会话行删了目录还留着的话，那些图片视频音频再也没有任何界面能看到它们，
/// 只会在存储页里变成一堆无名占用。
pub fn remove_session_media(session_id: &str) -> Result<(), MediaCacheError> {
    match std::fs::remove_dir_all(session_dir(session_id)?) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn kind_dir(
    session_id: &str,
    category: MediaCategory,
    kind: &str,
) -> Result<PathBuf, MediaCacheError> {
    Ok(cache_root(session_id, category)?.join(kind))
}

fn cache_key(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn mime_of(path: &Path, category: MediaCategory) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_lowercase();
    Some(match category {
        MediaCategory::Image => match extension.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            "tiff" | "tif" => "image/tiff",
            "ico" => "image/x-icon",
            "avif" => "image/avif",
            "svg" => "image/svg+xml",
            _ => return None,
        },
        MediaCategory::Video => match extension.as_str() {
            "mp4" | "m4v" => "video/mp4",
            "webm" => "video/webm",
            "mkv" => "video/x-matroska",
            "mov" => "video/quicktime",
            "avi" => "video/x-msvideo",
            "ogv" => "video/ogg",
            _ => return None,
        },
        MediaCategory::Audio => match extension.as_str() {
            "mp3" => "audio/mpeg",
            "flac" => "audio/flac",
            "ogg" => "audio/ogg",
            "wav" => "audio/wav",
            "m4a" => "audio/mp4",
            "aac" => "audio/aac",
            "opus" => "audio/opus",
            "wma" => "audio/x-ms-wma",
            _ => return None,
        },
    })
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default()
}

fn home_dir() -> Result<PathBuf, MediaCacheError> {
    let home = std::env::var_os("HOME").ok_or_else(|| MediaCacheError::Invalid("HOME 未设置".into()))?;
    Ok(PathBuf::from(home))
}

fn validate_home_file(
    path: &Path,
    category: MediaCategory,
    max_bytes: u64,
) -> Result<PathBuf, MediaCacheError> {
    let home = home_dir()?;
    let Ok(real) = std::fs::canonicalize(path) else {
        return Err(MediaCacheError::NotFound);
    };
    let Ok(real_home) = std::fs::canonicalize(&home) else {
        return Err(MediaCacheError::Invalid("家目录不可访问".into()));
    };
    if !real.starts_with(&real_home) {
        return Err(MediaCacheError::Forbidden);
    }
    if mime_of(&real, category).is_none() {
        return Err(MediaCacheError::Unsupported);
    }
    match std::fs::metadata(&real) {
        Ok(meta) if meta.len() > max_bytes => return Err(MediaCacheError::TooLarge),
        Ok(meta) if !meta.is_file() => {
            return Err(MediaCacheError::Invalid("不是文件".into()))
        }
        Ok(_) => {}
        Err(_) => return Err(MediaCacheError::NotFound),
    }
    Ok(real)
}

/// 问文件系统这份副本到底是硬链接还是拷贝。
///
/// 不记在结构体里而是现场看 `nlink`：副本可能是上一次运行留下的，
/// 那时候用的哪种方式没人记得，而 `nlink > 1` 是当下的事实。
fn retain_kind_of(path: &Path) -> RetainKind {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if std::fs::metadata(path).is_ok_and(|meta| meta.nlink() > 1) {
            return RetainKind::Hardlink;
        }
    }
    RetainKind::Copy
}

/// 留一份到 `dest`：能硬链接就硬链接，不能就拷贝。已存在则原样保留。
fn retain_copy(source: &Path, dest: &Path) -> Result<Retained, MediaCacheError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !dest.exists() {
        #[cfg(unix)]
        let linked = std::fs::hard_link(source, dest).is_ok();
        #[cfg(not(unix))]
        let linked = false;
        if !linked {
            std::fs::copy(source, dest)?;
        }
    }
    Ok(Retained {
        kind: retain_kind_of(dest),
        path: dest.to_path_buf(),
    })
}

/// 在留存目录里按 key 前缀找已有副本。
fn find_retained(dir: &Path, key: &str, category: MediaCategory) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(key) {
            continue;
        }
        let path = entry.path();
        if mime_of(&path, category).is_some() {
            return Some(path);
        }
    }
    None
}

fn default_stem(category: MediaCategory) -> &'static str {
    match category {
        MediaCategory::Image => "image",
        MediaCategory::Video => "video",
        MediaCategory::Audio => "audio",
    }
}

fn name_of(path: &Path, category: MediaCategory) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(default_stem(category))
        .to_string()
}

/// 提供本地媒体；按策略留一份副本。
///
/// **源文件不在了，但我们留过副本，就用副本。** 「源文件消失还能看」正是留存要防的
/// 情况，为此拦下来反而把这个能力抵消掉了。其余的拒绝理由（不在家目录、格式不支持、
/// 过大）照旧生效。
pub fn ensure_local(
    session_id: &str,
    source_path: &str,
    category: MediaCategory,
    limits: CategoryLimits,
) -> Result<CachedMedia, MediaCacheError> {
    let path = PathBuf::from(source_path);
    let key = cache_key(source_path);
    let retain_dir = kind_dir(session_id, category, "local")?;

    let real = match validate_home_file(&path, category, limits.max_bytes) {
        Ok(real) => real,
        Err(MediaCacheError::NotFound) => {
            let found =
                find_retained(&retain_dir, &key, category).ok_or(MediaCacheError::NotFound)?;
            let mime = mime_of(&found, category).ok_or(MediaCacheError::Unsupported)?;
            return Ok(CachedMedia {
                mime,
                source_path: Some(source_path.to_string()),
                origin_url: None,
                retained: Some(Retained {
                    kind: retain_kind_of(&found),
                    path: found.clone(),
                }),
                filename: name_of(&path, category),
                kind: "local",
                bytes: MediaBytes::File(found),
            });
        }
        Err(err) => return Err(err),
    };

    let mime = mime_of(&real, category).ok_or(MediaCacheError::Unsupported)?;
    let filename = name_of(&real, category);
    let source = Some(real.to_string_lossy().into_owned());

    if !limits.retain_local {
        return Ok(CachedMedia {
            mime,
            source_path: source,
            origin_url: None,
            retained: None,
            filename,
            kind: "local",
            bytes: MediaBytes::File(real),
        });
    }

    let dest = retain_dir.join(format!("{key}{}", extension_of(&real)));
    let retained = retain_copy(&real, &dest)?;
    Ok(CachedMedia {
        mime,
        source_path: source,
        origin_url: None,
        filename,
        kind: "local",
        bytes: MediaBytes::File(retained.path.clone()),
        retained: Some(retained),
    })
}

fn reject_bad_scheme(url: &str) -> Option<MediaCacheError> {
    let trimmed = url.trim();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return None;
    }
    Some(MediaCacheError::Invalid("只支持 http(s) URL".into()))
}

async fn ensure_public_url(url: &str, self_port: u16) -> Result<(), MediaCacheError> {
    if let Some(err) = reject_bad_scheme(url) {
        return Err(err);
    }
    let (host, port) = split_host_port(url)
        .ok_or_else(|| MediaCacheError::Invalid("URL 无法解析".into()))?;
    match classify_literal(&host, port, self_port) {
        Reach::SelfApi => {
            return Err(MediaCacheError::Forbidden);
        }
        Reach::Private => {
            return Err(MediaCacheError::Forbidden);
        }
        Reach::Public => {}
    }
    if classify_resolved(&host, port, self_port).await != Reach::Public {
        return Err(MediaCacheError::Forbidden);
    }
    Ok(())
}

fn guess_ext(content_type: Option<&str>, url: &str, category: MediaCategory) -> String {
    if let Some(ct) = content_type {
        let lower = ct.to_ascii_lowercase();
        match category {
            MediaCategory::Image => {
                if lower.contains("jpeg") || lower.contains("jpg") {
                    return ".jpg".into();
                }
                if lower.contains("png") {
                    return ".png".into();
                }
                if lower.contains("gif") {
                    return ".gif".into();
                }
                if lower.contains("webp") {
                    return ".webp".into();
                }
                if lower.contains("svg") {
                    return ".svg".into();
                }
                if lower.contains("avif") {
                    return ".avif".into();
                }
            }
            MediaCategory::Video => {
                if lower.contains("mp4") {
                    return ".mp4".into();
                }
                if lower.contains("webm") {
                    return ".webm".into();
                }
                if lower.contains("matroska") || lower.contains("mkv") {
                    return ".mkv".into();
                }
                if lower.contains("quicktime") {
                    return ".mov".into();
                }
                if lower.contains("ogg") && lower.contains("video") {
                    return ".ogv".into();
                }
            }
            MediaCategory::Audio => {
                if lower.contains("mpeg") || lower.contains("mp3") {
                    return ".mp3".into();
                }
                if lower.contains("flac") {
                    return ".flac".into();
                }
                if lower.contains("ogg") {
                    return ".ogg".into();
                }
                if lower.contains("wav") {
                    return ".wav".into();
                }
                if lower.contains("aac") {
                    return ".aac".into();
                }
                if lower.contains("opus") {
                    return ".opus".into();
                }
                if lower.contains("mp4") || lower.contains("m4a") {
                    return ".m4a".into();
                }
            }
        }
    }
    extension_of(Path::new(url.trim().split(['?', '#']).next().unwrap_or("")))
}

fn filename_from_url(url: &str, ext: &str, category: MediaCategory) -> String {
    let tail = url.trim().split(['?', '#']).next().unwrap_or(default_stem(category));
    let name = Path::new(tail)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .unwrap_or(default_stem(category));
    if Path::new(name).extension().is_some() {
        name.to_string()
    } else {
        format!("{name}{ext}")
    }
}

/// 提供远程媒体；按策略留一份副本。
///
/// **读和写分开判断**：留存目录里有就用，不管开关当下是什么状态；只有开着的时候才把
/// 新下载的写进去。这样关掉开关只影响以后，已经留下的不会突然命中不了又重下一遍。
pub async fn ensure_web(
    session_id: &str,
    url: &str,
    category: MediaCategory,
    http: &HttpClient,
    self_port: u16,
    limits: CategoryLimits,
) -> Result<CachedMedia, MediaCacheError> {
    let url = url.trim();
    ensure_public_url(url, self_port).await?;

    let key = cache_key(url);
    let retain_dir = kind_dir(session_id, category, "web")?;

    if let Some(found) = find_retained(&retain_dir, &key, category) {
        let mime = mime_of(&found, category).ok_or(MediaCacheError::Unsupported)?;
        return Ok(CachedMedia {
            filename: filename_from_url(url, &extension_of(&found), category),
            source_path: None,
            origin_url: Some(url.to_string()),
            retained: Some(Retained {
                kind: retain_kind_of(&found),
                path: found.clone(),
            }),
            bytes: MediaBytes::File(found),
            mime,
            kind: "web",
        });
    }

    let response = http
        .send(http.get(url))
        .await
        .map_err(|err| MediaCacheError::Network(format!("请求失败：{err}")))?;

    let landed = response.url().to_string();
    if landed != url {
        if let Some((host, port)) = split_host_port(&landed) {
            if classify_resolved(&host, port, self_port).await != Reach::Public {
                return Err(MediaCacheError::Forbidden);
            }
        }
    }

    if !response.status().is_success() {
        return Err(MediaCacheError::Network(format!(
            "HTTP {}",
            response.status()
        )));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let bytes = response
        .bytes()
        .await
        .map_err(|err| MediaCacheError::Network(format!("读取响应失败：{err}")))?;

    if bytes.len() as u64 > limits.max_bytes {
        return Err(MediaCacheError::TooLarge);
    }

    let ext = guess_ext(content_type.as_deref(), url, category);
    // 先按扩展名定 MIME：不留存时也要能判断类型，而这时候盘上不会有文件
    let mime = mime_of(Path::new(&format!("x{ext}")), category)
        .ok_or(MediaCacheError::Unsupported)?;
    let filename = filename_from_url(url, &ext, category);

    if !limits.retain_web {
        return Ok(CachedMedia {
            bytes: MediaBytes::Memory(bytes.to_vec()),
            source_path: None,
            origin_url: Some(url.to_string()),
            retained: None,
            filename,
            mime,
            kind: "web",
        });
    }

    std::fs::create_dir_all(&retain_dir)?;
    let dest = retain_dir.join(format!("{key}{ext}"));
    if !dest.exists() {
        std::fs::write(&dest, &bytes)?;
    }
    Ok(CachedMedia {
        source_path: None,
        origin_url: Some(url.to_string()),
        retained: Some(Retained {
            kind: retain_kind_of(&dest),
            path: dest.clone(),
        }),
        bytes: MediaBytes::File(dest),
        filename,
        mime,
        kind: "web",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable() {
        assert_eq!(cache_key("/a/b.png"), cache_key("/a/b.png"));
        assert_ne!(cache_key("/a/b.png"), cache_key("/a/c.png"));
    }

    #[test]
    fn session_id_rejects_path_traversal() {
        assert!(cache_root("../x", MediaCategory::Image).is_err());
        assert!(cache_root("abc/def", MediaCategory::Video).is_err());
    }

    #[test]
    fn mime_by_category() {
        assert_eq!(
            mime_of(Path::new("a.mp4"), MediaCategory::Video),
            Some("video/mp4")
        );
        assert_eq!(
            mime_of(Path::new("a.mp3"), MediaCategory::Audio),
            Some("audio/mpeg")
        );
        assert_eq!(mime_of(Path::new("a.mp4"), MediaCategory::Image), None);
    }

    #[test]
    fn hardlinked_retain_is_reported_as_such() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.png");
        std::fs::write(&source, [0u8; 8]).unwrap();
        let dest = dir.path().join("retain/a.png");

        let retained = retain_copy(&source, &dest).unwrap();

        assert_eq!(retained.kind, RetainKind::Hardlink);
        assert_eq!(retained.kind.as_str(), "hardlink");
        // 再来一次不该重写，也不该改判
        assert_eq!(retain_copy(&source, &dest).unwrap().kind, RetainKind::Hardlink);
    }

    #[test]
    fn plain_copy_is_reported_as_copy() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("web/a.png");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, [0u8; 8]).unwrap();

        assert_eq!(retain_kind_of(&dest), RetainKind::Copy);
    }

    #[test]
    fn retained_lookup_matches_by_key_and_type() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("abcd1234.png"), [0u8; 4]).unwrap();
        std::fs::write(dir.path().join("abcd1234.txt"), [0u8; 4]).unwrap();

        assert_eq!(
            find_retained(dir.path(), "abcd1234", MediaCategory::Image),
            Some(dir.path().join("abcd1234.png")),
            "同 key 下只认得出格式的那个"
        );
        assert_eq!(find_retained(dir.path(), "ffff", MediaCategory::Image), None);
        assert_eq!(
            find_retained(dir.path(), "abcd1234", MediaCategory::Video),
            None
        );
    }
}
