//! Context compression engine for managing conversation context windows.
//!
//! Orchestrates compaction with optional LLM-based summarization.
//! Hooks into `AgentRuntime::get_model_response()` to check if compaction
//! is needed before building messages for the provider.

use smartassist_core::context::{
    CompactionStrategy, ContextCompactor, ContextMonitor, ContextMonitorConfig,
};
use smartassist_core::types::Message;
use std::sync::Arc;
use tracing::{info, warn};

use crate::providers::Provider;
use crate::Result;

/// Configuration for the compression engine.
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Maximum context window tokens before compaction triggers.
    pub context_limit: usize,
    /// Fraction of context window at which compaction starts (0.0 - 1.0).
    pub compaction_threshold: f64,
    /// Number of head messages (system + initial exchanges) to preserve.
    pub head_messages: usize,
    /// Number of recent tail messages to preserve.
    pub tail_messages: usize,
    /// Whether to use LLM-based summarization for the middle section.
    pub use_llm_summarization: bool,
    /// Model to use for summarization (defaults to session model).
    pub summary_model: Option<String>,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            context_limit: 100_000,
            compaction_threshold: 0.8,
            head_messages: 2,
            tail_messages: 10,
            use_llm_summarization: false,
            summary_model: None,
        }
    }
}

/// Result of a compression operation.
#[derive(Debug, Clone)]
pub struct CompressionResult {
    /// Strategy that was applied.
    pub strategy: CompactionStrategy,
    /// Number of messages before compaction.
    pub messages_before: usize,
    /// Number of messages after compaction.
    pub messages_after: usize,
    /// Estimated tokens before compaction.
    pub tokens_before: usize,
    /// Estimated tokens after compaction.
    pub tokens_after: usize,
    /// Summary text if LLM summarization was used.
    pub summary: Option<String>,
    /// Number of tool-use pairs preserved.
    pub tool_pairs_preserved: usize,
}

/// The compression engine manages context window usage and applies
/// compaction strategies when the conversation grows too large.
pub struct CompressionEngine {
    /// Context monitor for token estimation and threshold detection.
    monitor: ContextMonitor,
    /// Configuration.
    config: CompressionConfig,
    /// Optional provider for LLM-based summarization.
    provider: Option<Arc<dyn Provider>>,
}

impl CompressionEngine {
    /// Create a new compression engine with the given configuration.
    pub fn new(config: CompressionConfig) -> Self {
        let monitor_config = ContextMonitorConfig {
            context_limit: config.context_limit,
            compaction_threshold: config.compaction_threshold,
            head_messages: config.head_messages,
            tail_messages: config.tail_messages,
        };
        let monitor = ContextMonitor::with_config(monitor_config);
        Self {
            monitor,
            config,
            provider: None,
        }
    }

    /// Create a compression engine with a provider for LLM summarization.
    pub fn with_provider(config: CompressionConfig, provider: Arc<dyn Provider>) -> Self {
        let monitor_config = ContextMonitorConfig {
            context_limit: config.context_limit,
            compaction_threshold: config.compaction_threshold,
            head_messages: config.head_messages,
            tail_messages: config.tail_messages,
        };
        let monitor = ContextMonitor::with_config(monitor_config);
        Self {
            monitor,
            config,
            provider: Some(provider),
        }
    }

    /// Check if compaction is needed for the given messages.
    pub fn should_compact(&self, messages: &[Message]) -> bool {
        self.monitor.needs_compaction(messages)
    }

    /// Get the current context usage as a fraction of the limit.
    pub fn usage_percent(&self, messages: &[Message]) -> f64 {
        self.monitor.usage_percent(messages)
    }

    /// Get the recommended compaction strategy.
    pub fn suggest_strategy(&self, messages: &[Message]) -> CompactionStrategy {
        self.monitor.suggest_strategy(messages)
    }

    /// Compact messages using the recommended strategy.
    ///
    /// If LLM summarization is enabled and a provider is available,
    /// generates a summary of the middle section using the LLM.
    /// Otherwise, uses extractive summarization (concat of key facts).
    pub async fn compact(&self, messages: &[Message]) -> Result<(Vec<Message>, Option<CompressionResult>)> {
        if !self.should_compact(messages) {
            return Ok((messages.to_vec(), None));
        }

        let strategy = self.suggest_strategy(messages);
        info!(
            "Context compaction triggered: strategy={:?}, usage={:.1}%",
            strategy,
            self.usage_percent(messages) * 100.0
        );

        self.apply_strategy(messages, &strategy).await
    }

