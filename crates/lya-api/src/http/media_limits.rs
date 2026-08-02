//! 从 `runtime.toml` 读取媒体限制。

use lya_config::Config;
use lya_media::{CategoryLimits, MediaLimits};

/// 当前生效的 `[media.*]` 限制；读配置失败时用默认值。
pub fn media_limits() -> MediaLimits {
    Config::load()
        .map(|cfg| MediaLimits {
            image: CategoryLimits {
                max_bytes: cfg.runtime.media.image.max_bytes,
                cache_local: cfg.runtime.media.image.cache_local,
                cache_web: cfg.runtime.media.image.cache_web,
            },
            video: CategoryLimits {
                max_bytes: cfg.runtime.media.video.max_bytes,
                cache_local: cfg.runtime.media.video.cache_local,
                cache_web: cfg.runtime.media.video.cache_web,
            },
            audio: CategoryLimits {
                max_bytes: cfg.runtime.media.audio.max_bytes,
                cache_local: cfg.runtime.media.audio.cache_local,
                cache_web: cfg.runtime.media.audio.cache_web,
            },
        })
        .unwrap_or_default()
}

/// 图片限制（`local-image` 与会话 image 端点）。
pub fn image_limits() -> CategoryLimits {
    media_limits().image
}

/// 视频限制。
pub fn video_limits() -> CategoryLimits {
    media_limits().video
}

/// 音频限制。
pub fn audio_limits() -> CategoryLimits {
    media_limits().audio
}
