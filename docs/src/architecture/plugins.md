# Plugin SDK Specification

## Overview

The Plugin SDK allows extending SmartAssist with custom functionality including tools, hooks, channels, providers, and HTTP routes.

## Plugin Definition

```rust
/// Plugin definition structure.
pub struct PluginDefinition {
    /// Unique plugin identifier.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Plugin description.
    pub description: Option<String>,

    /// Version string.
    pub version: Option<String>,

    /// Plugin kind.
    pub kind: Option<PluginKind>,

    /// Configuration schema.
    pub config_schema: Option<PluginConfigSchema>,
}

/// Plugin kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    Memory,
    Channel,
    Provider,
    Tool,
    General,
}

/// Plugin manifest (from smartassist.plugin.json).
#[derive(Debug, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub kind: Option<String>,
    pub channels: Option<Vec<String>>,
    pub providers: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    #[serde(rename = "configSchema")]
    pub config_schema: Option<serde_json::Value>,
    #[serde(rename = "uiHints")]
    pub ui_hints: Option<HashMap<String, PluginConfigUiHint>>,
}

/// UI hints for configuration fields.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfigUiHint {
    pub label: Option<String>,
    pub help: Option<String>,
    pub advanced: Option<bool>,
    pub sensitive: Option<bool>,
    pub placeholder: Option<String>,
}
```

## Plugin API

```rust
/// The main plugin API provided to plugins.
pub struct PluginApi {
    /// Plugin identifier.
    pub id: String,

    /// Plugin name.
    pub name: String,

    /// Plugin version.
    pub version: Option<String>,

    /// Plugin description.
    pub description: Option<String>,

    /// Source path.
    pub source: PathBuf,

    /// Current configuration.
    pub config: Arc<SmartAssistConfig>,

    /// Plugin-specific configuration.
    pub plugin_config: Option<serde_json::Value>,

    /// Runtime reference.
    pub runtime: Arc<PluginRuntime>,

    /// Logger instance.
    pub logger: PluginLogger,

    /// Internal registrations.
    registrations: Arc<Mutex<PluginRegistrations>>,
}

impl PluginApi {
    /// Register a tool.
    pub fn register_tool<T: Tool + 'static>(&self, tool: T) {
        self.registrations.lock().unwrap().tools.push(Arc::new(tool));
    }

    /// Register a tool factory.
    pub fn register_tool_factory(&self, factory: Box<dyn PluginToolFactory>) {
        self.registrations.lock().unwrap().tool_factories.push(factory);
    }

    /// Register a lifecycle hook.
    pub fn register_hook(&self, events: Vec<HookEvent>, handler: Box<dyn HookHandler>) {
        let mut regs = self.registrations.lock().unwrap();
        for event in events {
            regs.hooks.entry(event).or_default().push(handler.clone());
        }
    }

    /// Register an HTTP handler.
    pub fn register_http_handler(&self, handler: Box<dyn HttpHandler>) {
        self.registrations.lock().unwrap().http_handlers.push(handler);
    }

    /// Register an HTTP route.
    pub fn register_http_route(&self, path: &str, handler: Box<dyn HttpRouteHandler>) {
        self.registrations.lock().unwrap().http_routes.insert(path.to_string(), handler);
    }

    /// Register a channel.
    pub fn register_channel(&self, channel: Box<dyn Channel>) {
        self.registrations.lock().unwrap().channels.push(channel);
    }

    /// Register a gateway method.
    pub fn register_gateway_method(&self, method: Box<dyn GatewayMethod>) {
        self.registrations.lock().unwrap().gateway_methods.push(method);
    }

    /// Register a CLI command.
    pub fn register_cli(&self, command: PluginCliCommand) {
        self.registrations.lock().unwrap().cli_commands.push(command);
    }

    /// Register a service.
    pub fn register_service(&self, service: Box<dyn PluginService>) {
        self.registrations.lock().unwrap().services.push(service);
    }

    /// Register a provider.
    pub fn register_provider(&self, provider: ProviderPlugin) {
        self.registrations.lock().unwrap().providers.push(provider);
    }

    /// Register a slash command.
    pub fn register_command(&self, command: PluginCommandDefinition) {
        self.registrations.lock().unwrap().commands.push(command);
    }

    /// Resolve a path relative to plugin source.
    pub fn resolve_path(&self, input: &str) -> PathBuf {
        if Path::new(input).is_absolute() {
            PathBuf::from(input)
        } else {
            self.source.join(input)
        }
    }

    /// Register a hook with the on() syntax.
    pub fn on<H: HookHandler + 'static>(&self, event: HookEvent, handler: H) {
        self.register_hook(vec![event], Box::new(handler));
    }
}

struct PluginRegistrations {
    tools: Vec<Arc<dyn Tool>>,
    tool_factories: Vec<Box<dyn PluginToolFactory>>,
    hooks: HashMap<HookEvent, Vec<Box<dyn HookHandler>>>,
    http_handlers: Vec<Box<dyn HttpHandler>>,
    http_routes: HashMap<String, Box<dyn HttpRouteHandler>>,
    channels: Vec<Box<dyn Channel>>,
    gateway_methods: Vec<Box<dyn GatewayMethod>>,
    cli_commands: Vec<PluginCliCommand>,
    services: Vec<Box<dyn PluginService>>,
    providers: Vec<ProviderPlugin>,
    commands: Vec<PluginCommandDefinition>,
}
```

