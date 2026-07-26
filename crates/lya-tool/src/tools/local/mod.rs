//! 本地工具（相对 `$HOME` 的路径约定、文件读写等）。

pub mod file;
pub mod path;

pub use file::FileReadTool;
pub use path::{resolve_path, resolve_path_with_home, PathError, ResolvedPath};
