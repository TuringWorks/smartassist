# Gateway Specification Overview

## Protocol

JSON-RPC 2.0 over WebSocket with the following frame types:

```rust
/// Request frame from client.
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestFrame {
    #[serde(rename = "type")]
    pub frame_type: String, // "req"
    pub id: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// Response frame from server.
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseFrame {
    #[serde(rename = "type")]
    pub frame_type: String, // "res"
    pub id: String,
    pub ok: bool,
    pub payload: Option<serde_json::Value>,
    pub error: Option<ErrorShape>,
}

/// Event frame from server.
#[derive(Debug, Serialize, Deserialize)]
pub struct EventFrame {
    #[serde(rename = "type")]
    pub frame_type: String, // "event"
    pub event: String,
    pub payload: Option<serde_json::Value>,
    pub seq: Option<u64>,
    #[serde(rename = "stateVersion")]
    pub state_version: Option<StateVersion>,
}

/// Error shape for failed responses.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorShape {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub retryable: Option<bool>,
    #[serde(rename = "retryAfterMs")]
    pub retry_after_ms: Option<u64>,
}

/// State version for optimistic updates.
#[derive(Debug, Serialize, Deserialize)]
pub struct StateVersion {
    pub presence: Option<u64>,
    pub health: Option<u64>,
}
```

## Method Categories

46 registered RPC methods across 15 handler categories:

### 1. Chat Methods

- `chat` - Send a chat message and get response
- `chat.history` - Get conversation history
- `chat.abort` - Abort ongoing chat

### 2. Agent Methods

- `agent` - Run agent turn
- `agent.stream` - Streaming agent turn

### 3. Session Methods

- `sessions.list` - List sessions
- `sessions.resolve` - Resolve session label to key
- `sessions.patch` - Update session config
- `sessions.delete` - Delete session

### 4. Node Methods

- `node.pair.request` - Request node pairing
- `node.pair.approve` - Approve pairing
- `node.pair.reject` - Reject pairing
- `node.unpair` - Unpair node
- `node.rename` - Rename node
- `node.list` - List nodes
- `node.describe` - Get node details
- `node.invoke` - Invoke node command
- `node.invoke.result` - Node command result
- `node.event` - Node event

### 5. Device Methods

- `device.pair.list` - List paired devices
- `device.pair.approve` - Approve pairing
- `device.pair.reject` - Reject pairing
- `device.token.rotate` - Rotate device token
- `device.token.revoke` - Revoke device token

### 6. Exec Approval Methods

- `exec.approvals.get` - Get approval config
- `exec.approvals.set` - Set approval config
- `exec.approvals.node.get` - Get node approval config
- `exec.approvals.node.set` - Set node approval config
- `exec.approval.request` - Request exec approval
- `exec.approval.resolve` - Resolve approval

### 7. Config Methods

- `config.get` - Get config
- `config.set` - Set config
- `config.apply` - Apply config with restart
- `config.patch` - Patch config
- `config.schema` - Get config schema

### 8. Cron Methods

- `cron.list` - List cron jobs
- `cron.status` - Get cron status
- `cron.add` - Add cron job
- `cron.update` - Update cron job
- `cron.remove` - Remove cron job
- `cron.run` - Run cron job
- `cron.runs` - Get cron run history
- `wake` - Send wake event

### 9. Skill Methods

- `skills.status` - Get skills status
- `skills.bins` - Get skill binaries
- `skills.install` - Install skill
- `skills.update` - Update skill

### 10. Model Methods

- `models.list` - List available models

### 11. Health Methods

- `health` - Get health status
- `status` - Get system status
- `logs.tail` - Tail logs

### 12. System Methods

- `system-presence` - Get system presence
- `system-event` - Send system event
- `last-heartbeat` - Get last heartbeat
- `set-heartbeats` - Enable/disable heartbeats

### 13. Wizard Methods

- `wizard.start` - Start setup wizard
- `wizard.next` - Next wizard step
- `wizard.cancel` - Cancel wizard
- `wizard.status` - Get wizard status

### 14. Send Methods

- `send` - Send message
- `send.poll` - Send poll

### 15. Browser Methods

- `browser.request` - Browser HTTP request

## Gateway Server

```rust
pub struct GatewayServer {
    config: GatewayConfig,
    methods: HashMap<String, Box<dyn GatewayMethod>>,
    connections: RwLock<HashMap<String, GatewayConnection>>,
    event_tx: broadcast::Sender<EventFrame>,
}

#[async_trait]
pub trait GatewayMethod: Send + Sync {
    fn name(&self) -> &str;
    async fn handle(
        &self,
        params: serde_json::Value,
        ctx: &MethodContext,
    ) -> Result<serde_json::Value, GatewayError>;
}

pub struct MethodContext {
    pub connection_id: String,
    pub client_info: ClientInfo,
    pub config: Arc<SmartAssistConfig>,
    pub agent_runtime: Arc<AgentRuntime>,
    pub session_manager: Arc<SessionManager>,
    pub channel_registry: Arc<ChannelRegistry>,
    pub cron_scheduler: Arc<CronScheduler>,
    pub node_manager: Arc<NodeManager>,
}

pub struct GatewayConnection {
    pub id: String,
    pub client_info: ClientInfo,
    pub tx: mpsc::Sender<String>,
    pub subscriptions: HashSet<String>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

pub struct ClientInfo {
    pub client_id: String,
    pub client_display_name: String,
    pub mode: ClientMode,
    pub version: Option<String>,
    pub device_id: Option<String>,
    pub roles: Vec<String>,
    pub scopes: Vec<String>,
}

pub enum ClientMode {
    Backend,
    Frontend,
}
```

