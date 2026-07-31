//! `web_search`：DuckDuckGo 搜索。
//!
//! 选 DDG 是因为它的 HTML 端点不需要 API key，对本地优先的应用最省事——不用让
//! 用户再去申请一个搜索服务的凭据。代价是要解析 HTML，页面结构变了就得跟着改。

use lya_http::HttpClient;
use percent_encoding::percent_decode_str;
use scraper::{Html, Selector};
use serde_json::{json, Value};

use crate::meta::{ToolMeta, ToolResult};
use crate::permission::Permission;
use crate::context::ToolCtx;
use crate::traits::{Tool, ToolCallFuture};

/// DDG 的无脚本搜索端点。
const ENDPOINT: &str = "https://html.duckduckgo.com/html/";
/// 默认返回条数。
const DEFAULT_MAX_RESULTS: usize = 8;
/// 返回条数上限。
const MAX_RESULTS_CAP: usize = 20;
/// 摘要截断长度。
const SNIPPET_CHARS: usize = 200;

/// `web_search` 工具。
pub struct WebSearchTool {
    /// 静态 meta。
    meta: ToolMeta,
    /// OpenAI `parameters` JSON Schema。
    parameters: Value,
    /// 用法说明。
    prompt_hint: &'static str,
    /// 共享 HTTP 客户端。
    http: HttpClient,
}

impl WebSearchTool {
    /// 用共享的 HTTP 客户端构造。
    pub fn new(http: HttpClient) -> Self {
        Self {
            meta: ToolMeta::new(
                "web_search",
                "网页搜索",
                "用 DuckDuckGo 搜索，返回标题、网址与摘要列表",
                Permission::READ,
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "搜索词。"
                    },
                    "max_results": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "最多返回几条，默认 8，上限 20。"
                    },
                    "time_range": {
                        "type": "string",
                        "enum": ["day", "week", "month", "year"],
                        "description": "只要这段时间内的结果；不给则不限时间。"
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            prompt_hint: concat!(
                "使用 web_search 找资料：\n",
                "1) 它只返回标题、网址和摘要。摘要往往不足以回答问题，确定哪条相关后用 web_fetch 读正文。\n",
                "2) 查时效性强的东西（版本号、新闻、报错）时带上 time_range，避免翻到几年前的旧帖。\n",
                "3) 搜索词要具体。命中不好时换关键词，而不是一遍遍重搜同一个词。\n",
                "4) 已经搜到够用的信息就停下来回答，不要为了穷举一直搜。"
            ),
            http,
        }
    }
}

impl Tool for WebSearchTool {
    fn meta(&self) -> &ToolMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.parameters
    }

    fn prompt_hint(&self) -> &str {
        self.prompt_hint
    }

    fn call(&self, _ctx: ToolCtx, args: Value) -> ToolCallFuture<'_> {
        Box::pin(async move {
            let Some(query) = args.get("query").and_then(Value::as_str) else {
                return ToolResult::err("缺少必填参数 `query`");
            };
            let query = query.trim();
            if query.is_empty() {
                return ToolResult::err("`query` 不能为空");
            }
            let max_results = args
                .get("max_results")
                .and_then(Value::as_u64)
                .map(|n| (n as usize).clamp(1, MAX_RESULTS_CAP))
                .unwrap_or(DEFAULT_MAX_RESULTS);
            let time_range = match args.get("time_range").and_then(Value::as_str) {
                None => None,
                Some(value) => match time_code(value) {
                    Some(code) => Some(code),
                    None => {
                        return ToolResult::err(format!(
                            "未知的 time_range {value:?}，应为 day / week / month / year"
                        ));
                    }
                },
            };

            let mut request = self.http.get(ENDPOINT).query(&[("q", query)]);
            if let Some(code) = time_range {
                request = request.query(&[("df", code)]);
            }

            let response = match self.http.send(request).await {
                Ok(response) => response,
                Err(err) => return ToolResult::err(format!("搜索请求失败：{err}")),
            };
            if !response.is_success() {
                return ToolResult::err(format!("搜索返回 HTTP {}", response.status()));
            }
            let body = match response.text().await {
                Ok(body) => body,
                Err(err) => return ToolResult::err(format!("读取搜索结果失败：{err}")),
            };

            let hits = parse_results(&body, max_results);
            if hits.is_empty() {
                return ToolResult::ok(format!("「{query}」没有搜到结果。换个关键词试试。"));
            }
            ToolResult::ok(render(query, &hits))
        })
    }
}

/// 一条搜索结果。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Hit {
    title: String,
    url: String,
    snippet: String,
}

/// 把界面上的时间选项翻成 DDG 的参数。
fn time_code(value: &str) -> Option<&'static str> {
    match value {
        "day" => Some("d"),
        "week" => Some("w"),
        "month" => Some("m"),
        "year" => Some("y"),
        _ => None,
    }
}

