//! WebSocket gateway server.

use crate::error::GatewayError;
use crate::methods::MethodRegistry;
use crate::rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, State, WebSocketUpgrade,
    },
    http::{HeaderMap, HeaderValue, Method},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use smartassist_core::config::BindMode;
use smartassist_core::types::{AuthContext, Scope};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{debug, error, info, warn};

/// Default gateway port.
pub const DEFAULT_PORT: u16 = 18789;

/// Maximum connections per second before rate limiting kicks in.
const MAX_CONNECTIONS_PER_SECOND: u64 = 10;

/// Maximum messages per second per client.
const MAX_MESSAGES_PER_SECOND: u64 = 60;

/// Allowed origins for CORS and WebSocket origin validation.
const ALLOWED_ORIGINS: &[&str] = &[
    "http://localhost",
    "http://127.0.0.1",
    "https://localhost",
    "https://127.0.0.1",
    "https://app.smartassist.dev",
    "https://docs.smartassist.dev",
];

/// Gateway configuration.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Bind mode.
    pub bind: BindMode,

    /// Port number.
    pub port: u16,

    /// Enable CORS.
    pub cors: bool,

    /// Maximum connections.
    pub max_connections: usize,

    /// Authentication token (required for non-loopback binds).
    pub auth_token: Option<String>,

    /// Whether to require authentication.
    pub require_auth: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            bind: BindMode::Loopback,
            port: DEFAULT_PORT,
            cors: true,
            max_connections: 100,
            auth_token: None,
            require_auth: false,
        }
    }
}

/// Gateway server state.
pub struct GatewayState {
    /// Method registry.
    pub methods: Arc<MethodRegistry>,

    /// Connected clients.
    pub clients: RwLock<HashMap<String, ClientInfo>>,

    /// Broadcast channel for notifications.
    pub broadcast_tx: broadcast::Sender<String>,

    /// Configuration.
    pub config: GatewayConfig,

    /// Connection counter for rate limiting.
    connection_count: AtomicU64,

    /// Last rate limit reset timestamp (unix seconds).
    rate_limit_reset: AtomicU64,
}

impl GatewayState {
    /// Check and increment connection rate limit.
    fn check_rate_limit(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let last_reset = self.rate_limit_reset.load(Ordering::Relaxed);
        if now > last_reset {
            self.rate_limit_reset.store(now, Ordering::Relaxed);
            self.connection_count.store(1, Ordering::Relaxed);
            true
        } else {
            let count = self.connection_count.fetch_add(1, Ordering::Relaxed);
            count < MAX_CONNECTIONS_PER_SECOND
        }
    }

    /// Validate an auth token against the configured token.
    fn validate_token(&self, token: &str) -> Option<AuthContext> {
        if let Some(ref expected) = self.config.auth_token {
            if token == expected {
                return Some(AuthContext::admin("token"));
            }
        }
        None
    }

    /// Determine auth context from request headers and bind mode.
    fn authenticate(&self, headers: &HeaderMap) -> std::result::Result<AuthContext, GatewayError> {
        // Loopback connections are implicitly trusted
        if self.config.bind == BindMode::Loopback {
            return Ok(AuthContext::loopback());
        }

        // Non-loopback: require token authentication
        if let Some(auth_header) = headers.get("authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    if let Some(ctx) = self.validate_token(token) {
                        return Ok(ctx);
                    }
                }
            }
            return Err(GatewayError::Auth("Invalid authentication token".to_string()));
        }

        if self.config.require_auth {
            return Err(GatewayError::Auth(
                "Authentication required for non-loopback connections".to_string(),
            ));
        }

        // Non-loopback without auth configured: limited read-only access
        warn!("Unauthenticated connection on non-loopback bind — granting read-only access");
        Ok(AuthContext {
            client_id: "anonymous".to_string(),
            scopes: [Scope::Read].into_iter().collect(),
            identity: None,
            authenticated_at: chrono::Utc::now(),
        })
    }

    /// Validate the WebSocket Origin header.
    fn validate_origin(&self, headers: &HeaderMap) -> bool {
        // Loopback: any origin is fine
        if self.config.bind == BindMode::Loopback {
            return true;
        }

        let origin = match headers.get("origin").and_then(|v| v.to_str().ok()) {
            Some(o) => o,
            None => return true, // No origin header (non-browser client)
        };

        // Check against allowed origins (prefix match to handle ports)
        for allowed in ALLOWED_ORIGINS {
            if origin.starts_with(allowed) {
                return true;
            }
        }

        warn!("Rejected WebSocket connection from untrusted origin: {}", origin);
        false
    }
}

