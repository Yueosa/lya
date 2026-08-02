//! `bash`：执行 shell 命令，危险操作先请用户过目。

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};

use crate::confirm::{ConfirmRequest, ConfirmStep};
use crate::meta::{ToolMeta, ToolResult};
use crate::permission::Permission;
use crate::tools::local::path::resolve_path;
use crate::tools::shell::parse::{parse, ParsedCommand};
use crate::tools::shell::rules::{judge, pipeline_risks};
use crate::context::ToolCtx;
use crate::traits::{Tool, ToolCallFuture};

/// 默认超时秒数。
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// 超时秒数上限。
const MAX_TIMEOUT_SECS: u64 = 600;
/// 单个流最多捕获多少字节。
const MAX_CAPTURE_BYTES: usize = 50 * 1024;
/// 回给模型的每个流最多多少字符。
const MAX_REPORT_CHARS: usize = 2000;

/// 什么时候需要用户确认。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfirmPolicy {
    /// 每条命令都确认。
    Always,
    /// 已知只读的命令直接放行，其余都确认。
    #[default]
    Unknown,
    /// 只有命中风险规则才确认。
    Risky,
}

/// `bash` 工具。
pub struct BashTool {
    /// 静态 meta。
    meta: ToolMeta,
    /// OpenAI `parameters` JSON Schema。
    parameters: Value,
    /// 用法说明。
    prompt_hint: String,
    /// 确认策略。
    policy: ConfirmPolicy,
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new(ConfirmPolicy::default())
    }
}

