//! Responses API SSE 解析。

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::error::LlmError;
use crate::event::{StreamEvent, ToolCallDelta, WebSearchStatus};
use crate::responses::input::{normalize_web_search_call_item, web_search_call_item};

/// 有状态的 Responses SSE 解析器（维护 `item_id → index` 与原生搜索 id）。
///
/// **槽位按 `item_id` 索引，不是 `call_id`。** Responses 的
/// `response.function_call_arguments.delta` 只带 `item_id`（`fc_…`），
/// 而 `call_id`（`call_…`）只在 `response.output_item.added` 的 item 里出现一次。
/// 拿 `call_id` 去认增量会一条都对不上，参数最后全是空串。
#[derive(Debug, Default)]
pub struct ResponsesSseParser {
    call_indices: HashMap<String, usize>,
    /// `item_id → call_id`：增量事件只有前者，回灌 tool 结果要后者。
    item_to_call: HashMap<String, String>,
    /// 已经收过参数（增量或整段）的 item；防止 `done` 事件把参数追加第二遍。
    args_seen: HashSet<String>,
    next_index: usize,
    finished_emitted: bool,
    web_search_call_id: Option<String>,
    web_search_persisted: HashMap<String, ()>,
}

impl ResponsesSseParser {
    /// 解析一行 SSE 文本。
    pub fn parse_line(&mut self, line: &str) -> Result<Option<Vec<StreamEvent>>, LlmError> {
        let line = line.trim();
        if line.is_empty() || line.starts_with(':') {
            return Ok(None);
        }
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(None);
        };
        let data = data.trim();
        if data.is_empty() {
            return Ok(None);
        }
        let value: Value = serde_json::from_str(data)
            .map_err(|err| LlmError::Decode(format!("{err}; data={data}")))?;
        Ok(Some(self.extract_events(&value)))
    }

    /// 流结束后是否已见到终态事件。
    pub fn finished_emitted(&self) -> bool {
        self.finished_emitted
    }

    fn extract_events(&mut self, value: &Value) -> Vec<StreamEvent> {
        let Some(kind) = value.get("type").and_then(|t| t.as_str()) else {
            return Vec::new();
        };

        match kind {
            "response.output_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(|d| d.as_str()) {
                    if !delta.is_empty() {
                        return vec![StreamEvent::TextDelta(delta.to_string())];
                    }
                }
            }
            "response.reasoning_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(|d| d.as_str()) {
                    if !delta.is_empty() {
                        return vec![StreamEvent::ReasoningDelta(delta.to_string())];
                    }
                }
            }
            "response.output_item.added" => {
                if let Some(item) = value.get("item") {
                    match item.get("type").and_then(|t| t.as_str()) {
                        Some("function_call") => {
                            let item_id = str_field(item, "id");
                            let call_id = str_field(item, "call_id");
                            // 两个 id 至少要有一个，否则后面的增量无从归位
                            let Some(key) = item_id.or(call_id) else {
                                return Vec::new();
                            };
                            let index = self.index_for(key);
                            // call_id 也登记一份：有的实现在增量里回的是它
                            if let Some(call_id) = call_id {
                                self.call_indices.insert(call_id.to_string(), index);
                                if let Some(item_id) = item_id {
                                    self.item_to_call
                                        .insert(item_id.to_string(), call_id.to_string());
                                }
                            }
                            return vec![StreamEvent::ToolCallDelta(ToolCallDelta {
                                index,
                                id: Some(call_id.unwrap_or(key).to_string()),
                                name: str_field(item, "name").map(str::to_string),
                                arguments: None,
                            })];
                        }
                        Some("web_search_call") => {
                            let call_id = item
                                .get("id")
                                .or_else(|| item.get("call_id"))
                                .and_then(|c| c.as_str())
                                .unwrap_or("native")
                                .to_string();
                            self.web_search_call_id = Some(call_id);
                        }
                        _ => {}
                    }
                }
            }
            "response.output_item.done" => {
                if let Some(item) = value.get("item") {
                    match item.get("type").and_then(|t| t.as_str()) {
                        Some("web_search_call") => {
                            if let Some(persisted) = self.persist_web_search_raw(item) {
                                return vec![StreamEvent::WebSearchCallItem(persisted)];
                            }
                        }
                        // 兜底：只发终态 item、不发增量的实现也能拿到参数
                        Some("function_call") => {
                            let Some(key) = str_field(item, "id").or(str_field(item, "call_id"))
                            else {
                                return Vec::new();
                            };
                            return self.whole_arguments(
                                key,
                                str_field(item, "call_id"),
                                str_field(item, "name"),
                                str_field(item, "arguments"),
                            );
                        }
                        _ => {}
                    }
                }
            }
            "response.function_call_arguments.delta" => {
                let Some(key) = str_field(value, "item_id").or(str_field(value, "call_id")) else {
                    return Vec::new();
                };
                let index = self.index_for(key);
                self.args_seen.insert(key.to_string());
                return vec![StreamEvent::ToolCallDelta(ToolCallDelta {
                    index,
                    // 增量事件不带 call_id，用 added 时记下的映射回填
                    id: self.item_to_call.get(key).cloned(),
                    name: str_field(value, "name").map(str::to_string),
                    arguments: str_field(value, "delta").map(str::to_string),
                })];
            }
            "response.function_call_arguments.done" => {
                let Some(key) = str_field(value, "item_id").or(str_field(value, "call_id")) else {
                    return Vec::new();
                };
                return self.whole_arguments(
                    key,
                    str_field(value, "call_id"),
                    str_field(value, "name"),
                    str_field(value, "arguments"),
                );
            }
            "response.web_search_call.in_progress" => {
                return vec![StreamEvent::WebSearchStatus(WebSearchStatus::InProgress {
                    call_id: self.web_search_id(),
                })];
            }
            "response.web_search_call.searching" => {
                return vec![StreamEvent::WebSearchStatus(WebSearchStatus::Searching {
                    call_id: self.web_search_id(),
                })];
            }
            "response.web_search_call.completed" => {
                let queries = search_queries_from_event(value);
                let call_id = self.web_search_id();
                let query_label = queries.as_ref().and_then(|list| {
                    if list.is_empty() {
                        None
                    } else {
                        Some(list.join(" · "))
                    }
                });
                let mut events = vec![StreamEvent::WebSearchStatus(WebSearchStatus::Completed {
                    call_id: call_id.clone(),
                    query: query_label,
                })];
                if let Some(item) = self.persist_web_search_synthetic(
                    &call_id,
                    queries,
                    "completed",
                ) {
                    events.push(StreamEvent::WebSearchCallItem(item));
                }
                return events;
            }
            "response.web_search_call.failed" => {
                let message = value
                    .pointer("/error/message")
                    .or_else(|| value.get("message"))
                    .and_then(|m| m.as_str())
                    .map(str::to_string);
                let call_id = self.web_search_id();
                let mut events = vec![StreamEvent::WebSearchStatus(WebSearchStatus::Failed {
                    call_id: call_id.clone(),
                    message: message.clone(),
                })];
                if let Some(item) =
                    self.persist_web_search_synthetic(&call_id, None, "failed")
                {
                    events.push(StreamEvent::WebSearchCallItem(item));
                }
                return events;
            }
            "response.completed" | "response.incomplete" | "response.failed" => {
                self.finished_emitted = true;
                let reason = match kind {
                    "response.completed" => Some("completed".into()),
                    "response.incomplete" => Some("incomplete".into()),
                    _ => Some("failed".into()),
                };
                return vec![StreamEvent::Finished { reason }];
            }
            _ => {}
        }

        Vec::new()
    }

    fn web_search_id(&self) -> String {
        self.web_search_call_id
            .clone()
            .unwrap_or_else(|| "native".into())
    }

    fn persist_web_search_raw(&mut self, item: &Value) -> Option<Value> {
        let id = item
            .get("id")
            .or_else(|| item.get("call_id"))
            .and_then(|v| v.as_str())?;
        if self.web_search_persisted.contains_key(id) {
            return None;
        }
        // `output_item.done` 常早于 completed，且不带 action；等 completed 合成完整 item。
        item.get("action")?;
        self.web_search_persisted.insert(id.to_string(), ());
        Some(normalize_web_search_call_item(item))
    }

    fn persist_web_search_synthetic(
        &mut self,
        call_id: &str,
        queries: Option<Vec<String>>,
        status: &str,
    ) -> Option<Value> {
        if self.web_search_persisted.contains_key(call_id) {
            return None;
        }
        self.web_search_persisted.insert(call_id.to_string(), ());
        Some(web_search_call_item(call_id, status, queries))
    }

    /// 收到「整段参数」（`*.done`）时补一条增量。
    ///
    /// 已经按增量拼过的 item 直接跳过——再追加一遍会拼成 `{…}{…}`。
    fn whole_arguments(
        &mut self,
        key: &str,
        call_id: Option<&str>,
        name: Option<&str>,
        arguments: Option<&str>,
    ) -> Vec<StreamEvent> {
        if !self.args_seen.insert(key.to_string()) {
            return Vec::new();
        }
        let index = self.index_for(key);
        let id = call_id
            .map(str::to_string)
            .or_else(|| self.item_to_call.get(key).cloned());
        vec![StreamEvent::ToolCallDelta(ToolCallDelta {
            index,
            id,
            name: name.map(str::to_string),
            arguments: arguments.map(str::to_string),
        })]
    }

    fn index_for(&mut self, call_id: &str) -> usize {
        if let Some(&index) = self.call_indices.get(call_id) {
            return index;
        }
        let index = self.next_index;
        self.next_index += 1;
        self.call_indices.insert(call_id.to_string(), index);
        index
    }
}