/// Information about a connected client.
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// Client ID.
    pub id: String,

    /// Connection time.
    pub connected_at: chrono::DateTime<chrono::Utc>,

    /// Remote address.
    pub remote_addr: Option<SocketAddr>,

    /// Authentication context.
    pub auth: AuthContext,

    /// Message counter for per-client rate limiting.
    pub message_count: Arc<AtomicU64>,

    /// Last message rate reset (unix seconds).
    pub message_rate_reset: Arc<AtomicU64>,
}

/// The WebSocket gateway server.
pub struct Gateway {
    /// Server state.
    state: Arc<GatewayState>,
}

impl Gateway {
    /// Create a new gateway.
    pub fn new(config: GatewayConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);

        let state = Arc::new(GatewayState {
            methods: Arc::new(MethodRegistry::new()),
            clients: RwLock::new(HashMap::new()),
            broadcast_tx,
            config,
            connection_count: AtomicU64::new(0),
            rate_limit_reset: AtomicU64::new(0),
        });

        Self { state }
    }

    /// Create a new gateway with default handlers registered.
    pub async fn with_default_handlers(config: GatewayConfig) -> Self {
        let gateway = Self::new(config);

        // Create handler context with default config
        let context = crate::handlers::HandlerContext::new()
            .with_config(Arc::new(RwLock::new(serde_json::json!({}))));

        // Register all handlers
        crate::handlers::register_all(&gateway.state.methods, context).await;

        gateway
    }

    /// Create a new gateway with a model provider and default handlers.
    pub async fn with_provider(
        config: GatewayConfig,
        provider: std::sync::Arc<dyn smartassist_providers::Provider>,
    ) -> Self {
        let gateway = Self::new(config);

        // Create handler context with provider
        let context = crate::handlers::HandlerContext::new()
            .with_config(Arc::new(RwLock::new(serde_json::json!({}))))
            .with_provider(provider);

        // Register all handlers
        crate::handlers::register_all(&gateway.state.methods, context).await;

        gateway
    }

    /// Get the method registry for registering handlers.
    pub fn methods(&self) -> &Arc<MethodRegistry> {
        &self.state.methods
    }

    /// Run the gateway server.
    pub async fn run(&self) -> Result<()> {
        let addr = self.bind_address();

        // Security warning for non-loopback binds
        if self.state.config.bind != BindMode::Loopback {
            warn!("========================================");
            warn!("  SECURITY WARNING: Gateway binding to {}", addr);
            warn!("  The gateway is accessible from the network.");
            if self.state.config.auth_token.is_none() {
                warn!("  No auth token configured — connections will have limited access.");
                warn!("  Set --auth-token or SMARTASSIST_AUTH_TOKEN for full security.");
            }
            warn!("========================================");
        }

        let app = self.create_router();

        info!("Starting gateway server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(GatewayError::Io)?;

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| GatewayError::Internal(e.to_string()))?;

        Ok(())
    }

    /// Create the Axum router.
    fn create_router(&self) -> Router {
        let state = self.state.clone();

        let mut router = Router::new()
            .route("/ws", get(ws_handler))
            .route("/health", get(health_handler))
            .with_state(state);

        if self.state.config.cors {
            router = router.layer(Self::create_cors_layer(&self.state.config));
        }

        router
    }

    /// Create a strict CORS layer instead of permissive.
    fn create_cors_layer(config: &GatewayConfig) -> CorsLayer {
        if config.bind == BindMode::Loopback {
            // Loopback: allow localhost origins with any port
            let origins: Vec<HeaderValue> = ALLOWED_ORIGINS
                .iter()
                .filter(|o| o.contains("localhost") || o.contains("127.0.0.1"))
                .filter_map(|o| HeaderValue::from_str(o).ok())
                .collect();

            CorsLayer::new()
                .allow_origin(AllowOrigin::list(origins))
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([
                    "content-type".parse().unwrap(),
                    "authorization".parse().unwrap(),
                ])
                .max_age(std::time::Duration::from_secs(3600))
        } else {
            // Non-loopback: strict origin allowlist
            let origins: Vec<HeaderValue> = ALLOWED_ORIGINS
                .iter()
                .filter_map(|o| HeaderValue::from_str(o).ok())
                .collect();

            CorsLayer::new()
                .allow_origin(AllowOrigin::list(origins))
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([
                    "content-type".parse().unwrap(),
                    "authorization".parse().unwrap(),
                ])
                .max_age(std::time::Duration::from_secs(3600))
        }
    }

    /// Get the bind address.
    fn bind_address(&self) -> SocketAddr {
        let ip = match self.state.config.bind {
            BindMode::Loopback => [127, 0, 0, 1],
            BindMode::Lan | BindMode::Tailnet | BindMode::Auto => [0, 0, 0, 0],
        };

        SocketAddr::from((ip, self.state.config.port))
    }

    /// Broadcast a notification to all clients.
    pub fn broadcast(&self, message: &str) {
        let _ = self.state.broadcast_tx.send(message.to_string());
    }

    /// Get connected client count.
    pub async fn client_count(&self) -> usize {
        self.state.clients.read().await.len()
    }
}

