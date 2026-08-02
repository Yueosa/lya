//! 把一条 shell 命令拆成能逐段讲清楚的片段。
//!
//! **不是完整的 shell 解析器**，那是无底洞。目标只有一个：让用户看懂自己要放行
//! 什么。所以有一条铁律——
//!
//! > **看不懂就如实说看不懂，交给确认流程去拦。**
//!
//! 遇到命令替换、反引号、引号没闭合这类结构，就把 [`ParsedCommand::understood`]
//! 置 false 并记下原因。绝不能因为解析失败而当作安全放行：那恰恰是最该拦的情况。

/// 片段之间的连接方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connector {
    /// 第一段。
    First,
    /// `&&`：上一段成功才执行。
    And,
    /// `||`：上一段失败才执行。
    Or,
    /// `|`：接收上一段的输出。
    Pipe,
    /// `;`：无条件接着执行。
    Seq,
    /// `&`：放到后台。
    Background,
}

impl Connector {
    /// 给用户看的说法。
    pub const fn label(self) -> &'static str {
        match self {
            Self::First => "",
            Self::And => "成功后",
            Self::Or => "失败时",
            Self::Pipe => "接上一步输出",
            Self::Seq => "然后",
            Self::Background => "后台运行",
        }
    }
}

/// 一个命令片段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// 原始文本（已去首尾空白）。
    pub raw: String,
    /// 分词后的 argv；含无法静态判断的结构时为 `None`。
    pub argv: Option<Vec<String>>,
    /// 与上一段的连接方式。
    pub connector: Connector,
    /// 是否含输出重定向（`>` / `>>`）。
    pub redirects: bool,
}

impl Segment {
    /// 命令名（argv[0]）。
    pub fn program(&self) -> Option<&str> {
        self.argv.as_ref()?.first().map(String::as_str)
    }

    /// 除命令名外的参数。
    pub fn args(&self) -> &[String] {
        match &self.argv {
            Some(argv) if argv.len() > 1 => &argv[1..],
            _ => &[],
        }
    }
}

/// 一条命令的解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    /// 按连接符切开的片段。
    pub segments: Vec<Segment>,
    /// 是否完整看懂。false 时必须走确认。
    pub understood: bool,
    /// 没看懂的地方。
    pub caveats: Vec<String>,
}

/// 解析一条命令。
pub fn parse(command: &str) -> ParsedCommand {
    let mut segments = Vec::new();
    let mut caveats: Vec<String> = Vec::new();

    let mut current = String::new();
    let mut pending = Connector::First;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut substitution = 0usize;

    let chars: Vec<char> = command.chars().collect();
    let mut index = 0;

    let note = |caveats: &mut Vec<String>, text: &str| {
        if !caveats.iter().any(|existing| existing == text) {
            caveats.push(text.to_string());
        }
    };

    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied();

        if escaped {
            current.push(ch);
            escaped = false;
            index += 1;
            continue;
        }
        // 单引号里反斜杠不转义
        if ch == '\\' && quote != Some('\'') {
            current.push(ch);
            escaped = true;
            index += 1;
            continue;
        }
        if let Some(open) = quote {
            // 双引号里 bash 仍会执行 $() 与反引号；单引号里不会，当普通文本即可
            if open == '"' {
                if ch == '$' && next == Some('(') {
                    note(
                        &mut caveats,
                        "命令里有 $(...) 命令替换，实际执行内容取决于它的输出",
                    );
                    substitution += 1;
                    current.push_str("$(");
                    index += 2;
                    continue;
                }
                if ch == '`' {
                    note(
                        &mut caveats,
                        "命令里有反引号替换，实际执行内容取决于它的输出",
                    );
                    current.push(ch);
                    index += 1;
                    continue;
                }
            }
            current.push(ch);
            if ch == open {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            current.push(ch);
            index += 1;
            continue;
        }
        // 命令替换：里面跑什么静态看不出来
        if ch == '$' && next == Some('(') {
            note(&mut caveats, "命令里有 $(...) 命令替换，实际执行内容取决于它的输出");
            substitution += 1;
            current.push_str("$(");
            index += 2;
            continue;
        }
        if substitution > 0 {
            if ch == '(' {
                substitution += 1;
            } else if ch == ')' {
                substitution -= 1;
            }
            current.push(ch);
            index += 1;
            continue;
        }
        if ch == '`' {
            note(&mut caveats, "命令里有反引号替换，实际执行内容取决于它的输出");
            current.push(ch);
            index += 1;
            continue;
        }

        // 到这里才是真正的连接符
        let boundary = match (ch, next) {
            ('&', Some('&')) => Some((Connector::And, 2)),
            ('|', Some('|')) => Some((Connector::Or, 2)),
            ('|', _) => Some((Connector::Pipe, 1)),
            (';', _) => Some((Connector::Seq, 1)),
            ('&', _) if current.trim_end().ends_with('>') => {
                // fd 重定向：2>&1、>&2 里的 & 不是后台符
                current.push(ch);
                index += 1;
                continue;
            }
            ('&', _) => Some((Connector::Background, 1)),
            _ => None,
        };
        match boundary {
            Some((connector, width)) => {
                push_segment(&mut segments, &current, pending, &mut caveats);
                current.clear();
                pending = connector;
                index += width;
            }
            None => {
                current.push(ch);
                index += 1;
            }
        }
    }

    if quote.is_some() {
        note(&mut caveats, "引号没有闭合，无法确定命令边界");
    }
    if substitution > 0 {
        note(&mut caveats, "$( 没有闭合，无法确定命令边界");
    }
    push_segment(&mut segments, &current, pending, &mut caveats);

    if segments.is_empty() {
        caveats.push("命令为空".into());
    }
    let understood = caveats.is_empty() && segments.iter().all(|s| s.argv.is_some());
    ParsedCommand {
        segments,
        understood,
        caveats,
    }
}