## Error Codes

```rust
pub enum GatewayErrorCode {
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    RateLimited,
    Unavailable,
    AgentTimeout,
    NotPaired,
}

impl GatewayErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::MethodNotFound => "METHOD_NOT_FOUND",
            Self::InvalidParams => "INVALID_PARAMS",
            Self::InternalError => "INTERNAL_ERROR",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::RateLimited => "RATE_LIMITED",
            Self::Unavailable => "UNAVAILABLE",
            Self::AgentTimeout => "AGENT_TIMEOUT",
            Self::NotPaired => "NOT_PAIRED",
        }
    }
}
```

## Events

```rust
pub enum GatewayEvent {
    /// Device pairing challenge
    ConnectChallenge(ConnectChallengePayload),

    /// Agent execution event
    Agent(AgentEventPayload),

    /// Chat execution event
    Chat(ChatEventPayload),

    /// System presence update
    Presence(PresencePayload),

    /// Heartbeat tick
    Tick(TickPayload),

    /// Talk mode changed
    TalkMode(TalkModePayload),

    /// Gateway shutdown
    Shutdown(ShutdownPayload),

    /// Health status update
    Health(HealthPayload),

    /// Heartbeat event
    Heartbeat(HeartbeatPayload),

    /// Cron job execution
    Cron(CronPayload),

    /// Node pairing request
    NodePairRequested(NodePairRequestPayload),

    /// Node pairing resolved
    NodePairResolved(NodePairResolvedPayload),

    /// Node command invocation
    NodeInvokeRequest(NodeInvokeRequestPayload),

    /// Device pairing request
    DevicePairRequested(DevicePairRequestPayload),

    /// Device pairing resolved
    DevicePairResolved(DevicePairResolvedPayload),

    /// Voice wake triggers changed
    VoiceWakeChanged(VoiceWakeChangedPayload),

    /// Exec approval requested
    ExecApprovalRequested(ExecApprovalRequestPayload),

    /// Exec approval resolved
    ExecApprovalResolved(ExecApprovalResolvedPayload),
}
```

## Implementation

```rust
impl GatewayServer {
    pub fn new(config: GatewayConfig) -> Self {
        let (event_tx, _) = broadcast::channel(1024);

        let mut server = Self {
            config,
            methods: HashMap::new(),
            connections: RwLock::new(HashMap::new()),
            event_tx,
        };

        // Register all methods
        server.register_method(Box::new(ChatMethod::new()));
        server.register_method(Box::new(ChatStreamMethod::new()));
        server.register_method(Box::new(ChatHistoryMethod::new()));
        server.register_method(Box::new(AgentMethod::new()));
        server.register_method(Box::new(SessionsListMethod::new()));
        server.register_method(Box::new(SessionsResolveMethod::new()));
        server.register_method(Box::new(SessionsPatchMethod::new()));
        server.register_method(Box::new(SessionsDeleteMethod::new()));
        // ... register all other methods

        server
    }

    pub fn register_method(&mut self, method: Box<dyn GatewayMethod>) {
        self.methods.insert(method.name().to_string(), method);
    }

    pub async fn handle_request(
        &self,
        request: RequestFrame,
        ctx: &MethodContext,
    ) -> ResponseFrame {
        let method = match self.methods.get(&request.method) {
            Some(m) => m,
            None => {
                return ResponseFrame {
                    frame_type: "res".to_string(),
                    id: request.id,
                    ok: false,
                    payload: None,
                    error: Some(ErrorShape {
                        code: GatewayErrorCode::MethodNotFound.as_str().to_string(),
                        message: format!("Method '{}' not found", request.method),
                        details: None,
                        retryable: Some(false),
                        retry_after_ms: None,
                    }),
                };
            }
        };

        match method.handle(request.params.unwrap_or_default(), ctx).await {
            Ok(payload) => ResponseFrame {
                frame_type: "res".to_string(),
                id: request.id,
                ok: true,
                payload: Some(payload),
                error: None,
            },
            Err(e) => ResponseFrame {
                frame_type: "res".to_string(),
                id: request.id,
                ok: false,
                payload: None,
                error: Some(e.into()),
            },
        }
    }

    pub fn broadcast_event(&self, event: GatewayEvent) {
        let frame = EventFrame {
            frame_type: "event".to_string(),
            event: event.event_name().to_string(),
            payload: Some(event.to_payload()),
            seq: None,
            state_version: None,
        };

        let _ = self.event_tx.send(frame);
    }

    pub async fn run(&self) -> Result<(), GatewayError> {
        let listener = TcpListener::bind(&self.config.bind_address).await?;

        loop {
            let (stream, addr) = listener.accept().await?;
            let server = self.clone();

            tokio::spawn(async move {
                if let Err(e) = server.handle_connection(stream, addr).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }

    async fn handle_connection(
        &self,
        stream: TcpStream,
        addr: SocketAddr,
    ) -> Result<(), GatewayError> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let (write, read) = ws_stream.split();

        let connection_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(256);

        // Add connection
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id.clone(), GatewayConnection {
                id: connection_id.clone(),
                client_info: ClientInfo::default(),
                tx,
                subscriptions: HashSet::new(),
                created_at: Utc::now(),
                last_activity: Utc::now(),
            });
        }

        // Handle messages
        let read_task = self.handle_incoming(connection_id.clone(), read);
        let write_task = Self::handle_outgoing(rx, write);

        tokio::select! {
            _ = read_task => {}
            _ = write_task => {}
        }

        // Remove connection
        {
            let mut connections = self.connections.write().await;
            connections.remove(&connection_id);
        }

        Ok(())
    }
}
```
