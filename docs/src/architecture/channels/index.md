# Channel Traits Specification

## Core Channel Trait

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    /// Unique channel identifier (e.g., "telegram", "discord")
    fn id(&self) -> &str;

    /// Human-readable channel name
    fn name(&self) -> &str;

    /// Channel capabilities
    fn capabilities(&self) -> ChannelCapabilities;

    /// Connect to the channel service
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the channel service
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if connected
    fn is_connected(&self) -> bool;

    /// Get channel health status
    async fn health(&self) -> Result<ChannelHealth>;

    /// Send an outbound message
    async fn send(&self, message: OutboundMessage) -> Result<SendResult>;

    /// Edit a previously sent message
    async fn edit(&self, target: MessageTarget, new_content: &str) -> Result<()>;

    /// Delete a message
    async fn delete(&self, target: MessageTarget) -> Result<()>;

    /// React to a message
    async fn react(&self, target: MessageTarget, emoji: &str) -> Result<()>;

    /// Remove a reaction
    async fn unreact(&self, target: MessageTarget, emoji: &str) -> Result<()>;

    /// Start typing indicator
    async fn typing(&self, chat_id: &str) -> Result<()>;

    /// Get message handler stream
    fn messages(&self) -> Pin<Box<dyn Stream<Item = InboundMessage> + Send>>;
}
```

## Channel Capabilities

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCapabilities {
    /// Supported chat types
    pub chat_types: Vec<ChatType>,

    /// Media capabilities
    pub media: MediaCapabilities,

    /// Feature flags
    pub features: ChannelFeatures,

    /// Message limits
    pub limits: ChannelLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaCapabilities {
    pub images: bool,
    pub audio: bool,
    pub video: bool,
    pub files: bool,
    pub stickers: bool,
    pub voice_notes: bool,
    pub max_file_size_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelFeatures {
    pub reactions: bool,
    pub threads: bool,
    pub edits: bool,
    pub deletes: bool,
    pub typing_indicators: bool,
    pub read_receipts: bool,
    pub mentions: bool,
    pub polls: bool,
    pub buttons: bool,
    pub inline_queries: bool,
    pub commands: bool,
    pub markdown: bool,
    pub html: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelLimits {
    pub max_message_length: usize,
    pub max_caption_length: usize,
    pub max_buttons_per_row: usize,
    pub max_button_rows: usize,
}
```

## Message Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Unique message ID
    pub id: MessageId,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Channel identifier
    pub channel: String,

    /// Account/bot identifier
    pub account_id: String,

    /// Sender information
    pub sender: SenderInfo,

    /// Chat information
    pub chat: ChatInfo,

    /// Message text content
    pub text: String,

    /// Media attachments
    pub media: Vec<MediaAttachment>,

    /// Quoted/replied message
    pub quote: Option<QuotedMessage>,

    /// Thread information
    pub thread: Option<ThreadInfo>,

    /// Channel-specific metadata
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Target chat
    pub chat_id: String,

    /// Message text
    pub text: String,

    /// Media attachments
    pub media: Vec<MediaAttachment>,

    /// Reply to message ID
    pub reply_to: Option<String>,

    /// Thread ID
    pub thread_id: Option<String>,

    /// Inline buttons
    pub buttons: Option<Vec<Vec<Button>>>,

    /// Parse mode (markdown, html, none)
    pub parse_mode: Option<ParseMode>,

    /// Disable link previews
    pub disable_preview: bool,

    /// Silent/notification-less
    pub silent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderInfo {
    pub id: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub phone_number: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub id: String,
    pub chat_type: ChatType,
    pub title: Option<String>,
    pub guild_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    Direct,
    Group,
    Channel,
    Thread,
    Forum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    pub attachment_type: MediaType,
    pub url: Option<String>,
    pub file_id: Option<String>,
    pub file_path: Option<PathBuf>,
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Image,
    Audio,
    Video,
    Document,
    Sticker,
    Voice,
    Animation,
}
```

## Channel Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Whether channel is enabled
    pub enabled: bool,

    /// Account configurations
    pub accounts: Vec<AccountConfig>,

    /// Default agent for this channel
    pub default_agent: Option<String>,

    /// Allowlist configuration
    pub allowlist: AllowlistConfig,

    /// DM policy
    pub dm_policy: MessagePolicy,

    /// Group policy
    pub group_policy: MessagePolicy,

    /// Channel-specific settings
    pub settings: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    pub id: String,
    pub token: Option<SecretString>,
    pub api_key: Option<SecretString>,
    pub phone_number: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowlistConfig {
    pub enabled: bool,
    pub mode: AllowlistMode,
    pub users: Vec<String>,
    pub chats: Vec<String>,
    pub guilds: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllowlistMode {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessagePolicy {
    Allow,
    Deny,
    RequireAllowlist,
}
```

## Channel Registry

```rust
pub struct ChannelRegistry {
    channels: HashMap<String, Arc<dyn Channel>>,
    configs: HashMap<String, ChannelConfig>,
}

impl ChannelRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, channel: Arc<dyn Channel>, config: ChannelConfig);
    pub fn unregister(&mut self, channel_id: &str);
    pub fn get(&self, channel_id: &str) -> Option<Arc<dyn Channel>>;
    pub fn list(&self) -> Vec<&str>;
    pub fn list_enabled(&self) -> Vec<&str>;
    pub async fn connect_all(&self) -> Result<()>;
    pub async fn disconnect_all(&self) -> Result<()>;
    pub async fn health_all(&self) -> HashMap<String, ChannelHealth>;
}
```

## Message Router

```rust
pub struct MessageRouter {
    rules: Vec<RoutingRule>,
    default_agent: Option<AgentId>,
}

#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub id: String,
    pub priority: i32,
    pub conditions: RoutingConditions,
    pub agent_id: AgentId,
}

#[derive(Debug, Clone, Default)]
pub struct RoutingConditions {
    pub channel: Option<String>,
    pub account: Option<String>,
    pub chat_id: Option<String>,
    pub chat_type: Option<ChatType>,
    pub sender_id: Option<String>,
    pub guild_id: Option<String>,
    pub text_pattern: Option<String>,
}

impl MessageRouter {
    pub fn new() -> Self;
    pub fn with_default_agent(self, agent: AgentId) -> Self;
    pub fn add_rule(&mut self, rule: RoutingRule);
    pub fn route(&self, message: &InboundMessage) -> Result<AgentId>;
}
```

## Delivery Queue

```rust
pub struct DeliveryQueue {
    queue: VecDeque<QueuedMessage>,
    retry_policy: RetryPolicy,
    max_queue_size: usize,
}

#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub id: String,
    pub message: OutboundMessage,
    pub channel_id: String,
    pub account_id: String,
    pub attempts: u32,
    pub created_at: DateTime<Utc>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub status: DeliveryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    Pending,
    InFlight,
    Delivered,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_backoff: bool,
}

impl DeliveryQueue {
    pub fn new(policy: RetryPolicy) -> Self;
    pub async fn enqueue(&mut self, message: OutboundMessage, channel: &str, account: &str) -> String;
    pub async fn process(&mut self, registry: &ChannelRegistry) -> Result<()>;
    pub fn cancel(&mut self, message_id: &str) -> bool;
    pub fn stats(&self) -> QueueStats;
}
```
