//! Learning store — manages markdown files in `~/.smartassist/md/`.
//!
//! Each file is a markdown document with H2 sections as learning entries.
//! Metadata is stored in HTML comments for machine parsing while remaining
//! invisible in rendered markdown.

use crate::error::LearningsError;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use walkdir::WalkDir;

/// Source of a learning entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "detail")]
pub enum LearningSource {
    /// Learned from an uploaded document.
    Document { name: String },
    /// Extracted from a conversation.
    Conversation { session_id: String },
    /// Manually created.
    Manual,
}

impl std::fmt::Display for LearningSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LearningSource::Document { name } => write!(f, "document:{}", name),
            LearningSource::Conversation { session_id } => write!(f, "conversation:{}", session_id),
            LearningSource::Manual => write!(f, "manual"),
        }
    }
}

/// A single learning entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearningEntry {
    /// Unique identifier.
    pub id: String,
    /// Title / heading.
    pub title: String,
    /// Content body.
    pub content: String,
    /// Where the learning came from.
    pub source: LearningSource,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f32,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl LearningEntry {
    /// Create a new entry with auto-generated id and current timestamps.
    pub fn new(title: impl Into<String>, content: impl Into<String>, source: LearningSource) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            title: title.into(),
            content: content.into(),
            source,
            confidence: 0.8,
            tags: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Set confidence.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Render entry as markdown H2 section with HTML comment metadata.
    pub fn to_markdown(&self) -> String {
        let tags_str = self.tags.join(",");
        let meta_line = format!(
            "<!-- id: {} | source: {} | confidence: {:.2} | tags: {} | created: {} | updated: {} -->",
            self.id,
            self.source,
            self.confidence,
            tags_str,
            self.created_at.to_rfc3339(),
            self.updated_at.to_rfc3339(),
        );
        format!("## {}\n{}\n\n{}", self.title, meta_line, self.content)
    }

    /// Parse an entry from markdown text (H2 section with metadata comment).
    pub fn from_markdown(h2_title: &str, body: &str) -> Result<Self, LearningsError> {
        let title = h2_title.trim().to_string();

        // Extract metadata from HTML comment
        let meta_re = Regex::new(r"<!--\s*id:\s*(\S+)\s*\|\s*source:\s*(\S+)\s*\|\s*confidence:\s*([\d.]+)\s*\|\s*tags:\s*([^|]*)\s*\|\s*created:\s*([^|]+)\s*\|\s*updated:\s*([^>]+)\s*-->")
            .map_err(|e| LearningsError::Parse(format!("Regex error: {}", e)))?;

        let (id, source, confidence, tags, created_at, updated_at, content_start) = if let Some(caps) = meta_re.captures(body) {
            let id = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string()[..8].to_string());
            let source_str = caps.get(2).map(|m| m.as_str()).unwrap_or("manual");
            let confidence: f32 = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0.8);
            let tags_str = caps.get(4).map(|m| m.as_str().trim()).unwrap_or("");
            let tags: Vec<String> = if tags_str.is_empty() {
                Vec::new()
            } else {
                tags_str.split(',').map(|t| t.trim().to_string()).collect()
            };
            let created_at = caps.get(5).and_then(|m| DateTime::parse_from_rfc3339(m.as_str()).ok())
                .map(|dt| dt.to_utc())
                .unwrap_or_else(Utc::now);
            let updated_at = caps.get(6).and_then(|m| DateTime::parse_from_rfc3339(m.as_str()).ok())
                .map(|dt| dt.to_utc())
                .unwrap_or_else(Utc::now);
            let meta_end = caps.get(0).map(|m| m.end()).unwrap_or(0);
            (id, source_str.to_string(), confidence, tags, created_at, updated_at, meta_end)
        } else {
            // No metadata comment — create entry with defaults
            let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
            (id, "manual".to_string(), 0.8, Vec::new(), Utc::now(), Utc::now(), 0)
        };

        let source = parse_source(&source);
        let content = body[content_start..].trim().to_string();

        Ok(Self {
            id,
            title,
            content,
            source,
            confidence,
            tags,
            created_at,
            updated_at,
        })
    }
}

