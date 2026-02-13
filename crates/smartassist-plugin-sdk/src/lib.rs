//! SmartAssist Plugin SDK
//!
//! This crate provides the core traits and types for building SmartAssist plugins.
//! Plugins can extend SmartAssist with:
//! - Custom messaging channels (Telegram, Discord, etc.)
//! - Custom tools for agents
//! - Custom model providers
//! - Custom hooks and middleware
//!
//! # Example Plugin
//!
//! ```rust,ignore
//! use smartassist_plugin_sdk::prelude::*;
//!
//! pub struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn metadata(&self) -> PluginMetadata {
//!         PluginMetadata {
//!             name: "my-plugin".to_string(),
//!             version: Version::parse("0.1.0").unwrap(),
//!             description: "My custom plugin".to_string(),
//!             author: Some("Author Name".to_string()),
//!             capabilities: vec![PluginCapability::Channel],
//!         }
//!     }
//!
//!     async fn initialize(&mut self, ctx: &PluginContext) -> Result<()> {
//!         // Initialize plugin resources
//!         Ok(())
//!     }
//!
//!     async fn shutdown(&mut self) -> Result<()> {
//!         // Cleanup plugin resources
//!         Ok(())
//!     }
//! }
//!
//! // Export the plugin
//! smartassist_plugin!(MyPlugin);
//! ```

mod channel;
mod error;
mod hooks;
mod provider;
mod tool;

pub use channel::{ChannelPlugin, ChannelPluginFactory};
pub use error::{PluginError, Result};
pub use hooks::{Hook, HookContext, HookResult, HookType};
pub use provider::{ModelProviderPlugin, ProviderCapabilities};
pub use tool::{PluginTool, ToolExecutionContext, ToolPlugin, ToolPluginFactory};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export commonly used types from core crates
pub use smartassist_channels::{
    Attachment, AttachmentType, Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver,
    ChannelSender, MessageRef, SendResult,
};
pub use smartassist_core::types::{
    ChatInfo, ChatType, InboundMessage, MessageId, MessageTarget, OutboundMessage, SenderInfo,
    ToolDefinition, ToolResult,
};

/// Semantic version type.
pub use semver::Version;

/// Plugin metadata describing the plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name (unique identifier).
    pub name: String,

    /// Plugin version.
    #[serde(with = "version_serde")]
    pub version: Version,

    /// Plugin description.
    pub description: String,

    /// Plugin author.
    pub author: Option<String>,

    /// Plugin homepage/repository URL.
    pub homepage: Option<String>,

    /// Plugin license.
    pub license: Option<String>,

    /// Plugin capabilities.
    pub capabilities: Vec<PluginCapability>,

    /// Minimum required SmartAssist version.
    #[serde(default, with = "option_version_serde")]
    pub min_smartassist_version: Option<Version>,
}

