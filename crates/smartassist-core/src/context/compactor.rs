//! Context compactor that executes compaction strategies.
//!
//! Provides methods to reduce conversation history via summarization
//! or truncation, preserving system messages (head), recent messages (tail),
//! and tool-use/result pairs.

use super::monitor::ContextMonitor;
use crate::types::{ContentBlock, Message, MessageContent, Role};

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
    /// Number of head messages preserved.
    pub head_preserved: usize,
    /// Number of tail messages preserved.
    pub tail_preserved: usize,
    /// Number of tool-use pairs preserved.
    pub tool_pairs_preserved: usize,
}

impl ContextCompactor {
    /// Compact via summarization: preserve head and tail, summarize the middle.
    ///
    /// The algorithm:
    /// 1. Identify head section (system messages + first `head_keep` non-system messages)
    /// 2. Identify tail section (last `tail_keep` messages)
    /// 3. Ensure tool-use pairs are not split across boundaries
    /// 4. Summarize the middle section into a single system message
    /// 5. Return: [head] + [summary_system_msg] + [tail]
    pub fn compact_summarize(
        messages: &[Message],
        head_keep: usize,
        tail_keep: usize,
        summary_text: &str,
    ) -> (Vec<Message>, CompactionResult) {
        let tokens_before = ContextMonitor::estimate_tokens(messages);

        // Nothing to compact if we would keep everything
        if head_keep + tail_keep >= messages.len() {
            return (
                messages.to_vec(),
                CompactionResult {
                    messages_removed: 0,
                    tokens_before,
                    tokens_after: tokens_before,
                    summary: None,
                    head_preserved: messages.len(),
                    tail_preserved: 0,
                    tool_pairs_preserved: 0,
                },
            );
        }

        // Find the head boundary
        let head_end = ContextMonitor::find_head_boundary(messages, head_keep);

        // Find the tail boundary, respecting tool pairs
        let tail_start = Self::find_tail_start(messages, tail_keep, head_end);

        // If head and tail overlap, return original
        if tail_start <= head_end {
            return (
                messages.to_vec(),
                CompactionResult {
                    messages_removed: 0,
                    tokens_before,
                    tokens_after: tokens_before,
                    summary: None,
                    head_preserved: messages.len(),
                    tail_preserved: 0,
                    tool_pairs_preserved: 0,
                },
            );
        }

        // Build compacted list: head + summary + tail
        let head = &messages[..head_end];
        let middle = &messages[head_end..tail_start];
        let tail = &messages[tail_start..];

        // Count tool pairs in preserved sections
        let tool_pairs = Self::count_tool_pairs_in_range(messages, head_end, tail_start);

        let mut compacted = Vec::with_capacity(head.len() + 1 + tail.len());
        compacted.extend_from_slice(head);
        compacted.push(Message::system(summary_text));
        compacted.extend_from_slice(tail);

        let tokens_after = ContextMonitor::estimate_tokens(&compacted);

        let result = CompactionResult {
            messages_removed: middle.len(),
            tokens_before,
            tokens_after,
            summary: Some(summary_text.to_string()),
            head_preserved: head.len(),
            tail_preserved: tail.len(),
            tool_pairs_preserved: tool_pairs,
        };

        (compacted, result)
    }