/// Parse source string back into LearningSource.
fn parse_source(s: &str) -> LearningSource {
    if let Some(name) = s.strip_prefix("document:") {
        LearningSource::Document { name: name.to_string() }
    } else if let Some(session_id) = s.strip_prefix("conversation:") {
        LearningSource::Conversation { session_id: session_id.to_string() }
    } else {
        LearningSource::Manual
    }
}

/// Manages the learnings directory (`~/.smartassist/md/`).
pub struct LearningStore {
    base_dir: PathBuf,
}

impl LearningStore {
    /// Create a new store, using the default `~/.smartassist/md/` path.
    pub fn new() -> Result<Self, LearningsError> {
        let base_dir = smartassist_core::paths::learnings_dir()
            .map_err(|e| LearningsError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        std::fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    /// Create a store with a custom base directory (for testing).
    pub fn with_dir(base_dir: PathBuf) -> Result<Self, LearningsError> {
        std::fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    /// Get the base directory path.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Add a learning entry to a category file.
    pub fn add_entry(&self, category: &str, entry: &LearningEntry) -> Result<(), LearningsError> {
        let file_path = self.category_file(category);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let entry_md = entry.to_markdown();

        if file_path.exists() {
            let existing = std::fs::read_to_string(&file_path)?;
            let updated = format!("{}\n\n---\n\n{}", existing.trim_end(), entry_md);
            atomic_write(&file_path, &updated)?;
        } else {
            let title = category_to_title(category);
            let content = format!("# {}\n\n{}", title, entry_md);
            atomic_write(&file_path, &content)?;
        }

        info!("Added learning entry '{}' to category '{}'", entry.id, category);
        self.rebuild_index()?;
        Ok(())
    }

    /// Get a specific entry by category and id.
    pub fn get_entry(&self, category: &str, id: &str) -> Result<LearningEntry, LearningsError> {
        let entries = self.list_entries(category)?;
        entries.into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| LearningsError::NotFound(format!("{}/{}", category, id)))
    }

    /// List all entries in a category.
    pub fn list_entries(&self, category: &str) -> Result<Vec<LearningEntry>, LearningsError> {
        let file_path = self.category_file(category);
        if !file_path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&file_path)?;
        parse_entries_from_markdown(&content)
    }

    /// Search entries across all categories by keyword.
    pub fn search_entries(&self, query: &str, limit: usize) -> Result<Vec<(LearningEntry, String)>, LearningsError> {
        let query_lower = query.to_lowercase();
        let terms: Vec<&str> = query_lower.split_whitespace().collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let mut results: Vec<(LearningEntry, String, usize)> = Vec::new();

        for entry in WalkDir::new(&self.base_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
        {
            let path = entry.path();
            let rel = path.strip_prefix(&self.base_dir).unwrap_or(path);
            let category = rel.with_extension("").to_string_lossy().to_string();

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let entries = match parse_entries_from_markdown(&content) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries {
                let lower_title = entry.title.to_lowercase();
                let lower_content = entry.content.to_lowercase();
                let lower_tags: Vec<String> = entry.tags.iter().map(|t| t.to_lowercase()).collect();

                let mut score = 0usize;
                for term in &terms {
                    if lower_title.contains(term) { score += 3; }
                    if lower_content.contains(term) { score += 1; }
                    if lower_tags.iter().any(|t| t.contains(term)) { score += 2; }
                }

                if score > 0 {
                    results.push((entry, category.clone(), score));
                }
            }
        }

        results.sort_by(|a, b| b.2.cmp(&a.2));
        results.truncate(limit);
        Ok(results.into_iter().map(|(e, c, _)| (e, c)).collect())
    }

    /// Delete an entry by category and id.
    pub fn delete_entry(&self, category: &str, id: &str) -> Result<(), LearningsError> {
        let entries = self.list_entries(category)?;
        let filtered: Vec<&LearningEntry> = entries.iter().filter(|e| e.id != id).collect();

        if filtered.len() == entries.len() {
            return Err(LearningsError::NotFound(format!("{}/{}", category, id)));
        }

        let file_path = self.category_file(category);
        let title = category_to_title(category);
        let mut content = format!("# {}\n\n", title);
        for (i, entry) in filtered.iter().enumerate() {
            if i > 0 {
                content.push_str("\n\n---\n\n");
            }
            content.push_str(&entry.to_markdown());
        }

        if content.trim() == format!("# {}", title) {
            // File would be empty header only — delete the file
            std::fs::remove_file(&file_path)?;
        } else {
            atomic_write(&file_path, &content)?;
        }

        self.rebuild_index()?;
        Ok(())
    }

    /// Update an entry in place.
    pub fn update_entry(&self, category: &str, id: &str, updates: &EntryUpdates) -> Result<(), LearningsError> {
        let mut entries = self.list_entries(category)?;
        let entry = entries.iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| LearningsError::NotFound(format!("{}/{}", category, id)))?;

        if let Some(ref title) = updates.title {
            entry.title = title.clone();
        }
        if let Some(ref content) = updates.content {
            entry.content = content.clone();
        }
        if let Some(confidence) = updates.confidence {
            entry.confidence = confidence;
        }
        if let Some(ref tags) = updates.tags {
            entry.tags = tags.clone();
        }
        entry.updated_at = Utc::now();

        // Rewrite the entire file
        let file_path = self.category_file(category);
        let title = category_to_title(category);
        let mut file_content = format!("# {}\n\n", title);
        for (i, e) in entries.iter().enumerate() {
            if i > 0 {
                file_content.push_str("\n\n---\n\n");
            }
            file_content.push_str(&e.to_markdown());
        }
        atomic_write(&file_path, &file_content)?;

        self.rebuild_index()?;
        Ok(())
    }

    /// List all categories (subdirectories and .md files under base_dir).
    pub fn categories(&self) -> Result<Vec<CategoryInfo>, LearningsError> {
        let mut categories = Vec::new();

        if !self.base_dir.exists() {
            return Ok(categories);
        }

        for entry in WalkDir::new(&self.base_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
        {
            let path = entry.path();
            // Skip index.md
            if path.file_name() == Some(std::ffi::OsStr::new("index.md")) {
                continue;
            }
            let rel = path.strip_prefix(&self.base_dir).unwrap_or(path);
            let category = rel.with_extension("").to_string_lossy().to_string();

            let content = std::fs::read_to_string(path).unwrap_or_default();
            let count = parse_entries_from_markdown(&content)
                .map(|e| e.len())
                .unwrap_or(0);

            categories.push(CategoryInfo {
                name: category,
                entry_count: count,
                path: path.to_path_buf(),
            });
        }

        categories.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(categories)
    }

    /// Rebuild the index.md file with links to all learning files.
    pub fn rebuild_index(&self) -> Result<(), LearningsError> {
        let categories = self.categories()?;
        let mut index = String::from("# Learnings Index\n\n");

        if categories.is_empty() {
            index.push_str("No learning entries yet.\n");
        } else {
            for cat in &categories {
                index.push_str(&format!("- **{}** ({} entries)\n", cat.name, cat.entry_count));
            }
        }

        let index_path = self.base_dir.join("index.md");
        atomic_write(&index_path, &index)?;
        Ok(())
    }

    /// Get top entries formatted as context for injection into agent conversations.
    pub fn as_context(&self, max_entries: usize) -> Result<String, LearningsError> {
        let mut all_entries: Vec<LearningEntry> = Vec::new();

        for entry in WalkDir::new(&self.base_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
        {
            let path = entry.path();
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Ok(entries) = parse_entries_from_markdown(&content) {
                all_entries.extend(entries);
            }
        }

        // Sort by confidence descending, then recency
        all_entries.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });
        all_entries.truncate(max_entries);

        if all_entries.is_empty() {
            return Ok(String::new());
        }

        let mut context = String::from("## Relevant Learnings\n\n");
        for entry in &all_entries {
            context.push_str(&format!("### {}\n", entry.title));
            context.push_str(&entry.content);
            context.push_str("\n\n");
        }

        Ok(context)
    }

    /// Get relevant entries for a query, formatted as context.
    pub fn context_for_query(&self, query: &str, max_entries: usize) -> Result<String, LearningsError> {
        let results = self.search_entries(query, max_entries)?;
        if results.is_empty() {
            return Ok(String::new());
        }

        let mut context = String::from("## Relevant Learnings\n\n");
        for (entry, _category) in &results {
            context.push_str(&format!("### {}\n", entry.title));
            context.push_str(&entry.content);
            context.push_str("\n\n");
        }

        Ok(context)
    }

    fn category_file(&self, category: &str) -> PathBuf {
        // Sanitize category name
        let safe_category: String = category.chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' || c == '/' { c } else { '_' })
            .collect();
        self.base_dir.join(format!("{}.md", safe_category))
    }
}

/// Updates to apply to an entry.
#[derive(Debug, Clone, Default)]
pub struct EntryUpdates {
    /// New title.
    pub title: Option<String>,
    /// New content.
    pub content: Option<String>,
    /// New confidence.
    pub confidence: Option<f32>,
    /// New tags.
    pub tags: Option<Vec<String>>,
}

/// Information about a learning category.
#[derive(Debug, Clone)]
pub struct CategoryInfo {
    /// Category name (relative path without extension).
    pub name: String,
    /// Number of entries in this category.
    pub entry_count: usize,
    /// Path to the category file.
    pub path: PathBuf,
}

/// Convert a category name to a title.
fn category_to_title(category: &str) -> String {
    category.split(|c: char| c == '_' || c == '-' || c == '/')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse all learning entries from a markdown file.
fn parse_entries_from_markdown(content: &str) -> Result<Vec<LearningEntry>, LearningsError> {
    let mut entries = Vec::new();
    let h2_re = Regex::new(r"(?m)^## (.+)$")
        .map_err(|e| LearningsError::Parse(format!("Regex error: {}", e)))?;

    let positions: Vec<(usize, String)> = h2_re.captures_iter(content)
        .filter_map(|cap| {
            cap.get(1).map(|m| (m.start() - 3, m.as_str().to_string()))
        })
        .collect();

    if positions.is_empty() {
        return Ok(entries);
    }

    for (i, (start, title)) in positions.iter().enumerate() {
        let body_start = *start + title.len() + 3;
        let body_end = positions.get(i + 1).map(|(s, _)| *s).unwrap_or(content.len());
        let body = content[body_start..body_end].trim();

        match LearningEntry::from_markdown(title, body) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                debug!("Skipping malformed entry '{}': {}", title, e);
            }
        }
    }