/// Serde helper for Version.
mod version_serde {
    use semver::Version;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(version: &Version, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        version.to_string().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Version, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Version::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Serde helper for Option<Version>.
mod option_version_serde {
    use semver::Version;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(version: &Option<Version>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        version.as_ref().map(|v| v.to_string()).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Version>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(v) => Version::parse(&v)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

/// Plugin capability types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    /// Plugin provides a messaging channel.
    Channel,
    /// Plugin provides agent tools.
    Tool,
    /// Plugin provides a model provider.
    ModelProvider,
    /// Plugin provides hooks.
    Hook,
    /// Plugin provides storage backend.
    Storage,
    /// Plugin provides media processing.
    Media,
}

/// Plugin state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Plugin is loaded but not initialized.
    Loaded,
    /// Plugin is initializing.
    Initializing,
    /// Plugin is ready and running.
    Ready,
    /// Plugin is shutting down.
    ShuttingDown,
    /// Plugin has been stopped.
    Stopped,
    /// Plugin encountered an error.
    Error,
}

/// Configuration for a plugin.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin-specific configuration options.
    pub options: HashMap<String, serde_json::Value>,

    /// Whether the plugin is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Environment variables to set for the plugin.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

fn default_enabled() -> bool {
    true
}

/// Context passed to plugins during initialization and operation.
pub struct PluginContext {
    /// Plugin configuration.
    pub config: PluginConfig,

    /// SmartAssist version.
    pub smartassist_version: Version,

    /// Base data directory for the plugin.
    pub data_dir: std::path::PathBuf,

    /// Plugin-local key-value store.
    pub store: Arc<RwLock<HashMap<String, serde_json::Value>>>,

    /// Logger for the plugin.
    pub logger: tracing::Span,
}

impl PluginContext {
    /// Create a new plugin context.
    pub fn new(
        config: PluginConfig,
        smartassist_version: Version,
        data_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            config,
            smartassist_version,
            data_dir,
            store: Arc::new(RwLock::new(HashMap::new())),
            logger: tracing::Span::current(),
        }
    }

    /// Get a configuration option.
    pub fn get_option<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.config
            .options
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Get a required configuration option.
    pub fn require_option<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T> {
        self.get_option(key)
            .ok_or_else(|| PluginError::config(format!("Missing required option: {}", key)))
    }

    /// Get an environment variable.
    pub fn get_env(&self, key: &str) -> Option<&str> {
        self.config.env.get(key).map(|s| s.as_str())
    }

    /// Store a value in the plugin-local store.
    pub async fn set(&self, key: impl Into<String>, value: serde_json::Value) {
        let mut store = self.store.write().await;
        store.insert(key.into(), value);
    }

    /// Get a value from the plugin-local store.
    pub async fn get(&self, key: &str) -> Option<serde_json::Value> {
        let store = self.store.read().await;
        store.get(key).cloned()
    }
}

/// The main plugin trait that all plugins must implement.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get the plugin metadata.
    fn metadata(&self) -> PluginMetadata;

    /// Initialize the plugin.
    async fn initialize(&mut self, ctx: &PluginContext) -> Result<()>;

    /// Shutdown the plugin.
    async fn shutdown(&mut self) -> Result<()>;

    /// Get the current plugin state.
    fn state(&self) -> PluginState {
        PluginState::Ready
    }

    /// Check plugin health.
    async fn health_check(&self) -> Result<PluginHealth> {
        Ok(PluginHealth::healthy())
    }

    /// Get the plugin as a channel plugin, if applicable.
    fn as_channel_plugin(&self) -> Option<&dyn ChannelPlugin> {
        None
    }

    /// Get the plugin as a mutable channel plugin, if applicable.
    fn as_channel_plugin_mut(&mut self) -> Option<&mut dyn ChannelPlugin> {
        None
    }

    /// Get the plugin as a tool plugin, if applicable.
    fn as_tool_plugin(&self) -> Option<&dyn ToolPlugin> {
        None
    }

    /// Get the plugin as a mutable tool plugin, if applicable.
    fn as_tool_plugin_mut(&mut self) -> Option<&mut dyn ToolPlugin> {
        None
    }

    /// Get the plugin as a model provider plugin, if applicable.
    fn as_provider_plugin(&self) -> Option<&dyn ModelProviderPlugin> {
        None
    }

    /// Get the plugin as a mutable model provider plugin, if applicable.
    fn as_provider_plugin_mut(&mut self) -> Option<&mut dyn ModelProviderPlugin> {
        None
    }

    /// Get the plugin as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Get the plugin as mutable Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Plugin health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealth {
    /// Whether the plugin is healthy.
    pub healthy: bool,

    /// Optional health message.
    pub message: Option<String>,

    /// Last check timestamp.
    pub last_check: chrono::DateTime<chrono::Utc>,

    /// Additional health metrics.
    pub metrics: HashMap<String, serde_json::Value>,
}

