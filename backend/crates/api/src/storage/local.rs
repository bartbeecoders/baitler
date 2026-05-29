//! Local-filesystem [`Storage`] backend.

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;

use super::{validate_key, ObjectReader, Storage, StorageError};

/// Stores each object as a file named by its key under `root`.
#[derive(Debug, Clone)]
pub struct LocalStorage {
    root: PathBuf,
}

impl LocalStorage {
    /// Create the storage root if needed and return a handle.
    pub async fn new(root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let root = root.into();
        tokio::fs::create_dir_all(&root).await?;
        Ok(Self { root })
    }

    fn path_for(&self, key: &str) -> Result<PathBuf, StorageError> {
        validate_key(key)?;
        Ok(self.root.join(key))
    }
}

#[async_trait]
impl Storage for LocalStorage {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let path = self.path_for(key)?;
        // Write to a temp file then rename so a partial write never leaves a
        // corrupt object at the final key.
        let tmp = path.with_extension("tmp");
        let mut file = tokio::fs::File::create(&tmp).await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    async fn open(&self, key: &str) -> Result<ObjectReader, StorageError> {
        let path = self.path_for(key)?;
        match tokio::fs::File::open(&path).await {
            Ok(file) => Ok(Box::new(file)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(StorageError::NotFound),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = self.path_for(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
