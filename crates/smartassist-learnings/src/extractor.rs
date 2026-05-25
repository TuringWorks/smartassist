//! Learning extraction from documents.
//!
//! The extractor takes parsed documents and produces learning entries
//! using extractive heuristics (splitting by headings, key sentences, lists).
//! LLM-based summarization is handled at the gateway level where providers
//! are available.

use crate::error::LearningsError;
use crate::parser::ParsedDocument;
use crate::store::{LearningEntry, LearningSource};
use smartassist_core::config::LearningsConfig;

/// Extracts learning entries from documents.
pub struct LearningExtractor {
    /// Configuration.
    config: LearningsConfig,
}

impl LearningExtractor {
    /// Create an extractor with the given configuration.
    pub fn new(config: LearningsConfig) -> Self {
        Self { config }
    }

    /// Create an extractor with default configuration.
    pub fn extractive() -> Self {
        Self::new(LearningsConfig::default())
    }

    /// Extract learning entries from a parsed document.
    pub fn extract(
        &self,
        doc: &ParsedDocument,
        source: LearningSource,
        tags: Vec<String>,
    ) -> Result<Vec<LearningEntry>, LearningsError> {
        self.extract_extractive(doc, source, tags)
    }

    /// Extractive mode: split document into sections by headings and key content.
    fn extract_extractive(
        &self,
        doc: &ParsedDocument,
        source: LearningSource,
        tags: Vec<String>,
    ) -> Result<Vec<LearningEntry>, LearningsError> {
        let mut entries = Vec::new();
        let content = &doc.content;

        // Split by markdown headings
        let sections = split_by_headings(content);

        if sections.is_empty() {
            // No headings found — treat entire document as one entry
            let entry = LearningEntry::new(
                &doc.title,
                content.trim(),
                source.clone(),
            )
            .with_confidence(0.7)
            .with_tags(tags);

            if !entry.content.is_empty() {
                entries.push(entry);
            }
            return Ok(entries);
        }

        for (heading, body) in sections {
            if body.trim().is_empty() {
                continue;
            }

            let title = if heading.is_empty() {
                format!("{} - Section", doc.title)
            } else {
                heading
            };

            let confidence = calculate_confidence(&body);

            let entry = LearningEntry::new(title, body.trim(), source.clone())
                .with_confidence(confidence)
                .with_tags(tags.clone());

            entries.push(entry);
        }

        // Limit entries per file
        if entries.len() > self.config.max_entries_per_file {
            entries.truncate(self.config.max_entries_per_file);
        }

        Ok(entries)
    }

    /// Create learning entries from a plain text string (e.g., conversation summary).
    /// This is used for auto-extraction from conversations.
    pub fn extract_from_text(
        &self,
        title: &str,
        text: &str,
        source: LearningSource,
        tags: Vec<String>,
    ) -> Vec<LearningEntry> {
        let sections = split_by_headings(text);

        if sections.is_empty() {
            if text.trim().is_empty() {
                return Vec::new();
            }
            let entry = LearningEntry::new(title, text.trim(), source)
                .with_confidence(0.6)
                .with_tags(tags);
            return vec![entry];
        }

        let mut entries = Vec::new();
        for (heading, body) in sections {
            if body.trim().is_empty() {
                continue;
            }
            let entry_title = if heading.is_empty() {
                title.to_string()
            } else {
                heading
            };
            let entry = LearningEntry::new(entry_title, body.trim(), source.clone())
                .with_confidence(0.65)
                .with_tags(tags.clone());
            entries.push(entry);
        }

        if entries.len() > self.config.max_entries_per_file {
            entries.truncate(self.config.max_entries_per_file);
        }

        entries
    }
}

/// Split content by markdown headings (## or ###).
fn split_by_headings(content: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let heading_re = regex::Regex::new(r"(?m)^(#{1,3})\s+(.+)$").unwrap();

    let matches: Vec<(usize, String)> = heading_re
        .captures_iter(content)
        .filter_map(|cap| {
            cap.get(2).map(|m| (m.start(), m.as_str().to_string()))
        })
        .collect();

    if matches.is_empty() {
        return sections;
    }

    for (i, (start, heading)) in matches.iter().enumerate() {
        let body_start = content[*start..].find('\n').map(|p| *start + p + 1).unwrap_or(content.len());
        let body_end = matches.get(i + 1).map(|(s, _)| *s).unwrap_or(content.len());
        let body = content[body_start..body_end].trim().to_string();
        sections.push((heading.clone(), body));
    }

    // If there's content before the first heading, add it as an untitled section
    if !matches.is_empty() {
        let first_heading_start = matches[0].0;
        if first_heading_start > 0 {
            let pre_content = content[..first_heading_start].trim().to_string();
            if !pre_content.is_empty() {
                sections.insert(0, (String::new(), pre_content));
            }
        }
    }

    sections
}

