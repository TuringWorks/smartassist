# Model Providers Specification

## Overview

Model providers enable SmartAssist to interact with various LLM APIs including Anthropic, OpenAI, Google, and others.

## Provider Trait

```rust
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// Model provider trait.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Provider identifier.
    fn id(&self) -> &str;

    /// Provider display name.
    fn name(&self) -> &str;

    /// List available models.
    fn models(&self) -> &[ModelDefinition];

    /// Check if provider is configured.
    fn is_configured(&self) -> bool;

    /// Send a chat completion request.
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<ChatResponse, ProviderError>;

    /// Send a streaming chat request.
    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>;
}

/// Model definition.
#[derive(Debug, Clone)]
pub struct ModelDefinition {
    /// Model identifier.
    pub id: String,

    /// Display name.
    pub name: String,

    /// API type.
    pub api: ModelApi,

    /// Whether model supports reasoning/thinking.
    pub reasoning: bool,

    /// Supported input types.
    pub input_types: Vec<InputType>,

    /// Cost per 1M tokens.
    pub cost: ModelCost,

    /// Context window size.
    pub context_window: usize,

    /// Maximum output tokens.
    pub max_tokens: usize,

    /// Custom headers for this model.
    pub headers: HashMap<String, String>,

    /// Compatibility settings.
    pub compat: ModelCompat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelApi {
    AnthropicMessages,
    OpenAiCompletions,
    OpenAiResponses,
    GoogleGenerativeAi,
    GithubCopilot,
    BedrockConverseStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Text,
    Image,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelCost {
    /// Input cost per 1M tokens.
    pub input: f64,

    /// Output cost per 1M tokens.
    pub output: f64,

    /// Cache read cost per 1M tokens.
    pub cache_read: f64,

    /// Cache write cost per 1M tokens.
    pub cache_write: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ModelCompat {
    /// Supports prompt caching.
    pub supports_store: bool,

    /// Supports developer/system role.
    pub supports_developer_role: bool,

    /// Supports reasoning effort control.
    pub supports_reasoning_effort: bool,

    /// Field name for max tokens.
    pub max_tokens_field: MaxTokensField,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum MaxTokensField {
    #[default]
    MaxTokens,
    MaxCompletionTokens,
}
```

## Chat Request/Response

```rust
/// Chat completion request.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Model identifier.
    pub model: String,

    /// Conversation messages.
    pub messages: Vec<ChatMessage>,

    /// System prompt.
    pub system: Option<String>,

    /// Maximum tokens to generate.
    pub max_tokens: Option<usize>,

    /// Temperature (0.0 - 1.0).
    pub temperature: Option<f32>,

    /// Top-p sampling.
    pub top_p: Option<f32>,

    /// Stop sequences.
    pub stop: Option<Vec<String>>,

    /// Available tools.
    pub tools: Option<Vec<ToolDefinition>>,

    /// Tool choice behavior.
    pub tool_choice: Option<ToolChoice>,

    /// Thinking/reasoning configuration.
    pub thinking: Option<ThinkingConfig>,

    /// Request timeout.
    pub timeout_ms: Option<u64>,
}

/// Chat message.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: ChatContent,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone)]
pub enum ChatContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
    Thinking { thinking: String },
}

#[derive(Debug, Clone)]
pub enum ImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
}

/// Tool definition for LLM.
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Tool { name: String },
}

/// Thinking/reasoning configuration.
#[derive(Debug, Clone)]
pub struct ThinkingConfig {
    pub enabled: bool,
    pub effort: ThinkingEffort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingEffort {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    ExtraHigh,
}

/// Chat completion response.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    /// Response ID.
    pub id: String,

    /// Model used.
    pub model: String,

    /// Generated content.
    pub content: Vec<ContentBlock>,

    /// Stop reason.
    pub stop_reason: StopReason,

    /// Token usage.
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
}

/// Token usage statistics.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Streaming chat chunk.
#[derive(Debug, Clone)]
pub struct ChatChunk {
    pub delta: ChunkDelta,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone)]
pub enum ChunkDelta {
    Text { text: String },
    ToolUseStart { id: String, name: String },
    ToolUseInput { input: String },
    Thinking { thinking: String },
    Stop { reason: StopReason },
}
```