impl BashTool {
    /// 按给定策略构造。
    pub fn new(policy: ConfirmPolicy) -> Self {
        Self {
            meta: ToolMeta::new(
                "bash",
                "执行命令",
                "在本机执行 shell 命令；危险操作会先请用户确认",
                Permission::READ_WRITE_EXEC,
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "要执行的命令。尽量一次只做一件事，不要用 && 串一长串。"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "工作目录，默认家目录。~/ 或相对路径基于家目录。"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "minimum": 1,
                        "description": format!("超时秒数，默认 {DEFAULT_TIMEOUT_SECS}，上限 {MAX_TIMEOUT_SECS}。")
                    },
                    "steps": {
                        "type": "array",
                        "description": "可选。逐段向用户解释整条命令；不提供则自动按 shell 语法拆解 command。",
                        "items": {
                            "type": "object",
                            "properties": {
                                "raw": {
                                    "type": "string",
                                    "description": "这一段的命令原文"
                                },
                                "explain": {
                                    "type": "string",
                                    "description": "给用户看的说明"
                                }
                            },
                            "required": ["raw", "explain"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
            prompt_hint: format!(
                concat!(
                    "使用 bash 执行命令：\n",
                    "1) **有专门工具就别用 bash**。读文件用 file_read、列目录用 dir_list、",
                    "改文件用 file_edit、查环境用 system_info——它们的输出更整齐，也不用打扰用户确认。\n",
                    "2) **一次只做一件事，不要用 && 串一长串**。命令会拆开逐段展示给用户过目，",
                    "一长串既难看懂也难放行；分成几次调用反而更快。\n",
                    "3) 复杂命令（管道、多条串联）请用 steps 逐段说明在做什么；",
                    "简单单条命令不必填 steps。\n",
                    "4) 不要跑交互式程序（vim、top、less、python 交互模式），",
                    "它们会一直等输入直到超时（默认 {} 秒）。\n",
                    "5) 输出可能很长，自己带上 head / tail / grep 收窄，别指望全部拿回来。\n",
                    "6) 危险命令会挂起等用户放行。被拒绝就换个做法或问清楚，不要换个写法绕过去。"
                ),
                DEFAULT_TIMEOUT_SECS
            ),
            policy,
        }
    }

    /// 按策略判断这条命令要不要确认。
    fn needs_confirm(&self, parsed: &ParsedCommand, risky: bool, all_readonly: bool) -> bool {
        match self.policy {
            ConfirmPolicy::Always => true,
            // 看不懂一律确认——解析失败正是最该拦的情况
            ConfirmPolicy::Unknown => !parsed.understood || !all_readonly,
            ConfirmPolicy::Risky => !parsed.understood || risky,
        }
    }

    /// 模型提供的逐段说明只影响确认框展示；是否确认只看整条 `command` 的解析。
    fn confirm_from_llm_steps(&self, command: &str, raw_steps: &[Value]) -> Option<ConfirmRequest> {
        let parsed = parse(command);
        let mut reasons: Vec<String> = parsed.caveats.clone();
        let mut all_readonly = !parsed.segments.is_empty();
        let mut risky = false;

        for segment in &parsed.segments {
            let verdict = judge(segment);
            all_readonly &= verdict.readonly;
            if let Some(risk) = &verdict.risk {
                risky = true;
                reasons.push(format!("{}：{risk}", segment.raw));
            }
        }
        reasons.extend(pipeline_risks(&parsed.segments));
        if !reasons.is_empty() {
            risky = true;
        }

        if !self.needs_confirm(&parsed, risky, all_readonly) {
            return None;
        }

        let mut steps = Vec::with_capacity(raw_steps.len());
        for (index, item) in raw_steps.iter().enumerate() {
            let Some(raw) = item.get("raw").and_then(Value::as_str) else {
                continue;
            };
            let explain = item
                .get("explain")
                .and_then(Value::as_str)
                .unwrap_or(raw)
                .to_string();
            steps.push(ConfirmStep {
                raw: raw.to_string(),
                explain,
                risk: None,
                connector: if index == 0 {
                    String::new()
                } else {
                    "然后".into()
                },
            });
        }

        if steps.is_empty() {
            for segment in &parsed.segments {
                let verdict = judge(segment);
                steps.push(ConfirmStep {
                    raw: segment.raw.clone(),
                    explain: verdict.explain,
                    risk: verdict.risk,
                    connector: segment.connector.label().to_string(),
                });
            }
        }

        if reasons.is_empty() {
            reasons.push("这条命令会改动系统状态".into());
        }

        Some(ConfirmRequest {
            summary: format!("执行：{command}"),
            steps,
            reasons,
        })
    }
}

impl Tool for BashTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.parameters
    }

    fn prompt_hint(&self) -> &str {
        &self.prompt_hint
    }

    fn confirm_request(&self, args: &Value) -> Option<ConfirmRequest> {
        let command = args.get("command").and_then(Value::as_str)?;
        if let Some(steps) = args.get("steps").and_then(Value::as_array) {
            if !steps.is_empty() {
                return self.confirm_from_llm_steps(command, steps);
            }
        }
        let parsed = parse(command);

        let mut steps = Vec::with_capacity(parsed.segments.len());
        let mut reasons: Vec<String> = parsed.caveats.clone();
        let mut all_readonly = !parsed.segments.is_empty();

        for segment in &parsed.segments {
            let verdict = judge(segment);
            all_readonly &= verdict.readonly;
            if let Some(risk) = &verdict.risk {
                reasons.push(format!("{}：{risk}", segment.raw));
            }
            steps.push(ConfirmStep {
                raw: segment.raw.clone(),
                explain: verdict.explain,
                risk: verdict.risk,
                connector: segment.connector.label().to_string(),
            });
        }
        reasons.extend(pipeline_risks(&parsed.segments));

        let risky = !reasons.is_empty();
        if !self.needs_confirm(&parsed, risky, all_readonly) {
            return None;
        }
        if reasons.is_empty() {
            reasons.push("这条命令会改动系统状态".into());
        }

        Some(ConfirmRequest {
            summary: format!("执行：{command}"),
            steps,
            reasons,
        })
    }

    fn call(&self, ctx: ToolCtx, args: Value) -> ToolCallFuture<'_> {
        Box::pin(async move {
            let Some(command) = args.get("command").and_then(Value::as_str) else {
                return ToolResult::err("缺少必填参数 `command`");
            };
            let cwd = match resolve_cwd(&args) {
                Ok(cwd) => cwd,
                Err(msg) => return ToolResult::err(msg),
            };
            let timeout = args
                .get("timeout_secs")
                .and_then(Value::as_u64)
                .map(|secs| secs.clamp(1, MAX_TIMEOUT_SECS))
                .unwrap_or(DEFAULT_TIMEOUT_SECS);

            run(command, &cwd, Duration::from_secs(timeout), &ctx).await
        })
    }
}