/// Calculate confidence score based on content structure.
fn calculate_confidence(content: &str) -> f32 {
    let mut score = 0.6f32;

    // Longer content is more likely to contain useful information
    if content.len() > 100 {
        score += 0.1;
    }
    if content.len() > 500 {
        score += 0.1;
    }

    // Structured content (lists, code blocks) gets a bonus
    if content.contains("- ") || content.contains("* ") {
        score += 0.05;
    }
    if content.contains("```") {
        score += 0.05;
    }

    score.min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_config() -> LearningsConfig {
        LearningsConfig::default()
    }

    #[test]
    fn test_split_by_headings() {
        let content = "# Title\n\nIntro text\n\n## First Section\n\nFirst content.\n\n## Second Section\n\nSecond content.";
        let sections = split_by_headings(content);
        assert!(sections.len() >= 2);
    }

    #[test]
    fn test_split_by_headings_empty() {
        let content = "No headings here. Just plain text.";
        let sections = split_by_headings(content);
        assert!(sections.is_empty());
    }

    #[test]
    fn test_extract_extractive_with_headings() {
        let doc = ParsedDocument {
            title: "Test Doc".to_string(),
            content: "## Pattern A\n\nUse Arc<Mutex<T>> for shared state.\n\n## Pattern B\n\nPrefer enums over bool parameters.".to_string(),
            metadata: HashMap::new(),
        };

        let extractor = LearningExtractor::new(test_config());
        let entries = extractor.extract_extractive(
            &doc,
            LearningSource::Document { name: "test.md".to_string() },
            vec!["rust".to_string()],
        ).unwrap();

        // Should have at least 2 heading-based entries
        assert!(entries.len() >= 2, "Expected at least 2 entries, got {}", entries.len());
        let titles: Vec<&str> = entries.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.contains(&"Pattern A"), "Should contain 'Pattern A', got {:?}", titles);
        assert!(titles.contains(&"Pattern B"), "Should contain 'Pattern B', got {:?}", titles);
        assert_eq!(entries[0].tags, vec!["rust"]);
    }

    #[test]
    fn test_extract_extractive_no_headings() {
        let doc = ParsedDocument {
            title: "Simple Doc".to_string(),
            content: "Just some plain text without headings.".to_string(),
            metadata: HashMap::new(),
        };

        let extractor = LearningExtractor::new(test_config());
        let entries = extractor.extract_extractive(
            &doc,
            LearningSource::Manual,
            vec![],
        ).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Simple Doc");
    }

    #[test]
    fn test_calculate_confidence() {
        let short = "Short.";
        let long = "This is a longer piece of content that contains more than 100 characters. It should get a higher confidence score because it provides more context and information for the learning entry.";
        let structured = "- Item 1\n- Item 2\n```rust\nlet x = 1;\n```";

        assert!(calculate_confidence(long) > calculate_confidence(short));
        assert!(calculate_confidence(structured) > calculate_confidence(short));
    }

    #[test]
    fn test_extract_from_text() {
        let extractor = LearningExtractor::new(test_config());
        let entries = extractor.extract_from_text(
            "Key Facts",
            "Rust uses ownership for memory safety. Enums are powerful.",
            LearningSource::Conversation { session_id: "abc".to_string() },
            vec!["rust".to_string()],
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Key Facts");
    }

    #[test]
    fn test_extract_from_text_with_headings() {
        let extractor = LearningExtractor::new(test_config());
        let entries = extractor.extract_from_text(
            "Session Notes",
            "## Ownership\nRust uses ownership.\n\n## Lifetimes\nLifetimes prevent dangling references.",
            LearningSource::Conversation { session_id: "abc".to_string() },
            vec!["rust".to_string()],
        );

        assert!(entries.len() >= 2, "Expected at least 2 entries, got {}", entries.len());
    }
}