/// 收尾一个片段。
fn push_segment(
    segments: &mut Vec<Segment>,
    raw: &str,
    connector: Connector,
    caveats: &mut Vec<String>,
) {
    let raw = raw.trim();
    if raw.is_empty() {
        if connector != Connector::First {
            caveats.push(format!("`{}` 前后有空片段", connector.label()));
        }
        return;
    }
    let argv = tokenize(raw);
    if argv.is_none() {
        caveats.push(format!("`{raw}` 这一段没能完全解析"));
    }
    segments.push(Segment {
        raw: raw.to_string(),
        argv,
        connector,
        redirects: has_redirect(raw),
    });
}

/// 按 shell 规则分词；含无法静态判断的结构时返回 `None`。
///
/// 重定向记号会被丢掉——它们不是命令的参数，留着会干扰按 argv 判断风险。
/// 是否存在重定向另由 [`Segment::redirects`] 记录。
fn tokenize(raw: &str) -> Option<Vec<String>> {
    let mut argv = Vec::new();
    let mut token = String::new();
    let mut has_token = false;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut skip_next = false;

    let flush = |token: &mut String, has: &mut bool, argv: &mut Vec<String>| {
        if *has {
            argv.push(std::mem::take(token));
            *has = false;
        }
    };

    let chars: Vec<char> = raw.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if escaped {
            token.push(ch);
            has_token = true;
            escaped = false;
            index += 1;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            index += 1;
            continue;
        }
        if let Some(open) = quote {
            if open == '"' {
                if ch == '$' && chars.get(index + 1) == Some(&'(') {
                    return None;
                }
                if ch == '`' {
                    return None;
                }
            }
            if ch == open {
                quote = None;
            } else {
                token.push(ch);
            }
            has_token = true;
            index += 1;
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                has_token = true;
                index += 1;
            }
            '$' if chars.get(index + 1) == Some(&'(') => return None,
            '`' => return None,
            '>' | '<' => {
                // 重定向记号连同它的目标一起跳过
                flush(&mut token, &mut has_token, &mut argv);
                index += 1;
                if chars.get(index) == Some(&'>') {
                    index += 1;
                }
                skip_next = ch == '>' || ch == '<';
                while chars.get(index).is_some_and(|c| c.is_whitespace()) {
                    index += 1;
                }
            }
            c if c.is_whitespace() => {
                if skip_next && has_token {
                    // 这个 token 是重定向目标，丢掉
                    token.clear();
                    has_token = false;
                    skip_next = false;
                } else {
                    flush(&mut token, &mut has_token, &mut argv);
                }
                index += 1;
            }
            c => {
                token.push(c);
                has_token = true;
                index += 1;
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    if skip_next {
        token.clear();
        has_token = false;
    }
    flush(&mut token, &mut has_token, &mut argv);

    (!argv.is_empty()).then_some(argv)
}

/// 片段里是否有输出重定向。
fn has_redirect(raw: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in raw.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quote != Some('\'') => escaped = true,
            '\'' | '"' => match quote {
                Some(open) if open == ch => quote = None,
                None => quote = Some(ch),
                _ => {}
            },
            '>' if quote.is_none() => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn programs(parsed: &ParsedCommand) -> Vec<&str> {
        parsed.segments.iter().filter_map(Segment::program).collect()
    }

    #[test]
    fn splits_on_every_connector() {
        let parsed = parse("cd /tmp && rm build || echo fail ; ls | wc -l");
        assert_eq!(programs(&parsed), ["cd", "rm", "echo", "ls", "wc"]);
        assert_eq!(
            parsed.segments.iter().map(|s| s.connector).collect::<Vec<_>>(),
            [
                Connector::First,
                Connector::And,
                Connector::Or,
                Connector::Seq,
                Connector::Pipe
            ]
        );
        assert!(parsed.understood);
    }

    #[test]
    fn connectors_inside_quotes_are_not_boundaries() {
        let parsed = parse(r#"echo "a && b | c" ; ls"#);
        assert_eq!(programs(&parsed), ["echo", "ls"]);
        assert_eq!(parsed.segments[0].args(), ["a && b | c"]);
    }

    #[test]
    fn strips_quotes_and_escapes_in_argv() {
        let parsed = parse(r#"grep -n 'hello world' my\ file.txt"#);
        assert_eq!(
            parsed.segments[0].argv.as_ref().unwrap(),
            &["grep", "-n", "hello world", "my file.txt"]
        );
    }

    #[test]
    fn command_substitution_is_admitted_not_guessed() {
        let parsed = parse("rm -rf $(cat targets.txt)");
        assert!(!parsed.understood, "看不懂就该说看不懂");
        assert!(parsed.caveats.iter().any(|c| c.contains("$(...)")));
        // 边界也不能被里面的符号骗到
        assert_eq!(parsed.segments.len(), 1);
    }

    #[test]
    fn backticks_are_admitted_too() {
        let parsed = parse("echo `whoami`");
        assert!(!parsed.understood);
        assert!(parsed.caveats.iter().any(|c| c.contains("反引号")));
    }

    #[test]
    fn unclosed_quote_is_admitted() {
        let parsed = parse(r#"echo "没闭合"#);
        assert!(!parsed.understood);
        assert!(parsed.caveats.iter().any(|c| c.contains("引号没有闭合")));
    }

    #[test]
    fn redirect_target_is_not_an_argument() {
        let parsed = parse("echo hi > /tmp/out.txt");
        let segment = &parsed.segments[0];
        assert_eq!(segment.argv.as_ref().unwrap(), &["echo", "hi"]);
        assert!(segment.redirects, "重定向本身要记下来");
        assert!(parsed.understood);
    }

    #[test]
    fn append_redirect_is_detected() {
        let parsed = parse("cat a >> b");
        assert!(parsed.segments[0].redirects);
        assert_eq!(parsed.segments[0].argv.as_ref().unwrap(), &["cat", "a"]);
    }

    #[test]
    fn redirect_inside_quotes_is_just_text() {
        let parsed = parse(r#"echo "a > b""#);
        assert!(!parsed.segments[0].redirects);
    }

    #[test]
    fn background_and_empty_segments() {
        let parsed = parse("sleep 10 &");
        assert_eq!(programs(&parsed), ["sleep"]);

        let broken = parse("ls &&");
        assert!(!broken.understood, "悬空的连接符要报出来");
    }

    #[test]
    fn fd_redirect_does_not_split_on_ampersand() {
        let parsed = parse("which tree tokei 2>&1");
        assert_eq!(programs(&parsed), ["which"]);
        assert_eq!(parsed.segments.len(), 1);
        assert!(parsed.segments[0].redirects);
        assert!(parsed.understood);
    }

    #[test]
    fn substitution_inside_double_quotes_is_admitted() {
        let parsed = parse(r#"echo "$(rm -rf /)""#);
        assert!(!parsed.understood, "bash 会在双引号内执行命令替换");
        assert!(parsed.caveats.iter().any(|c| c.contains("$(...)")));
    }

    #[test]
    fn backticks_inside_double_quotes_are_admitted_too() {
        let parsed = parse(r#"echo "`rm -rf /`""#);
        assert!(!parsed.understood);
        assert!(parsed.caveats.iter().any(|c| c.contains("反引号")));
    }

    #[test]
    fn substitution_inside_single_quotes_is_literal() {
        let parsed = parse(r#"echo '$(whoami)'"#);
        assert!(parsed.understood);
        assert_eq!(parsed.segments[0].args(), ["$(whoami)"]);
    }

    #[test]
    fn empty_command_is_not_understood() {
        let parsed = parse("   ");
        assert!(!parsed.understood);
        assert!(parsed.segments.is_empty());
    }
}