/// 解析工作目录，默认家目录。
fn resolve_cwd(args: &Value) -> Result<PathBuf, String> {
    let raw = args.get("cwd").and_then(Value::as_str).unwrap_or("~");
    let resolved = resolve_path(raw).map_err(|err| err.to_string())?;
    if !resolved.absolute.is_dir() {
        return Err(format!("工作目录 {} 不存在", resolved.absolute.display()));
    }
    Ok(resolved.absolute)
}

/// 取消标志的轮询间隔。
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// 真正执行。
///
/// 超时和取消都靠丢弃执行 future 来生效——配上 `kill_on_drop`，子进程会跟着一起
/// 收掉。轮询而不是用 channel，是因为 [`ToolCtx`] 要能被任何工具随手检查，
/// 保持成一个原子布尔最省事。
async fn run(command: &str, cwd: &PathBuf, timeout: Duration, ctx: &ToolCtx) -> ToolResult {
    if ctx.is_cancelled() {
        return ToolResult::err("已取消，命令没有执行。");
    }

    let mut builder = tokio::process::Command::new("bash");
    builder
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        // 断开标准输入：交互式程序会立刻拿到 EOF 而不是把我们挂到超时
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // 超时后 future 被丢弃，子进程要跟着一起收掉
        .kill_on_drop(true);

    let output = tokio::select! {
        result = builder.output() => match result {
            Ok(output) => output,
            Err(err) => return ToolResult::err(format!("启动命令失败：{err}")),
        },
        _ = tokio::time::sleep(timeout) => {
            return ToolResult::err(format!(
                "命令超过 {} 秒未结束，已终止。若确实耗时，调大 timeout_secs；\
                 若是交互式程序，换成非交互写法。",
                timeout.as_secs()
            ));
        }
        _ = poll_cancel(ctx) => {
            return ToolResult::err("命令被用户中止。");
        }
    };

    let code = output.status.code();
    let stdout = decode(&output.stdout);
    let stderr = decode(&output.stderr);
    ToolResult {
        success: code == Some(0),
        content: report(code, &stdout, &stderr),
    }
}

/// 等到被取消为止；没被取消就一直等下去。
async fn poll_cancel(ctx: &ToolCtx) {
    while !ctx.is_cancelled() {
        tokio::time::sleep(CANCEL_POLL).await;
    }
}

/// 字节转文本，超量先按字节截断。
fn decode(bytes: &[u8]) -> String {
    let slice = if bytes.len() > MAX_CAPTURE_BYTES {
        &bytes[..MAX_CAPTURE_BYTES]
    } else {
        bytes
    };
    String::from_utf8_lossy(slice).trim_end().to_string()
}

/// 拼出回给模型的报告。
///
/// stdout 与 stderr 分开标注：很多命令即便成功也往 stderr 写进度，混在一起的话
/// 模型分不清哪句是报错。
fn report(code: Option<i32>, stdout: &str, stderr: &str) -> String {
    let mut out = match code {
        Some(0) => "退出码: 0".to_string(),
        Some(code) => format!("退出码: {code}（非零，命令失败）"),
        None => "命令被信号终止".to_string(),
    };
    if !stdout.is_empty() {
        out.push_str(&format!("\n\n--- stdout ---\n{}", clip(stdout)));
    }
    if !stderr.is_empty() {
        out.push_str(&format!("\n\n--- stderr ---\n{}", clip(stderr)));
    }
    if stdout.is_empty() && stderr.is_empty() {
        out.push_str("\n\n（没有输出）");
    }
    out
}

