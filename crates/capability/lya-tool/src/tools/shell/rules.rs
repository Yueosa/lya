//! 判断每段命令在干什么、危不危险。
//!
//! 全部**作用在分词后的 argv 上**，不对整条命令做正则。上一代实际跑的是正则匹配
//! 整串，会被引号骗：`echo "rm -rf /"` 误判成危险，`rm  -rf  /`（多个空格）反而
//! 可能漏判。按 argv 判断没有这些问题。

use super::parse::Segment;

/// 已知只读的命令。表外的一律当作可能改动系统。
///
/// 这是「默认确认、只读放行」策略的白名单——黑名单永远列不全，白名单漏了顶多
/// 多问一次。
const READONLY: &[&str] = &[
    "ls", "cat", "bat", "head", "tail", "grep", "rg", "ag", "find", "fd", "wc", "du", "df",
    "stat", "file", "which", "whereis", "type", "echo", "printf", "pwd", "whoami", "id",
    "hostname", "uname", "date", "uptime", "ps", "env", "printenv", "tree", "realpath",
    "basename", "dirname", "sort", "uniq", "cut", "tr", "jq", "xxd", "md5sum", "sha256sum",
    "diff", "cmp", "man", "locale", "lsblk", "lscpu", "free", "nproc", "readlink",
];

/// `git` 下只读的子命令。
///
/// 不含 `config`：`git config name value` 会写配置，整类放进白名单等于默许改仓库。
/// 读配置走 [`git_config_is_readonly`] 单独认。
const GIT_READONLY: &[&str] = &[
    "status", "log", "diff", "show", "branch", "remote", "blame", "shortlog", "describe",
    "rev-parse", "ls-files",
];

/// `cargo` 下只读的子命令。`build` / `test` 会跑构建脚本，不算。
const CARGO_READONLY: &[&str] = &["check", "tree", "metadata", "search", "--version"];

/// 交互式命令：跑起来会一直等输入，把工具挂死。
const INTERACTIVE: &[&str] = &[
    "vim", "vi", "nvim", "nano", "emacs", "less", "more", "top", "htop", "btop", "watch",
    "ssh", "telnet", "ftp", "mysql", "psql", "python", "python3", "node", "irb", "gdb",
];

/// 命令名 → 中文说法。
const VERBS: &[(&str, &str)] = &[
    ("ls", "列出目录"),
    ("cd", "切换工作目录到"),
    ("cat", "输出文件内容"),
    ("head", "查看开头"),
    ("tail", "查看结尾"),
    ("grep", "搜索文本"),
    ("rg", "搜索文本"),
    ("find", "查找文件"),
    ("fd", "查找文件"),
    ("rm", "删除"),
    ("cp", "复制"),
    ("mv", "移动或重命名"),
    ("mkdir", "创建目录"),
    ("rmdir", "删除空目录"),
    ("touch", "创建空文件或更新时间戳"),
    ("ln", "创建链接"),
    ("chmod", "修改权限"),
    ("chown", "修改所有者"),
    ("echo", "输出文本"),
    ("tee", "把输出同时写入文件"),
    ("curl", "下载或请求"),
    ("wget", "下载"),
    ("tar", "打包或解包"),
    ("unzip", "解压"),
    ("zip", "压缩"),
    ("kill", "终止进程"),
    ("pkill", "按名字终止进程"),
    ("killall", "按名字终止进程"),
    ("systemctl", "管理系统服务"),
    ("journalctl", "查看系统日志"),
    ("sed", "按规则替换文本"),
    ("awk", "按规则处理文本"),
    ("sort", "排序"),
    ("uniq", "去重"),
    ("wc", "统计行数字数"),
    ("df", "查看磁盘占用"),
    ("du", "统计目录大小"),
    ("ps", "列出进程"),
    ("git", "执行 git"),
    ("cargo", "执行 cargo"),
    ("npm", "执行 npm"),
    ("pip", "执行 pip"),
    ("make", "执行 make 构建"),
    ("docker", "操作 docker"),
    ("sudo", "以管理员身份执行"),
    ("dd", "按块读写设备或文件"),
    ("mkfs", "格式化文件系统"),
    ("eval", "把字符串当命令执行"),
];

/// 一段命令的判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// 人话说明。
    pub explain: String,
    /// 风险说明；`None` 表示这段看起来没问题。
    pub risk: Option<String>,
    /// 是否属于已知只读命令。
    pub readonly: bool,
}

