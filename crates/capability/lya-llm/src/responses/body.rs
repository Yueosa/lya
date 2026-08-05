//! Responses API 请求体组装。

use serde_json::{json, Map, Value};

use crate::endpoint::LlmEndpoint;
use lya_base::ApiMode;
use crate::error::LlmError;

/// 组装 `POST /responses` 请求体。
///
/// - 以 `endpoint.params(responses)` 为底（含 `model` 等）
/// - 写入 `instructions`、`input`、`stream`
/// - `tools` 非空或 `native_web_search` 时写入 Responses 形状的 `tools` + `tool_choice: auto`
pub fn build_responses_body(
    endpoint: &LlmEndpoint,
    instructions: &str,
    input: &[Value],
    tools: &[Value],
    stream: bool,
    native_web_search: bool,
) -> Result<Value, LlmError> {
    let mut body = Value::Object(endpoint.params(ApiMode::Responses)?.clone());
    let obj = body.as_object_mut().expect("params 是 object");
    obj.insert("instructions".into(), json!(instructions));
    obj.insert("input".into(), Value::Array(input.to_vec()));
    obj.insert("stream".into(), json!(stream));
    let mut tool_list: Vec<Value> = tools.iter().map(convert_tool_schema).collect();
    if native_web_search {
        tool_list.push(json!({ "type": "web_search" }));
    }
    if !tool_list.is_empty() {
        obj.insert("tools".into(), Value::Array(tool_list));
        obj.insert("tool_choice".into(), json!("auto"));
    }
    Ok(body)
}

/// 把 chat/completions 风格的 tool schema 转为 Responses 扁平形状。
fn convert_tool_schema(tool: &Value) -> Value {
    if let Some(function) = tool.get("function") {
        let mut out = Map::new();
        out.insert("type".into(), json!("function"));
        if let Some(name) = function.get("name") {
            out.insert("name".into(), name.clone());
        }
        if let Some(description) = function.get("description") {
            out.insert("description".into(), description.clone());
        }
        if let Some(parameters) = function.get("parameters") {
            out.insert("parameters".into(), parameters.clone());
        } else {
            out.insert("parameters".into(), json!({}));
        }
        return Value::Object(out);
    }
    tool.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::LlmEndpoint;
    use crate::responses::input::message_user;
    use serde_json::json;

    #[test]
    fn build_body_snapshot() {
        let ep = LlmEndpoint::new("https://api.deepseek.com/v1", "k")
            .with_mode_params(
                ApiMode::Responses,
                serde_json::from_value(json!({
                    "model": "deepseek-v4-flash",
                    "max_output_tokens": 8192,
                    "reasoning": { "effort": "high" },
                }))
                .unwrap(),
            );
        let input = vec![message_user("你好")];
        let tools = [json!({
            "type": "function",
            "function": {
                "name": "echo",
                "description": "回声",
                "parameters": { "type": "object", "properties": {} }
            }
        })];
        let body = build_responses_body(&ep, "你是助手", &input, &tools, true, false).unwrap();
        assert_eq!(body["model"], "deepseek-v4-flash");
        assert_eq!(body["stream"], true);
        assert_eq!(body["instructions"], "你是助手");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["name"], "echo");
        assert_eq!(body["tool_choice"], "auto");
        assert_eq!(
            ep.responses_url(),
            "https://api.deepseek.com/v1/responses"
        );
    }

    #[test]
    fn injects_native_web_search_tool() {
        let ep = LlmEndpoint::new("https://api.deepseek.com/v1", "k").with_mode_params(
            ApiMode::Responses,
            serde_json::from_value(json!({ "model": "deepseek-v4-flash" })).unwrap(),
        );
        let body =
            build_responses_body(&ep, "sys", &[], &[], true, true).unwrap();
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert_eq!(body["tools"][0]["type"], "web_search");
    }
}