## Lifecycle Hooks

```rust
/// Hook events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    /// Before agent starts processing.
    BeforeAgentStart,

    /// After agent finishes.
    AgentEnd,

    /// Before context compaction.
    BeforeCompaction,

    /// After context compaction.
    AfterCompaction,

    /// Message received from channel.
    MessageReceived,

    /// Message about to be sent.
    MessageSending,

    /// Message sent successfully.
    MessageSent,

    /// Before tool is called.
    BeforeToolCall,

    /// After tool completes.
    AfterToolCall,

    /// Tool result being persisted.
    ToolResultPersist,

    /// Session started.
    SessionStart,

    /// Session ended.
    SessionEnd,

    /// Gateway starting.
    GatewayStart,

    /// Gateway stopping.
    GatewayStop,
}

/// Hook handler trait.
#[async_trait]
pub trait HookHandler: Send + Sync {
    async fn handle(&self, event: &HookEventData) -> HookResult;
}

/// Hook event data.
#[derive(Debug)]
pub enum HookEventData {
    BeforeAgentStart(BeforeAgentStartEvent),
    AgentEnd(AgentEndEvent),
    BeforeCompaction(CompactionEvent),
    AfterCompaction(CompactionEvent),
    MessageReceived(MessageEvent),
    MessageSending(MessageEvent),
    MessageSent(MessageEvent),
    BeforeToolCall(ToolCallEvent),
    AfterToolCall(AfterToolCallEvent),
    ToolResultPersist(ToolResultEvent),
    SessionStart(SessionEvent),
    SessionEnd(SessionEvent),
    GatewayStart(GatewayEvent),
    GatewayStop(GatewayEvent),
}

/// Hook result.
pub struct HookResult {
    /// Whether to continue processing.
    pub continue_processing: bool,

    /// Modified data (if applicable).
    pub modified_data: Option<serde_json::Value>,

    /// Error message (if hook failed).
    pub error: Option<String>,
}

/// Before agent start event.
#[derive(Debug)]
pub struct BeforeAgentStartEvent {
    pub agent_id: String,
    pub session_key: String,
    pub system_prompt: String,
    pub model: String,
}

/// Tool call event.
#[derive(Debug)]
pub struct ToolCallEvent {
    pub tool_name: String,
    pub params: serde_json::Value,
    pub session_key: String,
}

/// After tool call event.
#[derive(Debug)]
pub struct AfterToolCallEvent {
    pub tool_name: String,
    pub params: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}
```

## Plugin Services

```rust
/// Plugin service trait.
#[async_trait]
pub trait PluginService: Send + Sync {
    /// Service identifier.
    fn id(&self) -> &str;

    /// Start the service.
    async fn start(&self, ctx: &PluginServiceContext) -> Result<(), PluginError>;

    /// Stop the service.
    async fn stop(&self, ctx: &PluginServiceContext) -> Result<(), PluginError>;
}

/// Service context.
pub struct PluginServiceContext {
    pub config: Arc<SmartAssistConfig>,
    pub plugin_config: Option<serde_json::Value>,
    pub runtime: Arc<PluginRuntime>,
    pub logger: PluginLogger,
}
```

