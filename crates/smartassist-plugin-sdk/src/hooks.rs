//! Hook system for plugin extension points.
//!
//! Hooks allow plugins to intercept and modify behavior at various points
//! in the SmartAssist lifecycle.

use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Types of hooks available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookType {
    /// Before a message is sent.
    PreSend,
    /// After a message is sent.
    PostSend,
    /// Before a message is received/processed.
    PreReceive,
    /// After a message is received/processed.
    PostReceive,
    /// Before a tool is executed.
    PreToolExecution,
    /// After a tool is executed.
    PostToolExecution,
    /// Before a model request.
    PreModelRequest,
    /// After a model response.
    PostModelResponse,
    /// On session start.
    SessionStart,
    /// On session end.
    SessionEnd,
    /// On agent start.
    AgentStart,
    /// On agent stop.
    AgentStop,
    /// On error.
    OnError,
    /// Custom hook type.
    Custom,
}

/// Priority for hook execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum HookPriority {
    /// Highest priority - runs first.
    Highest = 0,
    /// High priority.
    High = 25,
    /// Normal priority (default).
    Normal = 50,
    /// Low priority.
    Low = 75,
    /// Lowest priority - runs last.
    Lowest = 100,
}

impl Default for HookPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Context passed to hooks.
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Hook type being invoked.
    pub hook_type: HookType,

    /// Session ID (if applicable).
    pub session_id: Option<String>,

    /// Agent ID (if applicable).
    pub agent_id: Option<String>,

    /// Channel type (if applicable).
    pub channel_type: Option<String>,

    /// Payload data.
    pub data: HashMap<String, serde_json::Value>,

    /// Metadata.
    pub metadata: HashMap<String, String>,
}

impl HookContext {
    /// Create a new hook context.
    pub fn new(hook_type: HookType) -> Self {
        Self {
            hook_type,
            session_id: None,
            agent_id: None,
            channel_type: None,
            data: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set the agent ID.
    pub fn with_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Set the channel type.
    pub fn with_channel(mut self, channel_type: impl Into<String>) -> Self {
        self.channel_type = Some(channel_type.into());
        self
    }

    /// Add data to the context.
    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    /// Get data from the context.
    pub fn get_data<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

/// Result of hook execution.
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Continue processing with potentially modified data.
    Continue(Option<HashMap<String, serde_json::Value>>),
    /// Skip further processing.
    Skip,
    /// Abort with error.
    Abort(String),
}

impl HookResult {
    /// Create a continue result with no modifications.
    pub fn ok() -> Self {
        Self::Continue(None)
    }

    /// Create a continue result with modified data.
    pub fn modified(data: HashMap<String, serde_json::Value>) -> Self {
        Self::Continue(Some(data))
    }

    /// Create a skip result.
    pub fn skip() -> Self {
        Self::Skip
    }

    /// Create an abort result.
    pub fn abort(message: impl Into<String>) -> Self {
        Self::Abort(message.into())
    }

    /// Check if this is a continue result.
    pub fn is_continue(&self) -> bool {
        matches!(self, Self::Continue(_))
    }

    /// Check if this is a skip result.
    pub fn is_skip(&self) -> bool {
        matches!(self, Self::Skip)
    }

    /// Check if this is an abort result.
    pub fn is_abort(&self) -> bool {
        matches!(self, Self::Abort(_))
    }
}

/// Hook metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMetadata {
    /// Hook name.
    pub name: String,

    /// Hook type.
    pub hook_type: HookType,

    /// Hook priority.
    pub priority: HookPriority,

    /// Description.
    pub description: Option<String>,
}

/// Trait for hook implementations.
#[async_trait]
pub trait Hook: Send + Sync {
    /// Get hook metadata.
    fn metadata(&self) -> HookMetadata;

    /// Execute the hook.
    async fn execute(&self, ctx: &HookContext) -> Result<HookResult>;

