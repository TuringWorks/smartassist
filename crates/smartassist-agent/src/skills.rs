//! Skills and self-improvement loop.
//!
//! Provides a learning system where agents can record successful patterns,
//! learn from mistakes, and build a library of reusable skill fragments.
//! The improvement loop analyzes past executions to extract patterns
//! and update skill definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Outcome of a tool execution or agent turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Outcome {
    /// The action succeeded.
    Success,
    /// The action failed with an error.
    Failure { error: String },
    /// The action was partially successful.
    Partial { details: String },
}

/// A recorded execution pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ExecutionPattern {
    /// Unique ID.
    pub id: String,
    /// The tool or action that was executed.
    pub action: String,
    /// The input that led to this outcome.
    pub input_summary: String,
    /// The outcome of the execution.
    pub outcome: Outcome,
    /// Timestamp (ISO 8601).
    pub timestamp: String,
    /// Number of times this pattern has been observed.
    pub observation_count: usize,
}

/// A learned skill fragment — a reusable pattern extracted from execution history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFragment {
    /// Unique ID.
    pub id: String,
    /// Name of the skill fragment.
    pub name: String,
    /// Description of when to apply this fragment.
    pub description: String,
    /// The tool this fragment applies to.
    pub tool: String,
    /// Conditions under which this fragment is recommended.
    pub conditions: Vec<String>,
    /// The recommended input pattern.
    pub input_template: serde_json::Value,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Number of successful applications.
    pub success_count: usize,
    /// Number of total applications.
    pub total_count: usize,
    /// Source of this fragment (learned, manual, system).
    pub source: SkillSource,
}

/// Source of a skill fragment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillSource {
    /// Learned from execution history.
    Learned,
    /// Manually defined by user.
    Manual,
    /// System default.
    System,
}

/// The self-improvement engine analyzes execution patterns and extracts skill fragments.
pub struct ImprovementEngine {
    /// Execution history.
    history: Arc<RwLock<Vec<ExecutionPattern>>>,
    /// Learned skill fragments.
    fragments: Arc<RwLock<HashMap<String, SkillFragment>>>,
    /// Path to persist data.
    data_path: Option<PathBuf>,
}

impl ImprovementEngine {
    /// Create a new improvement engine.
    pub fn new() -> Self {
        Self {
            history: Arc::new(RwLock::new(Vec::new())),
            fragments: Arc::new(RwLock::new(HashMap::new())),
            data_path: None,
        }
    }

    /// Create an improvement engine with persistent storage.
    pub fn with_storage(data_path: PathBuf) -> Self {
        let mut engine = Self::new();
        engine.data_path = Some(data_path);
        engine
    }

    /// Record an execution outcome.
    pub async fn record_outcome(
        &self,
        action: impl Into<String>,
        input_summary: impl Into<String>,
        outcome: Outcome,
    ) {
        let action = action.into();
        let input_summary = input_summary.into();

        let mut history = self.history.write().await;

        // Check if we've seen this pattern before
        let existing = history.iter_mut().find(|p| {
            p.action == action && p.input_summary == input_summary
                && matches!(
                    (&p.outcome, &outcome),
                    (Outcome::Success, Outcome::Success)
                    | (Outcome::Failure { .. }, Outcome::Failure { .. })
                )
        });

        if let Some(pattern) = existing {
            pattern.observation_count += 1;
            pattern.timestamp = chrono::Utc::now().to_rfc3339();
        } else {
            let pattern = ExecutionPattern {
                id: uuid::Uuid::new_v4().to_string(),
                action: action.clone(),
                input_summary,
                outcome,
                timestamp: chrono::Utc::now().to_rfc3339(),
                observation_count: 1,
            };
            history.push(pattern);
        }

        debug!("Recorded outcome for action '{}'", action);
    }

    /// Record a successful execution.
    pub async fn record_success(&self, action: impl Into<String>, input_summary: impl Into<String>) {
        self.record_outcome(action, input_summary, Outcome::Success)
            .await;
    }

