//! Vector storage implementations.

use crate::embeddings::cosine_similarity;
use crate::{MemoryEntry, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;

/// Trait for vector stores.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Insert an entry.
    async fn insert(&self, entry: MemoryEntry) -> Result<()>;

    /// Insert multiple entries.
    async fn insert_batch(&self, entries: Vec<MemoryEntry>) -> Result<()>;

    /// Get an entry by ID.
    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>>;

    /// Delete an entry by ID.
    async fn delete(&self, id: &str) -> Result<()>;

    /// Search for similar entries.
    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(MemoryEntry, f32)>>;

    /// Count entries.
    async fn count(&self) -> Result<usize>;

    /// Clear all entries.
    async fn clear(&self) -> Result<()>;
}

/// In-memory vector store.
pub struct MemoryVectorStore {
    entries: RwLock<HashMap<String, MemoryEntry>>,
}

impl Default for MemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryVectorStore {
    /// Create a new in-memory vector store.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl VectorStore for MemoryVectorStore {
    async fn insert(&self, entry: MemoryEntry) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.insert(entry.id.clone(), entry);
        Ok(())
    }

    async fn insert_batch(&self, batch: Vec<MemoryEntry>) -> Result<()> {
        let mut entries = self.entries.write().await;
        for entry in batch {
            entries.insert(entry.id.clone(), entry);
        }
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.remove(id);
        Ok(())
    }

    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(MemoryEntry, f32)>> {
        let entries = self.entries.read().await;

        let mut results: Vec<(MemoryEntry, f32)> = entries
            .values()
            .map(|entry| {
                let score = cosine_similarity(query, &entry.embedding);
                (entry.clone(), score)
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        results.truncate(limit);

        Ok(results)
    }

    async fn count(&self) -> Result<usize> {
        let entries = self.entries.read().await;
        Ok(entries.len())
    }

    async fn clear(&self) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.clear();
        Ok(())
    }
}

/// File-backed vector store with JSON persistence.
///
/// All mutations are persisted to disk via atomic writes (write to tmp, then rename).
pub struct FileVectorStore {
    path: PathBuf,
    entries: RwLock<HashMap<String, MemoryEntry>>,
}

impl FileVectorStore {
    /// Create a new file-backed vector store.
    ///
    /// If the file at `path` exists, its contents are deserialized into memory.
    /// If the file does not exist, the store starts empty.
    pub fn new(path: PathBuf) -> Result<Self> {
        let entries = if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            serde_json::from_str(&data)?
        } else {
            HashMap::new()
        };

        Ok(Self {
            path,
            entries: RwLock::new(entries),
        })
    }

    /// Atomically persist the current entries to disk.
    ///
    /// Writes to a temporary file first, then renames to the target path
    /// to avoid partial writes on crash.
    fn save(&self, entries: &HashMap<String, MemoryEntry>) -> Result<()> {
        let tmp_path = self.path.with_extension("tmp");
        let data = serde_json::to_string_pretty(entries)?;
        std::fs::write(&tmp_path, data)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}

#[async_trait]
impl VectorStore for FileVectorStore {
    async fn insert(&self, entry: MemoryEntry) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.insert(entry.id.clone(), entry);
        self.save(&entries)?;
        Ok(())
    }

    async fn insert_batch(&self, batch: Vec<MemoryEntry>) -> Result<()> {
        let mut entries = self.entries.write().await;
        for entry in batch {
            entries.insert(entry.id.clone(), entry);
        }
        self.save(&entries)?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.remove(id);
        self.save(&entries)?;
        Ok(())
    }

    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(MemoryEntry, f32)>> {
        let entries = self.entries.read().await;

        let mut results: Vec<(MemoryEntry, f32)> = entries
            .values()
            .map(|entry| {
                let score = cosine_similarity(query, &entry.embedding);
                (entry.clone(), score)
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        results.truncate(limit);

        Ok(results)
    }

    async fn count(&self) -> Result<usize> {
        let entries = self.entries.read().await;
        Ok(entries.len())
    }

    async fn clear(&self) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.clear();
        self.save(&entries)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_store() {
        let store = MemoryVectorStore::new();

        let entry = MemoryEntry::new("test content", vec![1.0, 0.0, 0.0]);
        store.insert(entry.clone()).await.unwrap();

        let loaded = store.get(&entry.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().content, "test content");
    }

    #[tokio::test]
    async fn test_search() {
        let store = MemoryVectorStore::new();

        store
            .insert(MemoryEntry::new("first", vec![1.0, 0.0, 0.0]))
            .await
            .unwrap();
        store
            .insert(MemoryEntry::new("second", vec![0.0, 1.0, 0.0]))
            .await
            .unwrap();
        store
            .insert(MemoryEntry::new("third", vec![0.9, 0.1, 0.0]))
            .await
            .unwrap();

        let results = store.search(&[1.0, 0.0, 0.0], 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.content, "first");
    }

    #[tokio::test]
    async fn test_file_store_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");

        let entry_id;
        {
            let store = FileVectorStore::new(path.clone()).unwrap();
            let entry = MemoryEntry::new("persistent content", vec![1.0, 0.0, 0.0]);
            entry_id = entry.id.clone();
            store.insert(entry).await.unwrap();

            // Verify entry exists before drop
            let loaded = store.get(&entry_id).await.unwrap();
            assert!(loaded.is_some());
            assert_eq!(loaded.unwrap().content, "persistent content");
        }

        // Create new store from the same file path and verify data persisted
        {
            let store = FileVectorStore::new(path).unwrap();
            let loaded = store.get(&entry_id).await.unwrap();
            assert!(loaded.is_some());
            assert_eq!(loaded.unwrap().content, "persistent content");
        }
    }

    #[tokio::test]
    async fn test_file_store_insert_batch_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("batch_store.json");

        let store = FileVectorStore::new(path.clone()).unwrap();

        let entry1 = MemoryEntry::new("first", vec![1.0, 0.0, 0.0]);
        let entry2 = MemoryEntry::new("second", vec![0.0, 1.0, 0.0]);
        let id1 = entry1.id.clone();
        let id2 = entry2.id.clone();

        store.insert_batch(vec![entry1, entry2]).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);

        store.delete(&id1).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);
        assert!(store.get(&id1).await.unwrap().is_none());
        assert!(store.get(&id2).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_file_store_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clear_store.json");

        let store = FileVectorStore::new(path.clone()).unwrap();
        store
            .insert(MemoryEntry::new("data", vec![1.0, 0.0, 0.0]))
            .await
            .unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        store.clear().await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);

        // Verify cleared state persists
        let store2 = FileVectorStore::new(path).unwrap();
        assert_eq!(store2.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_file_store_search() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("search_store.json");

        let store = FileVectorStore::new(path).unwrap();

        store
            .insert(MemoryEntry::new("close match", vec![1.0, 0.0, 0.0]))
            .await
            .unwrap();
        store
            .insert(MemoryEntry::new("far away", vec![0.0, 1.0, 0.0]))
            .await
            .unwrap();

        let results = store.search(&[1.0, 0.0, 0.0], 1).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.content, "close match");
    }

    #[tokio::test]
    async fn test_file_store_new_empty_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");

        // File does not exist yet; store should start empty
        let store = FileVectorStore::new(path).unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }
}
