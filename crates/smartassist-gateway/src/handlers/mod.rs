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

use crate::methods::MethodRegistry;
use smartassist_providers::Provider;
use std::sync::Arc;

pub use agent::{AgentHandler, AgentStreamHandler};
pub use chat::{ChatAbortHandler, ChatHandler, ChatHistoryHandler};
pub use config::{ConfigGetHandler, ConfigPatchHandler, ConfigSchemaHandler, ConfigSetHandler};
pub use cron::{
    CronAddHandler, CronListHandler, CronRemoveHandler, CronRunHandler, CronRunsHandler,
    CronScheduler, CronStatusHandler, CronUpdateHandler, WakeHandler,
};
pub use device::{
    DevicePairApproveHandler, DevicePairListHandler, DevicePairRejectHandler,
    DeviceTokenRevokeHandler, DeviceTokenRotateHandler,
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
    SessionsDeleteHandler, SessionsListHandler, SessionsPatchHandler, SessionsResolveHandler,
};
pub use skills::{SkillsBinsHandler, SkillsInstallHandler, SkillsStatusHandler, SkillsUpdateHandler};
pub use system::{
    LastHeartbeatHandler, LogsTailHandler, SetHeartbeatsHandler, SystemEventHandler,
    SystemPresenceHandler,
};
pub use wizard::{WizardCancelHandler, WizardNextHandler, WizardStartHandler, WizardStatusHandler};

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
        .register("sessions.resolve", Arc::new(SessionsResolveHandler::new(ctx.clone())))
        .await;
    registry
        .register("sessions.patch", Arc::new(SessionsPatchHandler::new(ctx.clone())))
        .await;
    registry
        .register("sessions.delete", Arc::new(SessionsDeleteHandler::new(ctx.clone())))
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
        .register("device.pair.list", Arc::new(DevicePairListHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.pair.approve", Arc::new(DevicePairApproveHandler::new(ctx.clone())))
        .await;
    registry
        .register("device.pair.reject", Arc::new(DevicePairRejectHandler::new(ctx.clone())))
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
    pub cron_scheduler: Arc<CronScheduler>,

    /// Path to config file for persistence.
    pub config_path: Option<std::path::PathBuf>,
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
            cron_scheduler: Arc::new(CronScheduler::new()),
            config_path: None,
        }
    }
}

/// Simplified session data for handlers.
#[derive(Clone, Debug, Default)]
pub struct SessionData {
    pub key: String,
    pub agent_id: Option<String>,
    pub status: String,
    pub messages: Vec<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
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
}