/// WebSocket upgrade handler with authentication and origin validation.
async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<Arc<GatewayState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> std::result::Result<impl IntoResponse, axum::http::StatusCode> {
    // Rate limit check
    if !state.check_rate_limit() {
        warn!("Rate limit exceeded for connection from {}", addr);
        return Err(axum::http::StatusCode::TOO_MANY_REQUESTS);
    }

    // Max connections check
    let client_count = state.clients.read().await.len();
    if client_count >= state.config.max_connections {
        warn!("Max connections ({}) reached, rejecting {}", state.config.max_connections, addr);
        return Err(axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    // Origin validation (prevents CVE-2026-25253 - 1-click RCE via cross-origin token theft)
    if !state.validate_origin(&headers) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    // Authentication
    let auth = match state.authenticate(&headers) {
        Ok(ctx) => ctx,
        Err(e) => {
            warn!("Authentication failed from {}: {}", addr, e);
            return Err(axum::http::StatusCode::UNAUTHORIZED);
        }
    };

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, auth, addr)))
}

/// Handle a WebSocket connection.
async fn handle_socket(
    socket: WebSocket,
    state: Arc<GatewayState>,
    auth: AuthContext,
    remote_addr: SocketAddr,
) {
    let client_id = uuid::Uuid::new_v4().to_string();
    let message_count = Arc::new(AtomicU64::new(0));
    let message_rate_reset = Arc::new(AtomicU64::new(0));

    // Register client with auth context
    {
        let mut clients = state.clients.write().await;
        clients.insert(
            client_id.clone(),
            ClientInfo {
                id: client_id.clone(),
                connected_at: chrono::Utc::now(),
                remote_addr: Some(remote_addr),
                auth: auth.clone(),
                message_count: message_count.clone(),
                message_rate_reset: message_rate_reset.clone(),
            },
        );
    }

    info!(
        "Client connected: {} from {} (scopes: {:?})",
        client_id, remote_addr, auth.scopes
    );

    let (mut sender, mut receiver) = socket.split();
    let _broadcast_rx = state.broadcast_tx.subscribe();

    // Handle incoming messages
    let state_clone = state.clone();
    let client_id_clone = client_id.clone();
    let auth_clone = auth.clone();
    let msg_count = message_count.clone();
    let msg_reset = message_rate_reset.clone();

    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Per-client message rate limiting
                    if !check_message_rate(&msg_count, &msg_reset) {
                        let err_resp = JsonRpcResponse::error(
                            None,
                            JsonRpcError::new(-32000, "Rate limit exceeded".to_string()),
                        );
                        let err_str = serde_json::to_string(&err_resp).unwrap_or_default();
                        if sender.send(Message::Text(err_str)).await.is_err() {
                            break;
                        }
                        continue;
                    }

                    let response =
                        handle_message(&text, &state_clone, &auth_clone).await;
                    if let Err(e) = sender.send(Message::Text(response)).await {
                        error!("Failed to send response: {}", e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    debug!("Client {} closed connection", client_id_clone);
                    break;
                }
                Err(e) => {
                    warn!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for task to complete
    let _ = recv_task.await;

    // Unregister client
    {
        let mut clients = state.clients.write().await;
        clients.remove(&client_id);
    }

    info!("Client disconnected: {}", client_id);
}

/// Check per-client message rate limit.
fn check_message_rate(count: &AtomicU64, reset: &AtomicU64) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let last_reset = reset.load(Ordering::Relaxed);
    if now > last_reset {
        reset.store(now, Ordering::Relaxed);
        count.store(1, Ordering::Relaxed);
        true
    } else {
        let c = count.fetch_add(1, Ordering::Relaxed);
        c < MAX_MESSAGES_PER_SECOND
    }
}

/// Handle a JSON-RPC message with scope-based authorization.
async fn handle_message(text: &str, state: &GatewayState, auth: &AuthContext) -> String {
    // Parse request
    let request: JsonRpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            let response = JsonRpcResponse::error(
                None,
                JsonRpcError::parse_error(e.to_string()),
            );
            return serde_json::to_string(&response).unwrap_or_default();
        }
    };

    debug!("Received RPC request: {} (client: {})", request.method, auth.client_id);

    // Check authorization based on method name
    if let Some(required_scope) = required_scope_for_method(&request.method) {
        if !auth.has_scope(required_scope) {
            let response = JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(
                    -32001,
                    format!(
                        "Insufficient permissions: method '{}' requires scope '{:?}'",
                        request.method, required_scope
                    ),
                ),
            );
            return serde_json::to_string(&response).unwrap_or_default();
        }
    }

    // Dispatch to method handler
    let result = state.methods.call(&request.method, request.params.clone()).await;

    let response = match result {
        Ok(value) => JsonRpcResponse::success(request.id, value),
        Err(e) => JsonRpcResponse::error(
            request.id,
            JsonRpcError::new(e.code(), e.to_string()),
        ),
    };

    serde_json::to_string(&response).unwrap_or_default()
}

