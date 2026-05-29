//! Anthropic Messages API chat provider.
//!
//! NOTE: not exercised in CI (no API keys / network egress in this environment);
//! implemented against the documented API.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::provider::{
    stream_sse, truncate, ChatRequest, LlmError, LlmProvider, ModelInfo, TokenStream,
};

pub struct AnthropicProvider;

fn extract_delta(v: &Value) -> Option<String> {
    if v.get("type").and_then(Value::as_str) == Some("content_block_delta") {
        v["delta"]["text"].as_str().map(str::to_string)
    } else {
        None
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }
    fn label(&self) -> &'static str {
        "Anthropic"
    }
    fn requires_key(&self) -> bool {
        true
    }
    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo::text("claude-3-5-sonnet-latest", "Claude 3.5 Sonnet"),
            ModelInfo::text("claude-3-5-haiku-latest", "Claude 3.5 Haiku"),
        ]
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
        api_key: Option<&str>,
    ) -> Result<TokenStream, LlmError> {
        let key = api_key.ok_or(LlmError::MissingKey)?;

        // Anthropic carries the system prompt separately and requires max_tokens.
        let messages: Vec<Value> = req
            .messages
            .iter()
            .filter(|m| m.role == "user" || m.role == "assistant")
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();

        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens.unwrap_or(1024),
            "messages": messages,
            "stream": true,
        });
        if let Some(system) = &req.system {
            body["system"] = json!(system);
        }
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }

        let resp = reqwest::Client::new()
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let message = truncate(resp.text().await.unwrap_or_default(), 500);
            return Err(LlmError::Upstream { status, message });
        }

        Ok(stream_sse(resp, extract_delta))
    }
}