/// 太长的输出保头保尾，省中间——报错通常在末尾，上下文通常在开头。
fn clip(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_REPORT_CHARS {
        return text.to_string();
    }
    let head: String = chars[..MAX_REPORT_CHARS / 2].iter().collect();
    let tail: String = chars[chars.len() - MAX_REPORT_CHARS / 2..].iter().collect();
    format!(
        "{head}\n\n…… 中间省略 {} 字符 ……\n\n{tail}",
        chars.len() - MAX_REPORT_CHARS
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn confirm(policy: ConfirmPolicy, command: &str) -> Option<ConfirmRequest> {
        BashTool::new(policy).confirm_request(&json!({ "command": command }))
    }

    #[test]
    fn readonly_commands_pass_without_asking() {
        assert!(confirm(ConfirmPolicy::Unknown, "ls -la").is_none());
        assert!(confirm(ConfirmPolicy::Unknown, "git status").is_none());
        assert!(
            confirm(ConfirmPolicy::Unknown, "cat a.txt | wc -l").is_none(),
            "整条链路都只读才放行"
        );
    }

    #[test]
    fn anything_unknown_gets_confirmed() {
        // 表外命令即便看着无害也要问——黑名单永远列不全
        let request = confirm(ConfirmPolicy::Unknown, "mkdir tmp").expect("应当确认");
        assert_eq!(request.steps.len(), 1);
        assert!(!request.reasons.is_empty());
    }

    #[test]
    fn one_risky_segment_taints_the_whole_chain() {
        let request = confirm(ConfirmPolicy::Unknown, "ls && rm -rf build").expect("应当确认");
        assert_eq!(request.steps.len(), 2);
        assert!(request.steps[0].risk.is_none(), "ls 本身没问题");
        assert!(request.steps[1].risk.is_some());
        assert_eq!(request.steps[1].connector, "成功后");
        assert!(request.has_risk());
    }

    #[test]
    fn unparsed_command_is_always_confirmed() {
        for policy in [ConfirmPolicy::Unknown, ConfirmPolicy::Risky] {
            let request = confirm(policy, "rm -rf $(cat list)").expect("看不懂必须确认");
            assert!(request.reasons.iter().any(|r| r.contains("$(...)")));
        }
    }

    #[test]
    fn risky_policy_lets_plain_writes_through() {
        assert!(
            confirm(ConfirmPolicy::Risky, "mkdir tmp").is_none(),
            "risky 档只拦命中规则的"
        );
        assert!(confirm(ConfirmPolicy::Risky, "rm -rf build").is_some());
    }

    #[test]
    fn always_policy_confirms_even_ls() {
        assert!(confirm(ConfirmPolicy::Always, "ls").is_some());
    }

    #[test]
    fn steps_read_like_an_explanation() {
        let request = confirm(ConfirmPolicy::Always, "cd /tmp && rm -rf build").unwrap();
        assert_eq!(request.steps[0].explain, "切换工作目录到 /tmp");
        assert_eq!(request.steps[1].explain, "删除 build");
        assert!(request.summary.contains("cd /tmp && rm -rf build"));
    }

    #[test]
    fn forged_readonly_steps_cannot_bypass_dangerous_command() {
        let tool = BashTool::new(ConfirmPolicy::Unknown);
        let args = json!({
            "command": "rm -rf /tmp/lyatest_dir2",
            "steps": [{ "raw": "echo hi", "explain": "只是 echo" }]
        });
        assert!(
            tool.confirm_request(&args).is_some(),
            "决策必须看 command，不能因伪造 steps 放行"
        );
    }

    #[test]
    fn forged_steps_do_not_force_confirm_for_safe_command() {
        let tool = BashTool::new(ConfirmPolicy::Unknown);
        let args = json!({
            "command": "ls",
            "steps": [{ "raw": "rm -rf /", "explain": "看起来很危险" }]
        });
        assert!(
            tool.confirm_request(&args).is_none(),
            "展示可以撒谎，但不能把只读命令变成必须确认"
        );
    }

    #[test]
    fn llm_steps_preserve_custom_explain_text() {
        let tool = BashTool::new(ConfirmPolicy::Always);
        let args = json!({
            "command": "rm -rf build",
            "steps": [{ "raw": "rm -rf build", "explain": "自定义说明" }]
        });
        let request = tool.confirm_request(&args).unwrap();
        assert_eq!(request.steps[0].explain, "自定义说明");
    }

    #[tokio::test]
    async fn runs_and_reports_both_streams() {
        let tool = BashTool::default();
        let result = tool
            .call(ToolCtx::default(), json!({ "command": "echo 出来了; echo 错误 >&2" }))
            .await;
        assert!(result.success, "{}", result.content);
        assert!(result.content.contains("退出码: 0"));
        assert!(result.content.contains("--- stdout ---"));
        assert!(result.content.contains("出来了"));
        assert!(result.content.contains("--- stderr ---"));
        assert!(result.content.contains("错误"));
    }

    #[tokio::test]
    async fn non_zero_exit_is_a_failure() {
        let tool = BashTool::default();
        let result = tool.call(ToolCtx::default(), json!({ "command": "exit 3" })).await;
        assert!(!result.success);
        assert!(result.content.contains("退出码: 3"));
    }

    #[tokio::test]
    async fn timeout_kills_the_command() {
        let tool = BashTool::default();
        let result = tool
            .call(ToolCtx::default(), json!({ "command": "sleep 30", "timeout_secs": 1 }))
            .await;
        assert!(!result.success);
        assert!(result.content.contains("未结束"));
    }

    #[tokio::test]
    async fn stdin_is_closed_so_interactive_reads_return_eof() {
        let tool = BashTool::default();
        let result = tool
            .call(ToolCtx::default(), json!({ "command": "read line; echo done", "timeout_secs": 5 }))
            .await;
        // 若 stdin 没断开，这里会一直等到超时
        assert!(result.content.contains("done"), "{}", result.content);
    }

    #[tokio::test]
    async fn cancel_stops_a_long_command_without_waiting_for_timeout() {
        let tool = BashTool::default();
        let ctx = ToolCtx::default();
        let cancel = ctx.cancel.clone();

        // 一百秒的命令，取消后应该立刻回来而不是等超时
        let handle = tokio::spawn(async move {
            tool.call(ctx, json!({ "command": "sleep 100", "timeout_secs": 100 }))
                .await
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel.cancel();

        let result = tokio::time::timeout(Duration::from_secs(3), handle)
            .await
            .expect("取消后应当很快返回")
            .unwrap();
        assert!(!result.success);
        assert!(result.content.contains("中止"), "{}", result.content);
    }

    #[tokio::test]
    async fn cancelling_before_start_skips_execution() {
        let tool = BashTool::default();
        let ctx = ToolCtx::default();
        ctx.cancel.cancel();
        let result = tool.call(ctx, json!({ "command": "echo 不该执行" })).await;
        assert!(!result.success);
        assert!(!result.content.contains("不该执行"));
    }

    #[tokio::test]
    async fn bad_cwd_is_reported() {
        let tool = BashTool::default();
        let result = tool
            .call(ToolCtx::default(), json!({ "command": "pwd", "cwd": "/不存在的目录" }))
            .await;
        assert!(!result.success);
        assert!(result.content.contains("不存在"));
    }

    #[test]
    fn long_output_keeps_head_and_tail() {
        let text = "x".repeat(MAX_REPORT_CHARS * 2);
        let clipped = clip(&text);
        assert!(clipped.contains("中间省略"));
        assert!(clipped.chars().count() < text.chars().count());
    }
}
