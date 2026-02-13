//! Hello World Plugin for SmartAssist
//!
//! This example demonstrates how to create a simple plugin that:
//! - Implements the Plugin trait
//! - Provides a custom tool
//! - Uses the plugin lifecycle

use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use smartassist_plugin_sdk::prelude::*;
use std::any::Any;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

/// The Hello World plugin.
pub struct HelloPlugin {
    /// Plugin state.
    state: PluginState,

    /// Greeting message.
    greeting: String,

    /// Count of greetings sent.
    greeting_count: u32,
}

impl Default for HelloPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl HelloPlugin {
    /// Create a new Hello plugin.
    pub fn new() -> Self {
        Self {
            state: PluginState::Loaded,
            greeting: "Hello from SmartAssist!".to_string(),
            greeting_count: 0,
        }
    }

    /// Set a custom greeting message.
    pub fn with_greeting(mut self, greeting: impl Into<String>) -> Self {
        self.greeting = greeting.into();
        self
    }
}

#[async_trait]
impl Plugin for HelloPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "hello-plugin".to_string(),
            version: Version::parse("0.1.0").unwrap(),
            description: "A simple hello world plugin for SmartAssist".to_string(),
            author: Some("SmartAssist Contributors".to_string()),
            homepage: Some("https://github.com/smartassist/smartassist".to_string()),
            license: Some("MIT".to_string()),
            capabilities: vec![PluginCapability::Tool],
            min_smartassist_version: Some(Version::parse("0.1.0").unwrap()),
        }
    }

    async fn initialize(&mut self, ctx: &PluginContext) -> Result<()> {
        self.state = PluginState::Initializing;

        // Read custom greeting from config if provided
        if let Some(greeting) = ctx.get_option::<String>("greeting") {
            self.greeting = greeting;
        }

        info!(
            "Hello plugin initialized with greeting: '{}'",
            self.greeting
        );

        self.state = PluginState::Ready;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.state = PluginState::ShuttingDown;

        info!(
            "Hello plugin shutting down. Total greetings: {}",
            self.greeting_count
        );

        self.state = PluginState::Stopped;
        Ok(())
    }

    fn state(&self) -> PluginState {
        self.state
    }

    async fn health_check(&self) -> Result<PluginHealth> {
        Ok(PluginHealth::healthy()
            .with_metric("greeting_count", serde_json::json!(self.greeting_count)))
    }

    fn as_tool_plugin(&self) -> Option<&dyn ToolPlugin> {
        Some(self)
    }

    fn as_tool_plugin_mut(&mut self) -> Option<&mut dyn ToolPlugin> {
        Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[async_trait]
impl ToolPlugin for HelloPlugin {
    fn tools(&self) -> Vec<Arc<dyn PluginTool>> {
        vec![Arc::new(HelloTool {
            greeting: self.greeting.clone(),
        })]
    }

    fn factory(&self) -> &dyn ToolPluginFactory {
        self
    }
}

#[async_trait]
impl ToolPluginFactory for HelloPlugin {
    async fn create_tools(&self, _ctx: &PluginContext) -> Result<Vec<Arc<dyn PluginTool>>> {
        Ok(self.tools())
    }

    fn tool_group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// A simple hello world tool.
pub struct HelloTool {
    greeting: String,
}

#[async_trait]
impl PluginTool for HelloTool {
    fn name(&self) -> &str {
        "hello"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "hello".to_string(),
            description: "Send a friendly greeting".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name to greet (optional)"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolExecutionContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("World");

        let message = format!("{} {name}!", self.greeting);

        info!("Greeting sent: {}", message);

        let duration = start.elapsed();
        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "message": message,
                "name": name,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

// Export the plugin using the macro
smartassist_plugin!(HelloPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata() {
        let plugin = HelloPlugin::new();
        let metadata = plugin.metadata();

        assert_eq!(metadata.name, "hello-plugin");
        assert_eq!(metadata.version.to_string(), "0.1.0");
        assert!(metadata.capabilities.contains(&PluginCapability::Tool));
    }

    #[test]
    fn test_custom_greeting() {
        let plugin = HelloPlugin::new().with_greeting("Howdy");
        assert_eq!(plugin.greeting, "Howdy");
    }

    #[tokio::test]
    async fn test_plugin_lifecycle() {
        let mut plugin = HelloPlugin::new();

        assert_eq!(plugin.state(), PluginState::Loaded);

        let ctx = PluginContext::new(
            PluginConfig::default(),
            Version::parse("0.1.0").unwrap(),
            std::path::PathBuf::from("/tmp/test-plugin"),
        );

        plugin.initialize(&ctx).await.unwrap();
        assert_eq!(plugin.state(), PluginState::Ready);

        plugin.shutdown().await.unwrap();
        assert_eq!(plugin.state(), PluginState::Stopped);
    }

    #[tokio::test]
    async fn test_hello_tool() {
        let tool = HelloTool {
            greeting: "Hello".to_string(),
        };

        let args = serde_json::json!({
            "name": "SmartAssist"
        });

        let ctx = ToolExecutionContext::default();
        let result = tool.execute("test-id", args, &ctx).await.unwrap();

        assert!(!result.is_error);

        let output: serde_json::Value = result.output;
        assert_eq!(output["message"], "Hello SmartAssist!");
        assert_eq!(output["name"], "SmartAssist");
    }

    #[tokio::test]
    async fn test_hello_tool_default_name() {
        let tool = HelloTool {
            greeting: "Hi".to_string(),
        };

        let args = serde_json::json!({});

        let ctx = ToolExecutionContext::default();
        let result = tool.execute("test-id", args, &ctx).await.unwrap();

        assert!(!result.is_error);

        let output: serde_json::Value = result.output;
        assert_eq!(output["message"], "Hi World!");
    }

    #[test]
    fn test_tool_definition() {
        let tool = HelloTool {
            greeting: "Hello".to_string(),
        };

        let def = tool.definition();
        assert_eq!(def.name, "hello");
        assert!(def.description.contains("greeting"));
    }

    #[tokio::test]
    async fn test_health_check() {
        let plugin = HelloPlugin::new();
        let health = plugin.health_check().await.unwrap();

        assert!(health.healthy);
        assert!(health.metrics.contains_key("greeting_count"));
    }
}
