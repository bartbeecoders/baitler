//! HTTP handlers for the AI feature: provider listing, encrypted key management,
//! and streaming chat (SSE).

use std::collections::HashSet;
use std::convert::Infallible;

use async_stream::stream;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt};
use serde_json::json;

use crate::crypto;
use crate::error::{AppError, AppResult};
use crate::llm::ChatRequest;
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::image;
use super::model::{
    ChatBody, ImageBody, ImageResponse, KeysResponse, ProviderDto, ProvidersResponse, SetKeyBody,
};
use super::repo;

const MAX_KEY_LEN: usize = 500;
const MAX_CONTENT_LEN: usize = 100_000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ai/providers", get(providers))
        .route("/ai/keys", get(list_keys))
        .route(
            "/ai/keys/{provider}",
            axum::routing::put(set_key).delete(delete_key),
        )
        .route("/ai/chat", post(chat))
        .route("/ai/image", post(generate_image))
}

async fn providers(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
) -> AppResult<Json<ProvidersResponse>> {
    let configured: HashSet<String> = repo::list_configured(&state.db, &owner)
        .await?
        .into_iter()
        .collect();

    let providers = state
        .llm
        .all()
        .iter()
        .map(|p| ProviderDto {
            id: p.id().to_string(),
            label: p.label().to_string(),
            requires_key: p.requires_key(),
            configured: !p.requires_key() || configured.contains(p.id()),
            models: p.models(),
        })
        .collect();

    Ok(Json(ProvidersResponse { providers }))
}

async fn list_keys(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
) -> AppResult<Json<KeysResponse>> {
    Ok(Json(KeysResponse {
        configured: repo::list_configured(&state.db, &owner).await?,
    }))
}

async fn set_key(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(provider): Path<String>,
    Json(body): Json<SetKeyBody>,
) -> AppResult<StatusCode> {
    let llm = state
        .llm
        .get(&provider)
        .ok_or_else(|| AppError::BadRequest("unknown provider".into()))?;
    if !llm.requires_key() {
        return Err(AppError::BadRequest(
            "this provider does not take an API key".into(),
        ));
    }
    let key = body.api_key.trim();
    if key.is_empty() || key.len() > MAX_KEY_LEN {
        return Err(AppError::BadRequest("invalid API key".into()));
    }

    let ciphertext = crypto::encrypt(&state.config.secret_key, key)
        .map_err(|e| AppError::Internal(Box::new(e)))?;
    repo::set_key(&state.db, &owner, llm.id(), &ciphertext).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_key(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(provider): Path<String>,
) -> AppResult<StatusCode> {
    repo::delete_key(&state.db, &owner, &provider).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn chat(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<ChatBody>,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    if body.messages.is_empty() {
        return Err(AppError::BadRequest("messages must not be empty".into()));
    }
    if body
        .messages
        .iter()
        .any(|m| m.content.len() > MAX_CONTENT_LEN)
    {
        return Err(AppError::BadRequest("a message is too long".into()));
    }

    let provider = state.llm.get(&body.provider).ok_or(AppError::NotFound)?;

    // Resolve the API key (decrypted just-in-time) for providers that need one.
    let api_key: Option<String> = if provider.requires_key() {
        let ciphertext = repo::get_ciphertext(&state.db, &owner, provider.id())
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("no API key configured for {}", provider.id()))
            })?;
        let key = crypto::decrypt(&state.config.secret_key, &ciphertext)
            .map_err(|e| AppError::Internal(Box::new(e)))?;
        Some(key)
    } else {
        None
    };

    let req = ChatRequest {
        model: body.model,
        messages: body.messages,
        system: build_system(body.system, body.context),
        temperature: body.temperature,
        max_tokens: body.max_tokens,
    };

    // Connection-time errors surface as a normal HTTP error before the stream.
    let token_stream = provider.chat_stream(req, api_key.as_deref()).await?;

    let sse = stream! {
        let mut tokens = token_stream;
        while let Some(item) = tokens.next().await {
            match item {
                Ok(delta) => {
                    yield Ok::<_, Infallible>(
                        Event::default().json_data(json!({ "delta": delta })).unwrap(),
                    );
                }
                Err(e) => {
                    yield Ok(Event::default().json_data(json!({ "error": e.to_string() })).unwrap());
                    break;
                }
            }
        }
        yield Ok(Event::default().json_data(json!({ "done": true })).unwrap());
    };

    Ok(Sse::new(sse).keep_alive(KeepAlive::default()))
}

/// Generate an image for a canvas "image" part. Only the offline `mock`
/// provider is wired today; real providers slot in behind a stored key.
async fn generate_image(
    State(_state): State<AppState>,
    CurrentOwner(_owner): CurrentOwner,
    Json(body): Json<ImageBody>,
) -> AppResult<Json<ImageResponse>> {
    let prompt = body.prompt.trim();
    if prompt.is_empty() {
        return Err(AppError::BadRequest("prompt must not be empty".into()));
    }
    if prompt.chars().count() > image::MAX_PROMPT {
        return Err(AppError::BadRequest("prompt is too long".into()));
    }
    match body.provider.as_deref().unwrap_or("mock") {
        "mock" => {
            let img = image::generate_mock(prompt);
            Ok(Json(ImageResponse {
                data_url: img.data_url,
                mime: img.mime,
                provider: img.provider,
            }))
        }
        other => Err(AppError::BadRequest(format!(
            "image generation for `{other}` is not configured yet"
        ))),
    }
}

/// Fold optional grounding context into the system prompt.
fn build_system(system: Option<String>, context: Option<String>) -> Option<String> {
    match context {
        Some(ctx) if !ctx.trim().is_empty() => {
            let base = system.unwrap_or_default();
            Some(
                format!("{base}\n\nUse the following context to answer the user:\n\n{ctx}")
                    .trim()
                    .to_string(),
            )
        }
        _ => system,
    }
}
