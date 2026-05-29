//! Request/response shapes for the AI endpoints.

use serde::{Deserialize, Serialize};

use crate::llm::{ChatMessage, ModelInfo};

/// `PUT /ai/keys/:provider`
#[derive(Debug, Deserialize)]
pub struct SetKeyBody {
    pub api_key: String,
}

/// `POST /ai/chat`
#[derive(Debug, Deserialize)]
pub struct ChatBody {
    pub provider: String,
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub system: Option<String>,
    /// Optional grounding text (e.g. an idea/file the user attached).
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ProviderDto {
    pub id: String,
    pub label: String,
    pub requires_key: bool,
    /// Whether this owner can use the provider (mock, or a key is stored).
    pub configured: bool,
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderDto>,
}

#[derive(Debug, Serialize)]
pub struct KeysResponse {
    /// Provider ids that have a stored key for this owner.
    pub configured: Vec<String>,
}