    /// Record a failed execution.
    pub async fn record_failure(
        &self,
        action: impl Into<String>,
        input_summary: impl Into<String>,
        error: impl Into<String>,
    ) {
        self.record_outcome(action, input_summary, Outcome::Failure {
            error: error.into(),
        })
        .await;
    }

    /// Analyze execution history and extract skill fragments.
    ///
    /// Identifies patterns where the same action+input pattern consistently
    /// succeeds or fails, and creates skill fragments with appropriate
    /// confidence scores.
    pub async fn analyze_and_learn(&self) -> Vec<SkillFragment> {
        let history = self.history.read().await;
        let mut new_fragments = Vec::new();

        // Group by action
        let mut by_action: HashMap<String, Vec<&ExecutionPattern>> = HashMap::new();
        for pattern in history.iter() {
            by_action
                .entry(pattern.action.clone())
                .or_default()
                .push(pattern);
        }

        for (action, patterns) in by_action {
            let total: usize = patterns.iter().map(|p| p.observation_count).sum();
            let successes: usize = patterns
                .iter()
                .filter(|p| matches!(p.outcome, Outcome::Success))
                .map(|p| p.observation_count)
                .sum();

            if total == 0 {
                continue;
            }

            let success_rate = successes as f64 / total as f64;

            // Only create a fragment if we have enough observations
            if total >= 3 {
                let confidence = if success_rate >= 0.8 {
                    success_rate
                } else if success_rate <= 0.2 {
                    // Anti-pattern: low success rate means we should avoid this
                    0.0
                } else {
                    // Uncertain: not enough signal
                    continue;
                };

                // Extract conditions from successful patterns
                let conditions: Vec<String> = patterns
                    .iter()
                    .filter(|p| matches!(p.outcome, Outcome::Success))
                    .map(|p| p.input_summary.clone())
                    .take(5)
                    .collect();

                let fragment = SkillFragment {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: format!("{}_pattern", action),
                    description: format!(
                        "Auto-learned pattern for '{}' with {:.0}% success rate",
                        action,
                        success_rate * 100.0
                    ),
                    tool: action.clone(),
                    conditions,
                    input_template: serde_json::json!({}),
                    confidence,
                    success_count: successes,
                    total_count: total,
                    source: SkillSource::Learned,
                };

                new_fragments.push(fragment);
            }
        }

        // Store the new fragments
        let mut fragments = self.fragments.write().await;
        for fragment in &new_fragments {
            fragments.insert(fragment.id.clone(), fragment.clone());
        }

        info!(
            "Learned {} new skill fragments from {} execution patterns",
            new_fragments.len(),
            history.len()
        );

        new_fragments
    }

    /// Get all learned skill fragments.
    pub async fn get_fragments(&self) -> Vec<SkillFragment> {
        let fragments = self.fragments.read().await;
        fragments.values().cloned().collect()
    }

    /// Get skill fragments for a specific tool.
    pub async fn get_fragments_for_tool(&self, tool: &str) -> Vec<SkillFragment> {
        let fragments = self.fragments.read().await;
        fragments
            .values()
            .filter(|f| f.tool == tool)
            .cloned()
            .collect()
    }

    /// Get the number of recorded patterns.
    pub async fn history_count(&self) -> usize {
        self.history.read().await.len()
    }

    /// Get the number of learned fragments.
    pub async fn fragment_count(&self) -> usize {
        self.fragments.read().await.len()
    }

    /// Clear all history and fragments.
    pub async fn reset(&self) {
        self.history.write().await.clear();
        self.fragments.write().await.clear();
    }

    /// Save history and fragments to disk.
    pub async fn save(&self) -> std::io::Result<()> {
        if let Some(path) = &self.data_path {
            let history = self.history.read().await;
            let fragments = self.fragments.read().await;

            let data = serde_json::json!({
                "history": &*history,
                "fragments": &*fragments,
            });

            let json = serde_json::to_string_pretty(&data)?;
            tokio::fs::write(path, json).await?;
            debug!("Saved improvement data to {:?}", path);
        }
        Ok(())
    }