/// 取一个非空字符串字段。
fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn search_queries_from_event(value: &Value) -> Option<Vec<String>> {
    let from_array = value.pointer("/action/queries").and_then(|q| q.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect::<Vec<_>>()
    });
    if let Some(list) = from_array.filter(|q| !q.is_empty()) {
        return Some(list);
    }
    value
        .pointer("/action/query")
        .or_else(|| value.get("query"))
        .and_then(|q| q.as_str())
        .filter(|s| !s.is_empty())
        .map(|q| vec![q.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::event::CompletionAssembler;

    #[test]
    fn text_reasoning_and_finish() {
        let mut parser = ResponsesSseParser::default();
        let mut events = Vec::new();
        for line in [
            r#"data: {"type":"response.output_text.delta","delta":"你好"}"#,
            r#"data: {"type":"response.reasoning_text.delta","delta":"想一下"}"#,
            r#"data: {"type":"response.completed"}"#,
        ] {
            if let Ok(Some(batch)) = parser.parse_line(line) {
                events.extend(batch);
            }
        }
        assert_eq!(
            events,
            vec![
                StreamEvent::TextDelta("你好".into()),
                StreamEvent::ReasoningDelta("想一下".into()),
                StreamEvent::Finished {
                    reason: Some("completed".into())
                },
            ]
        );
    }

    fn assemble(lines: &[&str]) -> crate::event::ChatCompletion {
        let mut parser = ResponsesSseParser::default();
        let mut asm = CompletionAssembler::default();
        for line in lines {
            if let Ok(Some(batch)) = parser.parse_line(line) {
                for ev in batch {
                    asm.apply(&ev);
                }
            }
        }
        asm.into_completion()
    }

    /// 增量事件只带 `item_id`（`fc_…`），`call_id` 只在 added 出现一次。
    /// 照 `call_id` 认增量会把参数丢光，模型就会顶着空参数一路重试。
    #[test]
    fn function_call_stream_keys_on_item_id() {
        let done = assemble(&[
            r#"data: {"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"echo"}}"#,
            r#"data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":0,"delta":"{\"text\":\""}"#,
            r#"data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":0,"delta":"hi\"}"}"#,
            r#"data: {"type":"response.completed"}"#,
        ]);
        assert_eq!(done.tool_calls.len(), 1);
        assert_eq!(done.tool_calls[0].id, "call_1");
        assert_eq!(done.tool_calls[0].name, "echo");
        assert_eq!(done.tool_calls[0].arguments, r#"{"text":"hi"}"#);
    }

    #[test]
    fn parallel_function_calls_keep_separate_slots() {
        let done = assemble(&[
            r#"data: {"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"a"}}"#,
            r#"data: {"type":"response.output_item.added","output_index":1,"item":{"type":"function_call","id":"fc_2","call_id":"call_2","name":"b"}}"#,
            r#"data: {"type":"response.function_call_arguments.delta","item_id":"fc_2","delta":"{\"y\":2}"}"#,
            r#"data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"{\"x\":1}"}"#,
            r#"data: {"type":"response.completed"}"#,
        ]);
        assert_eq!(done.tool_calls.len(), 2);
        assert_eq!(done.tool_calls[0].id, "call_1");
        assert_eq!(done.tool_calls[0].arguments, r#"{"x":1}"#);
        assert_eq!(done.tool_calls[1].id, "call_2");
        assert_eq!(done.tool_calls[1].arguments, r#"{"y":2}"#);
    }

    /// 只发终态、不发增量的实现。
    #[test]
    fn arguments_done_fills_when_no_delta() {
        let done = assemble(&[
            r#"data: {"type":"response.output_item.added","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"echo"}}"#,
            r#"data: {"type":"response.function_call_arguments.done","item_id":"fc_1","name":"echo","arguments":"{\"text\":\"hi\"}"}"#,
            r#"data: {"type":"response.completed"}"#,
        ]);
        assert_eq!(done.tool_calls.len(), 1);
        assert_eq!(done.tool_calls[0].id, "call_1");
        assert_eq!(done.tool_calls[0].arguments, r#"{"text":"hi"}"#);
    }

    #[test]
    fn output_item_done_fills_when_no_delta() {
        let done = assemble(&[
            r#"data: {"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"echo","arguments":"{\"text\":\"hi\"}"}}"#,
            r#"data: {"type":"response.completed"}"#,
        ]);
        assert_eq!(done.tool_calls.len(), 1);
        assert_eq!(done.tool_calls[0].id, "call_1");
        assert_eq!(done.tool_calls[0].arguments, r#"{"text":"hi"}"#);
    }

    /// 增量 + 终态都发时不能把参数拼两遍。
    #[test]
    fn done_does_not_duplicate_streamed_arguments() {
        let done = assemble(&[
            r#"data: {"type":"response.output_item.added","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"echo"}}"#,
            r#"data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"{\"text\":\"hi\"}"}"#,
            r#"data: {"type":"response.function_call_arguments.done","item_id":"fc_1","arguments":"{\"text\":\"hi\"}"}"#,
            r#"data: {"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"echo","arguments":"{\"text\":\"hi\"}"}}"#,
            r#"data: {"type":"response.completed"}"#,
        ]);
        assert_eq!(done.tool_calls.len(), 1);
        assert_eq!(done.tool_calls[0].arguments, r#"{"text":"hi"}"#);
    }

    /// 有的实现在增量里回的是 call_id，不能因此丢参数。
    #[test]
    fn delta_keyed_on_call_id_still_works() {
        let done = assemble(&[
            r#"data: {"type":"response.output_item.added","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"echo"}}"#,
            r#"data: {"type":"response.function_call_arguments.delta","call_id":"call_1","delta":"{\"text\":\"hi\"}"}"#,
            r#"data: {"type":"response.completed"}"#,
        ]);
        assert_eq!(done.tool_calls.len(), 1);
        assert_eq!(done.tool_calls[0].id, "call_1");
        assert_eq!(done.tool_calls[0].arguments, r#"{"text":"hi"}"#);
    }

    #[test]
    fn web_search_status_and_persist() {
        let mut parser = ResponsesSseParser::default();
        let mut events = Vec::new();
        for line in [
            r#"data: {"type":"response.output_item.added","item":{"type":"web_search_call","id":"ws1"}}"#,
            r#"data: {"type":"response.web_search_call.searching"}"#,
            r#"data: {"type":"response.web_search_call.completed","action":{"query":"天气"}}"#,
        ] {
            if let Ok(Some(batch)) = parser.parse_line(line) {
                events.extend(batch);
            }
        }
        assert!(events.iter().any(|e| matches!(e, StreamEvent::WebSearchStatus(_))));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, StreamEvent::WebSearchCallItem(_)))
                .count(),
            1
        );
    }

    #[test]
    fn output_item_done_without_action_waits_for_completed() {
        let mut parser = ResponsesSseParser::default();
        let added = r#"data: {"type":"response.output_item.added","item":{"type":"web_search_call","id":"ws1"}}"#;
        let _ = parser.parse_line(added).unwrap();
        let done = r#"data: {"type":"response.output_item.done","item":{"type":"web_search_call","id":"ws1","status":"completed"}}"#;
        assert!(parser.parse_line(done).unwrap().unwrap().is_empty());
        let completed =
            r#"data: {"type":"response.web_search_call.completed","action":{"query":"天气"}}"#;
        let events = parser.parse_line(completed).unwrap().unwrap();
        let item = events
            .iter()
            .find_map(|e| match e {
                StreamEvent::WebSearchCallItem(v) => Some(v.clone()),
                _ => None,
            })
            .expect("completed should persist searchable item");
        assert_eq!(item["action"]["type"], "search");
        assert_eq!(item["action"]["queries"], json!(["天气"]));
    }

    #[test]
    fn output_item_done_persists_web_search() {
        let mut parser = ResponsesSseParser::default();
        let line = r#"data: {"type":"response.output_item.done","item":{"type":"web_search_call","id":"ws9","status":"completed","action":{"type":"search","queries":["rust"]}}}"#;
        let events = parser.parse_line(line).unwrap().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], StreamEvent::WebSearchCallItem(_)));
    }
}
