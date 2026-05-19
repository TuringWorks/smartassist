//! Context monitor for token estimation and compaction strategy selection.
//!
//! Tracks context window usage via a word-count heuristic and recommends
//! compaction strategies when usage exceeds configurable thresholds.
//! Supports head/tail-protected middle-turn summarization.

use crate::types::{ContentBlock, ImageSourceType, Message, MessageContent};

/// Average number of tokens per whitespace-delimited word.
const TOKENS_PER_WORD: f64 = 1.3;

/// Overhead tokens per message for role header / framing.
const MESSAGE_OVERHEAD: usize = 4;

/// Configuration for context monitoring.
#[derive(Debug, Clone)]
pub struct ContextMonitorConfig {
    /// Model-specific context window limit in tokens.
    pub context_limit: usize,
    /// Fraction of the context limit at which compaction is triggered (0.0 - 1.0).
    pub compaction_threshold: f64,
    /// Number of head messages (system + initial exchanges) to preserve during compaction.
    pub head_messages: usize,
    /// Number of recent tail messages to preserve during compaction.
    pub tail_messages: usize,
}

impl Default for ContextMonitorConfig {
    fn default() -> Self {
        Self {
            context_limit: 100_000,
            compaction_threshold: 0.8,
            head_messages: 2,
            tail_messages: 10,
        }
    }
}

/// Monitors context window usage and recommends compaction strategies.
#[derive(Debug, Clone)]
pub struct ContextMonitor {
    config: ContextMonitorConfig,
}

/// Strategy recommendation from the context monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionStrategy {
    /// No compaction needed (usage below threshold).
    None,
    /// Summarize middle messages, preserving head and tail.
    /// `head_keep` system/initial messages, `tail_keep` recent messages.
    Summarize {
        head_keep: usize,
        tail_keep: usize,
    },
    /// Aggressively truncate, preserving head and tail.
    /// `head_keep` system/initial messages, `tail_keep` recent messages.
    Truncate {
        head_keep: usize,
        tail_keep: usize,
    },
}

impl ContextMonitor {
    /// Create a new monitor with the given context window limit.
    ///
    /// The default compaction threshold is 0.8 (80%).
    pub fn new(context_limit: usize) -> Self {
        Self {
            config: ContextMonitorConfig {
                context_limit,
                ..Default::default()
            },
        }
    }

    /// Create a monitor from a full config.
    pub fn with_config(config: ContextMonitorConfig) -> Self {
        Self { config }
    }

