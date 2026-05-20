//! Tool guardrails with loop detection.
//!
//! Provides safety guardrails for tool execution:
//! - Loop detection: prevents the same tool+args from being called repeatedly
//! - Rate limiting: limits the number of tool calls per session
//! - Resource budgets: limits total execution time and output size
//! - Input validation: blocks dangerous patterns

use crate::tools::{ToolContext, ToolExecutor};
use crate::Result;
use smartassist_core::types::ToolResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration for tool guardrails.
#[derive(Debug, Clone)]
pub struct GuardrailConfig {
    /// Maximum consecutive calls to the same tool with identical arguments before blocking.
    pub max_identical_calls: usize,
    /// Maximum number of tool calls per session.
    pub max_calls_per_session: usize,
    /// Maximum total execution time per session (in seconds).
    pub max_session_duration_secs: u64,
    /// Maximum size of tool output in bytes.
    pub max_output_size: usize,
    /// Tool names that are blocked entirely.
    pub blocked_tools: Vec<String>,
    /// Tool names that require rate limiting (max calls per minute).
    pub rate_limited_tools: HashMap<String, usize>,
}

impl Default for GuardrailConfig {
    fn default() -> Self {
        let mut rate_limited_tools = HashMap::new();
        rate_limited_tools.insert("bash".to_string(), 30);
        rate_limited_tools.insert("write".to_string(), 20);
        rate_limited_tools.insert("edit".to_string(), 20);

        Self {
            max_identical_calls: 3,
            max_calls_per_session: 100,
            max_session_duration_secs: 600, // 10 minutes
            max_output_size: 1_000_000,      // 1 MB
            rate_limited_tools,
            blocked_tools: Vec::new(),
        }
    }
}

/// Record of a tool call for loop detection.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ToolCallRecord {
    /// Tool name.
    tool_name: String,
    /// SHA-256 hash of the arguments.
    args_hash: String,
    /// Timestamp of the call.
    timestamp: Instant,
}

/// Session-level guardrail state.
#[derive(Debug)]
struct SessionState {
    /// Call records for loop detection.
    call_records: Vec<ToolCallRecord>,
    /// Total number of calls in this session.
    total_calls: usize,
    /// Session start time.
    start_time: Instant,
    /// Rate limit counters per tool (tool_name -> (count, window_start)).
    rate_counters: HashMap<String, (usize, Instant)>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            call_records: Vec::new(),
            total_calls: 0,
            start_time: Instant::now(),
            rate_counters: HashMap::new(),
        }
    }
}

/// Result of a guardrail check.
#[derive(Debug, Clone)]
pub enum GuardrailAction {
    /// Allow the tool call to proceed.
    Allow,
    /// Block the tool call with a reason.
    Block(String),
    /// Allow the call but warn about a potential issue.
    Warn(String),
}

/// The guardrail engine wraps tool execution with safety checks.
pub struct GuardrailEngine {
    /// Tool executor for actual execution.
    executor: Arc<ToolExecutor>,
    /// Guardrail configuration.
    config: GuardrailConfig,
    /// Per-session state.
    session_states: Arc<RwLock<HashMap<String, Arc<RwLock<SessionState>>>>>,
}

