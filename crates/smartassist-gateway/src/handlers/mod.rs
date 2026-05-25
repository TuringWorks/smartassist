//! RPC method handlers.
//!
//! This module contains implementations for all gateway RPC methods.

pub mod agent;
pub mod chat;
pub mod config;
pub mod cron;
pub mod device;
pub mod exec;
pub mod health;
pub mod models;
pub mod nodes;
pub mod send;
pub mod sessions;
pub mod skills;
pub mod system;
pub mod wizard;
pub mod browser;
pub mod canvas;
pub mod channel;
pub mod talk;
pub mod learnings;

use crate::methods::MethodRegistry;
use smartassist_agent::{CompressionEngine, CompressionConfig, GuardrailEngine, ImprovementEngine, ToolExecutor, ToolRegistry};
use smartassist_learnings::{LearningContextProvider, LearningStore};
use smartassist_providers::{CredentialPoolManager, Provider};
use std::sync::Arc;
use std::time::Duration;

pub use agent::{AgentHandler, AgentStreamHandler, AgentStopHandler, AgentStatusHandler};
pub use chat::{ChatAbortHandler, ChatHandler, ChatHistoryHandler};
pub use config::{ConfigDiffHandler, ConfigGetHandler, ConfigPatchHandler, ConfigReloadHandler, ConfigSchemaHandler, ConfigSetHandler};
pub use cron::{
    CronAddHandler, CronListHandler, CronRemoveHandler, CronRunHandler, CronRunsHandler,
    CronStatusHandler, CronUpdateHandler, WakeHandler,
};
pub use device::{
    DevicePairApproveHandler, DevicePairInitiateHandler, DevicePairListHandler,
    DevicePairRejectHandler, DeviceStatusHandler, DeviceTokenRevokeHandler,
    DeviceTokenRotateHandler, DeviceUnpairHandler,
};
pub use exec::{
    ApprovalQueue, ExecApprovalRequestHandler, ExecApprovalResolveHandler,
    ExecApprovalsGetHandler, ExecApprovalsNodeGetHandler, ExecApprovalsNodeSetHandler,
    ExecApprovalsSetHandler,
};
pub use health::{HealthHandler, StatusHandler};
pub use models::ModelsListHandler;
pub use nodes::{
    NodeDescribeHandler, NodeInvokeHandler, NodeListHandler, NodePairApproveHandler,
    NodePairRejectHandler, NodePairRequestHandler, NodeRenameHandler, NodeUnpairHandler,
};
pub use send::{SendMessageHandler, SendPollHandler};
pub use sessions::{
    SessionsCreateHandler, SessionsDeleteHandler, SessionsHistoryHandler, SessionsListHandler,
    SessionsPatchHandler, SessionsResolveHandler,
};
pub use skills::{SkillsBinsHandler, SkillsInstallHandler, SkillsStatusHandler, SkillsUpdateHandler};
pub use system::{
    LastHeartbeatHandler, LogsTailHandler, SetHeartbeatsHandler, SystemEventHandler,
    SystemPresenceHandler,
};
pub use wizard::{WizardCancelHandler, WizardNextHandler, WizardStartHandler, WizardStatusHandler};
pub use browser::{BrowserLaunchHandler, BrowserCloseHandler, BrowserExecuteHandler, BrowserListHandler};
pub use canvas::{CanvasCreateHandler, CanvasDeleteHandler, CanvasExecuteHandler, CanvasListHandler, CanvasSubscribeHandler};
pub use channel::{ChannelListHandler, ChannelHealthHandler};
pub use talk::{
    TalkAudioHandler, TalkListHandler, TalkPttPressHandler, TalkPttReleaseHandler,
    TalkStartHandler, TalkStatusHandler, TalkStopHandler,
};
pub use learnings::{
    LearningsDeleteHandler, LearningsGetHandler, LearningsIngestHandler,
    LearningsListHandler, LearningsSearchHandler,
};