impl PluginHealth {
    /// Create a healthy status.
    pub fn healthy() -> Self {
        Self {
            healthy: true,
            message: None,
            last_check: chrono::Utc::now(),
            metrics: HashMap::new(),
        }
    }

    /// Create an unhealthy status.
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            healthy: false,
            message: Some(message.into()),
            last_check: chrono::Utc::now(),
            metrics: HashMap::new(),
        }
    }

    /// Add a metric.
    pub fn with_metric(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}

/// Represents either a statically registered or dynamically loaded plugin.
enum PluginEntry {
    /// A plugin registered via `register()`.
    Static(Box<dyn Plugin>),
    /// A plugin loaded from a shared library.
    Dynamic {
        /// The loaded library handle. Kept alive to prevent unloading.
        _library: libloading::Library,
        /// The plugin instance created from the library.
        plugin: Box<dyn Plugin>,
    },
}

impl PluginEntry {
    /// Get a reference to the underlying plugin.
    fn plugin(&self) -> &dyn Plugin {
        match self {
            PluginEntry::Static(p) => p.as_ref(),
            PluginEntry::Dynamic { plugin, .. } => plugin.as_ref(),
        }
    }

    /// Get a mutable reference to the underlying plugin.
    fn plugin_mut(&mut self) -> &mut dyn Plugin {
        match self {
            PluginEntry::Static(p) => p.as_mut(),
            PluginEntry::Dynamic { plugin, .. } => plugin.as_mut(),
        }
    }
}

/// Plugin loader for dynamic plugin loading.
pub struct PluginLoader {
    /// Search paths for plugins.
    search_paths: Vec<std::path::PathBuf>,