/// 从 DDG 的 HTML 里抠出结果列表。
///
/// 排除 `result--ad`：DDG 会把广告混在结果列表最前面，链接还指向自家的跳转
/// 统计页（`duckduckgo.com/y.js?ad_domain=…`）。不滤掉的话模型拿到的头几条
/// 全是广告，而它并不知道那是广告。
pub(crate) fn parse_results(body: &str, limit: usize) -> Vec<Hit> {
    let document = Html::parse_document(body);
    let (Ok(result_sel), Ok(title_sel), Ok(snippet_sel)) = (
        Selector::parse("div.result:not(.result--ad)"),
        Selector::parse("a.result__a"),
        Selector::parse(".result__snippet"),
    ) else {
        return Vec::new();
    };

    let mut hits = Vec::new();
    for node in document.select(&result_sel) {
        if hits.len() >= limit {
            break;
        }
        let Some(anchor) = node.select(&title_sel).next() else {
            continue;
        };
        let title = collapse(&anchor.text().collect::<String>());
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let url = unwrap_redirect(href);
        if title.is_empty() || url.is_empty() {
            continue;
        }
        let snippet = node
            .select(&snippet_sel)
            .next()
            .map(|node| collapse(&node.text().collect::<String>()))
            .unwrap_or_default();
        hits.push(Hit {
            title,
            url,
            snippet: truncate(&snippet, SNIPPET_CHARS),
        });
    }
    hits
}

/// DDG 的链接是包一层跳转的 `…/l/?uddg=<编码后的真实地址>`，得剥出来。
///
/// 直接把跳转链接给模型也能用，但它没法从 URL 判断来源站点是否可信，也不方便
/// 复述给用户看。
pub(crate) fn unwrap_redirect(href: &str) -> String {
    let Some(rest) = href.split("uddg=").nth(1) else {
        return normalize_scheme(href);
    };
    let encoded = rest.split('&').next().unwrap_or(rest);
    match percent_decode_str(encoded).decode_utf8() {
        Ok(decoded) => decoded.into_owned(),
        Err(_) => normalize_scheme(href),
    }
}

/// DDG 有些链接是 `//host/path` 这种省略协议的写法。
fn normalize_scheme(href: &str) -> String {
    if let Some(rest) = href.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        href.to_string()
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars).collect();
    format!("{cut}…")
}

/// 排成给模型看的列表。
fn render(query: &str, hits: &[Hit]) -> String {
    let mut out = format!("「{query}」的搜索结果（{} 条）：\n", hits.len());
    for (index, hit) in hits.iter().enumerate() {
        out.push_str(&format!("\n{}. {}\n   {}\n", index + 1, hit.title, hit.url));
        if !hit.snippet.is_empty() {
            out.push_str(&format!("   {}\n", hit.snippet));
        }
    }
    out.push_str("\n需要详细内容请用 web_fetch 读对应网址。");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 取自 DDG 真实响应的结构（含它混在最前面的广告位）。
    const SAMPLE: &str = r#"
        <div class="result results_links results_links_deep result--ad ">
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fduckduckgo.com%2Fy.js%3Fad_domain%3Debay.com&amp;rut=x">
                eBay 上的 Rust Book
            </a>
            <a class="result__snippet">广告内容</a>
        </div>
        <div class="result results_links results_links_deep web-result ">
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fdoc.rust%2Dlang.org%2Fbook%2F&amp;rut=abc">
                The  Rust
                Book
            </a>
            <a class="result__snippet">官方教程，涵盖所有权与生命周期。</a>
        </div>
        <div class="result web-result">
            <a class="result__a" href="https://example.com/plain">直链结果</a>
        </div>
    "#;

    #[test]
    fn skips_ads() {
        let hits = parse_results(SAMPLE, 10);
        assert!(
            !hits.iter().any(|hit| hit.url.contains("y.js")),
            "广告位不能混进结果：{hits:?}"
        );
    }

    #[test]
    fn parses_title_url_and_snippet() {
        let hits = parse_results(SAMPLE, 10);
        assert_eq!(hits.len(), 2, "三个容器里有一个是广告");
        assert_eq!(hits[0].title, "The Rust Book", "标题里的换行要压平");
        assert_eq!(hits[0].url, "https://doc.rust-lang.org/book/", "要剥掉跳转包装");
        assert!(hits[0].snippet.contains("所有权"));
        assert_eq!(hits[1].url, "https://example.com/plain");
        assert!(hits[1].snippet.is_empty());
    }

    #[test]
    fn respects_limit() {
        assert_eq!(parse_results(SAMPLE, 1).len(), 1);
    }

    #[test]
    fn handles_unparseable_page() {
        assert!(parse_results("<html><body>没有结果结构</body></html>", 5).is_empty());
    }

    #[test]
    fn scheme_less_links_get_https() {
        assert_eq!(unwrap_redirect("//example.com/a"), "https://example.com/a");
    }

    #[test]
    fn time_codes_are_mapped() {
        assert_eq!(time_code("week"), Some("w"));
        assert_eq!(time_code("decade"), None);
    }

    #[test]
    fn render_points_to_web_fetch() {
        let hits = parse_results(SAMPLE, 10);
        let text = render("rust", &hits);
        assert!(text.contains("1. The Rust Book"));
        assert!(text.contains("web_fetch"));
    }
}