## HTTP Routes

```rust
/// HTTP route handler trait.
#[async_trait]
pub trait HttpRouteHandler: Send + Sync {
    async fn handle(
        &self,
        request: HttpRequest,
        response: &mut HttpResponse,
    ) -> Result<(), PluginError>;
}

/// Generic HTTP handler for all requests.
#[async_trait]
pub trait HttpHandler: Send + Sync {
    /// Returns true if this handler handled the request.
    async fn handle(
        &self,
        request: &HttpRequest,
        response: &mut HttpResponse,
    ) -> Result<bool, PluginError>;
}

/// HTTP request wrapper.
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// HTTP response wrapper.
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json<T: Serialize>(&mut self, value: &T) -> Result<(), serde_json::Error> {
        self.headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.body = serde_json::to_vec(value)?;
        Ok(())
    }

    pub fn text(&mut self, text: impl Into<String>) {
        self.headers.insert("Content-Type".to_string(), "text/plain".to_string());
        self.body = text.into().into_bytes();
    }
}
```

## Plugin Commands

```rust
/// Slash command definition.
pub struct PluginCommandDefinition {
    /// Command name (without leading slash).
    pub name: String,

    /// Command description.
    pub description: String,

    /// Whether command accepts arguments.
    pub accepts_args: bool,

    /// Whether auth is required.
    pub require_auth: bool,

    /// Command handler.
    pub handler: Box<dyn PluginCommandHandler>,
}

/// Command handler trait.
#[async_trait]
pub trait PluginCommandHandler: Send + Sync {
    async fn handle(&self, ctx: &PluginCommandContext) -> PluginCommandResult;
}

/// Command context.
pub struct PluginCommandContext {
    pub sender_id: Option<String>,
    pub channel: String,
    pub is_authorized_sender: bool,
    pub args: Option<String>,
    pub command_body: String,
    pub config: Arc<SmartAssistConfig>,
}

/// Command result.
pub struct PluginCommandResult {
    pub text: Option<String>,
    pub error: Option<String>,
    pub handled: bool,
}
```

## Plugin Runtime

```rust
/// Runtime capabilities exposed to plugins.
pub struct PluginRuntime {
    config_loader: Arc<ConfigLoader>,
    media_handler: Arc<MediaHandler>,
    channel_registry: Arc<ChannelRegistry>,
    session_manager: Arc<SessionManager>,
}

impl PluginRuntime {
    /// Load configuration.
    pub async fn load_config(&self) -> Result<SmartAssistConfig, PluginError> {
        self.config_loader.load().await
    }

    /// Write configuration.
    pub async fn write_config(&self, config: &SmartAssistConfig) -> Result<(), PluginError> {
        self.config_loader.write(config).await
    }

    /// Send a system event.
    pub async fn enqueue_system_event(&self, text: &str) -> Result<(), PluginError> {
        // ...
        Ok(())
    }

    /// Run a command with timeout.
    pub async fn run_command_with_timeout(
        &self,
        command: &str,
        timeout_ms: u64,
    ) -> Result<CommandOutput, PluginError> {
        // ...
        todo!()
    }

    /// Load web media.
    pub async fn load_web_media(&self, url: &str) -> Result<Vec<u8>, PluginError> {
        self.media_handler.load_from_url(url).await
    }

    /// Detect MIME type.
    pub fn detect_mime(&self, data: &[u8]) -> Option<String> {
        self.media_handler.detect_mime(data)
    }

    /// Resize image to JPEG.
    pub async fn resize_to_jpeg(
        &self,
        data: &[u8],
        max_width: u32,
        max_height: u32,
    ) -> Result<Vec<u8>, PluginError> {
        self.media_handler.resize_to_jpeg(data, max_width, max_height).await
    }

    /// Chunk markdown text.
    pub fn chunk_markdown_text(&self, text: &str, max_length: usize) -> Vec<String> {
        // ...
        todo!()
    }

    /// Get channel by ID.
    pub fn get_channel(&self, id: &str) -> Option<Arc<dyn Channel>> {
        self.channel_registry.get(id)
    }

    /// Send message via channel.
    pub async fn send_message(
        &self,
        channel_id: &str,
        message: OutboundMessage,
    ) -> Result<SendResult, PluginError> {
        let channel = self.channel_registry.get(channel_id)
            .ok_or(PluginError::ChannelNotFound(channel_id.to_string()))?;
        channel.send(message).await.map_err(Into::into)
    }

    /// Get session.
    pub async fn get_session(&self, key: &SessionKey) -> Result<Session, PluginError> {
        self.session_manager.load(key).await.map_err(Into::into)
    }

    /// Resolve state directory.
    pub fn resolve_state_dir(&self) -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("smartassist")
    }
}
```

