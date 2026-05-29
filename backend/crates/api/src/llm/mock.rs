//! An offline provider that streams a canned reply. Always available (no key),
//! so the chat feature works end-to-end in dev/tests without any LLM egress.

use std::time::Duration;

use async_stream::stream;
use async_trait::async_trait;

use super::provider::{ChatRequest, LlmError, LlmProvider, ModelInfo, TokenStream};

pub struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    fn id(&self) -> &'static str {
        "mock"
    }
    fn label(&self) -> &'static str {
        "Mock (offline)"
    }
    fn requires_key(&self) -> bool {
        false
    }
    fn models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo::text("mock-1", "Mock model")]
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
        _api_key: Option<&str>,
    ) -> Result<TokenStream, LlmError> {
        let last_user = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.chars().take(200).collect::<String>())
            .unwrap_or_default();
        let reply = format!(
            "This is a mock response — no real LLM was called. You said: \"{last_user}\". \
             Add a provider API key in settings to chat with a real model."
        );

        let token_stream = stream! {
            for word in reply.split_inclusive(' ') {
                tokio::time::sleep(Duration::from_millis(15)).await;
                yield Ok(word.to_string());
            }
        };
        Ok(Box::pin(token_stream))
    }
}
