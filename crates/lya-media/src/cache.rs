//! 会话级媒体缓存：`~/.lya/sessions/{id}/img_cache/{local,web}/`。
//!
//! 聊天里的图片走这里，而不是每次读原路径或直连外网——本地文件被移动后仍能显示，
//! 远程图也只抓一次。后续 `vdo_cache` / `ado_cache` 沿用同一目录约定。

use std::io;
use std::path::{Path, PathBuf};
use lya_config::data_root;
use lya_http::HttpClient;
use lya_tool::tools::web::net::{Reach, classify_literal, classify_resolved, split_host_port};
use sha2::{Digest, Sha256};

/// 图片 serving 限制（来自 `runtime.toml` 的 `[media.image]`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaLimits {
    /// 单张图片大小上限（字节）。
    pub max_image_bytes: u64,
    /// 是否写入 `img_cache/local`。
    pub cache_local: bool,
    /// 是否写入 `img_cache/web`（关闭时仍临时拉取，但不进持久缓存目录）。
    pub cache_web: bool,
}

impl Default for MediaLimits {
    fn default() -> Self {
        Self {
            max_image_bytes: 32 * 1024 * 1024,
            cache_local: true,
            cache_web: true,
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
    /// 复制到剪贴板用的本地绝对路径；远程图为 `None`。
    pub copy_path: Option<String>,
    /// 复制到剪贴板用的原始 URL；本地图为 `None`。
    pub copy_url: Option<String>,
    /// 建议下载文件名。
    pub filename: String,
    /// `local` 或 `web`。
    pub kind: &'static str,
}

/// 会话 `img_cache` 根目录。
pub fn cache_root(session_id: &str) -> Result<PathBuf, MediaCacheError> {
    if session_id.is_empty() || session_id.contains('/') || session_id.contains('\\') {
        return Err(MediaCacheError::Invalid("session id 无效".into()));
    }
    Ok(data_root()
        .map_err(|err| MediaCacheError::Invalid(err.to_string()))?
        .join("sessions")
        .join(session_id)
        .join("img_cache"))
}

fn kind_dir(session_id: &str, kind: &str) -> Result<PathBuf, MediaCacheError> {
    Ok(cache_root(session_id)?.join(kind))
}

fn cache_key(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn mime_of(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_lowercase();
    Some(match extension.as_str() {
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

fn validate_home_file(path: &Path, max_bytes: u64) -> Result<PathBuf, MediaCacheError> {
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
    if mime_of(&real).is_none() {
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

/// 确保本地图片已缓存并返回元数据。
pub fn ensure_local(
    session_id: &str,
    source_path: &str,
    limits: MediaLimits,
) -> Result<CachedMedia, MediaCacheError> {
    let path = PathBuf::from(source_path);
    let real = validate_home_file(&path, limits.max_image_bytes)?;
    let mime = mime_of(&real).ok_or(MediaCacheError::Unsupported)?;
    let ext = extension_of(&real);
    let filename = real
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string();

    if limits.cache_local {
        let key = cache_key(source_path);
        let dest = kind_dir(session_id, "local")?.join(format!("{key}{ext}"));
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

fn guess_ext(content_type: Option<&str>, url: &str) -> String {
    if let Some(ct) = content_type {
        let lower = ct.to_ascii_lowercase();
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
    extension_of(Path::new(url.trim().split(['?', '#']).next().unwrap_or("")))
}

fn filename_from_url(url: &str, ext: &str) -> String {
    let tail = url.trim().split(['?', '#']).next().unwrap_or("image");
    let name = Path::new(tail)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .unwrap_or("image");
    if Path::new(name).extension().is_some() {
        name.to_string()
    } else {
        format!("{name}{ext}")
    }
}

/// 确保远程图片已下载并返回元数据。
pub async fn ensure_web(
    session_id: &str,
    url: &str,
    http: &HttpClient,
    self_port: u16,
    limits: MediaLimits,
) -> Result<CachedMedia, MediaCacheError> {
    let url = url.trim();
    ensure_public_url(url, self_port).await?;

    let key = cache_key(url);
    let dir = kind_dir(session_id, "web")?;
    let cache_dir = if limits.cache_web {
        dir.clone()
    } else {
        dir.join(".ephemeral")
    };
    std::fs::create_dir_all(&cache_dir)?;

    // 已有缓存：找同 key 前缀的文件
    if let Ok(read_dir) = std::fs::read_dir(&cache_dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&key) {
                let path = entry.path();
                if let Some(mime) = mime_of(&path) {
                    return Ok(CachedMedia {
                        filename: filename_from_url(url, &extension_of(&path)),
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

    if bytes.len() as u64 > limits.max_image_bytes {
        return Err(MediaCacheError::TooLarge);
    }

    let ext = guess_ext(content_type.as_deref(), url);
    let dest = cache_dir.join(format!("{key}{ext}"));
    if !dest.exists() {
        std::fs::write(&dest, &bytes)?;
    }

    let mime = mime_of(&dest).ok_or(MediaCacheError::Unsupported)?;
    Ok(CachedMedia {
        path: dest,
        copy_path: None,
        copy_url: Some(url.to_string()),
        filename: filename_from_url(url, &ext),
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
        assert!(cache_root("../x").is_err());
        assert!(cache_root("abc/def").is_err());
    }
}