    /// Compact via truncation: preserve head and tail, drop the middle.
    ///
    /// The algorithm:
    /// 1. Identify head section (system messages + first `head_keep` non-system messages)
    /// 2. Identify tail section (last `tail_keep` messages)
    /// 3. Ensure tool-use pairs are not split across boundaries
    /// 4. Return: [head] + [tail]
    pub fn compact_truncate(
        messages: &[Message],
        head_keep: usize,
        tail_keep: usize,
    ) -> (Vec<Message>, CompactionResult) {
        let tokens_before = ContextMonitor::estimate_tokens(messages);

        // If total keep >= total, nothing to remove
        if head_keep + tail_keep >= messages.len() {
            return (
                messages.to_vec(),
                CompactionResult {
                    messages_removed: 0,
                    tokens_before,
                    tokens_after: tokens_before,
                    summary: None,
                    head_preserved: messages.len(),
                    tail_preserved: 0,
                    tool_pairs_preserved: 0,
                },
            );
        }

        // Find the head boundary
        let head_end = ContextMonitor::find_head_boundary(messages, head_keep);

        // Find the tail boundary, respecting tool pairs
        let tail_start = Self::find_tail_start(messages, tail_keep, head_end);

        // If head and tail overlap, return original
        if tail_start <= head_end {
            return (
                messages.to_vec(),
                CompactionResult {
                    messages_removed: 0,
                    tokens_before,
                    tokens_after: tokens_before,
                    summary: None,
                    head_preserved: messages.len(),
                    tail_preserved: 0,
                    tool_pairs_preserved: 0,
                },
            );
        }

        let head = &messages[..head_end];
        let tail = &messages[tail_start..];
        let removed_count = tail_start - head_end;

        let tool_pairs = Self::count_tool_pairs_in_range(messages, head_end, tail_start);

        let mut compacted = Vec::with_capacity(head.len() + tail.len());
        compacted.extend_from_slice(head);
        compacted.extend_from_slice(tail);

        let tokens_after = ContextMonitor::estimate_tokens(&compacted);

        let result = CompactionResult {
            messages_removed: removed_count,
            tokens_before,
            tokens_after,
            summary: None,
            head_preserved: head.len(),
            tail_preserved: tail.len(),
            tool_pairs_preserved: tool_pairs,
        };

        (compacted, result)
    }

