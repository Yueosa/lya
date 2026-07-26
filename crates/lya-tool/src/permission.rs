//! 工具权限：R / W / X。
//!
//! 展示与配置文本形如：
//! - `-R-`：只读
//! - `-R-W-`：读写
//! - `-R-W-X-`：读写执行
//!
//! 筛选规则：**工具权限必须是允许权限的子集**
//!（`tool.permissions ⊆ allowed`）。例如模式只允许 `R` 时，
//! 带 `W` 或 `X` 的工具不可见。

use std::fmt;
use std::str::FromStr;

use crate::error::ToolError;

/// 工具权限位（可组合）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Permission(u8);

impl Permission {
    /// 读（Read）：读取文件、查询状态等无破坏操作。
    pub const READ: Self = Self(0b001);
    /// 写（Write）：修改文件、改配置、改外部状态等。
    pub const WRITE: Self = Self(0b010);
    /// 执行（eXec）：跑命令、启动进程等主动副作用。
    pub const EXEC: Self = Self(0b100);

    /// 无权限（占位 / 过滤结果为空时用）。
    pub const NONE: Self = Self(0);
    /// `-R-`
    pub const READ_ONLY: Self = Self::READ;
    /// `-R-W-`
    pub const READ_WRITE: Self = Self(Self::READ.0 | Self::WRITE.0);
    /// `-R-W-X-`
    pub const READ_WRITE_EXEC: Self =
        Self(Self::READ.0 | Self::WRITE.0 | Self::EXEC.0);

    /// 从原始位构造（主要用于测试）。
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & 0b111)
    }

    /// 原始位。
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// 是否包含读。
    pub const fn has_read(self) -> bool {
        self.0 & Self::READ.0 != 0
    }

    /// 是否包含写。
    pub const fn has_write(self) -> bool {
        self.0 & Self::WRITE.0 != 0
    }

    /// 是否包含执行。
    pub const fn has_exec(self) -> bool {
        self.0 & Self::EXEC.0 != 0
    }

    /// 并集。
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// 交集。
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// `self` 是否为 `allowed` 的子集（筛选用）。
    ///
    /// 即工具声明的每一位都必须落在允许集合内。
    pub const fn is_subset_of(self, allowed: Self) -> bool {
        self.0 & allowed.0 == self.0
    }

    /// 格式化为 `-R-W-X-` / `-R-` / `-R-W-` 等。
    ///
    /// 只输出实际拥有的位，两侧加 `-`，位之间用 `-` 连接。
    /// 全无权限时为 `---`（极少见）。
    pub fn to_prmt_string(self) -> String {
        let present: Vec<&str> = [
            self.has_read().then_some("R"),
            self.has_write().then_some("W"),
            self.has_exec().then_some("X"),
        ]
        .into_iter()
        .flatten()
        .collect();

        if present.is_empty() {
            return "---".to_string();
        }
        format!("-{}-", present.join("-"))
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_prmt_string())
    }
}

impl FromStr for Permission {
    type Err = ToolError;

    /// 解析 `-R-`、`-R-W-`、`-R-W-X-`、`R`、`RW`、`RWX` 等。
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw = s.trim();
        if raw.is_empty() {
            return Err(ToolError::InvalidPermission(s.to_string()));
        }

        let mut bits = 0u8;
        for ch in raw.chars() {
            match ch {
                'R' | 'r' => bits |= Self::READ.0,
                'W' | 'w' => bits |= Self::WRITE.0,
                'X' | 'x' => bits |= Self::EXEC.0,
                '-' | '_' | ' ' | '|' => {}
                other => {
                    return Err(ToolError::InvalidPermission(format!(
                        "unknown permission char `{other}` in `{s}`"
                    )));
                }
            }
        }
        Ok(Self(bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_and_parse() {
        assert_eq!(Permission::READ.to_prmt_string(), "-R-");
        assert_eq!(Permission::READ_WRITE.to_prmt_string(), "-R-W-");
        assert_eq!(Permission::READ_WRITE_EXEC.to_prmt_string(), "-R-W-X-");
        assert_eq!("-R-".parse::<Permission>().unwrap(), Permission::READ);
        assert_eq!(
            "-R-W-X-".parse::<Permission>().unwrap(),
            Permission::READ_WRITE_EXEC
        );
        assert_eq!("RW".parse::<Permission>().unwrap(), Permission::READ_WRITE);
    }

    #[test]
    fn subset_filter() {
        assert!(Permission::READ.is_subset_of(Permission::READ));
        assert!(Permission::READ.is_subset_of(Permission::READ_WRITE));
        assert!(!Permission::READ_WRITE.is_subset_of(Permission::READ));
        assert!(!Permission::READ_WRITE_EXEC.is_subset_of(Permission::READ_WRITE));
    }
}