    /// Apply a specific compaction strategy to messages.
    pub async fn apply_strategy(
        &self,
        messages: &[Message],
        strategy: &CompactionStrategy,
    ) -> Result<(Vec<Message>, Option<CompressionResult>)> {
        let tokens_before = ContextMonitor::estimate_tokens(messages);

        match strategy {
            CompactionStrategy::None => Ok((messages.to_vec(), None)),
            CompactionStrategy::Summarize { head_keep, tail_keep } => {
                let summary = self.generate_summary(messages, *head_keep, *tail_keep).await;
                let (compacted, result) = ContextCompactor::compact_summarize(
                    messages,
                    *head_keep,
                    *tail_keep,
                    &summary,
                );

                let compression_result = CompressionResult {
                    strategy: strategy.clone(),
                    messages_before: messages.len(),
                    messages_after: compacted.len(),
                    tokens_before,
                    tokens_after: ContextMonitor::estimate_tokens(&compacted),
                    summary: result.summary,
                    tool_pairs_preserved: result.tool_pairs_preserved,
                };

                info!(
                    "Compaction complete: {} -> {} messages, {} -> {} tokens (strategy: summarize)",
                    compression_result.messages_before,
                    compression_result.messages_after,
                    compression_result.tokens_before,
                    compression_result.tokens_after,
                );

                Ok((compacted, Some(compression_result)))
            }
            CompactionStrategy::Truncate { head_keep, tail_keep } => {
                let (compacted, result) = ContextCompactor::compact_truncate(
                    messages,
                    *head_keep,
                    *tail_keep,
                );

                let compression_result = CompressionResult {
                    strategy: strategy.clone(),
                    messages_before: messages.len(),
                    messages_after: compacted.len(),
                    tokens_before,
                    tokens_after: ContextMonitor::estimate_tokens(&compacted),
                    summary: None,
                    tool_pairs_preserved: result.tool_pairs_preserved,
                };

                info!(
                    "Compaction complete: {} -> {} messages, {} -> {} tokens (strategy: truncate)",
                    compression_result.messages_before,
                    compression_result.messages_after,
                    compression_result.tokens_before,
                    compression_result.tokens_after,
                );

                Ok((compacted, Some(compression_result)))
            }
        }
    }

    /// Generate a summary of the middle section of messages.
    ///
    /// If LLM summarization is enabled and a provider is available,
    /// calls the provider to generate a summary. Otherwise, uses
    /// a simple extractive summary (concat of key facts from messages).
    async fn generate_summary(
        &self,
        messages: &[Message],
        head_keep: usize,
        tail_keep: usize,
    ) -> String {
        let head_end = ContextMonitor::find_head_boundary(messages, head_keep);
        let naive_tail_start = messages.len().saturating_sub(tail_keep);
        let tail_start = if naive_tail_start <= head_end {
            head_end
        } else {
            naive_tail_start
        };

        let middle = &messages[head_end..tail_start];

        if middle.is_empty() {
            return "[Conversation context was compacted]".to_string();
        }

        if self.config.use_llm_summarization {
            if let Some(provider) = &self.provider {
                match self.llm_summarize(provider, middle).await {
                    Ok(summary) => return summary,
                    Err(e) => {
                        warn!("LLM summarization failed, falling back to extractive: {}", e);
                    }
                }
            }
        }

        // Fallback: extractive summary
        Self::extractive_summary(middle)
    }

    /// Use the LLM provider to generate a summary.
    async fn llm_summarize(
        &self,
        provider: &Arc<dyn Provider>,
        messages: &[Message],
    ) -> Result<String> {
        let prompt = ContextCompactor::build_summary_prompt(messages);
        let summary_messages = vec![
            Message::system(
                "You are a helpful assistant that creates concise summaries of conversations. \
                 Summarize the key points, decisions, and context that would be important \
                 for continuing the conversation. Be brief but preserve essential information.",
            ),
            Message::user(&prompt),
        ];

        let model = self
            .config
            .summary_model
            .as_deref()
            .unwrap_or("claude-sonnet-4-20250514");

        let options = smartassist_core::types::ChatOptions::with_max_tokens(1024);

        let response = provider
            .chat(model, &summary_messages, Some(options))
            .await
            .map_err(|e| crate::AgentError::ModelApi(format!("Summarization failed: {}", e)))?;

        Ok(response.to_text())
    }