/// Register all built-in method handlers.
pub async fn register_all(registry: &MethodRegistry, context: HandlerContext) {
    let ctx = Arc::new(context);

    // Chat methods
    registry
        .register("chat", Arc::new(ChatHandler::new(ctx.clone())))
        .await;
    registry
        .register("chat.history", Arc::new(ChatHistoryHandler::new(ctx.clone())))
        .await;
    registry
        .register("chat.abort", Arc::new(ChatAbortHandler::new(ctx.clone())))
        .await;

    // Session methods
    registry
        .register("sessions.list", Arc::new(SessionsListHandler::new(ctx.clone())))
        .await;
    registry
        .register("sessions.create", Arc::new(SessionsCreateHandler::new(ctx.clone())))
        .await;
    registry
        .register("sessions.resolve", Arc::new(SessionsResolveHandler::new(ctx.clone())))
        .await;
    registry
        .register("sessions.patch", Arc::new(SessionsPatchHandler::new(ctx.clone())))
        .await;
    registry
        .register("sessions.delete", Arc::new(SessionsDeleteHandler::new(ctx.clone())))
        .await;
    registry
        .register("sessions.history", Arc::new(SessionsHistoryHandler::new(ctx.clone())))
        .await;

    // Health methods
    registry
        .register("health", Arc::new(HealthHandler::new(ctx.clone())))
        .await;
    registry
        .register("status", Arc::new(StatusHandler::new(ctx.clone())))
        .await;

    // Models methods
    registry
        .register("models.list", Arc::new(ModelsListHandler::new(ctx.clone())))
        .await;

    // Config methods
    registry
        .register("config.get", Arc::new(ConfigGetHandler::new(ctx.clone())))
        .await;
    registry
        .register("config.set", Arc::new(ConfigSetHandler::new(ctx.clone())))
        .await;
    registry
        .register("config.patch", Arc::new(ConfigPatchHandler::new(ctx.clone())))
        .await;
    registry
        .register("config.schema", Arc::new(ConfigSchemaHandler::new(ctx.clone())))
        .await;
    registry
        .register("config.reload", Arc::new(ConfigReloadHandler::new(ctx.clone())))
        .await;
    registry
        .register("config.diff", Arc::new(ConfigDiffHandler::new(ctx.clone())))
        .await;

    // Node methods
    registry
        .register("node.list", Arc::new(NodeListHandler::new(ctx.clone())))
        .await;
    registry
        .register("node.describe", Arc::new(NodeDescribeHandler::new(ctx.clone())))
        .await;
    registry
        .register("node.pair.request", Arc::new(NodePairRequestHandler::new(ctx.clone())))
        .await;
    registry
        .register("node.pair.approve", Arc::new(NodePairApproveHandler::new(ctx.clone())))
        .await;
    registry
        .register("node.pair.reject", Arc::new(NodePairRejectHandler::new(ctx.clone())))
        .await;
    registry
        .register("node.unpair", Arc::new(NodeUnpairHandler::new(ctx.clone())))
        .await;
    registry
        .register("node.rename", Arc::new(NodeRenameHandler::new(ctx.clone())))
        .await;
    registry
        .register("node.invoke", Arc::new(NodeInvokeHandler::new(ctx.clone())))
        .await;

    // Cron methods
    registry
        .register("cron.list", Arc::new(CronListHandler::new(ctx.clone())))
        .await;
    registry
        .register("cron.status", Arc::new(CronStatusHandler::new(ctx.clone())))
        .await;
    registry
        .register("cron.add", Arc::new(CronAddHandler::new(ctx.clone())))
        .await;
    registry
        .register("cron.update", Arc::new(CronUpdateHandler::new(ctx.clone())))
        .await;
    registry
        .register("cron.remove", Arc::new(CronRemoveHandler::new(ctx.clone())))
        .await;
    registry
        .register("cron.run", Arc::new(CronRunHandler::new(ctx.clone())))
        .await;
    registry
        .register("cron.runs", Arc::new(CronRunsHandler::new(ctx.clone())))
        .await;
    registry
        .register("wake", Arc::new(WakeHandler::new(ctx.clone())))
        .await;

    // Device methods
    registry
        .register("device.pair", Arc::new(DevicePairInitiateHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.pair.list", Arc::new(DevicePairListHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.pair.approve", Arc::new(DevicePairApproveHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.pair.reject", Arc::new(DevicePairRejectHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.unpair", Arc::new(DeviceUnpairHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.status", Arc::new(DeviceStatusHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.token.rotate", Arc::new(DeviceTokenRotateHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.token.revoke", Arc::new(DeviceTokenRevokeHandler::new(ctx.clone())))
        .await;

    // Exec approval methods
    registry
        .register("exec.approvals.get", Arc::new(ExecApprovalsGetHandler::new(ctx.clone())))
        .await;
    registry
        .register("exec.approvals.set", Arc::new(ExecApprovalsSetHandler::new(ctx.clone())))
        .await;
    registry
        .register("exec.approvals.node.get", Arc::new(ExecApprovalsNodeGetHandler::new(ctx.clone())))
        .await;
    registry
        .register("exec.approvals.node.set", Arc::new(ExecApprovalsNodeSetHandler::new(ctx.clone())))
        .await;
    registry
        .register("exec.approval.request", Arc::new(ExecApprovalRequestHandler::new(ctx.clone())))
        .await;
    registry
        .register("exec.approval.resolve", Arc::new(ExecApprovalResolveHandler::new(ctx.clone())))
        .await;

    // Send methods
    registry
        .register("send", Arc::new(SendMessageHandler::new(ctx.clone())))
        .await;
    registry
        .register("send.poll", Arc::new(SendPollHandler::new(ctx.clone())))
        .await;

    // Channel methods
    registry
        .register("channel.list", Arc::new(ChannelListHandler::new(ctx.clone())))
        .await;
    registry
        .register("channel.health", Arc::new(ChannelHealthHandler::new(ctx.clone())))
        .await;

    // System methods
    registry
        .register("system-presence", Arc::new(SystemPresenceHandler::new(ctx.clone())))
        .await;
    registry
        .register("system-event", Arc::new(SystemEventHandler::new(ctx.clone())))
        .await;
    registry
        .register("last-heartbeat", Arc::new(LastHeartbeatHandler::new(ctx.clone())))
        .await;
    registry
        .register("set-heartbeats", Arc::new(SetHeartbeatsHandler::new(ctx.clone())))
        .await;
    registry
        .register("logs.tail", Arc::new(LogsTailHandler::new(ctx.clone())))
        .await;

    // Agent methods
    registry
        .register("agent", Arc::new(AgentHandler::new(ctx.clone())))
        .await;
    registry
        .register("agent.stream", Arc::new(AgentStreamHandler::new(ctx.clone())))
        .await;
    registry
        .register("agent.stop", Arc::new(AgentStopHandler::new(ctx.clone())))
        .await;
    registry
        .register("agent.status", Arc::new(AgentStatusHandler::new(ctx.clone())))
        .await;

    // Skills methods
    registry
        .register("skills.status", Arc::new(SkillsStatusHandler::new(ctx.clone())))
        .await;
    registry
        .register("skills.bins", Arc::new(SkillsBinsHandler::new(ctx.clone())))
        .await;
    registry
        .register("skills.install", Arc::new(SkillsInstallHandler::new(ctx.clone())))
        .await;
    registry
        .register("skills.update", Arc::new(SkillsUpdateHandler::new(ctx.clone())))
        .await;

    // Wizard methods
    registry
        .register("wizard.start", Arc::new(WizardStartHandler::new(ctx.clone())))
        .await;
    registry
        .register("wizard.next", Arc::new(WizardNextHandler::new(ctx.clone())))
        .await;
    registry
        .register("wizard.cancel", Arc::new(WizardCancelHandler::new(ctx.clone())))
        .await;
    registry
        .register("wizard.status", Arc::new(WizardStatusHandler::new(ctx.clone())))
        .await;

    // Browser methods
    registry
        .register("browser.launch", Arc::new(BrowserLaunchHandler::new(ctx.clone())))
        .await;
    registry
        .register("browser.close", Arc::new(BrowserCloseHandler::new(ctx.clone())))
        .await;
    registry
        .register("browser.execute", Arc::new(BrowserExecuteHandler::new(ctx.clone())))
        .await;
    registry
        .register("browser.list", Arc::new(BrowserListHandler::new(ctx.clone())))
        .await;

    // Canvas methods
    registry
        .register("canvas.create", Arc::new(CanvasCreateHandler::new(ctx.clone())))
        .await;
    registry
        .register("canvas.delete", Arc::new(CanvasDeleteHandler::new(ctx.clone())))
        .await;
    registry
        .register("canvas.execute", Arc::new(CanvasExecuteHandler::new(ctx.clone())))
        .await;
    registry
        .register("canvas.list", Arc::new(CanvasListHandler::new(ctx.clone())))
        .await;
    registry
        .register("canvas.subscribe", Arc::new(CanvasSubscribeHandler::new(ctx.clone())))
        .await;

    // Talk methods
    registry
        .register("talk.start", Arc::new(TalkStartHandler::new(ctx.clone())))
        .await;
    registry
        .register("talk.stop", Arc::new(TalkStopHandler::new(ctx.clone())))
        .await;
    registry
        .register("talk.ptt_press", Arc::new(TalkPttPressHandler::new(ctx.clone())))
        .await;
    registry
        .register("talk.ptt_release", Arc::new(TalkPttReleaseHandler::new(ctx.clone())))
        .await;
    registry
        .register("talk.status", Arc::new(TalkStatusHandler::new(ctx.clone())))
        .await;
    registry
        .register("talk.list", Arc::new(TalkListHandler::new(ctx.clone())))
        .await;
    registry
        .register("talk.audio", Arc::new(TalkAudioHandler::new(ctx.clone())))
        .await;

    // Learnings methods
    registry
        .register("learnings.list", Arc::new(LearningsListHandler::new(ctx.clone())))
        .await;
    registry
        .register("learnings.get", Arc::new(LearningsGetHandler::new(ctx.clone())))
        .await;
    registry
        .register("learnings.ingest", Arc::new(LearningsIngestHandler::new(ctx.clone())))
        .await;
    registry
        .register("learnings.search", Arc::new(LearningsSearchHandler::new(ctx.clone())))
        .await;
    registry
        .register("learnings.delete", Arc::new(LearningsDeleteHandler::new(ctx.clone())))
        .await;
}

/// Simplified session data for handlers.
#[derive(Clone, Debug, Default)]
pub struct SessionData {
    pub key: String,
    pub agent_id: Option<String>,
    pub status: String,
    pub messages: Vec<smartassist_core::types::Message>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// Simplified node data for handlers.
#[derive(Clone, Debug, Default)]
pub struct NodeData {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub paired: bool,
    pub online: bool,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

/// Simplified device data for handlers.
#[derive(Clone, Debug, Default)]
pub struct DeviceData {
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub paired_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
    pub connected: bool,
}

/// Simplified skill data for handlers.
#[derive(Clone, Debug)]
pub struct SkillData {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub builtin: bool,
    pub path: Option<String>,
}

impl Default for SkillData {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: "1.0.0".to_string(),
            description: None,
            enabled: true,
            builtin: false,
            path: None,
        }
    }
}

/// Shared context for method handlers.
#[derive(Clone)]
pub struct HandlerContext {
    /// Configuration.
    pub config: Option<Arc<tokio::sync::RwLock<serde_json::Value>>>,

    /// Active sessions (simplified in-memory storage for now).
    pub sessions: Arc<tokio::sync::RwLock<std::collections::HashMap<String, SessionData>>>,

    /// Active channels count.
    pub active_channels: Arc<std::sync::atomic::AtomicUsize>,

    /// Model provider (optional, for chat completions).
    pub provider: Option<Arc<dyn Provider>>,

    /// Default model to use.
    pub default_model: String,

    /// Approval queue for exec approval requests.
    pub approval_queue: Arc<ApprovalQueue>,

    /// Cron job scheduler.
    pub cron_scheduler: Arc<smartassist_cron::Scheduler>,

    /// Path to config file for persistence.
    pub config_path: Option<std::path::PathBuf>,

    /// Tool registry for agent tool execution.
    pub tool_registry: Option<Arc<ToolRegistry>>,

    /// Tool executor for running tools.
    pub tool_executor: Option<Arc<ToolExecutor>>,

    /// Registered nodes (simplified in-memory storage).
    pub nodes: Arc<tokio::sync::RwLock<std::collections::HashMap<String, NodeData>>>,

    /// Paired devices (simplified in-memory storage).
    pub devices: Arc<tokio::sync::RwLock<std::collections::HashMap<String, DeviceData>>>,

    /// Installed skills (simplified in-memory storage).
    pub skills: Arc<tokio::sync::RwLock<std::collections::HashMap<String, SkillData>>>,

    /// Browser automation manager.
    pub browser_manager: Option<Arc<smartassist_browser::BrowserManager>>,

    /// Canvas workspace manager.
    pub canvas_manager: Option<Arc<smartassist_canvas::CanvasManager>>,

    /// Talk / voice runtime.
    pub talk_runtime: Option<Arc<smartassist_talk::TalkRuntime>>,

    /// Channel manager for sending/receiving messages.
    pub channel_manager: Option<Arc<smartassist_channels::ChannelManager>>,

    /// Context compression configuration for long conversations.
    pub compression_config: Option<CompressionConfig>,

    /// Persistent compression engine (created from compression_config).
    pub compression_engine: Option<Arc<CompressionEngine>>,

    /// Guardrail engine for tool execution safety.
    pub guardrail_engine: Option<Arc<GuardrailEngine>>,

    /// Improvement engine for self-improvement loop.
    pub improvement_engine: Option<Arc<ImprovementEngine>>,

    /// Credential pool manager for API key rotation.
    pub credential_pool: Option<Arc<CredentialPoolManager>>,

    /// Learning store for knowledge persistence.
    pub learning_store: Option<Arc<LearningStore>>,

    /// Learning context provider for injecting learnings into conversations.
    pub learning_context: Option<Arc<LearningContextProvider>>,
}

impl Default for HandlerContext {
    fn default() -> Self {
        Self {
            config: None,
            sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            active_channels: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            provider: None,
            default_model: "claude-sonnet-4-20250514".to_string(),
            approval_queue: Arc::new(ApprovalQueue::new()),
            cron_scheduler: Arc::new(smartassist_cron::Scheduler::new(
                Arc::new(smartassist_cron::MemoryJobStore::new()),
                Arc::new(smartassist_cron::LogExecutor),
                Duration::from_secs(60),
            )),
            config_path: None,
            tool_registry: None,
            tool_executor: None,
            nodes: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            devices: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            skills: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            browser_manager: None,
            canvas_manager: None,
            talk_runtime: None,
            channel_manager: None,
            compression_config: None,
            compression_engine: None,
            guardrail_engine: None,
            improvement_engine: None,
            credential_pool: None,
            learning_store: None,
            learning_context: None,
        }
    }
}

impl HandlerContext {
    /// Create a new handler context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the configuration.
    pub fn with_config(mut self, config: Arc<tokio::sync::RwLock<serde_json::Value>>) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the model provider.
    pub fn with_provider(mut self, provider: Arc<dyn Provider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Set the config file path for persistence.
    pub fn with_config_path(mut self, path: std::path::PathBuf) -> Self {
        self.config_path = Some(path);
        self
    }

    /// Set the tool registry and executor.
    pub fn with_tools(
        mut self,
        registry: Arc<ToolRegistry>,
        executor: Arc<ToolExecutor>,
    ) -> Self {
        self.tool_registry = Some(registry);
        self.tool_executor = Some(executor);
        self
    }

    /// Set the browser manager.
    pub fn with_browser_manager(mut self, manager: Arc<smartassist_browser::BrowserManager>) -> Self {
        self.browser_manager = Some(manager);
        self
    }

    /// Set the canvas manager.
    pub fn with_canvas_manager(mut self, manager: Arc<smartassist_canvas::CanvasManager>) -> Self {
        self.canvas_manager = Some(manager);
        self
    }

    /// Set the talk runtime.
    pub fn with_talk_runtime(mut self, runtime: Arc<smartassist_talk::TalkRuntime>) -> Self {
        self.talk_runtime = Some(runtime);
        self
    }

    /// Set the channel manager.
    pub fn with_channel_manager(mut self, manager: Arc<smartassist_channels::ChannelManager>) -> Self {
        self.channel_manager = Some(manager);
        self
    }

    /// Set the compression configuration and create a persistent engine.
    pub fn with_compression_config(mut self, config: CompressionConfig) -> Self {
        self.compression_engine = Some(Arc::new(CompressionEngine::new(config.clone())));
        self.compression_config = Some(config);
        self
    }

    /// Set the guardrail engine.
    pub fn with_guardrail_engine(mut self, engine: Arc<GuardrailEngine>) -> Self {
        self.guardrail_engine = Some(engine);
        self
    }

    /// Set the improvement engine.
    pub fn with_improvement_engine(mut self, engine: Arc<ImprovementEngine>) -> Self {
        self.improvement_engine = Some(engine);
        self
    }

    /// Set the credential pool manager.
    pub fn with_credential_pool(mut self, pool: Arc<CredentialPoolManager>) -> Self {
        self.credential_pool = Some(pool);
        self
    }

    /// Set the learning store.
    pub fn with_learning_store(mut self, store: Arc<LearningStore>) -> Self {
        self.learning_store = Some(store);
        self
    }

    /// Set the learning context provider.
    pub fn with_learning_context(mut self, provider: Arc<LearningContextProvider>) -> Self {
        self.learning_context = Some(provider);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_context_default() {
        let ctx = HandlerContext::new();
        assert!(ctx.provider.is_none());
        assert!(ctx.compression_config.is_none());
        assert!(ctx.compression_engine.is_none());
        assert!(ctx.guardrail_engine.is_none());
        assert!(ctx.improvement_engine.is_none());
        assert!(ctx.credential_pool.is_none());
        assert!(ctx.learning_store.is_none());
        assert!(ctx.learning_context.is_none());
        assert_eq!(ctx.default_model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_handler_context_with_credential_pool() {
        let pool = Arc::new(CredentialPoolManager::new());
        let ctx = HandlerContext::new().with_credential_pool(pool);
        assert!(ctx.credential_pool.is_some());
    }

    #[test]
    fn test_handler_context_with_compression_config() {
        let config = CompressionConfig::default();
        let ctx = HandlerContext::new().with_compression_config(config);
        assert!(ctx.compression_config.is_some());
        assert!(ctx.compression_engine.is_some());
    }

    #[test]
    fn test_handler_context_with_guardrail() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(registry));
        let guardrail = Arc::new(GuardrailEngine::with_defaults(executor));
        let ctx = HandlerContext::new().with_guardrail_engine(guardrail);
        assert!(ctx.guardrail_engine.is_some());
    }

    #[test]
    fn test_handler_context_with_improvement() {
        let improvement = Arc::new(ImprovementEngine::new());
        let ctx = HandlerContext::new().with_improvement_engine(improvement);
        assert!(ctx.improvement_engine.is_some());
    }

    #[test]
    fn test_handler_context_with_learning_store() {
        let dir = std::env::temp_dir().join("smartassist_test_learning_store");
        let store = Arc::new(LearningStore::with_dir(dir).unwrap());
        let ctx = HandlerContext::new().with_learning_store(store);
        assert!(ctx.learning_store.is_some());
    }
}