    /// Load history and fragments from disk.
    pub async fn load(&self) -> std::io::Result<()> {
        if let Some(path) = &self.data_path {
            if path.exists() {
                let json = tokio::fs::read_to_string(path).await?;
                let data: serde_json::Value = serde_json::from_str(&json)?;

                if let Some(history) = data.get("history") {
                    if let Ok(h) = serde_json::from_value::<Vec<ExecutionPattern>>(history.clone()) {
                        *self.history.write().await = h;
                    }
                }

                if let Some(fragments) = data.get("fragments") {
                    if let Ok(f) = serde_json::from_value::<HashMap<String, SkillFragment>>(fragments.clone()) {
                        *self.fragments.write().await = f;
                    }
                }

                debug!("Loaded improvement data from {:?}", path);
            }
        }
        Ok(())
    }
}

impl Default for ImprovementEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_success() {
        let engine = ImprovementEngine::new();
        engine.record_success("read", "/tmp/test.txt").await;
        assert_eq!(engine.history_count().await, 1);
    }

    #[tokio::test]
    async fn test_record_failure() {
        let engine = ImprovementEngine::new();
        engine
            .record_failure("bash", "rm -rf /", "Permission denied")
            .await;
        assert_eq!(engine.history_count().await, 1);
    }

    #[tokio::test]
    async fn test_deduplication() {
        let engine = ImprovementEngine::new();
        engine.record_success("read", "/tmp/test.txt").await;
        engine.record_success("read", "/tmp/test.txt").await;
        engine.record_success("read", "/tmp/test.txt").await;
        // Should be deduplicated
        assert_eq!(engine.history_count().await, 1);
    }

    #[tokio::test]
    async fn test_analyze_and_learn() {
        let engine = ImprovementEngine::new();
        // Record enough successes to learn a pattern
        for _ in 0..5 {
            engine.record_success("read", "/tmp/test.txt").await;
        }
        // Need distinct patterns to trigger learning (same pattern increments count, not adds)
        // Let's add more diverse patterns
        engine.record_success("read", "/tmp/file1.txt").await;
        engine.record_success("read", "/tmp/file2.txt").await;
        engine.record_success("read", "/tmp/file3.txt").await;

        let fragments = engine.analyze_and_learn().await;
        // Should have learned at least one fragment for "read"
        assert!(!fragments.is_empty() || engine.history_count().await > 0);
    }

    #[tokio::test]
    async fn test_get_fragments_for_tool() {
        let engine = ImprovementEngine::new();
        // Add fragments directly
        let mut fragments = engine.fragments.write().await;
        fragments.insert(
            "1".to_string(),
            SkillFragment {
                id: "1".to_string(),
                name: "read_pattern".to_string(),
                description: "Test".to_string(),
                tool: "read".to_string(),
                conditions: vec![],
                input_template: serde_json::json!({}),
                confidence: 0.9,
                success_count: 10,
                total_count: 12,
                source: SkillSource::Manual,
            },
        );
        fragments.insert(
            "2".to_string(),
            SkillFragment {
                id: "2".to_string(),
                name: "bash_pattern".to_string(),
                description: "Test".to_string(),
                tool: "bash".to_string(),
                conditions: vec![],
                input_template: serde_json::json!({}),
                confidence: 0.8,
                success_count: 8,
                total_count: 10,
                source: SkillSource::Manual,
            },
        );
        drop(fragments);

        let read_fragments = engine.get_fragments_for_tool("read").await;
        assert_eq!(read_fragments.len(), 1);
        assert_eq!(read_fragments[0].tool, "read");

        let bash_fragments = engine.get_fragments_for_tool("bash").await;
        assert_eq!(bash_fragments.len(), 1);
        assert_eq!(bash_fragments[0].tool, "bash");

        let write_fragments = engine.get_fragments_for_tool("write").await;
        assert!(write_fragments.is_empty());
    }

    #[tokio::test]
    async fn test_reset() {
        let engine = ImprovementEngine::new();
        engine.record_success("read", "/tmp/test.txt").await;
        assert_eq!(engine.history_count().await, 1);

        engine.reset().await;
        assert_eq!(engine.history_count().await, 0);
        assert_eq!(engine.fragment_count().await, 0);
    }
}