    Ok(entries)
}

/// Atomic write: write to .tmp then rename.
fn atomic_write(path: &Path, content: &str) -> Result<(), LearningsError> {
    let tmp_path = path.with_extension("md.tmp");
    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_store() -> (TempDir, LearningStore) {
        let dir = TempDir::new().unwrap();
        let store = LearningStore::with_dir(dir.path().join("md")).unwrap();
        (dir, store)
    }

    #[test]
    fn test_learning_entry_new() {
        let entry = LearningEntry::new(
            "Test Entry",
            "This is a test learning.",
            LearningSource::Manual,
        );
        assert!(!entry.id.is_empty());
        assert_eq!(entry.title, "Test Entry");
        assert_eq!(entry.content, "This is a test learning.");
        assert_eq!(entry.source, LearningSource::Manual);
        assert!((entry.confidence - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_learning_entry_with_confidence() {
        let entry = LearningEntry::new("Test", "Content", LearningSource::Manual)
            .with_confidence(0.95);
        assert!((entry.confidence - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_learning_entry_with_tags() {
        let entry = LearningEntry::new("Test", "Content", LearningSource::Manual)
            .with_tags(vec!["rust".to_string(), "async".to_string()]);
        assert_eq!(entry.tags, vec!["rust", "async"]);
    }

    #[test]
    fn test_entry_markdown_roundtrip() {
        let entry = LearningEntry::new(
            "Error Handling",
            "Use thiserror for libraries.",
            LearningSource::Document { name: "guide.md".to_string() },
        ).with_confidence(0.9)
         .with_tags(vec!["rust".to_string(), "errors".to_string()]);

        let md = entry.to_markdown();
        assert!(md.starts_with("## Error Handling"));
        assert!(md.contains(&format!("id: {}", entry.id)));
        assert!(md.contains("source: document:guide.md"));
        assert!(md.contains("confidence: 0.90"));
        assert!(md.contains("tags: rust,errors"));
        assert!(md.contains("Use thiserror for libraries."));
    }

    #[test]
    fn test_entry_from_markdown_with_metadata() {
        let body = "<!-- id: abc123 | source: manual | confidence: 0.85 | tags: rust,async | created: 2024-01-15T00:00:00+00:00 | updated: 2024-01-16T00:00:00+00:00 -->\nUse Arc<RwLock<T>> for shared state.";
        let entry = LearningEntry::from_markdown("Async Patterns", body).unwrap();
        assert_eq!(entry.id, "abc123");
        assert_eq!(entry.title, "Async Patterns");
        assert_eq!(entry.content, "Use Arc<RwLock<T>> for shared state.");
        assert!((entry.confidence - 0.85).abs() < 0.01);
        assert_eq!(entry.tags, vec!["rust", "async"]);
    }

    #[test]
    fn test_entry_from_markdown_without_metadata() {
        let body = "Some learning content without metadata.";
        let entry = LearningEntry::from_markdown("Simple Entry", body).unwrap();
        assert_eq!(entry.title, "Simple Entry");
        assert_eq!(entry.content, "Some learning content without metadata.");
        assert_eq!(entry.source, LearningSource::Manual);
    }

    #[test]
    fn test_store_add_and_get_entry() {
        let (_dir, store) = setup_store();
        let entry = LearningEntry::new(
            "Test Learning",
            "This is important knowledge.",
            LearningSource::Manual,
        ).with_confidence(0.9);

        let entry_id = entry.id.clone();
        store.add_entry("general", &entry).unwrap();

        let retrieved = store.get_entry("general", &entry_id).unwrap();
        assert_eq!(retrieved.title, "Test Learning");
        assert_eq!(retrieved.content, "This is important knowledge.");
    }

    #[test]
    fn test_store_list_entries() {
        let (_dir, store) = setup_store();
        let e1 = LearningEntry::new("Entry 1", "Content 1", LearningSource::Manual);
        let e2 = LearningEntry::new("Entry 2", "Content 2", LearningSource::Manual);

        store.add_entry("general", &e1).unwrap();
        store.add_entry("general", &e2).unwrap();

        let entries = store.list_entries("general").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_store_delete_entry() {
        let (_dir, store) = setup_store();
        let e1 = LearningEntry::new("Entry 1", "Content 1", LearningSource::Manual);
        let e2 = LearningEntry::new("Entry 2", "Content 2", LearningSource::Manual);
        let e1_id = e1.id.clone();

        store.add_entry("general", &e1).unwrap();
        store.add_entry("general", &e2).unwrap();

        store.delete_entry("general", &e1_id).unwrap();

        let entries = store.list_entries("general").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Entry 2");
    }

    #[test]
    fn test_store_update_entry() {
        let (_dir, store) = setup_store();
        let entry = LearningEntry::new("Original", "Original content", LearningSource::Manual);
        let entry_id = entry.id.clone();

        store.add_entry("general", &entry).unwrap();

        store.update_entry("general", &entry_id, &EntryUpdates {
            title: Some("Updated".to_string()),
            content: Some("Updated content".to_string()),
            confidence: Some(0.95),
            tags: None,
        }).unwrap();

        let updated = store.get_entry("general", &entry_id).unwrap();
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.content, "Updated content");
        assert!((updated.confidence - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_store_search_entries() {
        let (_dir, store) = setup_store();
        let e1 = LearningEntry::new("Rust Async", "Use tokio for async runtime", LearningSource::Manual)
            .with_tags(vec!["rust".to_string(), "async".to_string()]);
        let e2 = LearningEntry::new("Python Testing", "Use pytest for testing", LearningSource::Manual)
            .with_tags(vec!["python".to_string(), "testing".to_string()]);
        let e3 = LearningEntry::new("Rust Errors", "Use thiserror for error types", LearningSource::Manual)
            .with_tags(vec!["rust".to_string(), "errors".to_string()]);

        store.add_entry("general", &e1).unwrap();
        store.add_entry("general", &e2).unwrap();
        store.add_entry("general", &e3).unwrap();

        let results = store.search_entries("rust", 10).unwrap();
        assert_eq!(results.len(), 2); // e1 and e3 match "rust"

        let results = store.search_entries("async", 10).unwrap();
        assert!(results.len() >= 1); // e1 matches "async"
    }

    #[test]
    fn test_store_categories() {
        let (_dir, store) = setup_store();
        let e1 = LearningEntry::new("Entry 1", "Content 1", LearningSource::Manual);
        let e2 = LearningEntry::new("Entry 2", "Content 2", LearningSource::Manual);

        store.add_entry("general", &e1).unwrap();
        store.add_entry("documents", &e2).unwrap();

        let cats = store.categories().unwrap();
        assert!(cats.len() >= 2);
    }

    #[test]
    fn test_store_as_context() {
        let (_dir, store) = setup_store();
        let e1 = LearningEntry::new("Important Pattern", "Always use Arc<RwLock<T>>", LearningSource::Manual)
            .with_confidence(0.9);

        store.add_entry("general", &e1).unwrap();

        let context = store.as_context(5).unwrap();
        assert!(context.contains("## Relevant Learnings"));
        assert!(context.contains("Important Pattern"));
    }

    #[test]
    fn test_store_context_for_query() {
        let (_dir, store) = setup_store();
        let e1 = LearningEntry::new("Rust Pattern", "Use Arc<RwLock<T>>", LearningSource::Manual);
        let e2 = LearningEntry::new("Python Tip", "Use list comprehensions", LearningSource::Manual);

        store.add_entry("general", &e1).unwrap();
        store.add_entry("general", &e2).unwrap();

        let context = store.context_for_query("rust", 5).unwrap();
        assert!(context.contains("Rust Pattern"));
    }

    #[test]
    fn test_category_to_title() {
        assert_eq!(category_to_title("general"), "General");
        assert_eq!(category_to_title("api_design"), "Api Design");
        assert_eq!(category_to_title("rust-patterns"), "Rust Patterns");
    }

    #[test]
    fn test_learning_source_display() {
        let doc = LearningSource::Document { name: "guide.pdf".to_string() };
        assert_eq!(doc.to_string(), "document:guide.pdf");

        let conv = LearningSource::Conversation { session_id: "abc".to_string() };
        assert_eq!(conv.to_string(), "conversation:abc");

        let manual = LearningSource::Manual;
        assert_eq!(manual.to_string(), "manual");
    }

    #[test]
    fn test_store_not_found() {
        let (_dir, store) = setup_store();
        let result = store.get_entry("general", "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_store_empty_category_list() {
        let (_dir, store) = setup_store();
        let entries = store.list_entries("nonexistent").unwrap();
        assert!(entries.is_empty());
    }
}