//! Shared application state passed to every handler.

use std::sync::Arc;

use crate::config::Config;
use crate::db::Db;

/// Cloneable handle to shared resources. `Clone` is cheap: it bumps the inner
/// `Arc` refcounts. Injected into handlers via axum's `State` extractor.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
}

impl AppState {
    pub fn new(config: Config, db: Db) -> Self {
        Self {
            config: Arc::new(config),
            db,
        }
    }
}
