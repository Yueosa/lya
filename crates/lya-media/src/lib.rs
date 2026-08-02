//! 会话级媒体缓存与校验（`img_cache` / 预留 vdo·ado）。

#![deny(missing_docs)]

mod cache;

pub use cache::{
    CachedMedia, MediaCacheError, MediaLimits, cache_root, ensure_local, ensure_web,
};
