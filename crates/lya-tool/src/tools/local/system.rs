//! `system_info`：一次性交代清楚这台机器长什么样。
//!
//! 存在的意义是**省掉模型反复探测环境**——没有它，模型想知道装没装 `rg` 就得
//! 跑一条命令，想知道发行版又得跑一条，每条都要用户看着一个 shell 调用发呆。
//!
//! 实现上全部走 `/proc` 与 `/etc` 的文件读取和 `$PATH` 扫描，不起子进程，
//! 所以是纯只读工具（`-R-`），ask 模式也能用。

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::meta::{ToolMeta, ToolResult};
use crate::permission::Permission;
use crate::traits::{Tool, ToolCallFuture};

/// 要探测存在性的常用命令行工具。
const PROBED_COMMANDS: &[&str] = &[
    "git", "rg", "fd", "fzf", "curl", "wget", "jq", "docker", "podman", "systemctl", "pacman",
    "apt", "dnf", "python3", "node", "npm", "cargo", "rustc", "go", "ffmpeg", "nvim", "tmux",
];

/// `system_info` 工具。
pub struct SystemInfoTool {
    /// 静态 meta。
    meta: ToolMeta,
    /// OpenAI `parameters` JSON Schema。
    parameters: Value,
    /// 用法说明。
    prompt_hint: &'static str,
}

impl SystemInfoTool {
    /// 构造工具实例。
    pub fn new() -> Self {
        Self {
            meta: ToolMeta::new(
                "system_info",
                "系统信息",
                "获取本机环境：发行版、内核、CPU、内存、shell，以及常用命令行工具装了哪些",
                Permission::READ,
            ),
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            prompt_hint: concat!(
                "使用 system_info 了解运行环境：\n",
                "1) 需要判断「这台机器有没有装某个工具」「是什么发行版」时先调它，",
                "不要用一堆 shell 命令逐个探测——那样又慢又要用户逐条放行。\n",
                "2) 一次调用就够了，同一轮内不要重复调；结果在本次对话里不会变。\n",
                "3) 它只报告常见工具的有无。要查列表之外的命令，仍需自己想办法确认。"
            ),
        }
    }
}

impl Default for SystemInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for SystemInfoTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.parameters
    }

    fn prompt_hint(&self) -> &str {
        self.prompt_hint
    }

    fn call(&self, _args: Value) -> ToolCallFuture<'_> {
        Box::pin(async move { ToolResult::ok(collect()) })
    }
}

/// 汇总一份环境报告。
fn collect() -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "系统: {}\n",
        os_pretty_name().unwrap_or_else(|| std::env::consts::OS.to_string())
    ));
    if let Some(kernel) = read_trimmed("/proc/sys/kernel/osrelease") {
        out.push_str(&format!("内核: {kernel}\n"));
    }
    out.push_str(&format!("架构: {}\n", std::env::consts::ARCH));

    if let Some((model, cores)) = cpu_info() {
        out.push_str(&format!("CPU: {model}（{cores} 逻辑核）\n"));
    }
    if let Some((total, available)) = memory_info() {
        out.push_str(&format!("内存: 共 {total}，可用 {available}\n"));
    }

    if let Some(shell) = std::env::var_os("SHELL") {
        out.push_str(&format!("Shell: {}\n", shell.to_string_lossy()));
    }
    if let Some(home) = std::env::var_os("HOME") {
        out.push_str(&format!("家目录: {}\n", home.to_string_lossy()));
    }

    let (found, missing) = probe_commands();
    out.push_str(&format!("\n已安装: {}\n", join_or_dash(&found)));
    out.push_str(&format!("未找到: {}\n", join_or_dash(&missing)));
    out
}

/// 从 `/etc/os-release` 取发行版名称。
fn os_pretty_name() -> Option<String> {
    let text = fs::read_to_string("/etc/os-release").ok()?;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

/// CPU 型号与逻辑核数。
fn cpu_info() -> Option<(String, usize)> {
    let text = fs::read_to_string("/proc/cpuinfo").ok()?;
    let mut model = None;
    let mut cores = 0;
    for line in text.lines() {
        if line.starts_with("processor") {
            cores += 1;
        } else if model.is_none()
            && let Some((_, value)) = line.split_once(':')
            && line.starts_with("model name")
        {
            model = Some(value.trim().to_string());
        }
    }
    Some((model.unwrap_or_else(|| "未知".into()), cores.max(1)))
}

/// 内存总量与可用量。
fn memory_info() -> Option<(String, String)> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = None;
    let mut available = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = parse_kib(rest);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = parse_kib(rest);
        }
    }
    Some((total?, available?))
}

/// `/proc/meminfo` 里形如 `  16316420 kB` 的值。
fn parse_kib(text: &str) -> Option<String> {
    let kib: u64 = text.split_whitespace().next()?.parse().ok()?;
    Some(crate::tools::local::file::manage::human_size(kib * 1024))
}

/// 扫 `$PATH` 判断命令是否存在，不起子进程。
fn probe_commands() -> (Vec<&'static str>, Vec<&'static str>) {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<_> = std::env::split_paths(&path).collect();

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for command in PROBED_COMMANDS {
        if dirs.iter().any(|dir| is_executable(&dir.join(command))) {
            found.push(*command);
        } else {
            missing.push(*command);
        }
    }
    (found, missing)
}

/// 存在且是文件即认为可用（不细究权限位）。
fn is_executable(path: &Path) -> bool {
    fs::metadata(path).map(|meta| meta.is_file()).unwrap_or(false)
}

fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn join_or_dash(items: &[&str]) -> String {
    if items.is_empty() {
        "（无）".into()
    } else {
        items.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_covers_the_basics() {
        let report = collect();
        assert!(report.contains("系统:"));
        assert!(report.contains("架构:"));
        assert!(report.contains("已安装:"));
        assert!(report.contains("未找到:"));
    }

    #[test]
    fn probing_partitions_every_command() {
        let (found, missing) = probe_commands();
        assert_eq!(found.len() + missing.len(), PROBED_COMMANDS.len());
        // cargo 在跑测试的环境里必然存在
        assert!(found.contains(&"cargo"), "found={found:?}");
    }

    #[test]
    fn meminfo_line_is_parsed_into_readable_size() {
        assert_eq!(parse_kib("  2048 kB").as_deref(), Some("2.0 MiB"));
        assert_eq!(parse_kib("不是数字"), None);
    }
}
