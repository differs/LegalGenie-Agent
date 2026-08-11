//! Object storage abstraction (P3): local filesystem today, S3/MinIO later.
//!
//! All artifact paths in the codebase are relative to a storage root
//! (`files/<case>/...`, `parsed/...`, `exports/...`). Code should go through
//! `ObjectStore` so a remote backend (S3-compatible) can be swapped in
//! without touching business logic. Streaming writes still use `full_path`
//! on the local backend; remote adapters will layer streaming on top.
use std::path::{Path, PathBuf};

#[async_trait::async_trait]
pub trait ObjectStore: Send + Sync {
    /// Resolves a storage-relative path to a backend-specific location.
    /// Local backend returns the absolute filesystem path; remote backends
    /// return a temp path used for staging.
    fn full_path(&self, rel_path: &str) -> PathBuf;

    async fn put_bytes(&self, rel_path: &str, bytes: &[u8]) -> anyhow::Result<()>;

    async fn read_bytes(&self, rel_path: &str) -> anyhow::Result<Vec<u8>>;

    async fn delete(&self, rel_path: &str) -> anyhow::Result<()>;

    /// Total bytes stored under a relative prefix (used by exports/cleanup).
    async fn size_of(&self, rel_path: &str) -> anyhow::Result<u64>;
}

pub struct LocalObjectStore {
    root: PathBuf,
}

impl LocalObjectStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, rel_path: &str) -> PathBuf {
        // Defend against path traversal.
        let clean: PathBuf = Path::new(rel_path)
            .components()
            .filter(|c| {
                !matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
            .collect();
        self.root.join(clean)
    }
}

#[async_trait::async_trait]
impl ObjectStore for LocalObjectStore {
    fn full_path(&self, rel_path: &str) -> PathBuf {
        self.resolve(rel_path)
    }

    async fn put_bytes(&self, rel_path: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let full = self.resolve(rel_path);
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&full, bytes).await?;
        Ok(())
    }

    async fn read_bytes(&self, rel_path: &str) -> anyhow::Result<Vec<u8>> {
        Ok(tokio::fs::read(self.resolve(rel_path)).await?)
    }

    async fn delete(&self, rel_path: &str) -> anyhow::Result<()> {
        let full = self.resolve(rel_path);
        if full.exists() {
            tokio::fs::remove_file(full).await?;
        }
        Ok(())
    }

    async fn size_of(&self, rel_path: &str) -> anyhow::Result<u64> {
        Ok(tokio::fs::metadata(self.resolve(rel_path)).await?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_store_roundtrip_and_traversal_guard() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = LocalObjectStore::new(tmp.path());

        store.put_bytes("dir/a.txt", b"hello").await.unwrap();
        assert_eq!(store.read_bytes("dir/a.txt").await.unwrap(), b"hello");
        assert_eq!(store.size_of("dir/a.txt").await.unwrap(), 5);

        // Traversal must stay inside the root.
        let evil = store.full_path("../../etc/passwd");
        assert!(evil.starts_with(tmp.path()));
        assert!(!evil.exists());
    }
}
