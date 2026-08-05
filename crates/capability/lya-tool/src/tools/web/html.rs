//! HTML 抽正文。
//!
//! 目标不是还原排版，而是给模型一份**能读的纯文本**：丢掉脚本样式与导航噪音，
//! 保留段落分隔，把连续空白压掉。

use ego_tree::NodeRef;
use scraper::node::Node;
use scraper::Html;

/// 完全跳过的元素：内容对读者没有意义。
const SKIPPED: &[&str] = &[
    "script", "style", "noscript", "svg", "head", "template", "iframe", "canvas",
];

/// 结束后需要换行的块级元素。
const BLOCK: &[&str] = &[
    "p", "div", "section", "article", "header", "footer", "main", "aside", "nav", "li", "tr",
    "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "pre", "table", "ul", "ol", "form", "hr",
    "br",
];

/// 把 HTML 抽成纯文本。
pub fn to_text(html: &str) -> String {
    let document = Html::parse_document(html);
    let mut buffer = String::new();
    walk(document.tree.root(), &mut buffer, false);
    tidy(&buffer)
}

/// 深度优先收集文本，遇到块级元素补换行。
///
/// `pre` 表示当前处在 `<pre>` 里，此时原样保留空白——技术文档的代码块靠缩进
/// 表意，压掉就读不懂了。
fn walk(node: NodeRef<'_, Node>, out: &mut String, pre: bool) {
    match node.value() {
        Node::Text(text) => push_text(&text.text, out, pre),
        Node::Element(element) => {
            let name = element.name();
            if SKIPPED.contains(&name) {
                return;
            }
            let pre = pre || name == "pre";
            for child in node.children() {
                walk(child, out, pre);
            }
            if BLOCK.contains(&name) {
                out.push('\n');
            }
        }
        _ => {
            for child in node.children() {
                walk(child, out, pre);
            }
        }
    }
}

/// 追加文本；非 `<pre>` 时把连续空白（含源码换行）压成一个空格。
///
/// 源码里的换行不该变成正文换行——只有块级元素结束才算换行，否则一段话会被
/// HTML 的排版缩进切得七零八落。
fn push_text(text: &str, out: &mut String, pre: bool) {
    if pre {
        out.push_str(text);
        return;
    }
    // 行首（刚补过换行）不留缩进
    let mut after_space = out.is_empty() || out.ends_with(char::is_whitespace);
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !after_space {
                out.push(' ');
                after_space = true;
            }
        } else {
            out.push(ch);
            after_space = false;
        }
    }
}

/// 去行尾空白、把连续空行压成一个、掐掉首尾空行。
fn tidy(raw: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut blank_run = 0;

    for line in raw.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank_run += 1;
            if blank_run == 1 && !lines.is_empty() {
                lines.push("");
            }
        } else {
            blank_run = 0;
            lines.push(line);
        }
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// 取 `<title>`。
pub fn title_of(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = scraper::Selector::parse("title").ok()?;
    let title = document
        .select(&selector)
        .next()?
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!title.is_empty()).then_some(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_script_and_style() {
        let html = r#"
            <html><head><title>标题</title><style>body{color:red}</style></head>
            <body>
                <script>console.log("不该出现")</script>
                <p>第一段</p>
                <p>第二段</p>
            </body></html>
        "#;
        let text = to_text(html);
        assert!(text.contains("第一段"));
        assert!(text.contains("第二段"));
        assert!(!text.contains("console.log"));
        assert!(!text.contains("color:red"));
    }

    #[test]
    fn keeps_paragraph_breaks_and_caps_blank_runs() {
        // 嵌套块级元素会连着补换行，但空行最多留一个
        let html = "<div><p>一</p><p>二</p></div><div><p>三</p></div>";
        assert_eq!(to_text(html), "一\n二\n\n三");
    }

    #[test]
    fn source_newlines_do_not_break_a_paragraph() {
        let html = "<p>很多      空格\n\t和换行</p>";
        assert_eq!(to_text(html), "很多 空格 和换行");
    }

    #[test]
    fn preformatted_indentation_survives() {
        let html = "<pre>def f():\n    return 1\n</pre>";
        assert_eq!(to_text(html), "def f():\n    return 1");
    }

    #[test]
    fn extracts_title() {
        assert_eq!(
            title_of("<html><head><title> 页面  标题 </title></head></html>").as_deref(),
            Some("页面 标题")
        );
        assert_eq!(title_of("<html><body>无标题</body></html>"), None);
    }
}
