//! Tracing/logging initialization.

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Initialize the global tracing subscriber.
///
/// Honors `RUST_LOG` (e.g. `RUST_LOG=baitler_api=debug,info`); falls back to a
/// sensible default. Set `LOG_FORMAT=json` for structured JSON logs (useful in
/// production); otherwise human-readable output is used.
///
/// Safe to call more than once — a second call is a no-op rather than a panic,
/// which keeps parallel integration tests from aborting.
pub fn init() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,baitler_api=debug,tower_http=debug"));

    let json = std::env::var("LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    let registry = tracing_subscriber::registry().with(env_filter);

    let result = if json {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .try_init()
    } else {
        registry.with(tracing_subscriber::fmt::layer()).try_init()
    };

    // A failure here means a subscriber is already set (e.g. another test in the
    // same process initialized it first) — that is fine to ignore.
    let _ = result;
}
