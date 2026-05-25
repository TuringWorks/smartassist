//! Learning context provider — injects relevant learnings into agent context.
//!
//! Before sending messages to the LLM, the context provider searches the
//! learnings store for entries relevant to the user's query and formats
//! them as a system message to prepend.

use crate::error::LearningsError;
use crate::store::LearningStore;
use std::sync::Arc;

/// Provides learning context for injection into agent conversations.
pub struct LearningContextProvider {
    /// The learning store to search.
    store: Arc<LearningStore>,
    /// Maximum number of entries to include in context.
    max_entries: usize,
}

impl LearningContextProvider {
    /// Create a new context provider.
    pub fn new(store: Arc<LearningStore>, max_entries: usize) -> Self {
        Self { store, max_entries }
    }

    /// Build a context message for a given query.
    ///
    /// Searches the learning store for entries relevant to the query
    /// and formats them as a system message suitable for prepending
    /// to the conversation.
    pub fn build_context_message(&self, query: &str) -> Result<Option<String>, LearningsError> {
        let context = self.store.context_for_query(query, self.max_entries)?;

        if context.is_empty() {
            Ok(None)
        } else {
            Ok(Some(context))
        }
    }

    /// Build a general context message with top entries (no specific query).
    ///
    /// Useful for injecting general knowledge at the start of a conversation.
    pub fn build_general_context(&self) -> Result<Option<String>, LearningsError> {
        let context = self.store.as_context(self.max_entries)?;

        if context.is_empty() {
            Ok(None)
        } else {
            Ok(Some(context))
        }
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &Arc<LearningStore> {
        &self.store
    }

    /// Get the max entries limit.
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{LearningEntry, LearningSource};
    use tempfile::TempDir;

    fn setup() -> (TempDir, Arc<LearningStore>, LearningContextProvider) {
        let dir = TempDir::new().unwrap();
        let store = LearningStore::with_dir(dir.path().join("md")).unwrap();
        let store = Arc::new(store);
        let provider = LearningContextProvider::new(store.clone(), 5);
        (dir, store, provider)
    }

    #[test]
    fn test_context_for_query() {
        let (_dir, store, provider) = setup();
        let entry = LearningEntry::new("Rust Pattern", "Use Arc<RwLock<T>> for shared mutable state", LearningSource::Manual)
            .with_tags(vec!["rust".to_string(), "async".to_string()]);
        store.add_entry("general", &entry).unwrap();

        let result = provider.build_context_message("rust shared state").unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();
        assert!(msg.contains("Rust Pattern"));
    }

    #[test]
    fn test_empty_context() {
        let (_dir, _store, provider) = setup();
        let result = provider.build_context_message("anything").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_general_context() {
        let (_dir, store, provider) = setup();
        let entry = LearningEntry::new("General Tip", "Always use version control", LearningSource::Manual)
            .with_confidence(0.95);
        store.add_entry("general", &entry).unwrap();

        let result = provider.build_general_context().unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("General Tip"));
    }
}