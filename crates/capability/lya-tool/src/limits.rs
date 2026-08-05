//! 内置 tool 的硬编码数值限制。
//!
//! 执行层的默认值与上限集中在此，**不**通过 `runtime.toml` 配置。
//! 前端只读展示见 `web/src/utils/toolLimits.ts`；改数值时请同步两处。

/// `bash`
pub mod bash {
    /// 默认超时（秒）。
    pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
    /// 超时上限（秒）。
    pub const MAX_TIMEOUT_SECS: u64 = 600;
    /// 单流捕获字节上限。
    pub const MAX_CAPTURE_BYTES: usize = 50 * 1024;
    /// 回灌模型的单流字符上限。
    pub const MAX_REPORT_CHARS: usize = 2000;
}

/// `file_read`
pub mod file_read {
    /// 全文模式最大行数。
    pub const MAX_FULL_LINES: usize = 2000;
    /// 全文模式最大字节。
    pub const MAX_FULL_BYTES: usize = 256 * 1024;
    /// 任意读取硬上限（字节）。
    pub const HARD_MAX_BYTES: u64 = 8 * 1024 * 1024;
}

/// `file_write`
pub mod file_write {
    /// 单次写入字节上限。
    pub const MAX_WRITE_BYTES: usize = 1024 * 1024;
}

/// `file_edit`
pub mod file_edit {
    /// 可编辑文件大小上限（字节）。
    pub const MAX_EDIT_BYTES: u64 = 8 * 1024 * 1024;
}

/// `dir_list`
pub mod dir_list {
    /// 默认递归深度。
    pub const DEFAULT_DEPTH: usize = 1;
    /// 深度上限。
    pub const MAX_DEPTH: usize = 8;
    /// 默认条目数。
    pub const DEFAULT_LIMIT: usize = 300;
    /// 条目上限。
    pub const MAX_LIMIT: usize = 2000;
}

/// `image_scan`
pub mod image_scan {
    /// 默认条目数。
    pub const DEFAULT_LIMIT: usize = 100;
    /// 条目上限。
    pub const MAX_LIMIT: usize = 1000;
    /// 递归深度上限。
    pub const MAX_DEPTH: usize = 8;
}

/// `web_search`
pub mod web_search {
    /// 默认结果数。
    pub const DEFAULT_MAX_RESULTS: usize = 8;
    /// 结果数上限。
    pub const MAX_RESULTS_CAP: usize = 20;
    /// 摘要字符数。
    pub const SNIPPET_CHARS: usize = 200;
}

/// `web_fetch`
pub mod web_fetch {
    /// 默认返回字符数。
    pub const DEFAULT_MAX_CHARS: usize = 6000;
    /// 字符数上限。
    pub const MAX_CHARS_CAP: usize = 20_000;
    /// 下载体积上限（字节）。
    pub const MAX_DOWNLOAD_BYTES: usize = 4 * 1024 * 1024;
}