/// 判定一段命令。
pub fn judge(segment: &Segment) -> Verdict {
    let Some(program) = segment.program() else {
        return Verdict {
            explain: format!("无法解析：{}", segment.raw),
            risk: Some("这一段没能看懂，无法判断它会做什么".into()),
            readonly: false,
        };
    };
    let args = segment.args();
    let name = base_name(program);

    Verdict {
        explain: explain(name, args),
        risk: risk_of(name, args, segment),
        readonly: is_readonly(name, args, segment),
    }
}

/// 去掉路径前缀：`/usr/bin/rm` 与 `rm` 是一回事。
fn base_name(program: &str) -> &str {
    program.rsplit('/').next().unwrap_or(program)
}

/// 拼出人话说明。
fn explain(name: &str, args: &[String]) -> String {
    let verb = VERBS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, verb)| *verb);

    // 子命令型的把子命令带上更清楚
    if matches!(name, "git" | "cargo" | "npm" | "pip" | "docker" | "systemctl")
        && let Some(sub) = args.first()
    {
        let rest = plain_args(&args[1..]);
        return if rest.is_empty() {
            format!("{name} {sub}")
        } else {
            format!("{name} {sub} {rest}")
        };
    }

    let targets = plain_args(args);
    match (verb, targets.is_empty()) {
        (Some(verb), true) => verb.to_string(),
        (Some(verb), false) => format!("{verb} {targets}"),
        (None, true) => format!("运行 {name}"),
        (None, false) => format!("运行 {name}，参数 {targets}"),
    }
}

/// 非选项参数拼成一串。
fn plain_args(args: &[String]) -> String {
    args.iter()
        .filter(|arg| !arg.starts_with('-'))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 是否属于已知只读。
fn is_readonly(name: &str, args: &[String], segment: &Segment) -> bool {
    // 一旦有输出重定向，再只读的命令也会写文件
    if segment.redirects {
        return false;
    }
    match name {
        "git" => match args.first().map(String::as_str) {
            Some("config") => git_config_is_readonly(args),
            Some(sub) => GIT_READONLY.contains(&sub),
            None => false,
        },
        "cargo" => args.first().is_some_and(|sub| CARGO_READONLY.contains(&sub.as_str())),
        // sed -i 是原地改写
        "sed" => !args.iter().any(|arg| arg == "-i" || arg.starts_with("-i")),
        // find 默认只读，但这些动作会删文件或跑子命令
        "find" | "fd" => !find_mutates(args),
        // sort -o / --output 会写文件
        "sort" => !sort_writes(args),
        name => READONLY.contains(&name),
    }
}

/// `git config` 只有明确的读取形态才算只读。
///
/// `git config user.name alice` 没有 `--get` 也会写入，不能靠「只有一个参数」猜。
fn git_config_is_readonly(args: &[String]) -> bool {
    // args[0] 是 "config"
    args.iter().skip(1).any(|arg| {
        matches!(
            arg.as_str(),
            "--get"
                | "--get-all"
                | "--get-regexp"
                | "--get-color"
                | "--get-colorbool"
                | "--list"
                | "-l"
                | "--show-origin"
                | "--show-scope"
        )
    })
}

/// `find` / `fd` 会改动文件系统或执行外部命令的选项。
fn find_mutates(args: &[String]) -> bool {
    args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "-delete"
                | "-exec"
                | "-execdir"
                | "-ok"
                | "-okdir"
                | "-fprint"
                | "-fprintf"
                | "-fls"
                // fd 的执行形态
                | "-x"
                | "--exec"
                | "-X"
                | "--exec-batch"
        ) || arg.starts_with("-fprintf")
            || arg.starts_with("-fprint")
    })
}

/// `sort` 把结果写进文件而不是 stdout。
fn sort_writes(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "-o" || arg == "--output" || arg.starts_with("--output="))
}