    /// Check if the hook should run for the given context.
    fn should_run(&self, ctx: &HookContext) -> bool {
        ctx.hook_type == self.metadata().hook_type
    }
}

/// Registry for managing hooks.
pub struct HookRegistry {
    hooks: HashMap<HookType, Vec<Arc<dyn Hook>>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Create a new hook registry.
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Register a hook.
    pub fn register(&mut self, hook: Arc<dyn Hook>) {
        let hook_type = hook.metadata().hook_type;
        let hooks = self.hooks.entry(hook_type).or_default();
        hooks.push(hook);

        // Sort by priority
        hooks.sort_by(|a, b| a.metadata().priority.cmp(&b.metadata().priority));
    }

    /// Unregister a hook by name.
    pub fn unregister(&mut self, name: &str) {
        for hooks in self.hooks.values_mut() {
            hooks.retain(|h| h.metadata().name != name);
        }
    }

    /// Get hooks for a type.
    pub fn get(&self, hook_type: HookType) -> Vec<Arc<dyn Hook>> {
        self.hooks.get(&hook_type).cloned().unwrap_or_default()
    }

    /// Execute all hooks of a type.
    pub async fn execute(&self, ctx: &HookContext) -> Result<HookResult> {
        let hooks = self.get(ctx.hook_type);
        let mut current_data = ctx.data.clone();

        for hook in hooks {
            if !hook.should_run(ctx) {
                continue;
            }

            let mut hook_ctx = ctx.clone();
            hook_ctx.data = current_data.clone();

            match hook.execute(&hook_ctx).await? {
                HookResult::Continue(Some(new_data)) => {
                    // Merge modified data
                    for (k, v) in new_data {
                        current_data.insert(k, v);
                    }
                }
                HookResult::Continue(None) => {
                    // Continue without modification
                }
                HookResult::Skip => {
                    return Ok(HookResult::Skip);
                }
                HookResult::Abort(msg) => {
                    return Ok(HookResult::Abort(msg));
                }
            }
        }

        if current_data != ctx.data {
            Ok(HookResult::Continue(Some(current_data)))
        } else {
            Ok(HookResult::Continue(None))
        }
    }

    /// List all registered hooks.
    pub fn list(&self) -> Vec<HookMetadata> {
        self.hooks
            .values()
            .flat_map(|hooks| hooks.iter().map(|h| h.metadata()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHook {
        name: String,
    }

    #[async_trait]
    impl Hook for TestHook {
        fn metadata(&self) -> HookMetadata {
            HookMetadata {
                name: self.name.clone(),
                hook_type: HookType::PreSend,
                priority: HookPriority::Normal,
                description: None,
            }
        }

        async fn execute(&self, _ctx: &HookContext) -> Result<HookResult> {
            Ok(HookResult::ok())
        }
    }

    #[test]
    fn test_hook_context() {
        let ctx = HookContext::new(HookType::PreSend)
            .with_session("sess123")
            .with_agent("agent456")
            .with_data("count", serde_json::json!(42));

        assert_eq!(ctx.hook_type, HookType::PreSend);
        assert_eq!(ctx.session_id, Some("sess123".to_string()));
        assert_eq!(ctx.agent_id, Some("agent456".to_string()));
        assert_eq!(ctx.get_data::<i32>("count"), Some(42));
    }

    #[test]
    fn test_hook_result() {
        assert!(HookResult::ok().is_continue());
        assert!(HookResult::skip().is_skip());
        assert!(HookResult::abort("error").is_abort());
    }

    #[test]
    fn test_hook_registry() {
        let mut registry = HookRegistry::new();

        let hook = Arc::new(TestHook {
            name: "test-hook".to_string(),
        });

        registry.register(hook);

        let hooks = registry.get(HookType::PreSend);
        assert_eq!(hooks.len(), 1);

        let all_hooks = registry.list();
        assert_eq!(all_hooks.len(), 1);
        assert_eq!(all_hooks[0].name, "test-hook");

        registry.unregister("test-hook");
        assert!(registry.get(HookType::PreSend).is_empty());
    }

    #[tokio::test]
    async fn test_hook_execution() {
        let mut registry = HookRegistry::new();

        let hook = Arc::new(TestHook {
            name: "test-hook".to_string(),
        });

        registry.register(hook);

        let ctx = HookContext::new(HookType::PreSend);
        let result = registry.execute(&ctx).await.unwrap();

        assert!(result.is_continue());
    }
}
