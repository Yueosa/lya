//! Responses API 的 input item 构造辅助。

use serde_json::{json, Value};

/// user 消息 item。
pub fn message_user(content: impl Into<String>) -> Value {
    json!({
        "type": "message",
        "role": "user",
        "content": content.into(),
    })
}

/// assistant 消息 item。
pub fn message_assistant(content: impl Into<String>) -> Value {
    json!({
        "type": "message",
        "role": "assistant",
        "content": content.into(),
    })
}

/// system 消息 item（除 `instructions` 外的额外系统节点）。
pub fn message_system(content: impl Into<String>) -> Value {
    json!({
        "type": "message",
        "role": "system",
        "content": content.into(),
    })
}

/// function 调用 item。
pub fn function_call(call_id: impl Into<String>, name: impl Into<String>, arguments: impl Into<String>) -> Value {
    json!({
        "type": "function_call",
        "call_id": call_id.into(),
        "name": name.into(),
        "arguments": arguments.into(),
    })
}

/// function 调用结果 item。
pub fn function_call_output(call_id: impl Into<String>, output: impl Into<String>) -> Value {
    json!({
        "type": "function_call_output",
        "call_id": call_id.into(),
        "output": output.into(),
    })
}

/// 合成最小 `web_search_call` item（SSE 未给完整 item 时落库 / 回灌用）。
pub fn web_search_call_item(
    id: impl Into<String>,
    status: impl Into<String>,
    queries: Option<Vec<String>>,
) -> Value {
    json!({
        "type": "web_search_call",
        "id": id.into(),
        "status": status.into(),
        "action": normalize_search_action(&json!({}), queries.as_deref()),
    })
}

/// 回放前补全 provider 要求的字段（DeepSeek 必填 `action.queries`）。
pub fn normalize_web_search_call_item(item: &Value) -> Value {
    if item.get("type").and_then(|t| t.as_str()) != Some("web_search_call") {
        return item.clone();
    }
    let mut out = item.clone();
    let action = out.get("action").cloned().unwrap_or_else(|| json!({}));
    out["action"] = normalize_search_action(&action, None);
    out
}

/// 补全 search action：DeepSeek 回放要求 `queries` 数组（`query` 单字段不够）。
fn normalize_search_action(action: &Value, fallback_queries: Option<&[String]>) -> Value {
    if let Some(kind) = action.get("type").and_then(|t| t.as_str()) {
        if kind != "search" {
            return action.clone();
        }
    }
    if action.get("queries").and_then(|q| q.as_array()).is_some() {
        let mut out = action.clone();
        if out.get("type").is_none() {
            out["type"] = json!("search");
        }
        return out;
    }
    if let Some(q) = action
        .get("query")
        .and_then(|q| q.as_str())
        .filter(|s| !s.is_empty())
    {
        return json!({ "type": "search", "queries": [q] });
    }
    if let Some(list) = fallback_queries.filter(|q| !q.is_empty()) {
        return json!({ "type": "search", "queries": list });
    }
    json!({ "type": "search", "queries": [] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_queries_from_legacy_query() {
        let item = json!({
            "type": "web_search_call",
            "id": "ws1",
            "status": "completed",
            "action": { "type": "search", "query": "天气" }
        });
        let out = normalize_web_search_call_item(&item);
        assert_eq!(out["action"]["queries"], json!(["天气"]));
    }

    #[test]
    fn normalize_fixes_empty_query_only_action() {
        let item = json!({
            "type": "web_search_call",
            "id": "ws1",
            "status": "completed",
            "action": { "type": "search", "query": "" }
        });
        let out = normalize_web_search_call_item(&item);
        assert_eq!(out["action"]["queries"], json!([]));
    }

    #[test]
    fn normalize_preserves_existing_queries() {
        let item = json!({
            "type": "web_search_call",
            "id": "ws1",
            "status": "completed",
            "action": { "type": "search", "queries": ["a", "b"] }
        });
        let out = normalize_web_search_call_item(&item);
        assert_eq!(out["action"]["queries"], json!(["a", "b"]));
    }
}
