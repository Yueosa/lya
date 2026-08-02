//! 从 `runtime.toml` 读取媒体限制。

use lya_config::Config;
use lya_media::MediaLimits;

/// 当前生效的 `[media.image]` 限制；读配置失败时用默认值。
pub fn image_limits() -> MediaLimits {
    Config::load()
        .map(|cfg| MediaLimits {
            max_image_bytes: cfg.runtime.media.image.max_bytes,
            cache_local: cfg.runtime.media.image.cache_local,
            cache_web: cfg.runtime.media.image.cache_web,
        })
        .unwrap_or_default()
}