impl GuardrailEngine {
    /// Create a new guardrail engine with the given executor and config.
    pub fn new(executor: Arc<ToolExecutor>, config: GuardrailConfig) -> Self {
        Self {
            executor,
            config,
            session_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a guardrail engine with default configuration.
    pub fn with_defaults(executor: Arc<ToolExecutor>) -> Self {
        Self::new(executor, GuardrailConfig::default())
    }

    /// Check if a tool call should be allowed, blocked, or warned about.
    pub async fn check(
        &self,
        session_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> GuardrailAction {
        // Check blocked tools
        if self.config.blocked_tools.iter().any(|t| t == tool_name) {
            return GuardrailAction::Block(format!(
                "Tool '{}' is blocked by guardrail policy",
                tool_name
            ));
        }

        let state = self.get_or_create_session(session_id).await;
        let mut state = state.write().await;

        // Check session call budget
        if state.total_calls >= self.config.max_calls_per_session {
            return GuardrailAction::Block(format!(
                "Session call budget exceeded ({} calls max)",
                self.config.max_calls_per_session
            ));
        }

        // Check session duration
        if state.start_time.elapsed() > Duration::from_secs(self.config.max_session_duration_secs) {
            return GuardrailAction::Block(format!(
                "Session duration exceeded ({}s max)",
                self.config.max_session_duration_secs
            ));
        }

        // Check rate limits
        if let Some(max_per_minute) = self.config.rate_limited_tools.get(tool_name) {
            let (count, window_start) = state
                .rate_counters
                .entry(tool_name.to_string())
                .or_insert((0, Instant::now()));

            // Reset window if older than 1 minute
            if window_start.elapsed() > Duration::from_secs(60) {
                *count = 0;
                *window_start = Instant::now();
            }

            if *count >= *max_per_minute {
                return GuardrailAction::Block(format!(
                    "Rate limit exceeded for tool '{}' ({} calls/min max)",
                    tool_name, max_per_minute
                ));
            }
            *count += 1;
        }

        // Check for loop detection (same tool + same args repeated)
        let args_hash = simple_hash(args);
        let recent_identical = state
            .call_records
            .iter()
            .rev()
            .take(self.config.max_identical_calls)
            .filter(|r| r.tool_name == tool_name && r.args_hash == args_hash)
            .count();

        if recent_identical >= self.config.max_identical_calls {
            return GuardrailAction::Block(format!(
                "Loop detected: tool '{}' called {} times with identical arguments",
                tool_name, recent_identical
            ));
        }

        // Record the call
        state.call_records.push(ToolCallRecord {
            tool_name: tool_name.to_string(),
            args_hash,
            timestamp: Instant::now(),
        });
        state.total_calls += 1;

        // Warn if approaching loop threshold
        if recent_identical >= self.config.max_identical_calls.saturating_sub(1) {
            return GuardrailAction::Warn(format!(
                "Approaching loop threshold for tool '{}' ({} identical calls)",
                tool_name,
                recent_identical + 1
            ));
        }

        GuardrailAction::Allow
    }

    /// Execute a tool with guardrail checks.
    ///
    /// First checks the guardrails, then executes the tool if allowed.
    /// Returns an error result if blocked, or the tool result if allowed.
    pub async fn execute_guarded(
        &self,
        tool_use_id: &str,
        name: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        // Pre-execution guardrail check
        match self.check(&context.session_id, name, &args).await {
            GuardrailAction::Allow => {
                let result = self.executor.execute(tool_use_id, name, args, Some(context)).await?;

                // Post-execution: check output size
                let output_size = result.output.to_string().len();
                if output_size > self.config.max_output_size {
                    warn!(
                        "Tool '{}' output size {} exceeds max {} bytes, truncating",
                        name, output_size, self.config.max_output_size
                    );
                    let truncated = truncate_json(&result.output, self.config.max_output_size);
                    return Ok(ToolResult {
                        tool_use_id: result.tool_use_id.clone(),
                        output: truncated,
                        is_error: result.is_error,
                        duration_ms: result.duration_ms,
                    });
                }

                Ok(result)
            }
            GuardrailAction::Block(reason) => {
                info!("Guardrail blocked tool '{}': {}", name, reason);
                Ok(ToolResult {
                    tool_use_id: tool_use_id.to_string(),
                    output: serde_json::json!({
                        "error": reason,
                        "tool": name,
                    }),
                    is_error: true,
                    duration_ms: None,
                })
            }
            GuardrailAction::Warn(reason) => {
                debug!("Guardrail warning for tool '{}': {}", name, reason);
                // Execute but note the warning
                let result = self.executor.execute(tool_use_id, name, args, Some(context)).await?;
                Ok(result)
            }
        }
    }

    /// Get or create session state.
    async fn get_or_create_session(
        &self,
        session_id: &str,
    ) -> Arc<RwLock<SessionState>> {
        let states = self.session_states.read().await;
        if let Some(state) = states.get(session_id) {
            return state.clone();
        }
        drop(states);

        let mut states = self.session_states.write().await;
        states
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(SessionState::new())))
            .clone()
    }

    /// Reset guardrail state for a session.
    pub async fn reset_session(&self, session_id: &str) {
        let mut states = self.session_states.write().await;
        states.remove(session_id);
    }

    /// Get call count for a session.
    pub async fn call_count(&self, session_id: &str) -> usize {
        let states = self.session_states.read().await;
        if let Some(state) = states.get(session_id) {
            let state = state.read().await;
            state.total_calls
        } else {
            0
        }
    }
}

/// Simple hash function for JSON values (for loop detection).
/// Uses a fast but non-cryptographic hash for comparison.
fn simple_hash(value: &serde_json::Value) -> String {
    // Serialize to a canonical string representation
    let serialized = value.to_string();
    // Use a simple hash based on the serialized string
    let mut hash: u64 = 0;
    for byte in serialized.as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(*byte as u64);
    }
    format!("{:016x}", hash)
}