    /// Generate a simple extractive summary from messages.
    ///
    /// Concatenates key facts from each message type (user questions,
    /// assistant decisions, tool results) into a concise narrative.
    fn extractive_summary(messages: &[Message]) -> String {
        let mut summary_parts = Vec::new();
        let mut user_count = 0;
        let mut assistant_count = 0;
        let mut tool_count = 0;

        for msg in messages {
            match msg.role {
                smartassist_core::types::Role::User => {
                    user_count += 1;
                    let text = msg.content.to_text();
                    // Truncate long messages
                    let truncated = if text.len() > 200 {
                        format!("{}...", &text[..200])
                    } else {
                        text
                    };
                    summary_parts.push(format!("User asked: {}", truncated));
                }
                smartassist_core::types::Role::Assistant => {
                    assistant_count += 1;
                    let text = msg.content.to_text();
                    let truncated = if text.len() > 200 {
                        format!("{}...", &text[..200])
                    } else {
                        text
                    };
                    summary_parts.push(format!("Assistant responded: {}", truncated));
                }
                smartassist_core::types::Role::Tool => {
                    tool_count += 1;
                    // Brief tool result mention
                    let text = msg.content.to_text();
                    let truncated = if text.len() > 100 {
                        format!("{}...", &text[..100])
                    } else {
                        text
                    };
                    summary_parts.push(format!("Tool result: {}", truncated));
                }
                smartassist_core::types::Role::System => {
                    // Skip system messages in summary — they're preserved in the head
                }
            }
        }

        if summary_parts.is_empty() {
            return format!(
                "[Compacted: {} user, {} assistant, {} tool messages]",
                user_count, assistant_count, tool_count
            );
        }

        // Limit total summary length
        let mut summary = format!(
            "[Compacted conversation: {} user messages, {} assistant responses, {} tool results]\n",
            user_count, assistant_count, tool_count
        );
        for part in summary_parts.iter().take(10) {
            summary.push_str(part);
            summary.push('\n');
        }

        if summary_parts.len() > 10 {
            summary.push_str(&format!("... and {} more interactions", summary_parts.len() - 10));
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert_eq!(config.context_limit, 100_000);
        assert!((config.compaction_threshold - 0.8).abs() < f64::EPSILON);
        assert_eq!(config.head_messages, 2);
        assert_eq!(config.tail_messages, 10);
        assert!(!config.use_llm_summarization);
        assert!(config.summary_model.is_none());
    }

    #[test]
    fn test_compression_engine_new() {
        let config = CompressionConfig::default();
        let engine = CompressionEngine::new(config);
        assert!(!engine.should_compact(&[]));
    }

    #[test]
    fn test_compression_engine_usage_empty() {
        let config = CompressionConfig::default();
        let engine = CompressionEngine::new(config);
        let usage = engine.usage_percent(&[]);
        assert!((usage - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compression_engine_no_compaction_needed() {
        let config = CompressionConfig {
            context_limit: 100_000,
            ..Default::default()
        };
        let engine = CompressionEngine::new(config);
        let messages = vec![Message::user("Hello")];
        assert!(!engine.should_compact(&messages));
    }

    #[test]
    fn test_compression_engine_compaction_needed() {
        let config = CompressionConfig {
            context_limit: 10, // Very small limit
            ..Default::default()
        };
        let engine = CompressionEngine::new(config);
        let messages = vec![
            Message::user("This is a longer message that should use many tokens"),
            Message::assistant("And this is an equally long response with more words"),
        ];
        assert!(engine.should_compact(&messages));
    }

    #[tokio::test]
    async fn test_compact_no_compaction_needed() {
        let config = CompressionConfig {
            context_limit: 100_000,
            ..Default::default()
        };
        let engine = CompressionEngine::new(config);
        let messages = vec![Message::user("Hello")];
        let (compacted, result) = engine.compact(&messages).await.unwrap();
        assert_eq!(compacted.len(), 1);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_compact_with_truncation() {
        let config = CompressionConfig {
            context_limit: 50, // Very small to trigger truncation
            compaction_threshold: 0.7,
            head_messages: 1,
            tail_messages: 2,
            ..Default::default()
        };
        let engine = CompressionEngine::new(config);

        // Create enough messages to exceed threshold
        let mut messages = vec![Message::system("You are helpful.")];
        for i in 0..10 {
            messages.push(Message::user(format!("Question {} with some extra text to add tokens", i)));
            messages.push(Message::assistant(format!("Answer {} with some additional content here", i)));
        }

        let (compacted, result) = engine.compact(&messages).await.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(compacted.len() < messages.len());
        assert!(result.messages_after < result.messages_before);
    }

    #[test]
    fn test_extractive_summary_basic() {
        let messages = vec![
            Message::user("What is Rust?"),
            Message::assistant("Rust is a systems programming language."),
        ];
        let summary = CompressionEngine::extractive_summary(&messages);
        assert!(summary.contains("User asked"));
        assert!(summary.contains("Assistant responded"));
        assert!(summary.contains("What is Rust?"));
    }

    #[test]
    fn test_extractive_summary_empty() {
        let messages: Vec<Message> = vec![];
        let summary = CompressionEngine::extractive_summary(&messages);
        assert!(summary.contains("Compacted"));
    }

    #[test]
    fn test_extractive_summary_system_skipped() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
        ];
        let summary = CompressionEngine::extractive_summary(&messages);
        assert!(!summary.contains("You are a helpful assistant"));
        assert!(summary.contains("Hello"));
    }

    #[test]
    fn test_compression_result_debug() {
        let result = CompressionResult {
            strategy: CompactionStrategy::None,
            messages_before: 10,
            messages_after: 5,
            tokens_before: 1000,
            tokens_after: 500,
            summary: Some("Summary text".to_string()),
            tool_pairs_preserved: 2,
        };
        assert_eq!(result.messages_before, 10);
        assert_eq!(result.messages_after, 5);
    }
}