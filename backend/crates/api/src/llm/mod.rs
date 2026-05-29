//! Multi-provider LLM layer: a provider-neutral trait and a registry of
//! adapters (mock + OpenAI-compatible + Anthropic).

mod anthropic;
mod mock;
mod openai;
pub mod provider;

use std::sync::Arc;

use anthropic::AnthropicProvider;
use mock::MockProvider;
use openai::OpenAiCompatProvider;

pub use provider::{ChatMessage, ChatRequest, LlmError, LlmProvider, ModelInfo};

/// Registry of available providers. Cheap to share via `Arc`.
pub struct LlmRegistry {
    providers: Vec<Arc<dyn LlmProvider>>,
}

impl LlmRegistry {
    /// Build the default registry. The mock is always present; the real
    /// adapters require a per-owner API key to actually run.
    pub fn with_defaults() -> Self {
        Self {
            providers: vec![
                Arc::new(MockProvider),
                Arc::new(OpenAiCompatProvider::openai()),
                Arc::new(OpenAiCompatProvider::openrouter()),
                Arc::new(AnthropicProvider),
            ],
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn LlmProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    pub fn all(&self) -> &[Arc<dyn LlmProvider>] {
        &self.providers
    }
}

impl Default for LlmRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
