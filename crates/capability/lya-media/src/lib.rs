//! 会话级媒体留存与校验（`img_cache` / `vdo_cache` / `ado_cache`）。

#![deny(missing_docs)]

mod cache;

pub use cache::{
    CachedMedia, CategoryLimits, MediaBytes, MediaCacheError, MediaCategory, MediaLimits,
    RetainKind, Retained, cache_root, ensure_local, ensure_web, remove_session_media, session_dir,
};