## Provider Configuration

```rust
/// Provider configuration.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Base URL for API.
    pub base_url: String,

    /// API key.
    pub api_key: Option<SecretString>,

    /// Authentication mode.
    pub auth: AuthMode,

    /// API type.
    pub api: ModelApi,

    /// Custom headers.
    pub headers: HashMap<String, String>,

    /// Whether to include auth header.
    pub auth_header: bool,

    /// Available models.
    pub models: Vec<ModelDefinition>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum AuthMode {
    #[default]
    ApiKey,
    AwsSdk,
    OAuth,
    Token,
}
```

## Built-in Providers

### Anthropic Provider

```rust
pub struct AnthropicProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: SecretString) -> Self {
        Self {
            config: ProviderConfig {
                base_url: "https://api.anthropic.com".to_string(),
                api_key: Some(api_key),
                auth: AuthMode::ApiKey,
                api: ModelApi::AnthropicMessages,
                headers: HashMap::from([
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ]),
                auth_header: true,
                models: vec![
                    ModelDefinition {
                        id: "claude-sonnet-4-20250514".to_string(),
                        name: "Claude Sonnet 4".to_string(),
                        api: ModelApi::AnthropicMessages,
                        reasoning: true,
                        input_types: vec![InputType::Text, InputType::Image],
                        cost: ModelCost {
                            input: 3.0,
                            output: 15.0,
                            cache_read: 0.3,
                            cache_write: 3.75,
                        },
                        context_window: 200000,
                        max_tokens: 16000,
                        headers: HashMap::new(),
                        compat: ModelCompat {
                            supports_store: true,
                            supports_developer_role: true,
                            supports_reasoning_effort: true,
                            ..Default::default()
                        },
                    },
                    ModelDefinition {
                        id: "claude-opus-4-20250514".to_string(),
                        name: "Claude Opus 4".to_string(),
                        api: ModelApi::AnthropicMessages,
                        reasoning: true,
                        input_types: vec![InputType::Text, InputType::Image],
                        cost: ModelCost {
                            input: 15.0,
                            output: 75.0,
                            cache_read: 1.5,
                            cache_write: 18.75,
                        },
                        context_window: 200000,
                        max_tokens: 32000,
                        headers: HashMap::new(),
                        compat: ModelCompat {
                            supports_store: true,
                            supports_developer_role: true,
                            supports_reasoning_effort: true,
                            ..Default::default()
                        },
                    },
                    ModelDefinition {
                        id: "claude-3-5-haiku-20241022".to_string(),
                        name: "Claude 3.5 Haiku".to_string(),
                        api: ModelApi::AnthropicMessages,
                        reasoning: true,
                        input_types: vec![InputType::Text, InputType::Image],
                        cost: ModelCost {
                            input: 1.0,
                            output: 5.0,
                            cache_read: 0.1,
                            cache_write: 1.25,
                        },
                        context_window: 200000,
                        max_tokens: 8192,
                        headers: HashMap::new(),
                        compat: ModelCompat {
                            supports_store: true,
                            supports_developer_role: true,
                            supports_reasoning_effort: true,
                            ..Default::default()
                        },
                    },
                ],
            },
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn id(&self) -> &str { "anthropic" }
    fn name(&self) -> &str { "Anthropic" }
    fn models(&self) -> &[ModelDefinition] { &self.config.models }
    fn is_configured(&self) -> bool { self.config.api_key.is_some() }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let api_key = self.config.api_key.as_ref()
            .ok_or(ProviderError::NotConfigured)?;

        let body = self.build_request_body(&request)?;

        let response = self.client
            .post(format!("{}/v1/messages", self.config.base_url))
            .header("x-api-key", api_key.expose_secret())
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error: serde_json::Value = response.json().await?;
            return Err(ProviderError::Api {
                code: error["error"]["type"].as_str().unwrap_or("unknown").to_string(),
                message: error["error"]["message"].as_str().unwrap_or("Unknown error").to_string(),
            });
        }

        let result: serde_json::Value = response.json().await?;
        self.parse_response(result)
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError> {
        // Streaming implementation parses SSE events from the provider API
    }
}
```

