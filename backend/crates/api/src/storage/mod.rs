//! Object storage abstraction.
//!
//! Files are stored under opaque, server-generated keys (UUIDs) — never under
//! client-supplied names — so there is no path-traversal surface. The [`Storage`]
//! trait is dyn-compatible (via `async-trait`) so the backend can be swapped at
//! runtime; Phase 4 ships only [`local::LocalStorage`], with an S3-compatible
//! backend planned for later.

pub mod local;

use async_trait::async_trait;
use thiserror::Error;
use tokio::io::AsyncRead;

/// A readable byte stream for a stored object.
pub type ObjectReader = Box<dyn AsyncRead + Send + Unpin>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage object not found")]
    NotFound,

    #[error("invalid storage key")]
    InvalidKey,

    #[error("storage io error: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait Storage: Send + Sync {
    /// Write `bytes` to `key`, overwriting any existing object.
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError>;

    /// Open `key` for streaming reads.
    async fn open(&self, key: &str) -> Result<ObjectReader, StorageError>;

    /// Delete `key`. Succeeds even if the object is already gone.
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
}

/// Reject keys that could escape the storage root. Keys are server-generated
/// UUIDs, so this is defense-in-depth.
fn validate_key(key: &str) -> Result<(), StorageError> {
    let safe = !key.is_empty()
        && !key.contains('/')
        && !key.contains('\\')
        && !key.contains("..")
        && !key.contains('\0');
    if safe {
        Ok(())
    } else {
        Err(StorageError::InvalidKey)
    }
}
