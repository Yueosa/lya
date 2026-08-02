//! 会话级媒体缓存：`img_cache` / `vdo_cache` / `ado_cache` 下的 `{local,web}/`。
//!
//! 聊天里的媒体走这里，而不是每次读原路径或直连外网——本地文件被移动后仍能播放，
//! 远程资源也只抓一次。

use std::io;
use std::path::{Path, PathBuf};
use lya_config::data_root;
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

/// 单类媒体的 serving 限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryLimits {
    /// 单文件大小上限（字节）。
    pub max_bytes: u64,
    /// 是否写入 `{cache}/local`。
    pub cache_local: bool,
    /// 是否写入 `{cache}/web`（关闭时仍临时拉取，但不进持久 web 目录）。
    pub cache_web: bool,
}

impl Default for CategoryLimits {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024 * 1024,
            cache_local: true,
            cache_web: true,
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
                cache_local: true,
                cache_web: true,
            },
            video: CategoryLimits {
                max_bytes: 512 * 1024 * 1024,
                cache_local: true,
                cache_web: true,
            },
            audio: CategoryLimits {
                max_bytes: 128 * 1024 * 1024,
                cache_local: true,
                cache_web: true,
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

/// 已缓存或可读取的媒体。
#[derive(Debug, Clone)]
pub struct CachedMedia {
    /// 磁盘上的缓存文件。
    pub path: PathBuf,
    /// MIME。
    pub mime: &'static str,
    /// 复制用的本地绝对路径；远程为 `None`。
    pub copy_path: Option<String>,
    /// 复制用的原始 URL；本地为 `None`。
    pub copy_url: Option<String>,
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

/// 会话媒体缓存根目录。
pub fn cache_root(session_id: &str, category: MediaCategory) -> Result<PathBuf, MediaCacheError> {
    if session_id.is_empty() || session_id.contains('/') || session_id.contains('\\') {
        return Err(MediaCacheError::Invalid("session id 无效".into()));
    }
    Ok(data_root()
        .map_err(|err| MediaCacheError::Invalid(err.to_string()))?
        .join("sessions")
        .join(session_id)
        .join(cache_dir_name(category)))
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

fn write_cached(source: &Path, dest: &Path) -> Result<(), MediaCacheError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if dest.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        if std::fs::hard_link(source, dest).is_ok() {
            return Ok(());
        }
    }
    std::fs::copy(source, dest)?;
    Ok(())
}

fn default_stem(category: MediaCategory) -> &'static str {
    match category {
        MediaCategory::Image => "image",
        MediaCategory::Video => "video",
        MediaCategory::Audio => "audio",
    }
}

/// 确保本地媒体已缓存并返回元数据。
pub fn ensure_local(
    session_id: &str,
    source_path: &str,
    category: MediaCategory,
    limits: CategoryLimits,
) -> Result<CachedMedia, MediaCacheError> {
    let path = PathBuf::from(source_path);
    let real = validate_home_file(&path, category, limits.max_bytes)?;
    let mime = mime_of(&real, category).ok_or(MediaCacheError::Unsupported)?;
    let ext = extension_of(&real);
    let filename = real
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(default_stem(category))
        .to_string();

    if limits.cache_local {
        let key = cache_key(source_path);
        let dest = kind_dir(session_id, category, "local")?.join(format!("{key}{ext}"));
        write_cached(&real, &dest)?;
        Ok(CachedMedia {
            path: dest,
            mime,
            copy_path: Some(real.to_string_lossy().into_owned()),
            copy_url: None,
            filename,
            kind: "local",
        })
    } else {
        Ok(CachedMedia {
            path: real,
            mime,
            copy_path: Some(path.to_string_lossy().into_owned()),
            copy_url: None,
            filename,
            kind: "local",
        })
    }
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

/// 确保远程媒体已下载并返回元数据。
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
    let dir = kind_dir(session_id, category, "web")?;
    let cache_dir = if limits.cache_web {
        dir.clone()
    } else {
        dir.join(".ephemeral")
    };
    std::fs::create_dir_all(&cache_dir)?;

    if let Ok(read_dir) = std::fs::read_dir(&cache_dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&key) {
                let path = entry.path();
                if let Some(mime) = mime_of(&path, category) {
                    return Ok(CachedMedia {
                        filename: filename_from_url(url, &extension_of(&path), category),
                        copy_path: None,
                        copy_url: Some(url.to_string()),
                        path,
                        mime,
                        kind: "web",
                    });
                }
            }
        }
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
    let dest = cache_dir.join(format!("{key}{ext}"));
    if !dest.exists() {
        std::fs::write(&dest, &bytes)?;
    }

    let mime = mime_of(&dest, category).ok_or(MediaCacheError::Unsupported)?;
    Ok(CachedMedia {
        path: dest,
        copy_path: None,
        copy_url: Some(url.to_string()),
        filename: filename_from_url(url, &ext, category),
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
}
