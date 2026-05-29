//! Application configuration, loaded from environment variables.
//!
//! Everything is read once at startup into an immutable [`Config`]. Defaults are
//! chosen so the server runs out of the box with zero external dependencies:
//! `SURREAL_URL=memory` spins up an embedded in-process datastore, so neither a
//! `surreal` server nor Docker is required for local development or tests.
//!
//! Validation is fail-fast: anything that would otherwise surface as a confusing
//! runtime panic or a silent misconfiguration (an unparseable CORS origin, a
//! wildcard origin incompatible with credentialed CORS, a half-set credential
//! pair) is rejected here with a clear [`ConfigError`].

use std::env;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use axum::http::HeaderValue;
use thiserror::Error;

/// Errors that can occur while loading [`Config`] from the environment.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("environment variable `{name}` has invalid value {value:?}: {reason}")]
    Invalid {
        name: &'static str,
        value: String,
        reason: String,
    },
}

impl ConfigError {
    fn invalid(name: &'static str, value: impl Into<String>, reason: impl Into<String>) -> Self {
        ConfigError::Invalid {
            name,
            value: value.into(),
            reason: reason.into(),
        }
    }
}

/// Fully-resolved runtime configuration.
#[derive(Clone)]
pub struct Config {
    /// Address the HTTP server binds to.
    pub bind_addr: SocketAddr,
    /// Origins allowed by CORS (the frontend dev/prod URLs). Validated at load
    /// time: each is a parseable origin and none is the `*` wildcard.
    pub cors_allowed_origins: Vec<String>,
    /// Upper bound on a single database operation (e.g. the health-check ping),
    /// so a slow or half-open datastore can't make a request hang forever.
    pub db_timeout: Duration,
    /// SurrealDB connection settings.
    pub surreal: SurrealConfig,
    /// File storage settings.
    pub storage: StorageConfig,
}

/// File storage settings.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Backend kind: `local` (only option in Phase 4; S3 comes later).
    pub backend: String,
    /// Filesystem root for the `local` backend.
    pub local_path: String,
    /// Maximum accepted upload size, in bytes.
    pub max_upload_bytes: usize,
}

/// SurrealDB connection settings.
#[derive(Clone)]
pub struct SurrealConfig {
    /// Connection endpoint understood by the SurrealDB `any` engine, e.g.
    /// `memory`, `ws://127.0.0.1:8000`, `http://…`, `rocksdb://path`.
    pub url: String,
    /// Root username for remote servers. `None` for the embedded `memory`
    /// engine, which has no auth. Must be set together with `password`.
    pub username: Option<String>,
    /// Root password for remote servers. Must be set together with `username`.
    pub password: Option<String>,
    /// Namespace to select after connecting.
    pub namespace: String,
    /// Database to select after connecting.
    pub database: String,
}

impl Config {
    /// Load configuration from the process environment, applying defaults.
    ///
    /// Reads (with defaults in parentheses):
    /// - `PORT` (8080), `BIND_HOST` (0.0.0.0)
    /// - `CORS_ALLOWED_ORIGINS` (http://localhost:5173) — comma-separated
    /// - `SURREAL_TIMEOUT_SECS` (5)
    /// - `SURREAL_URL` (memory), `SURREAL_USER`, `SURREAL_PASS`,
    ///   `SURREAL_NS` (baitler), `SURREAL_DB` (baitler)
    ///
    /// Note: the bind host is `BIND_HOST`, not `HOST` — the latter is commonly
    /// set by shells/build tooling (e.g. conda sets it to a target triple) and
    /// would otherwise be misread as a bind address.
    pub fn from_env() -> Result<Self, ConfigError> {
        let host: IpAddr = match env::var("BIND_HOST") {
            Ok(raw) => raw
                .parse()
                .map_err(|e| ConfigError::invalid("BIND_HOST", raw, format!("{e}")))?,
            Err(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        };

        let port: u16 = match env::var("PORT") {
            Ok(raw) => raw
                .parse()
                .map_err(|e| ConfigError::invalid("PORT", raw, format!("{e}")))?,
            Err(_) => 8080,
        };

        let cors_allowed_origins = parse_cors_origins(
            &env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
        )?;
        if cors_allowed_origins.is_empty() {
            tracing::warn!(
                "CORS_ALLOWED_ORIGINS is empty — all cross-origin browser requests will be rejected"
            );
        }

        let db_timeout = match env::var("SURREAL_TIMEOUT_SECS") {
            Ok(raw) => {
                let secs: u64 = raw.parse().map_err(|e| {
                    ConfigError::invalid("SURREAL_TIMEOUT_SECS", raw, format!("{e}"))
                })?;
                Duration::from_secs(secs)
            }
            Err(_) => Duration::from_secs(5),
        };

        let username = non_empty(env::var("SURREAL_USER").ok());
        let password = non_empty(env::var("SURREAL_PASS").ok());
        // A half-set credential pair would silently connect unauthenticated;
        // reject it rather than swallow the misconfiguration.
        if username.is_some() != password.is_some() {
            return Err(ConfigError::invalid(
                "SURREAL_USER/SURREAL_PASS",
                "<partial>",
                "both must be set together, or neither",
            ));
        }

        let surreal = SurrealConfig {
            url: env::var("SURREAL_URL").unwrap_or_else(|_| "memory".to_string()),
            username,
            password,
            namespace: env::var("SURREAL_NS").unwrap_or_else(|_| "baitler".to_string()),
            database: env::var("SURREAL_DB").unwrap_or_else(|_| "baitler".to_string()),
        };

        let max_upload_mb: u64 = match env::var("MAX_UPLOAD_MB") {
            Ok(raw) => raw
                .parse()
                .map_err(|e| ConfigError::invalid("MAX_UPLOAD_MB", raw, format!("{e}")))?,
            Err(_) => 50,
        };
        let storage = StorageConfig {
            backend: env::var("STORAGE_BACKEND").unwrap_or_else(|_| "local".to_string()),
            local_path: env::var("STORAGE_LOCAL_PATH")
                .unwrap_or_else(|_| "./data/files".to_string()),
            max_upload_bytes: (max_upload_mb * 1024 * 1024) as usize,
        };

        Ok(Self {
            bind_addr: SocketAddr::new(host, port),
            cors_allowed_origins,
            db_timeout,
            surreal,
            storage,
        })
    }
}

/// Parse and validate the comma-separated CORS origins list.
///
/// Rejects the `*` wildcard (incompatible with credentialed CORS, and a startup
/// panic in tower-http's origin list) and any value that is not a valid HTTP
/// header value. Empty/whitespace entries are dropped.
fn parse_cors_origins(raw: &str) -> Result<Vec<String>, ConfigError> {
    let mut origins = Vec::new();
    for origin in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if origin == "*" {
            return Err(ConfigError::invalid(
                "CORS_ALLOWED_ORIGINS",
                origin,
                "wildcard `*` is not allowed because credentialed CORS is enabled; \
                 list explicit origins instead",
            ));
        }
        if origin.parse::<HeaderValue>().is_err() {
            return Err(ConfigError::invalid(
                "CORS_ALLOWED_ORIGINS",
                origin,
                "not a valid origin header value",
            ));
        }
        origins.push(origin.to_string());
    }
    Ok(origins)
}