/// Determine the required scope for an RPC method.
fn required_scope_for_method(method: &str) -> Option<Scope> {
    // Read-only methods
    if method.starts_with("status.")
        || method.starts_with("channels.status")
        || method == "ping"
        || method == "system.info"
        || method == "system.methods"
    {
        return Some(Scope::Read);
    }

    // Write methods (chat, agent, config changes)
    if method.starts_with("chat.")
        || method.starts_with("agent.")
        || method.starts_with("sessions.")
        || method.starts_with("message.")
        || method.starts_with("config.")
    {
        return Some(Scope::Write);
    }

    // Execution approval methods
    if method.starts_with("exec.") {
        return Some(Scope::Approvals);
    }

    // Pairing methods
    if method.starts_with("node.pair")
        || method.starts_with("device.")
    {
        return Some(Scope::Pairing);
    }

    // Admin methods (gateway management, restart, etc.)
    if method.starts_with("gateway.")
        || method.starts_with("node.invoke")
        || method.starts_with("node.unpair")
    {
        return Some(Scope::Admin);
    }

    // Unknown methods default to requiring Admin
    Some(Scope::Admin)
}

/// Health check handler.
async fn health_handler(State(state): State<Arc<GatewayState>>) -> impl IntoResponse {
    let clients = state.clients.read().await.len();
    serde_json::json!({
        "status": "ok",
        "clients": clients,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_config_default() {
        let config = GatewayConfig::default();
        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.bind, BindMode::Loopback);
        assert!(!config.require_auth);
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn test_required_scope_read_methods() {
        assert_eq!(required_scope_for_method("ping"), Some(Scope::Read));
        assert_eq!(required_scope_for_method("system.info"), Some(Scope::Read));
        assert_eq!(required_scope_for_method("status.get"), Some(Scope::Read));
    }

    #[test]
    fn test_required_scope_write_methods() {
        assert_eq!(required_scope_for_method("chat.send"), Some(Scope::Write));
        assert_eq!(required_scope_for_method("agent.run"), Some(Scope::Write));
        assert_eq!(required_scope_for_method("message.send"), Some(Scope::Write));
    }

    #[test]
    fn test_required_scope_admin_methods() {
        assert_eq!(required_scope_for_method("gateway.restart"), Some(Scope::Admin));
        assert_eq!(required_scope_for_method("node.invoke"), Some(Scope::Admin));
    }

    #[test]
    fn test_required_scope_exec_methods() {
        assert_eq!(required_scope_for_method("exec.approval.request"), Some(Scope::Approvals));
    }

    #[test]
    fn test_origin_validation_loopback() {
        let state = GatewayState {
            methods: Arc::new(MethodRegistry::new()),
            clients: RwLock::new(HashMap::new()),
            broadcast_tx: broadcast::channel(10).0,
            config: GatewayConfig::default(), // Loopback
            connection_count: AtomicU64::new(0),
            rate_limit_reset: AtomicU64::new(0),
        };
        let headers = HeaderMap::new();
        assert!(state.validate_origin(&headers));
    }

    #[test]
    fn test_origin_validation_rejects_unknown() {
        let state = GatewayState {
            methods: Arc::new(MethodRegistry::new()),
            clients: RwLock::new(HashMap::new()),
            broadcast_tx: broadcast::channel(10).0,
            config: GatewayConfig {
                bind: BindMode::Lan,
                ..Default::default()
            },
            connection_count: AtomicU64::new(0),
            rate_limit_reset: AtomicU64::new(0),
        };
        let mut headers = HeaderMap::new();
        headers.insert("origin", "https://evil.com".parse().unwrap());
        assert!(!state.validate_origin(&headers));
    }

    #[test]
    fn test_origin_validation_allows_localhost() {
        let state = GatewayState {
            methods: Arc::new(MethodRegistry::new()),
            clients: RwLock::new(HashMap::new()),
            broadcast_tx: broadcast::channel(10).0,
            config: GatewayConfig {
                bind: BindMode::Lan,
                ..Default::default()
            },
            connection_count: AtomicU64::new(0),
            rate_limit_reset: AtomicU64::new(0),
        };
        let mut headers = HeaderMap::new();
        headers.insert("origin", "http://localhost:3000".parse().unwrap());
        assert!(state.validate_origin(&headers));
    }

    #[test]
    fn test_auth_loopback_implicit_trust() {
        let state = GatewayState {
            methods: Arc::new(MethodRegistry::new()),
            clients: RwLock::new(HashMap::new()),
            broadcast_tx: broadcast::channel(10).0,
            config: GatewayConfig::default(), // Loopback
            connection_count: AtomicU64::new(0),
            rate_limit_reset: AtomicU64::new(0),
        };
        let headers = HeaderMap::new();
        let auth = state.authenticate(&headers).unwrap();
        assert!(auth.has_scope(Scope::Admin));
    }

    #[test]
    fn test_auth_non_loopback_requires_token() {
        let state = GatewayState {
            methods: Arc::new(MethodRegistry::new()),
            clients: RwLock::new(HashMap::new()),
            broadcast_tx: broadcast::channel(10).0,
            config: GatewayConfig {
                bind: BindMode::Lan,
                require_auth: true,
                auth_token: Some("secret".to_string()),
                ..Default::default()
            },
            connection_count: AtomicU64::new(0),
            rate_limit_reset: AtomicU64::new(0),
        };
        // No auth header → rejected
        let headers = HeaderMap::new();
        assert!(state.authenticate(&headers).is_err());

        // Wrong token → rejected
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer wrong".parse().unwrap());
        assert!(state.authenticate(&headers).is_err());

        // Correct token → OK
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer secret".parse().unwrap());
        let auth = state.authenticate(&headers).unwrap();
        assert!(auth.has_scope(Scope::Admin));
    }
}
