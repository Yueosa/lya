//! [`LlmClient`]：基于 [`lya_http::HttpClient`] 的双栈 LLM 调用。

use std::pin::Pin;

use futures_core::Stream;
use futures_util::StreamExt;
use lya_http::{header, HttpClient};
use serde_json::Value;

use crate::endpoint::{ApiMode, LlmEndpoint};
use crate::error::LlmError;
use crate::event::{ChatCompletion, CompletionAssembler, StreamEvent};
use crate::message::{build_chat_body, ChatMessage};
use crate::request::ChatStreamRequest;
use crate::responses::{build_responses_body, ResponsesSseParser};
use crate::sse::{extract_completion, parse_sse_line};

/// 流式事件流类型（`'static` + `Send`，便于跨任务）。
pub type ChatEventStream =
    Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>;

/// OpenAI 兼容 LLM 客户端。
///
/// 内部只持有一个可 clone 的 [`HttpClient`]；多个 `LlmClient` / 任务
/// 应共享同一 http 池。
#[derive(Clone, Debug)]
pub struct LlmClient {
    /// 共享出站 HTTP 客户端。
    http: HttpClient,
}

impl LlmClient {
    /// 用已有 [`HttpClient`] 构造。
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// 访问内部 HTTP 客户端。
    pub fn http(&self) -> &HttpClient {
        &self.http
    }

    /// 非流式 chat（仅 completions 栈）：一次返回完整 [`ChatCompletion`]。
    pub async fn chat(
        &self,
        endpoint: &LlmEndpoint,
        messages: &[ChatMessage],
        tools: &[Value],
    ) -> Result<ChatCompletion, LlmError> {
        let body = build_chat_body(endpoint, messages, tools, false)?;
        let response = self
            .post(&endpoint.chat_completions_url(), endpoint, &body)
            .await?;
        let value: Value = response.json().await?;
        extract_completion(&value)
    }

    /// 流式 chat：按 [`ApiMode`] 分派到 completions 或 responses。
    pub async fn chat_stream(
        &self,
        mode: ApiMode,
        endpoint: &LlmEndpoint,
        request: ChatStreamRequest,
        tools: &[Value],
    ) -> Result<ChatEventStream, LlmError> {
        match mode {
            ApiMode::Completions => {
                let ChatStreamRequest::Completions(messages) = request else {
                    return Err(LlmError::Other(
                        "completions 栈需要 ChatStreamRequest::Completions".into(),
                    ));
                };
                self.chat_stream_completions(endpoint, &messages, tools).await
            }
            ApiMode::Responses => {
                let ChatStreamRequest::Responses {
                    instructions,
                    input,
                    native_web_search,
                } = request
                else {
                    return Err(LlmError::Other(
                        "responses 栈需要 ChatStreamRequest::Responses".into(),
                    ));
                };
                self.chat_stream_responses(
                    endpoint,
                    &instructions,
                    &input,
                    tools,
                    native_web_search,
                )
                .await
            }
        }
    }

    /// 流式调用并拼成完整 [`ChatCompletion`]。
    pub async fn chat_collect(
        &self,
        mode: ApiMode,
        endpoint: &LlmEndpoint,
        request: ChatStreamRequest,
        tools: &[Value],
        mut on_event: impl FnMut(&StreamEvent),
    ) -> Result<ChatCompletion, LlmError> {
        let mut stream = self
            .chat_stream(mode, endpoint, request, tools)
            .await?;
        let mut assembler = CompletionAssembler::default();
        while let Some(item) = stream.next().await {
            let event = item?;
            on_event(&event);
            assembler.apply(&event);
        }
        Ok(assembler.into_completion())
    }

