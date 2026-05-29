//! SurrealDB connection management.
//!
//! Uses the SurrealDB `any` engine so the connection target is chosen at runtime
//! from a single URL string:
//! - `memory` — embedded, in-process, ephemeral (default; great for dev/tests)
//! - `ws://host:port` / `wss://…` — remote server over WebSocket
//! - `http://host:port` / `https://…` — remote server over HTTP
//! - `rocksdb://path` / `surrealkv://path` — embedded persistent (requires the
//!   matching `kv-*` cargo feature, not enabled in Phase 1)

use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

use crate::config::SurrealConfig;

/// A handle to the connected database. Cheap to clone (internally reference
/// counted) and shared across requests via application state.
pub type Db = Surreal<Any>;

/// Connect to SurrealDB, authenticate if credentials are configured, and select
/// the configured namespace and database.
pub async fn connect(cfg: &SurrealConfig) -> Result<Db, surrealdb::Error> {
    tracing::info!(
        url = %redact_url(&cfg.url),
        ns = %cfg.namespace,
        db = %cfg.database,
        "connecting to SurrealDB"
    );

    if cfg.url == "memory" {
        tracing::warn!(
            "SURREAL_URL=memory: using an ephemeral in-process datastore — all data is lost on \
             restart. Set a ws:// or persistent URL for anything but dev/tests."
        );
    }

    let db = any::connect(&cfg.url).await?;

    // The embedded `memory` engine has no auth; only sign in when credentials
    // are provided (i.e. for remote servers). Config guarantees the pair is
    // either fully set or fully unset.
    if let (Some(user), Some(pass)) = (&cfg.username, &cfg.password) {
        db.signin(Root {
            username: user,
            password: pass,
        })
        .await?;
    }

    db.use_ns(&cfg.namespace).use_db(&cfg.database).await?;

    Ok(db)
}

/// Liveness/readiness probe: confirms the database answers a trivial query.
pub async fn ping(db: &Db) -> Result<(), surrealdb::Error> {
    db.query("RETURN true").await?.check()?;
    Ok(())
}

/// Strip any `user:pass@` userinfo from a connection URL before logging, so a
/// credential accidentally embedded in `SURREAL_URL` never reaches the logs.
/// Schemes without an authority (`memory`, `rocksdb://path`) pass through.
fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    match rest.split_once('@') {
        Some((_userinfo, host)) => format!("{scheme}://***@{host}"),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::redact_url;

    #[test]
    fn redacts_userinfo() {
        assert_eq!(
            redact_url("ws://root:secret@db.internal:8000"),
            "ws://***@db.internal:8000"
        );
    }

    #[test]
    fn passes_through_urls_without_userinfo() {
        assert_eq!(redact_url("memory"), "memory");
        assert_eq!(redact_url("ws://127.0.0.1:8000"), "ws://127.0.0.1:8000");
        assert_eq!(redact_url("rocksdb://./data/x.db"), "rocksdb://./data/x.db");
    }
}
