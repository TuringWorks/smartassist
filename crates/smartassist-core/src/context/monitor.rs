//! Context monitor for token estimation and compaction strategy selection.
//!
//! Tracks context window usage via a word-count heuristic and recommends
//! compaction strategies when usage exceeds configurable thresholds.

use crate::types::{Message, MessageContent, ContentBlock};

/// Average number of tokens per whitespace-delimited word.
const TOKENS_PER_WORD: f64 = 1.3;

/// Overhead tokens per message for role header / framing.
const MESSAGE_OVERHEAD: usize = 4;

/// Monitors context window usage and recommends compaction strategies.
#[derive(Debug, Clone)]
pub struct ContextMonitor {
    /// Model-specific context window limit in tokens.
    context_limit: usize,
    /// Fraction of the context limit at which compaction is triggered (0.0 - 1.0).
    compaction_threshold: f64,
}

/// Strategy recommendation from the context monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionStrategy {
    /// No compaction needed (usage below threshold).
    None,
    /// Summarize older messages, keeping the most recent `keep_recent` turns.
    Summarize { keep_recent: usize },
    /// Aggressively truncate, keeping only the most recent `keep_recent` turns.
    Truncate { keep_recent: usize },
}

impl ContextMonitor {
    /// Create a new monitor with the given context window limit.
    ///
    /// The default compaction threshold is 0.8 (80%).
    pub fn new(context_limit: usize) -> Self {
        Self {
            context_limit,
            compaction_threshold: 0.8,
        }
    }

    /// Override the compaction threshold (fraction of context limit).
    ///
    /// Values should be between 0.0 and 1.0.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.compaction_threshold = threshold;
        self
    }

    /// Estimate the token count for a slice of messages.
    ///
    /// Uses a word-count heuristic: split on whitespace, multiply by
    /// `TOKENS_PER_WORD` (1.3). Each message also adds a fixed overhead
    /// for the role header. For `ToolUse` and `ToolResult` content blocks,
    /// the estimated JSON size divided by 4 is used instead of word counting.
    pub fn estimate_tokens(messages: &[Message]) -> usize {
        let mut total: f64 = 0.0;

        for msg in messages {
            // Per-message overhead for role header framing
            total += MESSAGE_OVERHEAD as f64;

            match &msg.content {
                MessageContent::Text(text) => {
                    total += estimate_text_tokens(text);
                }
                MessageContent::Blocks(blocks) => {
                    for block in blocks {
                        total += estimate_block_tokens(block);
                    }
                }
            }
        }

        total.ceil() as usize
    }

    /// Return the current usage as a fraction (0.0 - 1.0+) of the context limit.
    pub fn usage_percent(&self, messages: &[Message]) -> f64 {
        let tokens = Self::estimate_tokens(messages) as f64;
        tokens / self.context_limit as f64
    }

    /// Check whether the messages exceed the compaction threshold.
    pub fn needs_compaction(&self, messages: &[Message]) -> bool {
        self.usage_percent(messages) >= self.compaction_threshold
    }

    /// Suggest a compaction strategy based on current usage.
    ///
    /// - Below 80%: `None`
    /// - 80% to 90%: `Summarize` (keep 10 recent messages)
    /// - Above 90%: `Truncate` (keep 5 recent messages)
    pub fn suggest_strategy(&self, messages: &[Message]) -> CompactionStrategy {
        let usage = self.usage_percent(messages);
        if usage < 0.8 {
            CompactionStrategy::None
        } else if usage < 0.9 {
            CompactionStrategy::Summarize { keep_recent: 10 }
        } else {
            CompactionStrategy::Truncate { keep_recent: 5 }
        }
    }
}

/// Estimate tokens for a plain text string using the word-count heuristic.
fn estimate_text_tokens(text: &str) -> f64 {
    let word_count = text.split_whitespace().count();
    word_count as f64 * TOKENS_PER_WORD
}

