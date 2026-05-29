//! Shared application state passed to every handler.

use std::sync::Arc;

use crate::config::Config;
use crate::db::Db;
use crate::storage::Storage;

/// Cloneable handle to shared resources. `Clone` is cheap: it bumps the inner
/// `Arc` refcounts. Injected into handlers via axum's `State` extractor.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
    pub storage: Arc<dyn Storage>,
}

impl AppState {
    pub fn new(config: Config, db: Db, storage: Arc<dyn Storage>) -> Self {
        Self {
            config: Arc::new(config),
            db,
            storage,
        }
    }
}
