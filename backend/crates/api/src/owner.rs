//! The current resource owner.
//!
//! STUB: authentication is added in the final phase. Until then, every request
//! resolves to a single fixed dev owner, so the data model and all queries are
//! already owner-scoped — the auth phase only has to replace this extractor to
//! derive the owner from the session, with no changes to repositories/handlers.

use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

/// Owner id used for all requests until auth lands.
pub const DEV_OWNER: &str = "dev";

/// The owner of the resources a request may access.
#[derive(Debug, Clone)]
pub struct CurrentOwner(pub String);

impl<S> FromRequestParts<S> for CurrentOwner
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(CurrentOwner(DEV_OWNER.to_string()))
    }
}