    /// Override the compaction threshold (fraction of context limit).
    ///
    /// Values should be between 0.0 and 1.0.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.config.compaction_threshold = threshold;
        self
    }

    /// Get the context limit.
    pub fn context_limit(&self) -> usize {
        self.config.context_limit
    }

    /// Get the compaction threshold.
    pub fn compaction_threshold(&self) -> f64 {
        self.config.compaction_threshold
    }

    /// Get the head messages config.
    pub fn head_messages(&self) -> usize {
        self.config.head_messages
    }

    /// Get the tail messages config.
    pub fn tail_messages(&self) -> usize {
        self.config.tail_messages
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
        tokens / self.config.context_limit as f64
    }

    /// Check whether the messages exceed the compaction threshold.
    pub fn needs_compaction(&self, messages: &[Message]) -> bool {
        self.usage_percent(messages) >= self.config.compaction_threshold
    }

    /// Suggest a compaction strategy based on current usage.
    ///
    /// - Below threshold: `None`
    /// - Threshold to 90%: `Summarize` (with configured head/tail keep)
    /// - Above 90%: `Truncate` (with configured head/tail keep)
    pub fn suggest_strategy(&self, messages: &[Message]) -> CompactionStrategy {
        let usage = self.usage_percent(messages);
        if usage < self.config.compaction_threshold {
            CompactionStrategy::None
        } else if usage < 0.9 {
            CompactionStrategy::Summarize {
                head_keep: self.config.head_messages,
                tail_keep: self.config.tail_messages,
            }
        } else {
            CompactionStrategy::Truncate {
                head_keep: self.config.head_messages,
                tail_keep: self.config.tail_messages,
            }
        }
    }

    /// Find tool-use pairs in messages.
    ///
    /// Returns a set of indices that must stay together (a ToolUse message
    /// and its corresponding ToolResult message). This ensures compaction
    /// never splits a tool call from its result.
    pub fn find_tool_pairs(messages: &[Message]) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        // Collect ToolUse IDs from assistant messages and match them
        // to ToolResult messages.
        for (i, msg) in messages.iter().enumerate() {
            if msg.role == Role::Assistant {
                if let MessageContent::Blocks(blocks) = &msg.content {
                    for block in blocks {
                        if let ContentBlock::ToolUse { id, .. } = block {
                            // Find the ToolResult message with this tool_use_id
                            for (j, other) in messages.iter().enumerate().skip(i + 1) {
                                if other.role == Role::Tool {
                                    if let MessageContent::Blocks(other_blocks) = &other.content {
                                        for ob in other_blocks {
                                            if let ContentBlock::ToolResult { tool_use_id, .. } = ob {
                                                if tool_use_id == id {
                                                    pairs.push((i, j));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        pairs
    }

    /// Identify the head section: system messages and initial exchanges.
    ///
    /// Returns the index after the last message that should be in the head.
    /// System messages are always in the head. After that, we include up to
    /// `head_messages` non-system messages.
    pub fn find_head_boundary(messages: &[Message], head_messages: usize) -> usize {
        let mut boundary = 0;
        let mut non_system_count = 0;

        for (i, msg) in messages.iter().enumerate() {
            if msg.role == Role::System {
                boundary = i + 1;
                continue;
            }
            if non_system_count < head_messages {
                non_system_count += 1;
                boundary = i + 1;
            } else {
                break;
            }
        }

        boundary
    }
}

use crate::types::Role;

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
            let type_str = match source.source_type {
                ImageSourceType::Base64 => "base64",
                ImageSourceType::Url => "url",
            };
            let json_size = source.data.len() + source.media_type.len() + type_str.len();
            json_size as f64 / 4.0
        }
        ContentBlock::ToolUse { id, name, input } => {
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
        assert_eq!(monitor.context_limit(), 100_000);
        assert!((monitor.compaction_threshold() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_with_config() {
        let config = ContextMonitorConfig {
            context_limit: 200_000,
            compaction_threshold: 0.7,
            head_messages: 3,
            tail_messages: 8,
        };
        let monitor = ContextMonitor::with_config(config);
        assert_eq!(monitor.context_limit(), 200_000);
        assert!((monitor.compaction_threshold() - 0.7).abs() < f64::EPSILON);
        assert_eq!(monitor.head_messages(), 3);
        assert_eq!(monitor.tail_messages(), 8);
    }

    #[test]
    fn test_with_threshold() {
        let monitor = ContextMonitor::new(100_000).with_threshold(0.5);
        assert!((monitor.compaction_threshold() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        let tokens = ContextMonitor::estimate_tokens(&[]);
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_tokens_single_text_message() {
        let messages = vec![Message::user("Hello world")];
        let tokens = ContextMonitor::estimate_tokens(&messages);
        assert_eq!(tokens, 7);
    }

    #[test]
    fn test_estimate_tokens_multiple_messages() {
        let messages = vec![
            Message::user("Hello world"),
            Message::assistant("Hi there friend"),
        ];
        let tokens = ContextMonitor::estimate_tokens(&messages);
        assert_eq!(tokens, 15);
    }

    #[test]
    fn test_estimate_tokens_empty_text() {
        let messages = vec![Message::user("")];
        let tokens = ContextMonitor::estimate_tokens(&messages);
        assert_eq!(tokens, 4);
    }

    #[test]
    fn test_estimate_tokens_tool_use_block() {
        let msg = Message {
            role: Role::Assistant,
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
        assert!(tokens > 4);
    }

    #[test]
    fn test_estimate_tokens_tool_result_block() {
        let msg = Message::tool_result("tool_1", "File contents here", false);
        let tokens = ContextMonitor::estimate_tokens(&[msg]);
        assert_eq!(tokens, 10);
    }

    #[test]
    fn test_estimate_tokens_thinking_block() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::Thinking {
                thinking: "Let me think about this carefully".to_string(),
            }]),
            name: None,
            tool_use_id: None,
            timestamp: chrono::Utc::now(),
        };
        let tokens = ContextMonitor::estimate_tokens(&[msg]);
        assert_eq!(tokens, 12);
    }

    #[test]
    fn test_usage_percent() {
        let monitor = ContextMonitor::new(100);
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
        let monitor = ContextMonitor::new(8);
        let messages = vec![Message::user("Hello world")];
        assert_eq!(
            monitor.suggest_strategy(&messages),
            CompactionStrategy::Summarize {
                head_keep: 2,
                tail_keep: 10,
            }
        );
    }

    #[test]
    fn test_suggest_strategy_truncate() {
        let monitor = ContextMonitor::new(7);
        let messages = vec![Message::user("Hello world")];
        assert_eq!(
            monitor.suggest_strategy(&messages),
            CompactionStrategy::Truncate {
                head_keep: 2,
                tail_keep: 10,
            }
        );
    }

    #[test]
    fn test_custom_threshold_affects_needs_compaction() {
        let monitor = ContextMonitor::new(100).with_threshold(0.05);
        let messages = vec![Message::user("Hello world")];
        assert!(monitor.needs_compaction(&messages));
    }

    #[test]
    fn test_estimate_tokens_system_message() {
        let messages = vec![Message::system("You are a helpful assistant")];
        let tokens = ContextMonitor::estimate_tokens(&messages);
        assert_eq!(tokens, 11);
    }

    #[test]
    fn test_find_tool_pairs() {
        let messages = vec![
            Message::user("Read the file"),
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "tu_1".to_string(),
                    name: "read".to_string(),
                    input: json!({"path": "/tmp/test.txt"}),
                }]),
                name: None,
                tool_use_id: None,
                timestamp: chrono::Utc::now(),
            },
            Message::tool_result("tu_1", "File contents", false),
            Message::assistant("The file contains..."),
        ];

        let pairs = ContextMonitor::find_tool_pairs(&messages);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (1, 2)); // assistant with ToolUse at index 1, Tool result at index 2
    }

    #[test]
    fn test_find_tool_pairs_multiple() {
        let messages = vec![
            Message::user("Read two files"),
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![
                    ContentBlock::ToolUse {
                        id: "tu_1".to_string(),
                        name: "read".to_string(),
                        input: json!({"path": "/a.txt"}),
                    },
                    ContentBlock::ToolUse {
                        id: "tu_2".to_string(),
                        name: "read".to_string(),
                        input: json!({"path": "/b.txt"}),
                    },
                ]),
                name: None,
                tool_use_id: None,
                timestamp: chrono::Utc::now(),
            },
            Message::tool_result("tu_1", "Content A", false),
            Message::tool_result("tu_2", "Content B", false),
        ];

        let pairs = ContextMonitor::find_tool_pairs(&messages);
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn test_find_head_boundary_system_messages() {
        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi"),
            Message::user("Question"),
        ];

        // head_messages = 2 means keep system + 2 non-system
        let boundary = ContextMonitor::find_head_boundary(&messages, 2);
        assert_eq!(boundary, 3); // system(0) + user(1) + assistant(2)
    }

    #[test]
    fn test_find_head_boundary_only_system() {
        let messages = vec![
            Message::system("System prompt"),
            Message::user("Question"),
        ];

        // head_messages = 0 means keep only system messages
        let boundary = ContextMonitor::find_head_boundary(&messages, 0);
        assert_eq!(boundary, 1); // only system message
    }

    #[test]
    fn test_find_head_boundary_no_system() {
        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi"),
            Message::user("Question"),
        ];

        // head_messages = 1 means keep 1 non-system message
        let boundary = ContextMonitor::find_head_boundary(&messages, 1);
        assert_eq!(boundary, 1);
    }

    #[test]
    fn test_strategy_at_boundary_summarize_range() {
        let monitor = ContextMonitor::new(12);
        let messages = vec![Message::user("a b c d")];
        let usage = monitor.usage_percent(&messages);
        assert!(usage >= 0.8 && usage < 0.9, "usage was {}", usage);
        assert_eq!(
            monitor.suggest_strategy(&messages),
            CompactionStrategy::Summarize {
                head_keep: 2,
                tail_keep: 10,
            }
        );
    }

    #[test]
    fn test_estimate_tokens_image_block() {
        let msg = Message {
            role: Role::User,
            content: MessageContent::Blocks(vec![ContentBlock::Image {
                source: ImageSource::base64("image/png", "iVBORw0KGgo="),
            }]),
            name: None,
            tool_use_id: None,
            timestamp: chrono::Utc::now(),
        };
        let tokens = ContextMonitor::estimate_tokens(&[msg]);
        assert_eq!(tokens, 11);
    }
}