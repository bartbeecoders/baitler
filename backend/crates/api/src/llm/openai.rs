//! OpenAI-compatible chat provider (covers OpenAI and OpenRouter, which share
//! the `/chat/completions` streaming API).
//!
//! NOTE: not exercised in CI (no API keys / network egress in this environment);
//! implemented against the documented API.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::provider::{
    stream_sse, truncate, ChatRequest, LlmError, LlmProvider, ModelInfo, TokenStream,
};

pub struct OpenAiCompatProvider {
    id: &'static str,
    label: &'static str,
    base_url: String,
    models: Vec<ModelInfo>,
}

impl OpenAiCompatProvider {
    pub fn openai() -> Self {
        Self {
            id: "openai",
            label: "OpenAI",
            base_url: "https://api.openai.com/v1".to_string(),
            models: vec![
                ModelInfo::text("gpt-4o", "GPT-4o"),
                ModelInfo::text("gpt-4o-mini", "GPT-4o mini"),
            ],
        }
    }

    pub fn openrouter() -> Self {
        Self {
            id: "openrouter",
            label: "OpenRouter",
            base_url: "https://openrouter.ai/api/v1".to_string(),
            models: vec![
                ModelInfo::text("openai/gpt-4o-mini", "GPT-4o mini (OpenRouter)"),
                ModelInfo::text(
                    "anthropic/claude-3.5-sonnet",
                    "Claude 3.5 Sonnet (OpenRouter)",
                ),
            ],
        }
    }
}

fn extract_delta(v: &Value) -> Option<String> {
    v["choices"][0]["delta"]["content"]
        .as_str()
        .map(str::to_string)
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    fn id(&self) -> &'static str {
        self.id
    }
    fn label(&self) -> &'static str {
        self.label
    }
    fn requires_key(&self) -> bool {
        true
    }
    fn models(&self) -> Vec<ModelInfo> {
        self.models.clone()
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
        api_key: Option<&str>,
    ) -> Result<TokenStream, LlmError> {
        let key = api_key.ok_or(LlmError::MissingKey)?;

        let mut messages: Vec<Value> = Vec::new();
        if let Some(system) = &req.system {
            messages.push(json!({ "role": "system", "content": system }));
        }
        for m in &req.messages {
            messages.push(json!({ "role": m.role, "content": m.content }));
        }

        let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(mt) = req.max_tokens {
            body["max_tokens"] = json!(mt);
        }

        let resp = reqwest::Client::new()
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(key)
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
