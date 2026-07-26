//! 本地路径解析。
//!
//! # 约定
//!
//! - 根目录默认是用户家目录 `$HOME`
//! - 以 `/` 开头：视为**绝对路径**，允许访问家目录之外
//! - `~` / `~/...`：展开为家目录
//! - 其余：相对家目录解析，并支持 `./`、`../` 词法归一
//! - **禁止**通过相对路径（含 `~/../`）逃出家目录；若需要访问非家目录，
//!   必须使用以 `/` 开头的绝对路径

use std::env;
use std::path::{Component, Path, PathBuf};

/// 路径解析错误。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    /// 输入为空或仅空白。
    #[error("path is empty")]
    Empty,

    /// 无法读取 `$HOME`。
    #[error("HOME is not set or invalid")]
    NoHome,

    /// 相对路径试图逃出家目录。
    #[error(
        "检测到家目录逃逸行为, 若需要访问非家目录请使用绝对路径 (以 / 开头)"
    )]
    HomeEscape,
}

/// 解析成功后的路径信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPath {
    /// 词法归一后的绝对路径（未跟随符号链接）。
    pub absolute: PathBuf,
    /// 是否以「绝对路径输入」（`/` 开头）解析；`false` 表示相对/`~` 且仍在家目录内。
    pub from_absolute_input: bool,
    /// 解析时使用的家目录。
    pub home: PathBuf,
}

/// 按约定解析用户给出的路径字符串（家目录取自 `$HOME`）。
pub fn resolve_path(input: &str) -> Result<ResolvedPath, PathError> {
    let home = home_dir()?;
    resolve_path_with_home(input, &home)
}

/// 使用指定家目录解析（便于测试与注入）。
pub fn resolve_path_with_home(input: &str, home: &Path) -> Result<ResolvedPath, PathError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(PathError::Empty);
    }

    let home = lexical_normalize(home);

    if input.starts_with('/') {
        let absolute = lexical_normalize(Path::new(input));
        return Ok(ResolvedPath {
            absolute,
            from_absolute_input: true,
            home,
        });
    }

    // `~` / `~/...`
    let under_home: PathBuf = if input == "~" {
        home.clone()
    } else if let Some(rest) = input.strip_prefix("~/") {
        home.join(rest)
    } else {
        home.join(input)
    };

    let absolute = lexical_normalize(&under_home);
    if !is_within_home(&absolute, &home) {
        return Err(PathError::HomeEscape);
    }

    Ok(ResolvedPath {
        absolute,
        from_absolute_input: false,
        home,
    })
}

/// 读取 `$HOME`。
fn home_dir() -> Result<PathBuf, PathError> {
    let home = env::var_os("HOME").ok_or(PathError::NoHome)?;
    if home.is_empty() {
        return Err(PathError::NoHome);
    }
    Ok(PathBuf::from(home))
}

/// 词法归一：展开 `.` / `..`，不访问文件系统、不解析符号链接。
pub fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let is_absolute = path.is_absolute();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => {
                out.push(Component::RootDir.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                match out.components().next_back() {
                    None | Some(Component::RootDir) => {
                        // 已在根：绝对路径停在根；相对路径忽略多余 `..`
                    }
                    Some(Component::Normal(_)) => {
                        out.pop();
                    }
                    Some(_) => {
                        out.pop();
                    }
                }
            }
            Component::Normal(s) => out.push(s),
        }
    }
    if out.as_os_str().is_empty() {
        if is_absolute {
            out.push(Component::RootDir.as_os_str());
        } else {
            out.push(Component::CurDir.as_os_str());
        }
    }
    out
}

/// `path` 是否位于 `home` 之下（或等于 home）。
fn is_within_home(path: &Path, home: &Path) -> bool {
    let path = lexical_normalize(path);
    let home = lexical_normalize(home);
    path.starts_with(&home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_under_home() {
        let home = Path::new("/home/demo");
        let r = resolve_path_with_home("foo/./bar", home).unwrap();
        assert_eq!(r.absolute, PathBuf::from("/home/demo/foo/bar"));
        assert!(!r.from_absolute_input);
    }

    #[test]
    fn tilde_and_parent_inside() {
        let home = Path::new("/home/demo");
        let r = resolve_path_with_home("~/a/b/../c", home).unwrap();
        assert_eq!(r.absolute, PathBuf::from("/home/demo/a/c"));
    }

    #[test]
    fn escape_via_dotdot_forbidden() {
        let home = Path::new("/home/demo");
        assert_eq!(
            resolve_path_with_home("../etc/passwd", home).unwrap_err(),
            PathError::HomeEscape
        );
        assert_eq!(
            resolve_path_with_home("~/../etc/passwd", home).unwrap_err(),
            PathError::HomeEscape
        );
        assert_eq!(
            resolve_path_with_home("foo/../../..", home).unwrap_err(),
            PathError::HomeEscape
        );
    }

    #[test]
    fn absolute_allowed() {
        let home = Path::new("/home/demo");
        let r = resolve_path_with_home("/etc/passwd", home).unwrap();
        assert_eq!(r.absolute, PathBuf::from("/etc/passwd"));
        assert!(r.from_absolute_input);
    }
}