/// Treat empty/whitespace-only env values as absent.
fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// Hand-written `Debug` impls so a stray `?config` / `dbg!` never dumps the root
// password (which would otherwise land in structured logs).
impl fmt::Debug for SurrealConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SurrealConfig")
            .field("url", &self.url)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .field("namespace", &self.namespace)
            .field("database", &self.database)
            .finish()
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bind_addr", &self.bind_addr)
            .field("cors_allowed_origins", &self.cors_allowed_origins)
            .field("db_timeout", &self.db_timeout)
            .field("surreal", &self.surreal)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize env-mutating tests; the process environment is global state.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const VARS: &[&str] = &[
        "BIND_HOST",
        "PORT",
        "CORS_ALLOWED_ORIGINS",
        "SURREAL_TIMEOUT_SECS",
        "SURREAL_URL",
        "SURREAL_USER",
        "SURREAL_PASS",
        "SURREAL_NS",
        "SURREAL_DB",
    ];

    fn clear_env() {
        for v in VARS {
            env::remove_var(v);
        }
    }

    #[test]
    fn defaults_apply_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();

        let cfg = Config::from_env().expect("defaults should be valid");
        assert_eq!(cfg.bind_addr.port(), 8080);
        assert!(cfg.bind_addr.ip().is_unspecified());
        assert_eq!(cfg.cors_allowed_origins, vec!["http://localhost:5173"]);
        assert_eq!(cfg.db_timeout, Duration::from_secs(5));
        assert_eq!(cfg.surreal.url, "memory");
        assert_eq!(cfg.surreal.namespace, "baitler");
        assert!(cfg.surreal.username.is_none());
    }

    #[test]
    fn invalid_port_is_rejected() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        env::set_var("PORT", "not-a-number");

        let err = Config::from_env().unwrap_err();
        let ConfigError::Invalid { name, .. } = err;
        assert_eq!(name, "PORT");
        clear_env();
    }

    #[test]
    fn invalid_bind_host_is_rejected() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        env::set_var("BIND_HOST", "x86_64-conda-linux-gnu");

        let err = Config::from_env().unwrap_err();
        let ConfigError::Invalid { name, .. } = err;
        assert_eq!(name, "BIND_HOST");
        clear_env();
    }

    #[test]
    fn cors_origins_are_split_and_trimmed() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        env::set_var("CORS_ALLOWED_ORIGINS", " http://a.test , , http://b.test ");

        let cfg = Config::from_env().expect("valid origins");
        assert_eq!(
            cfg.cors_allowed_origins,
            vec!["http://a.test", "http://b.test"]
        );
        clear_env();
    }

    #[test]
    fn wildcard_cors_origin_is_rejected() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        env::set_var("CORS_ALLOWED_ORIGINS", "*");

        let err = Config::from_env().unwrap_err();
        let ConfigError::Invalid { name, .. } = err;
        assert_eq!(name, "CORS_ALLOWED_ORIGINS");
        clear_env();
    }

    #[test]
    fn partial_credentials_are_rejected() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        env::set_var("SURREAL_USER", "root");
        // SURREAL_PASS intentionally unset.

        let err = Config::from_env().unwrap_err();
        let ConfigError::Invalid { name, .. } = err;
        assert_eq!(name, "SURREAL_USER/SURREAL_PASS");
        clear_env();
    }

    #[test]
    fn non_empty_maps_whitespace_to_none() {
        assert_eq!(non_empty(Some("  ".to_string())), None);
        assert_eq!(non_empty(Some("x".to_string())), Some("x".to_string()));
        assert_eq!(non_empty(None), None);
    }

    #[test]
    fn debug_redacts_password() {
        let cfg = SurrealConfig {
            url: "ws://localhost:8000".to_string(),
            username: Some("root".to_string()),
            password: Some("super-secret".to_string()),
            namespace: "ns".to_string(),
            database: "db".to_string(),
        };
        let rendered = format!("{cfg:?}");
        assert!(
            !rendered.contains("super-secret"),
            "password leaked: {rendered}"
        );
        assert!(rendered.contains("***"));
    }
}