    /// Legacy compact_summarize that only keeps recent messages.
    ///
    /// This preserves system messages and keeps the last `keep_recent` messages,
    /// replacing everything in between with a summary.
    pub fn compact_summarize_simple(
        messages: &[Message],
        keep_recent: usize,
        summary_text: &str,
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
                    head_preserved: 0,
                    tail_preserved: messages.len(),
                    tool_pairs_preserved: 0,
                },
            );
        }

        // Preserve any leading system messages
        let system_end = messages.iter().position(|m| m.role != Role::System).unwrap_or(0);
        let split_point = messages.len() - keep_recent;

        // Ensure we don't split system messages from the tail start
        let actual_split = if split_point < system_end {
            system_end
        } else {
            split_point
        };

        let system_msgs = &messages[..system_end.min(actual_split)];
        let recent = &messages[actual_split..];

        let mut compacted = Vec::with_capacity(system_msgs.len() + 1 + recent.len());
        compacted.extend_from_slice(system_msgs);
        compacted.push(Message::system(summary_text));
        compacted.extend_from_slice(recent);

        let tokens_after = ContextMonitor::estimate_tokens(&compacted);
        let removed = actual_split - system_msgs.len();

        let result = CompactionResult {
            messages_removed: removed,
            tokens_before,
            tokens_after,
            summary: Some(summary_text.to_string()),
            head_preserved: system_msgs.len(),
            tail_preserved: recent.len(),
            tool_pairs_preserved: 0,
        };

        (compacted, result)
    }

    /// Legacy compact_truncate that only keeps recent messages.
    ///
    /// Drops oldest messages, keeping system messages and the last `keep_recent`
    /// messages.
    pub fn compact_truncate_simple(
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
                    head_preserved: 0,
                    tail_preserved: messages.len(),
                    tool_pairs_preserved: 0,
                },
            );
        }

        // Preserve system messages
        let system_end = messages.iter().position(|m| m.role != Role::System).unwrap_or(0);
        let split_point = messages.len() - keep_recent;
        let actual_split = if split_point < system_end {
            system_end
        } else {
            split_point
        };

        let system_msgs = &messages[..system_end.min(actual_split)];
        let recent = &messages[actual_split..];

        let mut compacted = Vec::with_capacity(system_msgs.len() + recent.len());
        compacted.extend_from_slice(system_msgs);
        compacted.extend_from_slice(recent);

        let tokens_after = ContextMonitor::estimate_tokens(&compacted);
        let removed = actual_split - system_msgs.len();

        let result = CompactionResult {
            messages_removed: removed,
            tokens_before,
            tokens_after,
            summary: None,
            head_preserved: system_msgs.len(),
            tail_preserved: recent.len(),
            tool_pairs_preserved: 0,
        };

        (compacted, result)
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

    /// Find the start index of the tail section, adjusting for tool pairs.
    ///
    /// Walks backward from the naive tail start position and adjusts
    /// if we would split a tool-use/result pair.
    fn find_tail_start(messages: &[Message], tail_keep: usize, head_end: usize) -> usize {
        if tail_keep == 0 || messages.len() <= head_end {
            return messages.len();
        }

        let naive_start = messages.len().saturating_sub(tail_keep);

        // Don't go before the head
        if naive_start <= head_end {
            return head_end;
        }

        // Check if the message at naive_start is a ToolResult that belongs
        // to a ToolUse before it — if so, expand tail to include the pair.
        let mut adjusted_start = naive_start;

        // Look backward to find any ToolUse that pairs with a ToolResult at the boundary
        if adjusted_start > 0 {
            // Collect tool_use_ids from ToolResult messages at and after the boundary
            let tool_result_ids: Vec<String> = messages[adjusted_start..]
                .iter()
                .flat_map(|msg| {
                    if msg.role == Role::Tool {
                        if let MessageContent::Blocks(blocks) = &msg.content {
                            blocks.iter().filter_map(|b| {
                                if let ContentBlock::ToolResult { tool_use_id, .. } = b {
                                    Some(tool_use_id.clone())
                                } else {
                                    None
                                }
                            }).collect()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    }
                })
                .collect();

            // Walk backward to include paired ToolUse messages
            while adjusted_start > head_end {
                let msg = &messages[adjusted_start - 1];
                if msg.role == Role::Assistant {
                    if let MessageContent::Blocks(blocks) = &msg.content {
                        let has_matching_tool_use = blocks.iter().any(|b| {
                            if let ContentBlock::ToolUse { id, .. } = b {
                                tool_result_ids.contains(id)
                            } else {
                                false
                            }
                        });
                        if has_matching_tool_use {
                            adjusted_start -= 1;
                            continue;
                        }
                    }
                }
                break;
            }
        }

        adjusted_start
    }

    /// Count tool-use pairs that are fully within a range of messages.
    fn count_tool_pairs_in_range(messages: &[Message], start: usize, end: usize) -> usize {
        let pairs = ContextMonitor::find_tool_pairs(messages);
        pairs.iter().filter(|(a, b)| *a >= start && *a < end && *b >= start && *b < end).count()
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
    use serde_json::json;

    /// Helper: create a simple conversation of `n` user/assistant turn pairs.
    fn make_conversation(n: usize) -> Vec<Message> {
        let mut messages = Vec::with_capacity(n * 2);
        for i in 0..n {
            messages.push(Message::user(format!("Question {}", i)));
            messages.push(Message::assistant(format!("Answer {}", i)));
        }
        messages
    }

    /// Helper: create a conversation with system message prefix.
    fn make_conversation_with_system(n: usize) -> Vec<Message> {
        let mut messages = vec![Message::system("You are a helpful assistant.")];
        for i in 0..n {
            messages.push(Message::user(format!("Question {}", i)));
            messages.push(Message::assistant(format!("Answer {}", i)));
        }
        messages
    }

    /// Helper: create a conversation with a tool-use pair.
    fn make_conversation_with_tool_pair() -> Vec<Message> {
        vec![
            Message::system("You are helpful."),
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
            Message::tool_result("tu_1", "File contents here", false),
            Message::assistant("The file contains..."),
        ]
    }

    // -- compact_summarize tests (head/tail) --

    #[test]
    fn test_summarize_preserves_head_and_tail() {
        let messages = make_conversation_with_system(5); // 11 messages total
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            2, // head: system + first 2 non-system
            2, // tail: last 2 messages
            "Summary of earlier conversation.",
        );

        // Head: system(1) + 2 non-system = 3, tail: 2, summary: 1 => 6
        assert_eq!(compacted.len(), 6);
        // compacted = [System, User(Q0), Assistant(A0), System(summary), User(Q4), Assistant(A4)]
        assert_eq!(compacted[0].role, Role::System); // original system msg
        assert_eq!(compacted[0].content.to_text(), "You are a helpful assistant.");
        assert_eq!(compacted[1].role, Role::User); // Q0
        assert_eq!(compacted[2].role, Role::Assistant); // A0
        assert_eq!(compacted[3].role, Role::System); // summary system msg
        assert_eq!(compacted[3].content.to_text(), "Summary of earlier conversation.");
        assert_eq!(compacted[4].role, Role::User); // Q4
        assert_eq!(compacted[5].role, Role::Assistant); // A4
        assert!(result.messages_removed > 0);
        assert_eq!(result.head_preserved, 3);
        assert_eq!(result.tail_preserved, 2);
    }

    #[test]
    fn test_summarize_preserves_system_messages() {
        let messages = make_conversation_with_system(3); // 7 messages
        let (compacted, _result) = ContextCompactor::compact_summarize(
            &messages,
            1, // head: system + 1 non-system
            2, // tail: last 2 messages
            "Summary.",
        );

        // compacted = [System, User(Q0), System(summary), User(Q2), Assistant(A2)]
        // First message should still be the system message
        assert_eq!(compacted[0].role, Role::System);
        assert_eq!(compacted[0].content.to_text(), "You are a helpful assistant.");
        // Second should be user message from head
        assert_eq!(compacted[1].role, Role::User);
        // Third should be summary
        assert_eq!(compacted[2].role, Role::System);
        assert!(compacted[2].content.to_text().contains("Summary"));
    }

    #[test]
    fn test_summarize_tool_pair_integrity() {
        let messages = make_conversation_with_tool_pair();
        let (compacted, _result) = ContextCompactor::compact_summarize(
            &messages,
            0, // head: only system
            2, // tail: last 2 messages
            "Summary.",
        );

        // Verify no ToolResult without its ToolUse
        let tool_result_ids: Vec<String> = compacted.iter()
            .flat_map(|msg| {
                if let MessageContent::Blocks(blocks) = &msg.content {
                    blocks.iter().filter_map(|b| {
                        if let ContentBlock::ToolResult { tool_use_id, .. } = b {
                            Some(tool_use_id.clone())
                        } else {
                            None
                        }
                    }).collect()
                } else {
                    Vec::new()
                }
            })
            .collect();

        let tool_use_ids: Vec<String> = compacted.iter()
            .flat_map(|msg| {
                if let MessageContent::Blocks(blocks) = &msg.content {
                    blocks.iter().filter_map(|b| {
                        if let ContentBlock::ToolUse { id, .. } = b {
                            Some(id.clone())
                        } else {
                            None
                        }
                    }).collect()
                } else {
                    Vec::new()
                }
            })
            .collect();

        // Every tool result should have a matching tool use in the compacted messages
        for tr_id in &tool_result_ids {
            assert!(tool_use_ids.contains(tr_id), "ToolResult {} has no matching ToolUse", tr_id);
        }
    }

    #[test]
    fn test_summarize_no_compaction_needed() {
        let messages = make_conversation(2); // 4 messages
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            10,
            10,
            "This summary should not be used.",
        );

        assert_eq!(compacted.len(), 4);
        assert_eq!(result.messages_removed, 0);
        assert!(result.summary.is_none());
        assert_eq!(result.tokens_before, result.tokens_after);
    }

    #[test]
    fn test_summarize_empty_messages() {
        let messages: Vec<Message> = vec![];
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            5,
            5,
            "Summary.",
        );

        assert!(compacted.is_empty());
        assert_eq!(result.messages_removed, 0);
        assert!(result.summary.is_none());
    }

    // -- compact_truncate tests (head/tail) --

    #[test]
    fn test_truncate_preserves_head_and_tail() {
        let messages = make_conversation_with_system(5); // 11 messages total
        let (compacted, result) = ContextCompactor::compact_truncate(
            &messages,
            2, // head: system + 2 non-system
            2, // tail: last 2 messages
        );

        // Head: 3 messages, Tail: 2 messages => 5
        assert_eq!(compacted.len(), 5);
        assert_eq!(compacted[0].role, Role::System);
        assert!(result.messages_removed > 0);
        assert_eq!(result.head_preserved, 3);
        assert_eq!(result.tail_preserved, 2);
    }

    #[test]
    fn test_truncate_tool_pair_integrity() {
        let messages = make_conversation_with_tool_pair();
        let (compacted, _result) = ContextCompactor::compact_truncate(
            &messages,
            0, // head: only system
            4, // tail: last 4 messages
        );

        // Check tool pairs stay together
        let tool_result_ids: Vec<String> = compacted.iter()
            .flat_map(|msg| {
                if let MessageContent::Blocks(blocks) = &msg.content {
                    blocks.iter().filter_map(|b| {
                        if let ContentBlock::ToolResult { tool_use_id, .. } = b {
                            Some(tool_use_id.clone())
                        } else {
                            None
                        }
                    }).collect()
                } else {
                    Vec::new()
                }
            })
            .collect();

        let tool_use_ids: Vec<String> = compacted.iter()
            .flat_map(|msg| {
                if let MessageContent::Blocks(blocks) = &msg.content {
                    blocks.iter().filter_map(|b| {
                        if let ContentBlock::ToolUse { id, .. } = b {
                            Some(id.clone())
                        } else {
                            None
                        }
                    }).collect()
                } else {
                    Vec::new()
                }
            })
            .collect();

        for tr_id in &tool_result_ids {
            assert!(tool_use_ids.contains(tr_id), "ToolResult {} has no matching ToolUse", tr_id);
        }
    }

    #[test]
    fn test_truncate_no_compaction_needed() {
        let messages = make_conversation(2); // 4 messages
        let (compacted, result) = ContextCompactor::compact_truncate(
            &messages,
            10,
            10,
        );

        assert_eq!(compacted.len(), 4);
        assert_eq!(result.messages_removed, 0);
        assert_eq!(result.tokens_before, result.tokens_after);
    }

    // -- Legacy simple compaction tests --

    #[test]
    fn test_summarize_simple_basic() {
        let messages = make_conversation(5); // 10 messages
        let (compacted, result) = ContextCompactor::compact_summarize_simple(
            &messages,
            4,
            "Summary of earlier conversation.",
        );

        assert_eq!(compacted.len(), 5); // 1 summary + 4 recent
        assert_eq!(result.messages_removed, 6);
        assert_eq!(result.summary, Some("Summary of earlier conversation.".to_string()));
        assert!(result.tokens_after < result.tokens_before);
        assert_eq!(compacted[0].role, Role::System);
    }

    #[test]
    fn test_truncate_simple_basic() {
        let messages = make_conversation(5); // 10 messages
        let (compacted, result) = ContextCompactor::compact_truncate_simple(&messages, 4);

        assert_eq!(compacted.len(), 4);
        assert_eq!(result.messages_removed, 6);
        assert!(result.summary.is_none());
        assert!(result.tokens_after < result.tokens_before);
        assert_eq!(compacted[0].content.to_text(), "Question 3");
        assert_eq!(compacted[3].content.to_text(), "Answer 4");
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
    }

    #[test]
    fn test_build_summary_prompt_empty() {
        let messages: Vec<Message> = vec![];
        let prompt = ContextCompactor::build_summary_prompt(&messages);
        assert!(prompt.contains("concise summary"));
        assert!(!prompt.contains("User:"));
    }

    // -- CompactionResult fields tests --

    #[test]
    fn test_compaction_result_fields() {
        let messages = make_conversation_with_system(10); // 21 messages total
        let (compacted, result) = ContextCompactor::compact_summarize(
            &messages,
            2,
            4,
            "Brief summary.",
        );

        // Verify result fields
        assert!(result.tokens_after < result.tokens_before);
        assert!(result.head_preserved > 0);
        assert!(result.tail_preserved > 0);
        assert!(result.summary.is_some());

        // Verify compacted starts with system message
        assert_eq!(compacted[0].role, Role::System);
    }
}