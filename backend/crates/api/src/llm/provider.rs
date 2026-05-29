//! Provider-neutral LLM abstraction.

use std::pin::Pin;

use async_stream::stream;
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A stream of text deltas from a chat completion.
pub type TokenStream = Pin<Box<dyn Stream<Item = Result<String, LlmError>> + Send>>;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("provider request failed: {0}")]
    Http(String),
    #[error("provider returned an error ({status})")]
    Upstream { status: u16, message: String },
    #[error("no API key configured for this provider")]
    MissingKey,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub system: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub modalities: Vec<String>,
}

impl ModelInfo {
    pub fn text(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            modalities: vec!["text".to_string()],
        }
    }
}

/// A chat LLM provider. Implementations are stateless and shared via the registry.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Whether an API key must be configured to use this provider.
    fn requires_key(&self) -> bool;
    fn models(&self) -> Vec<ModelInfo>;
    /// Start a streaming chat completion. Connection-time failures are returned
    /// as `Err`; per-token failures surface as `Err` items in the stream.
    async fn chat_stream(
        &self,
        req: ChatRequest,
        api_key: Option<&str>,
    ) -> Result<TokenStream, LlmError>;
}

/// Parse a provider's `text/event-stream` response into text deltas. `extract`
/// pulls the delta text out of one decoded `data:` JSON payload.
pub(crate) fn stream_sse(
    resp: reqwest::Response,
    extract: fn(&serde_json::Value) -> Option<String>,
) -> TokenStream {
    Box::pin(stream! {
        let mut bytes = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        while let Some(chunk) = bytes.next().await {
            match chunk {
                Ok(c) => buf.extend_from_slice(&c),
                Err(e) => {
                    yield Err(LlmError::Http(e.to_string()));
                    return;
                }
            }
            // Emit completed lines; keep any partial line in the buffer.
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let raw: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&raw);
                let Some(payload) = line.trim().strip_prefix("data:") else {
                    continue;
                };
                let payload = payload.trim();
                if payload.is_empty() {
                    continue;
                }
                if payload == "[DONE]" {
                    return;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) {
                    if let Some(text) = extract(&json) {
                        if !text.is_empty() {
                            yield Ok(text);
                        }
                    }
                }
            }
        }
    })
}

/// Truncate an upstream error body for safe inclusion in our error.
pub(crate) fn truncate(mut s: String, max: usize) -> String {
    if s.chars().count() > max {
        s = s.chars().take(max).collect();
        s.push('…');
    }
    s
}