/// Estimate tokens for a single content block.
fn estimate_block_tokens(block: &ContentBlock) -> f64 {
    match block {
        ContentBlock::Text { text } => estimate_text_tokens(text),
        ContentBlock::Image { source } => {
            // Estimate based on JSON-serialized size of the image source
            let json_size = source.data.len() + source.media_type.len() + source.source_type.len();
            json_size as f64 / 4.0
        }
        ContentBlock::ToolUse { id, name, input } => {
            // Estimate based on JSON-serialized size
            let input_str = serde_json::to_string(input).unwrap_or_default();
            let json_size = id.len() + name.len() + input_str.len();
            json_size as f64 / 4.0
        }
        ContentBlock::ToolResult { tool_use_id, content, .. } => {
            let json_size = tool_use_id.len() + content.len();
            json_size as f64 / 4.0
        }
        ContentBlock::Thinking { thinking } => estimate_text_tokens(thinking),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ImageSource;
    use serde_json::json;

    #[test]
    fn test_new_default_threshold() {
        let monitor = ContextMonitor::new(100_000);
        assert_eq!(monitor.context_limit, 100_000);
        assert!((monitor.compaction_threshold - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_with_threshold() {
        let monitor = ContextMonitor::new(100_000).with_threshold(0.5);
        assert!((monitor.compaction_threshold - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        let tokens = ContextMonitor::estimate_tokens(&[]);
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_tokens_single_text_message() {
        // "Hello world" = 2 words * 1.3 = 2.6 + 4 overhead = 6.6 -> ceil = 7
        let messages = vec![Message::user("Hello world")];
        let tokens = ContextMonitor::estimate_tokens(&messages);
        assert_eq!(tokens, 7);
    }

    #[test]
    fn test_estimate_tokens_multiple_messages() {
        let messages = vec![
            Message::user("Hello world"),        // 2 words * 1.3 + 4 = 6.6
            Message::assistant("Hi there friend"), // 3 words * 1.3 + 4 = 7.9
        ];
        let tokens = ContextMonitor::estimate_tokens(&messages);
        // 6.6 + 7.9 = 14.5 -> ceil = 15
        assert_eq!(tokens, 15);
    }

    #[test]
    fn test_estimate_tokens_empty_text() {
        let messages = vec![Message::user("")];
        let tokens = ContextMonitor::estimate_tokens(&messages);
        // 0 words * 1.3 + 4 overhead = 4
        assert_eq!(tokens, 4);
    }

    #[test]
    fn test_estimate_tokens_tool_use_block() {
        let msg = Message {
            role: crate::types::Role::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
                input: json!({"path": "/tmp/test.txt"}),
            }]),
            name: None,
            tool_use_id: None,
            timestamp: chrono::Utc::now(),
        };
        let tokens = ContextMonitor::estimate_tokens(&[msg]);
        // JSON size: "tool_1"(6) + "read_file"(9) + json string len / 4 + 4 overhead
        assert!(tokens > 4); // must be greater than just overhead
    }

    #[test]
    fn test_estimate_tokens_tool_result_block() {
        let msg = Message::tool_result("tool_1", "File contents here", false);
        let tokens = ContextMonitor::estimate_tokens(&[msg]);
        // tool_use_id(6) + content(18) = 24 / 4 = 6.0 + 4 overhead = 10
        assert_eq!(tokens, 10);
    }

    #[test]
    fn test_estimate_tokens_mixed_blocks() {
        let msg = Message {
            role: crate::types::Role::Assistant,
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "Here is the result".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "bash".to_string(),
                    input: json!({"cmd": "ls"}),
                },
            ]),
            name: None,
            tool_use_id: None,
            timestamp: chrono::Utc::now(),
        };
        let tokens = ContextMonitor::estimate_tokens(&[msg]);
        // Text: 4 words * 1.3 = 5.2
        // ToolUse: ("t1"(2) + "bash"(4) + json_str_len) / 4
        // + 4 overhead
        assert!(tokens > 4);
    }

    #[test]
    fn test_estimate_tokens_thinking_block() {
        let msg = Message {
            role: crate::types::Role::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::Thinking {
                thinking: "Let me think about this carefully".to_string(),
            }]),
            name: None,
            tool_use_id: None,
            timestamp: chrono::Utc::now(),
        };
        let tokens = ContextMonitor::estimate_tokens(&[msg]);
        // 6 words * 1.3 = 7.8 + 4 overhead = 11.8 -> ceil = 12
        assert_eq!(tokens, 12);
    }

    #[test]
    fn test_estimate_tokens_image_block() {
        let msg = Message {
            role: crate::types::Role::User,
            content: MessageContent::Blocks(vec![ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64".to_string(),
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgo=".to_string(), // small base64 snippet
                },
            }]),
            name: None,
            tool_use_id: None,
            timestamp: chrono::Utc::now(),
        };
        let tokens = ContextMonitor::estimate_tokens(&[msg]);
        // (12 + 9 + 6) / 4 = 6.75 + 4 = 10.75 -> ceil = 11
        assert_eq!(tokens, 11);
    }

    #[test]
    fn test_usage_percent() {
        let monitor = ContextMonitor::new(100);
        // "Hello world" = 2 words * 1.3 + 4 overhead = 6.6, ceil = 7
        // estimate_tokens returns 7, so usage = 7.0 / 100.0 = 0.07
        let messages = vec![Message::user("Hello world")];
        let usage = monitor.usage_percent(&messages);
        assert!((usage - 0.07).abs() < 0.001);
    }

    #[test]
    fn test_needs_compaction_below_threshold() {
        let monitor = ContextMonitor::new(1000);
        let messages = vec![Message::user("short")];
        assert!(!monitor.needs_compaction(&messages));
    }

    #[test]
    fn test_needs_compaction_above_threshold() {
        // Create a monitor with a small limit so a few messages exceed the threshold
        let monitor = ContextMonitor::new(10);
        let messages = vec![
            Message::user("This is a longer message that should use many tokens"),
            Message::assistant("And this is an equally long response with more words"),
        ];
        assert!(monitor.needs_compaction(&messages));
    }

    #[test]
    fn test_suggest_strategy_none() {
        let monitor = ContextMonitor::new(100_000);
        let messages = vec![Message::user("Hello")];
        assert_eq!(
            monitor.suggest_strategy(&messages),
            CompactionStrategy::None
        );
    }

    #[test]
    fn test_suggest_strategy_summarize() {
        // We need usage between 80% and 90%.
        // With context_limit=10, we need 8-9 estimated tokens.
        // "Hello world" = 2*1.3 + 4 = 6.6. Two such messages = 13.2, too high for limit 10.
        // With context_limit=15: 6.6/15 = 0.44. Need higher.
        // Let's use context_limit=8: 6.6/8 = 0.825 -> Summarize
        let monitor = ContextMonitor::new(8);
        let messages = vec![Message::user("Hello world")];
        assert_eq!(
            monitor.suggest_strategy(&messages),
            CompactionStrategy::Summarize { keep_recent: 10 }
        );
    }

    #[test]
    fn test_suggest_strategy_truncate() {
        // Need usage >= 90%. With context_limit=7: 6.6/7 = 0.943 -> Truncate
        let monitor = ContextMonitor::new(7);
        let messages = vec![Message::user("Hello world")];
        assert_eq!(
            monitor.suggest_strategy(&messages),
            CompactionStrategy::Truncate { keep_recent: 5 }
        );
    }

    #[test]
    fn test_strategy_at_boundary_summarize_range() {
        // "a b c d" = 4 words * 1.3 = 5.2 + 4 overhead = 9.2, ceil = 10
        // With limit=12: 10/12 = 0.833 -> in the 80-90% range -> Summarize
        let monitor = ContextMonitor::new(12);
        let messages = vec![Message::user("a b c d")];
        let usage = monitor.usage_percent(&messages);
        assert!(usage >= 0.8 && usage < 0.9, "usage was {}", usage);
        assert_eq!(
            monitor.suggest_strategy(&messages),
            CompactionStrategy::Summarize { keep_recent: 10 }
        );
    }

    #[test]
    fn test_custom_threshold_affects_needs_compaction() {
        let monitor = ContextMonitor::new(100).with_threshold(0.05);
        // "Hello world" = 6.6 tokens, 6.6/100 = 0.066 >= 0.05
        let messages = vec![Message::user("Hello world")];
        assert!(monitor.needs_compaction(&messages));
    }

    #[test]
    fn test_estimate_tokens_system_message() {
        let messages = vec![Message::system("You are a helpful assistant")];
        let tokens = ContextMonitor::estimate_tokens(&messages);
        // 5 words * 1.3 = 6.5 + 4 = 10.5 -> ceil = 11
        assert_eq!(tokens, 11);
    }
}