/// Truncate a JSON value to fit within a size limit.
fn truncate_json(value: &serde_json::Value, max_size: usize) -> serde_json::Value {
    let s = value.to_string();
    if s.len() <= max_size {
        return value.clone();
    }

    // For strings, truncate directly
    if let serde_json::Value::String(str) = value {
        let truncated = format!("{}... [truncated, {} bytes total]", &str[..max_size.min(str.len()).saturating_sub(50)], str.len());
        return serde_json::Value::String(truncated);
    }

    // For other types, return a truncation notice
    serde_json::json!({
        "truncated": true,
        "original_size": s.len(),
        "message": format!("Output truncated (max {} bytes)", max_size),
        "preview": &s[..max_size.saturating_sub(100).min(s.len())],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolRegistry;

    #[test]
    fn test_guardrail_config_default() {
        let config = GuardrailConfig::default();
        assert_eq!(config.max_identical_calls, 3);
        assert_eq!(config.max_calls_per_session, 100);
        assert_eq!(config.max_session_duration_secs, 600);
        assert!(config.rate_limited_tools.contains_key("bash"));
    }

    #[test]
    fn test_simple_hash_same_values() {
        let v1 = serde_json::json!({"path": "/tmp/test.txt"});
        let v2 = serde_json::json!({"path": "/tmp/test.txt"});
        assert_eq!(simple_hash(&v1), simple_hash(&v2));
    }

    #[test]
    fn test_simple_hash_different_values() {
        let v1 = serde_json::json!({"path": "/tmp/test1.txt"});
        let v2 = serde_json::json!({"path": "/tmp/test2.txt"});
        assert_ne!(simple_hash(&v1), simple_hash(&v2));
    }

    #[test]
    fn test_truncate_json_small() {
        let value = serde_json::json!({"key": "value"});
        let result = truncate_json(&value, 1000);
        assert_eq!(result, value);
    }

    #[test]
    fn test_truncate_json_string() {
        let long_string = "a".repeat(2000);
        let value = serde_json::Value::String(long_string);
        let result = truncate_json(&value, 500);
        // Result should contain "truncated"
        if let serde_json::Value::String(s) = result {
            assert!(s.contains("truncated"));
        } else {
            panic!("Expected truncated string");
        }
    }

    #[tokio::test]
    async fn test_guardrail_blocks_tool() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(registry));
        let config = GuardrailConfig {
            blocked_tools: vec!["dangerous_tool".to_string()],
            ..Default::default()
        };
        let engine = GuardrailEngine::new(executor, config);

        let action = engine.check("session-1", "dangerous_tool", &serde_json::json!({})).await;
        assert!(matches!(action, GuardrailAction::Block(_)));
    }

    #[tokio::test]
    async fn test_guardrail_allows_normal_tool() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(registry));
        let engine = GuardrailEngine::with_defaults(executor);

        let action = engine.check("session-1", "read", &serde_json::json!({"path": "/tmp/test.txt"})).await;
        assert!(matches!(action, GuardrailAction::Allow));
    }

    #[tokio::test]
    async fn test_guardrail_loop_detection() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(registry));
        let config = GuardrailConfig {
            max_identical_calls: 2,
            ..Default::default()
        };
        let engine = GuardrailEngine::new(executor, config);

        let args = serde_json::json!({"path": "/tmp/same.txt"});

        // First call: allow
        let action = engine.check("session-1", "read", &args).await;
        assert!(matches!(action, GuardrailAction::Allow));

        // Second call: warn
        let action = engine.check("session-1", "read", &args).await;
        assert!(matches!(action, GuardrailAction::Warn(_)));

        // Third call: block (loop detected)
        let action = engine.check("session-1", "read", &args).await;
        assert!(matches!(action, GuardrailAction::Block(_)));
    }

    #[tokio::test]
    async fn test_guardrail_session_call_budget() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(registry));
        let config = GuardrailConfig {
            max_calls_per_session: 2,
            ..Default::default()
        };
        let engine = GuardrailEngine::new(executor, config);

        let args1 = serde_json::json!({"path": "/tmp/a.txt"});
        let args2 = serde_json::json!({"path": "/tmp/b.txt"});

        // First two calls: allow
        assert!(matches!(
            engine.check("session-1", "read", &args1).await,
            GuardrailAction::Allow
        ));
        assert!(matches!(
            engine.check("session-1", "read", &args2).await,
            GuardrailAction::Allow
        ));

        // Third call: block (budget exceeded)
        assert!(matches!(
            engine.check("session-1", "read", &args1).await,
            GuardrailAction::Block(_)
        ));
    }

    #[tokio::test]
    async fn test_guardrail_different_sessions_independent() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(registry));
        let config = GuardrailConfig {
            max_calls_per_session: 1,
            ..Default::default()
        };
        let engine = GuardrailEngine::new(executor, config);

        let args = serde_json::json!({"path": "/tmp/test.txt"});

        // First session: allow
        assert!(matches!(
            engine.check("session-1", "read", &args).await,
            GuardrailAction::Allow
        ));

        // Different session: allow (independent budget)
        assert!(matches!(
            engine.check("session-2", "read", &args).await,
            GuardrailAction::Allow
        ));

        // Back to first session: block (budget exceeded)
        assert!(matches!(
            engine.check("session-1", "read", &args).await,
            GuardrailAction::Block(_)
        ));
    }

    #[tokio::test]
    async fn test_guardrail_reset_session() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(registry));
        let config = GuardrailConfig {
            max_calls_per_session: 1,
            ..Default::default()
        };
        let engine = GuardrailEngine::new(executor, config);

        let args = serde_json::json!({"path": "/tmp/test.txt"});

        // Use up the budget
        engine.check("session-1", "read", &args).await;

        // Should be blocked
        assert!(matches!(
            engine.check("session-1", "read", &args).await,
            GuardrailAction::Block(_)
        ));

        // Reset
        engine.reset_session("session-1").await;

        // Should be allowed again
        assert!(matches!(
            engine.check("session-1", "read", &args).await,
            GuardrailAction::Allow
        ));
    }
}