/// 有没有风险，有的话说清楚是什么。
fn risk_of(name: &str, args: &[String], segment: &Segment) -> Option<String> {
    let has = |flag: &str| args.iter().any(|arg| arg == flag);
    // 合并式短选项：-rf 里既有 r 也有 f
    let short = |letter: char| {
        args.iter().any(|arg| {
            arg.starts_with('-') && !arg.starts_with("--") && arg[1..].contains(letter)
        })
    };
    let targets: Vec<&str> = args.iter().filter(|a| !a.starts_with('-')).map(String::as_str).collect();
    let hits_root = targets.iter().any(|t| matches!(*t, "/" | "/*" | "~" | "~/" | "$HOME"));

    match name {
        "rm" if hits_root => Some("删除根目录或家目录，会摧毁整个系统或全部个人数据".into()),
        "rm" if short('r') => Some("递归删除整个目录，删掉就找不回来了".into()),
        "rm" => Some("删除文件，无法撤销".into()),
        "mkfs" | "mkfs.ext4" | "mkfs.xfs" | "mkfs.btrfs" => {
            Some("格式化文件系统，目标分区上的数据会全部消失".into())
        }
        "dd" => Some("直接按块读写设备，写错目标会毁掉整块磁盘".into()),
        "shutdown" | "reboot" | "poweroff" | "halt" => Some("关机或重启这台机器".into()),
        "init" if targets.iter().any(|t| *t == "0" || *t == "6") => Some("关机或重启".into()),
        "sudo" | "doas" => Some("以管理员权限执行，能改动系统任何部分".into()),
        "su" => Some("切换到其它用户身份".into()),
        "chmod" if targets.iter().any(|t| t.starts_with('/')) && has("777") => {
            Some("把系统路径权限放开到所有人可写".into())
        }
        "chmod" | "chown" if short('R') => Some("递归修改权限或所有者".into()),
        "chmod" | "chown" => Some("修改权限或所有者".into()),
        "kill" if has("-9") => Some("强制终止进程，目标来不及保存数据".into()),
        "pkill" | "killall" => Some("按名字批量终止进程，可能误伤同名进程".into()),
        "systemctl"
            if args
                .first()
                .is_some_and(|sub| matches!(sub.as_str(), "stop" | "disable" | "mask")) =>
        {
            Some("停止或禁用系统服务".into())
        }
        "eval" => Some("把字符串当命令执行，实际跑什么取决于运行时内容".into()),
        "sh" | "bash" | "zsh" if has("-c") => Some("间接执行一段命令".into()),
        "truncate" => Some("截断文件内容".into()),
        "git" if args.first().is_some_and(|s| s == "reset") && has("--hard") => {
            Some("丢弃所有未提交的改动".into())
        }
        "git" if args.first().is_some_and(|s| s == "clean") && short('f') => {
            Some("删除未被 git 跟踪的文件".into())
        }
        "git" if args.first().is_some_and(|s| s == "push") && (has("--force") || has("-f")) => {
            Some("强制推送，会覆盖远端历史".into())
        }
        "git" if args.first().is_some_and(|s| s == "config") && !git_config_is_readonly(args) => {
            Some("写入 git 配置".into())
        }
        "find" | "fd" if find_mutates(args) => {
            Some("查找过程中会删除文件或执行其它命令".into())
        }
        "sort" if sort_writes(args) => Some("把排序结果写入文件".into()),
        "apt" | "apt-get" | "dnf" | "yum"
            if args
                .first()
                .is_some_and(|sub| matches!(sub.as_str(), "remove" | "purge" | "autoremove")) =>
        {
            Some("卸载系统软件包".into())
        }
        "pacman" if short('R') => Some("卸载系统软件包".into()),
        "pip" | "pip3" if args.first().is_some_and(|s| s == "uninstall") => {
            Some("卸载 Python 包".into())
        }
        _ if segment.redirects && segment.raw.contains("/dev/") => {
            Some("向设备文件写入".into())
        }
        _ if INTERACTIVE.contains(&name) => {
            Some("这是交互式程序，会一直等待输入直到超时".into())
        }
        _ => None,
    }
}

/// 整条命令层面的风险：单看某一段看不出来的那些。
pub fn pipeline_risks(segments: &[Segment]) -> Vec<String> {
    let mut risks = Vec::new();

    // 下载后直接交给 shell 执行——真正跑什么完全取决于下载到的内容
    let downloads = segments
        .iter()
        .any(|s| matches!(s.program().map(base_name), Some("curl" | "wget")));
    let pipes_to_shell = segments.iter().any(|s| {
        matches!(s.program().map(base_name), Some("sh" | "bash" | "zsh"))
            && s.connector == super::parse::Connector::Pipe
    });
    if downloads && pipes_to_shell {
        risks.push("把下载到的内容直接交给 shell 执行，实际会跑什么无法预先判断".into());
    }

    // fork 炸弹的典型形状
    if segments
        .iter()
        .any(|s| s.raw.replace(' ', "").contains(":(){"))
    {
        risks.push("疑似 fork 炸弹，会耗尽系统进程".into());
    }
    risks
}