### OpenAI Provider

```rust
pub struct OpenAiProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: SecretString) -> Self {
        Self {
            config: ProviderConfig {
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: Some(api_key),
                auth: AuthMode::ApiKey,
                api: ModelApi::OpenAiCompletions,
                headers: HashMap::new(),
                auth_header: true,
                models: vec![
                    ModelDefinition {
                        id: "gpt-4o-mini".to_string(),
                        name: "GPT-4o Mini".to_string(),
                        api: ModelApi::OpenAiCompletions,
                        reasoning: false,
                        input_types: vec![InputType::Text, InputType::Image],
                        cost: ModelCost {
                            input: 0.15,
                            output: 0.6,
                            cache_read: 0.0,
                            cache_write: 0.0,
                        },
                        context_window: 128000,
                        max_tokens: 16384,
                        headers: HashMap::new(),
                        compat: ModelCompat {
                            max_tokens_field: MaxTokensField::MaxCompletionTokens,
                            ..Default::default()
                        },
                    },
                    ModelDefinition {
                        id: "gpt-4o".to_string(),
                        name: "GPT-4o".to_string(),
                        api: ModelApi::OpenAiCompletions,
                        reasoning: false,
                        input_types: vec![InputType::Text, InputType::Image],
                        cost: ModelCost {
                            input: 2.5,
                            output: 10.0,
                            cache_read: 0.0,
                            cache_write: 0.0,
                        },
                        context_window: 128000,
                        max_tokens: 16384,
                        headers: HashMap::new(),
                        compat: ModelCompat {
                            max_tokens_field: MaxTokensField::MaxCompletionTokens,
                            ..Default::default()
                        },
                    },
                ],
            },
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelProvider for OpenAiProvider {
    fn id(&self) -> &str { "openai" }
    fn name(&self) -> &str { "OpenAI" }
    fn models(&self) -> &[ModelDefinition] { &self.config.models }
    fn is_configured(&self) -> bool { self.config.api_key.is_some() }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let api_key = self.config.api_key.as_ref()
            .ok_or(ProviderError::NotConfigured)?;

        let body = self.build_request_body(&request)?;

        let response = self.client
            .post(format!("{}/chat/completions", self.config.base_url))
            .bearer_auth(api_key.expose_secret())
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error: serde_json::Value = response.json().await?;
            return Err(ProviderError::Api {
                code: error["error"]["type"].as_str().unwrap_or("unknown").to_string(),
                message: error["error"]["message"].as_str().unwrap_or("Unknown error").to_string(),
            });
        }

        let result: serde_json::Value = response.json().await?;
        self.parse_response(result)
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError> {
        // Streaming implementation parses SSE events from the provider API
    }
}
```

### Google Provider