## Plugin Loader

```rust
/// Plugin loader.
pub struct PluginLoader {
    plugin_dirs: Vec<PathBuf>,
    loaded: HashMap<String, LoadedPlugin>,
}

/// Loaded plugin.
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub source: PathBuf,
    pub registrations: PluginRegistrations,
    pub enabled: bool,
}

impl PluginLoader {
    pub fn new(plugin_dirs: Vec<PathBuf>) -> Self {
        Self {
            plugin_dirs,
            loaded: HashMap::new(),
        }
    }

    /// Discover available plugins.
    pub async fn discover(&mut self) -> Result<Vec<PluginManifest>, PluginError> {
        let mut manifests = Vec::new();

        for dir in &self.plugin_dirs {
            if !dir.exists() {
                continue;
            }

            let mut entries = tokio::fs::read_dir(dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let manifest_path = path.join("smartassist.plugin.json");

                if manifest_path.exists() {
                    let content = tokio::fs::read_to_string(&manifest_path).await?;
                    let manifest: PluginManifest = serde_json::from_str(&content)?;
                    manifests.push(manifest);
                }
            }
        }

        Ok(manifests)
    }

    /// Load a plugin.
    pub async fn load(
        &mut self,
        plugin_id: &str,
        config: &SmartAssistConfig,
        runtime: Arc<PluginRuntime>,
    ) -> Result<(), PluginError> {
        // Find plugin
        let source = self.find_plugin_source(plugin_id)?;

        // Load manifest
        let manifest_path = source.join("smartassist.plugin.json");
        let manifest: PluginManifest = serde_json::from_str(
            &tokio::fs::read_to_string(&manifest_path).await?
        )?;

        // Get plugin-specific config
        let plugin_config = config.plugins.get(plugin_id).cloned();

        // Validate config against schema if present
        if let Some(schema) = &manifest.config_schema {
            if let Some(ref pc) = plugin_config {
                // Validate...
            }
        }

        // Create API
        let api = PluginApi {
            id: plugin_id.to_string(),
            name: manifest.name.clone().unwrap_or(plugin_id.to_string()),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            source: source.clone(),
            config: Arc::new(config.clone()),
            plugin_config,
            runtime,
            logger: PluginLogger::new(plugin_id),
            registrations: Arc::new(Mutex::new(PluginRegistrations::default())),
        };

        // Load and execute plugin entry point
        // (In Rust, plugins would be dynamic libraries or WASM modules)
        // ...

        // Store loaded plugin
        self.loaded.insert(plugin_id.to_string(), LoadedPlugin {
            manifest,
            source,
            registrations: api.registrations.lock().unwrap().clone(),
            enabled: true,
        });

        Ok(())
    }

    fn find_plugin_source(&self, plugin_id: &str) -> Result<PathBuf, PluginError> {
        for dir in &self.plugin_dirs {
            let path = dir.join(plugin_id);
            if path.exists() {
                return Ok(path);
            }
        }
        Err(PluginError::NotFound(plugin_id.to_string()))
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Plugin load failed: {0}")]
    LoadFailed(String),

    #[error("Config validation failed: {0}")]
    ConfigValidation(String),

    #[error("Hook failed: {0}")]
    HookFailed(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
```
