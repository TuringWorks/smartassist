//! Context compactor that executes compaction strategies.
//!
//! Provides methods to reduce conversation history via summarization
//! or truncation, keeping the most recent messages intact.

use super::monitor::ContextMonitor;
use crate::types::{Message, Role};

/// Stateless compactor that applies compaction strategies to message lists.
pub struct ContextCompactor;

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// Number of messages removed from the conversation.
    pub messages_removed: usize,
    /// Estimated token count before compaction.
    pub tokens_before: usize,
    /// Estimated token count after compaction.
    pub tokens_after: usize,
    /// Summary text if summarization was used, `None` for truncation.
    pub summary: Option<String>,
}

impl ContextCompactor {
    /// Compact via summarization: replace older messages with a summary,
    /// keeping the most recent `keep_recent` messages.
    ///
    /// Returns a new message list starting with a system message containing
    /// `summary_text`, followed by the last `keep_recent` messages from the
    /// original list. If `keep_recent` >= `messages.len()`, returns the
    /// original messages unchanged (no compaction needed).
    pub fn compact_summarize(
        messages: &[Message],
        keep_recent: usize,
        summary_text: &str,
    ) -> (Vec<Message>, CompactionResult) {
        let tokens_before = ContextMonitor::estimate_tokens(messages);

        // Nothing to compact if we would keep everything
        if keep_recent >= messages.len() {
            return (
                messages.to_vec(),
                CompactionResult {
                    messages_removed: 0,
                    tokens_before,
                    tokens_after: tokens_before,
                    summary: None,
                },
            );
        }

        let split_point = messages.len() - keep_recent;
        let recent = &messages[split_point..];

        // Build new message list: summary + recent messages
        let mut compacted = Vec::with_capacity(1 + keep_recent);
        compacted.push(Message::system(summary_text));
        compacted.extend_from_slice(recent);

        let tokens_after = ContextMonitor::estimate_tokens(&compacted);

        let result = CompactionResult {
            messages_removed: split_point,
            tokens_before,
            tokens_after,
            summary: Some(summary_text.to_string()),
        };

        (compacted, result)
    }

    /// Compact via truncation: drop oldest messages, keeping only the
    /// most recent `keep_recent` messages.
    ///
    /// If `keep_recent` >= `messages.len()`, returns the original messages
    /// unchanged. No summary is generated.
    pub fn compact_truncate(
        messages: &[Message],
        keep_recent: usize,
    ) -> (Vec<Message>, CompactionResult) {
        let tokens_before = ContextMonitor::estimate_tokens(messages);

        if keep_recent >= messages.len() {
            return (
                messages.to_vec(),
                CompactionResult {
                    messages_removed: 0,
                    tokens_before,
                    tokens_after: tokens_before,
                    summary: None,
                },
            );
        }

        let split_point = messages.len() - keep_recent;
        let recent = messages[split_point..].to_vec();
        let tokens_after = ContextMonitor::estimate_tokens(&recent);

        let result = CompactionResult {
            messages_removed: split_point,
            tokens_before,
            tokens_after,
            summary: None,
        };

        (recent, result)
    }

    /// Build a prompt asking a model to summarize the given messages.
    ///
    /// Formats each message as "Role: content" and appends instructions
    /// requesting a concise summary of the conversation so far.
    pub fn build_summary_prompt(messages_to_summarize: &[Message]) -> String {
        let mut prompt = String::from(
            "Please provide a concise summary of the following conversation:\n\n",
        );

        for msg in messages_to_summarize {
            let role_str = format_role(msg.role);
            let content_str = msg.content.to_text();
            prompt.push_str(&format!("{}: {}\n", role_str, content_str));
        }

        prompt.push_str(
            "\nSummarize the key points, decisions, and context from this conversation \
             in a concise paragraph that preserves essential information for continuing \
             the conversation.",
        );

        prompt
    }
}