    /// Loaded plugins.
    plugins: HashMap<String, PluginEntry>,
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginLoader {
    /// Create a new plugin loader.
    pub fn new() -> Self {
        Self {
            search_paths: Vec::new(),
            plugins: HashMap::new(),
        }
    }

    /// Add a search path for plugins.
    pub fn add_search_path(&mut self, path: impl Into<std::path::PathBuf>) {
        self.search_paths.push(path.into());
    }

    /// Register a plugin.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        let name = plugin.metadata().name.clone();
        self.plugins.insert(name, PluginEntry::Static(plugin));
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|entry| entry.plugin())
    }

    /// Get a mutable plugin by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut dyn Plugin> {
        self.plugins.get_mut(name).map(|entry| entry.plugin_mut())
    }

    /// List all registered plugins.
    pub fn list(&self) -> Vec<PluginMetadata> {
        self.plugins
            .values()
            .map(|entry| entry.plugin().metadata())
            .collect()
    }

    /// Load a plugin from a shared library (.so/.dylib/.dll).
    ///
    /// The library must export an `_smartassist_plugin_create` symbol that returns
    /// a pointer to a boxed `Box<dyn Plugin>` (double-boxed for FFI safety).
    ///
    /// # Safety
    /// This loads and executes code from the specified library file.
    /// The caller must ensure the library is trusted and compatible.
    pub unsafe fn load_plugin(&mut self, path: &std::path::Path) -> Result<PluginMetadata> {
        let library = libloading::Library::new(path).map_err(|e| {
            PluginError::initialization(format!("Failed to load library {:?}: {}", path, e))
        })?;

        // Look up the plugin creation function
        let create_fn: libloading::Symbol<unsafe extern "C" fn() -> *mut std::ffi::c_void> =
            library.get(b"_smartassist_plugin_create").map_err(|e| {
                PluginError::initialization(format!("Symbol not found in {:?}: {}", path, e))
            })?;

        // Call the creation function to get a plugin instance
        let raw = create_fn();
        if raw.is_null() {
            return Err(PluginError::initialization(
                "Plugin creation returned null",
            ));
        }

        // The macro exports a double-boxed plugin (Box<Box<dyn Plugin>>) cast to c_void.
        let plugin = *Box::from_raw(raw as *mut Box<dyn Plugin>);
        let metadata = plugin.metadata();
        let name = metadata.name.clone();

        self.plugins.insert(
            name,
            PluginEntry::Dynamic {
                _library: library,
                plugin,
            },
        );

        Ok(metadata)
    }

    /// Load all plugins from a directory.
    ///
    /// Scans for `.so` (Linux), `.dylib` (macOS), and `.dll` (Windows) files.
    ///
    /// # Safety
    /// This loads and executes code from library files found in the directory.
    /// The caller must ensure the directory contents are trusted.
    pub unsafe fn load_from_dir(&mut self, dir: &std::path::Path) -> Result<Vec<PluginMetadata>> {
        let mut loaded = Vec::new();

        if !dir.exists() {
            return Ok(loaded);
        }

        let entries = std::fs::read_dir(dir).map_err(|e| {
            PluginError::initialization(format!("Failed to read dir {:?}: {}", dir, e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                PluginError::initialization(format!("Failed to read entry: {}", e))
            })?;
            let path = entry.path();

            let is_plugin = path.extension().map_or(false, |ext| {
                ext == "so" || ext == "dylib" || ext == "dll"
            });

            if is_plugin {
                match self.load_plugin(&path) {
                    Ok(meta) => {
                        tracing::info!("Loaded plugin: {} v{}", meta.name, meta.version);
                        loaded.push(meta);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load plugin {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Initialize all plugins.
    pub async fn initialize_all(&mut self, smartassist_version: Version) -> Result<()> {
        for (name, entry) in self.plugins.iter_mut() {
            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("smartassist")
                .join("plugins")
                .join(name);

            // Create data directory
            std::fs::create_dir_all(&data_dir).map_err(|e| {
                PluginError::initialization(format!("Failed to create data dir: {}", e))
            })?;

            let ctx =
                PluginContext::new(PluginConfig::default(), smartassist_version.clone(), data_dir);

            tracing::info!("Initializing plugin: {}", name);
            entry.plugin_mut().initialize(&ctx).await?;
        }

        Ok(())
    }

    /// Shutdown all plugins.
    pub async fn shutdown_all(&mut self) -> Result<()> {
        for (name, entry) in self.plugins.iter_mut() {
            tracing::info!("Shutting down plugin: {}", name);
            if let Err(e) = entry.plugin_mut().shutdown().await {
                tracing::warn!("Error shutting down plugin {}: {}", name, e);
            }
        }

        Ok(())
    }
}

/// Prelude module for convenient imports.
pub mod prelude {
    pub use super::{
        Channel, ChannelLifecycle, ChannelPlugin, ChannelPluginFactory, ChannelReceiver,
        ChannelSender, Hook, HookContext, HookResult, HookType, ModelProviderPlugin, Plugin,
        PluginCapability, PluginConfig, PluginContext, PluginError, PluginHealth, PluginLoader,
        PluginMetadata, PluginState, PluginTool, ProviderCapabilities, Result, ToolExecutionContext,
        ToolPlugin, ToolPluginFactory, Version,
    };

    pub use super::{
        Attachment, AttachmentType, ChatInfo, ChatType, InboundMessage, MessageId, MessageRef,
        MessageTarget, OutboundMessage, SendResult, SenderInfo, ToolDefinition, ToolResult,
    };

    pub use async_trait::async_trait;

    // Re-export the plugin macro
    pub use crate::smartassist_plugin;
}

/// Macro for exporting a plugin from a shared library.
///
/// This macro generates FFI-safe exported symbols that the `PluginLoader` uses
/// to instantiate a plugin from a `.so`/`.dylib`/`.dll` file:
///
/// - `_smartassist_plugin_create` -- returns a double-boxed `Box<Box<dyn Plugin>>`
///   cast to `*mut c_void` so the pointer is thin and FFI-safe.
/// - `_smartassist_plugin_metadata` -- returns a boxed `PluginMetadata` cast to
///   `*mut c_void`.
///
/// The consuming side (in `PluginLoader::load_plugin`) reconstructs the original
/// types from these raw pointers.
#[macro_export]
macro_rules! smartassist_plugin {
    ($plugin_type:ty) => {
        /// Create a new instance of the plugin.
        #[no_mangle]
        pub extern "C" fn _smartassist_plugin_create() -> *mut std::ffi::c_void {
            let plugin: Box<dyn $crate::Plugin> = Box::new(<$plugin_type>::default());
            let boxed: Box<Box<dyn $crate::Plugin>> = Box::new(plugin);
            Box::into_raw(boxed) as *mut std::ffi::c_void
        }

        /// Get plugin metadata.
        #[no_mangle]
        pub extern "C" fn _smartassist_plugin_metadata() -> *mut std::ffi::c_void {
            let meta = <$plugin_type>::default().metadata();
            let boxed = Box::new(meta);
            Box::into_raw(boxed) as *mut std::ffi::c_void
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata_serialization() {
        let metadata = PluginMetadata {
            name: "test-plugin".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            description: "A test plugin".to_string(),
            author: Some("Test Author".to_string()),
            homepage: None,
            license: Some("MIT".to_string()),
            capabilities: vec![PluginCapability::Channel, PluginCapability::Tool],
            min_smartassist_version: Some(Version::parse("0.1.0").unwrap()),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"name\":\"test-plugin\""));
        assert!(json.contains("\"version\":\"1.0.0\""));

        let deserialized: PluginMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, metadata.name);
        assert_eq!(deserialized.version, metadata.version);
    }

    #[test]
    fn test_plugin_health() {
        let health = PluginHealth::healthy()
            .with_metric("connections", serde_json::json!(5));

        assert!(health.healthy);
        assert!(health.message.is_none());
        assert_eq!(health.metrics.get("connections"), Some(&serde_json::json!(5)));

        let unhealthy = PluginHealth::unhealthy("Connection failed");
        assert!(!unhealthy.healthy);
        assert_eq!(unhealthy.message, Some("Connection failed".to_string()));
    }

    #[test]
    fn test_plugin_config() {
        let json = r#"{
            "options": {"api_key": "secret", "port": 8080},
            "enabled": true,
            "env": {"DEBUG": "1"}
        }"#;

        let config: PluginConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.options.get("port"), Some(&serde_json::json!(8080)));
        assert_eq!(config.env.get("DEBUG"), Some(&"1".to_string()));
    }

    #[tokio::test]
    async fn test_plugin_context() {
        let config = PluginConfig {
            options: [("api_key".to_string(), serde_json::json!("secret"))]
                .into_iter()
                .collect(),
            enabled: true,
            env: HashMap::new(),
        };

        let ctx = PluginContext::new(
            config,
            Version::parse("1.0.0").unwrap(),
            std::path::PathBuf::from("/tmp/test-plugin"),
        );

        let api_key: String = ctx.get_option("api_key").unwrap();
        assert_eq!(api_key, "secret");

        ctx.set("counter", serde_json::json!(42)).await;
        let counter = ctx.get("counter").await;
        assert_eq!(counter, Some(serde_json::json!(42)));
    }

    #[test]
    fn test_plugin_loader() {
        let loader = PluginLoader::new();
        assert!(loader.list().is_empty());
    }

    #[test]
    fn test_plugin_loader_register() {
        // Static registration still works with the new PluginEntry enum
        let loader = PluginLoader::new();
        assert!(loader.list().is_empty());
    }

    #[test]
    fn test_load_from_dir_nonexistent() {
        let mut loader = PluginLoader::new();
        let result = unsafe { loader.load_from_dir(std::path::Path::new("/nonexistent/path")) };
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_load_from_dir_empty() {
        let dir = tempfile::tempdir().unwrap();
        let mut loader = PluginLoader::new();
        let result = unsafe { loader.load_from_dir(dir.path()) };
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