    async fn chat_stream_completions(
        &self,
        endpoint: &LlmEndpoint,
        messages: &[ChatMessage],
        tools: &[Value],
    ) -> Result<ChatEventStream, LlmError> {
        let body = build_chat_body(endpoint, messages, tools, true)?;
        let response = self
            .post(&endpoint.chat_completions_url(), endpoint, &body)
            .await?;
        let byte_stream = response.bytes_stream();

        let stream = async_stream::stream! {
            let mut line_buf = String::new();
            let mut bytes = byte_stream;
            let mut finished_emitted = false;

            while let Some(item) = bytes.next().await {
                let chunk = match item {
                    Ok(c) => c,
                    Err(err) => {
                        yield Err(LlmError::Http(err));
                        return;
                    }
                };
                line_buf.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = line_buf.find('\n') {
                    let mut line = line_buf[..pos].to_string();
                    line_buf.drain(..=pos);
                    if line.ends_with('\r') {
                        line.pop();
                    }
                    match parse_sse_line(&line) {
                        Ok(Some(events)) => {
                            for ev in events {
                                if matches!(ev, StreamEvent::Finished { .. }) {
                                    finished_emitted = true;
                                }
                                yield Ok(ev);
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            yield Err(err);
                            return;
                        }
                    }
                }
            }

            if !line_buf.is_empty() {
                let line = line_buf.trim_end_matches('\r');
                match parse_sse_line(line) {
                    Ok(Some(events)) => {
                        for ev in events {
                            if matches!(ev, StreamEvent::Finished { .. }) {
                                finished_emitted = true;
                            }
                            yield Ok(ev);
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        yield Err(err);
                        return;
                    }
                }
            }

            if !finished_emitted {
                yield Ok(StreamEvent::Finished { reason: None });
            }
        };

        Ok(Box::pin(stream))
    }

    async fn chat_stream_responses(
        &self,
        endpoint: &LlmEndpoint,
        instructions: &str,
        input: &[Value],
        tools: &[Value],
        native_web_search: bool,
    ) -> Result<ChatEventStream, LlmError> {
        let body = build_responses_body(
            endpoint,
            instructions,
            input,
            tools,
            true,
            native_web_search,
        )?;
        let response = self
            .post(&endpoint.responses_url(), endpoint, &body)
            .await?;
        let byte_stream = response.bytes_stream();

        let stream = async_stream::stream! {
            let mut line_buf = String::new();
            let mut bytes = byte_stream;
            let mut parser = ResponsesSseParser::default();

            while let Some(item) = bytes.next().await {
                let chunk = match item {
                    Ok(c) => c,
                    Err(err) => {
                        yield Err(LlmError::Http(err));
                        return;
                    }
                };
                line_buf.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = line_buf.find('\n') {
                    let mut line = line_buf[..pos].to_string();
                    line_buf.drain(..=pos);
                    if line.ends_with('\r') {
                        line.pop();
                    }
                    match parser.parse_line(&line) {
                        Ok(Some(events)) => {
                            for ev in events {
                                yield Ok(ev);
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            yield Err(err);
                            return;
                        }
                    }
                }
            }

            if !line_buf.is_empty() {
                let line = line_buf.trim_end_matches('\r');
                match parser.parse_line(line) {
                    Ok(Some(events)) => {
                        for ev in events {
                            yield Ok(ev);
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        yield Err(err);
                        return;
                    }
                }
            }

            if !parser.finished_emitted() {
                yield Ok(StreamEvent::Finished { reason: None });
            }
        };

        Ok(Box::pin(stream))
    }

    async fn post(
        &self,
        url: &str,
        endpoint: &LlmEndpoint,
        body: &Value,
    ) -> Result<lya_http::HttpResponse, LlmError> {
        let auth = format!("Bearer {}", endpoint.api_key);
        let (auth_name, auth_value) = header("Authorization", &auth)?;
        let (ct_name, ct_value) = header("Content-Type", "application/json")?;
        let (accept_name, accept_value) = header("Accept", "application/json")?;

        let builder = self
            .http
            .post(url)
            .header(auth_name, auth_value)
            .header(ct_name, ct_value)
            .header(accept_name, accept_value)
            .json(body);

        let response = self.http.send(builder).await?.error_for_status().await?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::ApiMode;
    use crate::message::Role;
    use serde_json::json;

    #[test]
    fn build_body_merges_params() {
        let ep = LlmEndpoint::new("https://api.example.com/v1", "k")
            .with_param("model", json!("demo"))
            .with_param("temperature", json!(0.2));
        let msgs = [ChatMessage::user("hi")];
        let tools = [json!({"type":"function","function":{"name":"done"}})];
        let body = build_chat_body(&ep, &msgs, &tools, true).unwrap();
        assert_eq!(body["model"], "demo");
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["content"], "hi");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["tools"][0]["function"]["name"], "done");
        assert_eq!(
            ep.chat_completions_url(),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(msgs[0].role, Role::User);
    }

    #[test]
    fn responses_mode_requires_responses_params() {
        let ep = LlmEndpoint::new("https://api.example.com/v1", "k").with_param("model", json!("x"));
        assert!(build_responses_body(&ep, "sys", &[], &[], true, false).is_err());
        let ep = ep.with_mode_params(
            ApiMode::Responses,
            serde_json::from_value(json!({ "model": "flash" })).unwrap(),
        );
        assert!(build_responses_body(&ep, "sys", &[], &[], true, false).is_ok());
        assert!(build_responses_body(&ep, "sys", &[], &[], true, true).is_ok());
    }
}
