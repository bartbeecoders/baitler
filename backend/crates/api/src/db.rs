//! SurrealDB connection management.
//!
//! Uses the SurrealDB `any` engine so the connection target is chosen at runtime
//! from a single URL string:
//! - `rocksdb://path` — embedded persistent, file-based (the default; data
//!   survives restarts). Backed by the `kv-rocksdb` cargo feature.
//! - `memory` — embedded, in-process, ephemeral (great for tests)
//! - `ws://host:port` / `wss://…` — remote server over WebSocket
//! - `http://host:port` / `https://…` — remote server over HTTP
//! - `surrealkv://path` — alternative embedded persistent engine (requires the
//!   `kv-surrealkv` feature, not enabled here)

use std::error::Error;
use std::path::Path;

use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

use crate::config::SurrealConfig;

/// Boxed error so connecting can surface both SurrealDB failures and the
/// filesystem error from preparing an embedded engine's data directory.
type ConnectError = Box<dyn Error + Send + Sync>;

/// A handle to the connected database. Cheap to clone (internally reference
/// counted) and shared across requests via application state.
pub type Db = Surreal<Any>;

/// Connect to SurrealDB, authenticate if credentials are configured, and select
/// the configured namespace and database.
pub async fn connect(cfg: &SurrealConfig) -> Result<Db, ConnectError> {
    tracing::info!(
        url = %redact_url(&cfg.url),
        ns = %cfg.namespace,
        db = %cfg.database,
        "connecting to SurrealDB"
    );

    if cfg.url == "memory" {
        tracing::warn!(
            "SURREAL_URL=memory: using an ephemeral in-process datastore — all data is lost on \
             restart. Set a rocksdb:// or ws:// URL for anything but tests."
        );
    }

    // Embedded persistent engines (rocksdb://) store their files in a directory.
    // RocksDB creates the leaf directory itself but not missing parents, so make
    // sure the full path exists before connecting — otherwise the open fails with
    // an opaque "IO error" instead of a clear startup error.
    ensure_storage_dir(&cfg.url)?;

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

/// Create the on-disk directory for an embedded persistent engine, if the URL
/// names one. Non-file URLs (`memory`, `ws://…`, `http://…`) are left untouched.
///
/// `rocksdb://./data/surreal.db` stores its files inside `./data/surreal.db`;
/// we `create_dir_all` that path so a fresh checkout (with no `./data` yet)
/// starts cleanly instead of erroring on a missing parent directory.
fn ensure_storage_dir(url: &str) -> std::io::Result<()> {
    // Embedded engines whose target is a filesystem path. `file` is RocksDB's
    // legacy alias; include it so either spelling works.
    const FILE_SCHEMES: &[&str] = &["rocksdb", "surrealkv", "file"];

    let Some((scheme, path)) = url.split_once("://") else {
        return Ok(());
    };
    if FILE_SCHEMES.contains(&scheme) && !path.is_empty() {
        std::fs::create_dir_all(Path::new(path))?;
    }
    Ok(())
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
    use super::{ensure_storage_dir, redact_url};

    #[test]
    fn ensure_storage_dir_creates_rocksdb_path() {
        let base = std::env::temp_dir().join("baitler-db-test-rocksdb");
        let _ = std::fs::remove_dir_all(&base);
        // Nested path: exercises that missing parents are created too.
        let db_path = base.join("nested").join("surreal.db");
        let url = format!("rocksdb://{}", db_path.display());

        ensure_storage_dir(&url).expect("should create the directory tree");
        assert!(db_path.is_dir(), "rocksdb data dir was not created");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ensure_storage_dir_ignores_non_file_urls() {
        // No scheme, and remote/ephemeral schemes: nothing to create, no error.
        ensure_storage_dir("memory").expect("memory needs no directory");
        ensure_storage_dir("ws://127.0.0.1:8000").expect("ws needs no directory");
        ensure_storage_dir("http://127.0.0.1:8000").expect("http needs no directory");
    }

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