/// Format a role enum variant as a human-readable string.
fn format_role(role: Role) -> &'static str {
    match role {
        Role::User => "User",
        Role::Assistant => "Assistant",
        Role::System => "System",
        Role::Tool => "Tool",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a simple conversation of `n` user/assistant turn pairs.
    fn make_conversation(n: usize) -> Vec<Message> {
        let mut messages = Vec::with_capacity(n * 2);
        for i in 0..n {
            messages.push(Message::user(format!("Question {}", i)));
            messages.push(Message::assistant(format!("Answer {}", i)));
        }
        messages
    }

    // -- compact_summarize tests --

    #[test]
    fn test_summarize_basic() {
        let messages = make_conversation(5); // 10 messages
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            4,
            "Summary of earlier conversation.",
        );

        // Should have 1 summary + 4 recent = 5 messages
        assert_eq!(compacted.len(), 5);
        assert_eq!(result.messages_removed, 6);
        assert_eq!(result.summary, Some("Summary of earlier conversation.".to_string()));
        assert!(result.tokens_after < result.tokens_before);

        // First message should be the system summary
        assert_eq!(compacted[0].role, Role::System);
        assert_eq!(compacted[0].content.to_text(), "Summary of earlier conversation.");
    }

    #[test]
    fn test_summarize_keep_all() {
        let messages = make_conversation(2); // 4 messages
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            10, // keep_recent > len
            "This summary should not be used.",
        );

        // No compaction: all original messages returned
        assert_eq!(compacted.len(), 4);
        assert_eq!(result.messages_removed, 0);
        assert!(result.summary.is_none());
        assert_eq!(result.tokens_before, result.tokens_after);
    }

    #[test]
    fn test_summarize_keep_exact_length() {
        let messages = make_conversation(3); // 6 messages
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            6,
            "Summary.",
        );

        // keep_recent == len, no compaction
        assert_eq!(compacted.len(), 6);
        assert_eq!(result.messages_removed, 0);
    }

    #[test]
    fn test_summarize_keep_one() {
        let messages = make_conversation(3); // 6 messages
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            1,
            "Conversation summary.",
        );

        // 1 summary + 1 recent = 2
        assert_eq!(compacted.len(), 2);
        assert_eq!(result.messages_removed, 5);
        // Last message should be the original last message
        assert_eq!(compacted[1].content.to_text(), "Answer 2");
    }

    #[test]
    fn test_summarize_empty_messages() {
        let messages: Vec<Message> = vec![];
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            5,
            "Summary.",
        );

        assert!(compacted.is_empty());
        assert_eq!(result.messages_removed, 0);
        assert!(result.summary.is_none());
    }

    // -- compact_truncate tests --

    #[test]
    fn test_truncate_basic() {
        let messages = make_conversation(5); // 10 messages
        let (compacted, result) = ContextCompactor::compact_truncate(&messages, 4);

        assert_eq!(compacted.len(), 4);
        assert_eq!(result.messages_removed, 6);
        assert!(result.summary.is_none());
        assert!(result.tokens_after < result.tokens_before);

        // Should keep the last 4 messages
        assert_eq!(compacted[0].content.to_text(), "Question 3");
        assert_eq!(compacted[3].content.to_text(), "Answer 4");
    }

    #[test]
    fn test_truncate_keep_all() {
        let messages = make_conversation(2); // 4 messages
        let (compacted, result) = ContextCompactor::compact_truncate(&messages, 10);

        assert_eq!(compacted.len(), 4);
        assert_eq!(result.messages_removed, 0);
        assert_eq!(result.tokens_before, result.tokens_after);
    }

    #[test]
    fn test_truncate_keep_one() {
        let messages = make_conversation(3); // 6 messages
        let (compacted, result) = ContextCompactor::compact_truncate(&messages, 1);

        assert_eq!(compacted.len(), 1);
        assert_eq!(result.messages_removed, 5);
        assert_eq!(compacted[0].content.to_text(), "Answer 2");
    }

    #[test]
    fn test_truncate_empty_messages() {
        let messages: Vec<Message> = vec![];
        let (compacted, result) = ContextCompactor::compact_truncate(&messages, 5);

        assert!(compacted.is_empty());
        assert_eq!(result.messages_removed, 0);
    }

    // -- build_summary_prompt tests --

    #[test]
    fn test_build_summary_prompt_basic() {
        let messages = vec![
            Message::user("What is Rust?"),
            Message::assistant("Rust is a systems programming language."),
        ];

        let prompt = ContextCompactor::build_summary_prompt(&messages);

        assert!(prompt.contains("User: What is Rust?"));
        assert!(prompt.contains("Assistant: Rust is a systems programming language."));
        assert!(prompt.contains("concise summary"));
        assert!(prompt.contains("key points"));
    }

    #[test]
    fn test_build_summary_prompt_with_system_message() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
        ];

        let prompt = ContextCompactor::build_summary_prompt(&messages);
        assert!(prompt.contains("System: You are a helpful assistant."));
        assert!(prompt.contains("User: Hello"));
    }

    #[test]
    fn test_build_summary_prompt_with_tool_message() {
        let messages = vec![
            Message::user("Read the file"),
            Message::tool_result("tool_1", "File contents here", false),
        ];

        let prompt = ContextCompactor::build_summary_prompt(&messages);
        assert!(prompt.contains("User: Read the file"));
        assert!(prompt.contains("Tool:"));
    }

    #[test]
    fn test_build_summary_prompt_empty() {
        let messages: Vec<Message> = vec![];
        let prompt = ContextCompactor::build_summary_prompt(&messages);

        // Should still have the instruction text
        assert!(prompt.contains("concise summary"));
        // But no role lines
        assert!(!prompt.contains("User:"));
    }

    // -- CompactionResult tests --

    #[test]
    fn test_compaction_result_tokens_decrease_on_summarize() {
        // Build a longer conversation to ensure meaningful compaction
        let messages = make_conversation(20); // 40 messages
        let (_, result) = ContextCompactor::compact_summarize(
            &messages,
            4,
            "Brief summary.",
        );

        assert!(result.tokens_after < result.tokens_before);
        assert_eq!(result.messages_removed, 36);
    }

    #[test]
    fn test_compaction_result_tokens_decrease_on_truncate() {
        let messages = make_conversation(20); // 40 messages
        let (_, result) = ContextCompactor::compact_truncate(&messages, 4);

        assert!(result.tokens_after < result.tokens_before);
        assert_eq!(result.messages_removed, 36);
    }
}