#[cfg(test)]
mod tests {
    use super::super::parse::parse;
    use super::*;

    fn judge_one(command: &str) -> Verdict {
        let parsed = parse(command);
        judge(&parsed.segments[0])
    }

    #[test]
    fn readonly_commands_are_recognized() {
        assert!(judge_one("ls -la").readonly);
        assert!(judge_one("git status").readonly);
        assert!(judge_one("rg TODO src").readonly);
        assert!(judge_one("/usr/bin/cat a.txt").readonly, "带路径也要认出来");
    }

    #[test]
    fn write_like_usage_is_not_readonly() {
        assert!(!judge_one("echo hi > out.txt").readonly, "重定向会写文件");
        assert!(!judge_one("git commit -m x").readonly);
        assert!(!judge_one("cargo build").readonly, "构建会跑 build 脚本");
        assert!(!judge_one("sed -i s/a/b/ f.txt").readonly, "-i 是原地改写");
        assert!(judge_one("sed s/a/b/ f.txt").readonly);
        assert!(!judge_one("mkdir tmp").readonly, "表外命令一律当作会改动");
        // 这几条曾经整类放进只读白名单，等于默许改盘
        assert!(!judge_one("git config user.name alice").readonly);
        assert!(judge_one("git config --get user.name").readonly);
        assert!(judge_one("git config --list").readonly);
        assert!(!judge_one("find . -delete").readonly);
        assert!(judge_one("find . -name '*.rs'").readonly);
        assert!(!judge_one("sort -o out.txt in.txt").readonly);
        assert!(judge_one("sort in.txt").readonly);
    }

    #[test]
    fn deletion_risk_scales_with_target() {
        assert!(judge_one("rm a.txt").risk.is_some());
        assert!(judge_one("rm -rf build").risk.unwrap().contains("递归"));
        assert!(judge_one("rm -rf /").risk.unwrap().contains("摧毁"));
    }

    #[test]
    fn combined_short_flags_are_seen() {
        // -rf 里的 r 要能识别出来
        assert!(judge_one("rm -rf x").risk.unwrap().contains("递归"));
        assert!(judge_one("chmod -R 755 x").risk.unwrap().contains("递归"));
    }

    #[test]
    fn quoted_text_is_not_mistaken_for_a_command() {
        // 上一代的正则会把这条判成危险
        let verdict = judge_one(r#"echo "rm -rf /""#);
        assert_eq!(verdict.risk, None, "引号里的内容只是文本");
        assert!(verdict.readonly);
    }

    #[test]
    fn privilege_and_power_commands_are_flagged() {
        assert!(judge_one("sudo pacman -Syu").risk.unwrap().contains("管理员"));
        assert!(judge_one("reboot").risk.is_some());
        assert!(judge_one("kill -9 1234").risk.unwrap().contains("强制"));
    }

    #[test]
    fn interactive_programs_are_flagged() {
        assert!(judge_one("vim a.txt").risk.unwrap().contains("交互式"));
    }

    #[test]
    fn unparsed_segment_is_treated_as_risky() {
        let parsed = parse("rm -rf $(cat list)");
        let verdict = judge(&parsed.segments[0]);
        assert!(verdict.risk.is_some(), "看不懂就得当有风险");
        assert!(!verdict.readonly);
    }

    #[test]
    fn explains_in_plain_words() {
        assert_eq!(judge_one("rm -rf build").explain, "删除 build");
        assert_eq!(judge_one("cd /tmp").explain, "切换工作目录到 /tmp");
        // 选项的取值也会带出来，这里反而有用：能看见提交信息写的什么
        assert_eq!(judge_one("git commit -m 修好了登录").explain, "git commit 修好了登录");
        assert_eq!(judge_one("mycmd --flag x").explain, "运行 mycmd，参数 x");
    }

    #[test]
    fn curl_piped_to_shell_is_caught_at_pipeline_level() {
        let parsed = parse("curl https://x.sh | sh");
        let risks = pipeline_risks(&parsed.segments);
        assert!(risks.iter().any(|r| r.contains("下载")));

        // 单独下载不算这条
        let plain = parse("curl https://x.sh -o x.sh");
        assert!(pipeline_risks(&plain.segments).is_empty());
    }

    #[test]
    fn fork_bomb_shape_is_caught() {
        let parsed = parse(":(){ :|:& };:");
        assert!(!pipeline_risks(&parsed.segments).is_empty());
    }
}