```rust
pub struct GoogleProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl GoogleProvider {
    pub fn new(api_key: SecretString) -> Self {
        Self {
            config: ProviderConfig {
                base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
                api_key: Some(api_key),
                auth: AuthMode::ApiKey,
                api: ModelApi::GoogleGenerativeAi,
                headers: HashMap::new(),
                auth_header: false,
                models: vec![
                    ModelDefinition {
                        id: "gemini-2.0-flash".to_string(),
                        name: "Gemini 2.0 Flash".to_string(),
                        api: ModelApi::GoogleGenerativeAi,
                        reasoning: false,
                        input_types: vec![InputType::Text, InputType::Image],
                        cost: ModelCost {
                            input: 0.1,
                            output: 0.4,
                            cache_read: 0.0,
                            cache_write: 0.0,
                        },
                        context_window: 1000000,
                        max_tokens: 8192,
                        headers: HashMap::new(),
                        compat: Default::default(),
                    },
                    ModelDefinition {
                        id: "gemini-1.5-pro".to_string(),
                        name: "Gemini 1.5 Pro".to_string(),
                        api: ModelApi::GoogleGenerativeAi,
                        reasoning: false,
                        input_types: vec![InputType::Text, InputType::Image],
                        cost: ModelCost {
                            input: 1.25,
                            output: 5.0,
                            cache_read: 0.0,
                            cache_write: 0.0,
                        },
                        context_window: 2000000,
                        max_tokens: 8192,
                        headers: HashMap::new(),
                        compat: Default::default(),
                    },
                ],
            },
            client: reqwest::Client::new(),
        }
    }
}
```

## Provider Registry

```rust
/// Provider registry.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn ModelProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn ModelProvider>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn ModelProvider>> {
        self.providers.get(id).cloned()
    }

    pub fn get_for_model(&self, model_id: &str) -> Option<Arc<dyn ModelProvider>> {
        // Parse provider/model format
        if let Some((provider_id, _)) = model_id.split_once('/') {
            return self.get(provider_id);
        }

        // Search all providers for model
        for provider in self.providers.values() {
            if provider.models().iter().any(|m| m.id == model_id) {
                return Some(provider.clone());
            }
        }

        None
    }

    pub fn list_models(&self) -> Vec<&ModelDefinition> {
        self.providers.values()
            .flat_map(|p| p.models().iter())
            .collect()
    }

    pub fn list_configured(&self) -> Vec<&str> {
        self.providers.values()
            .filter(|p| p.is_configured())
            .map(|p| p.id())
            .collect()
    }
}

/// Create default provider registry.
pub fn create_default_registry(config: &SmartAssistConfig) -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();

    // Anthropic
    if let Some(key) = config.providers.anthropic.api_key.as_ref() {
        registry.register(Arc::new(AnthropicProvider::new(key.clone())));
    }

    // OpenAI
    if let Some(key) = config.providers.openai.api_key.as_ref() {
        registry.register(Arc::new(OpenAiProvider::new(key.clone())));
    }

    // Google
    if let Some(key) = config.providers.google.api_key.as_ref() {
        registry.register(Arc::new(GoogleProvider::new(key.clone())));
    }

    // Add more providers...

    registry
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Provider not configured")]
    NotConfigured,

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("API error: {code} - {message}")]
    Api { code: String, message: String },

    #[error("Rate limited, retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("Timeout")]
    Timeout,

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),
}
```

## Auth Profile Management

```rust
/// Auth profile store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProfileStore {
    pub version: u32,
    pub profiles: HashMap<String, AuthProfileCredential>,
    pub order: Option<HashMap<String, Vec<String>>>,
    pub last_good: Option<HashMap<String, String>>,
    pub usage_stats: Option<HashMap<String, ProfileUsageStats>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthProfileCredential {
    #[serde(rename = "api_key")]
    ApiKey {
        provider: String,
        key: String,
        email: Option<String>,
    },
    #[serde(rename = "token")]
    Token {
        provider: String,
        token: String,
        expires: Option<u64>,
        email: Option<String>,
    },
    #[serde(rename = "oauth")]
    OAuth {
        provider: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        expires_at: Option<u64>,
        client_id: Option<String>,
        email: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUsageStats {
    pub last_used: Option<u64>,
    pub cooldown_until: Option<u64>,
    pub disabled_until: Option<u64>,
    pub disabled_reason: Option<FailureReason>,
    pub error_count: Option<u32>,
    pub failure_counts: Option<HashMap<FailureReason, u32>>,
    pub last_failure_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureReason {
    Auth,
    Format,
    RateLimit,
    Billing,
    Timeout,
    Unknown,
}
```
