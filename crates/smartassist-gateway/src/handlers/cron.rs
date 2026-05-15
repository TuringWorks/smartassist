//! Cron job RPC method handlers.
//!
//! Handles scheduling and management of cron jobs.
//! Delegates to the [`smartassist_cron`] crate for scheduling logic
//! and the [`smartassist_cron::Scheduler`] stored in [`HandlerContext`].

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::debug;

// ---------------------------------------------------------------------------
// CronJobInfo (wire type returned by list/status endpoints)
// ---------------------------------------------------------------------------

/// Cron job info returned in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobInfo {
    /// Job ID.
    pub id: String,
    /// Cron schedule expression.
    pub schedule: String,
    /// Job description.
    pub description: Option<String>,
    /// Agent ID to run.
    pub agent_id: String,
    /// Prompt to send.
    pub prompt: String,
    /// Enabled status.
    pub enabled: bool,
    /// Next run time.
    pub next_run: Option<String>,
    /// Last run time.
    pub last_run: Option<String>,
    /// Number of times this job has been triggered.
    pub run_count: u64,
}

/// Convert a [`smartassist_cron::Job`] to the wire-format [`CronJobInfo`].
fn job_to_info(job: &smartassist_cron::Job) -> CronJobInfo {
    CronJobInfo {
        id: job.id.clone(),
        schedule: job.schedule.clone(),
        description: job.description.clone(),
        agent_id: job.agent_id.clone(),
        prompt: job.prompt.clone(),
        enabled: job.enabled,
        next_run: job.compute_next_run().map(|t| t.to_rfc3339()),
        last_run: job.last_run.map(|t| t.to_rfc3339()),
        run_count: job.run_count,
    }
}

// ---------------------------------------------------------------------------
// CronListHandler
// ---------------------------------------------------------------------------

/// Cron list handler.
pub struct CronListHandler {
    context: Arc<HandlerContext>,
}

impl CronListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CronListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Cron list request");

        let jobs = self
            .context
            .cron_scheduler
            .store()
            .list()
            .await
            .map_err(|e| GatewayError::Internal(e.to_string()))?;
        let infos: Vec<CronJobInfo> = jobs.iter().map(|j| job_to_info(j)).collect();
        let count = infos.len();

        Ok(serde_json::json!({
            "jobs": infos,
            "count": count,
        }))
    }
}

// ---------------------------------------------------------------------------
// CronStatusHandler
// ---------------------------------------------------------------------------

/// Cron status handler.
pub struct CronStatusHandler {
    context: Arc<HandlerContext>,
}

impl CronStatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CronStatusHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Cron status request");

        let jobs = self
            .context
            .cron_scheduler
            .store()
            .list()
            .await
            .map_err(|e| GatewayError::Internal(e.to_string()))?;
        let job_count = jobs.len();

        // Find the earliest upcoming fire time across all enabled jobs.
        let next_job: Option<serde_json::Value> = jobs
            .iter()
            .filter(|j| j.enabled)
            .filter_map(|j| {
                j.compute_next_run().map(|t| {
                    serde_json::json!({
                        "id": j.id,
                        "next_run": t.to_rfc3339(),
                    })
                })
            })
            .min_by_key(|v| v["next_run"].as_str().unwrap_or("").to_string());

        Ok(serde_json::json!({
            "enabled": true,
            "job_count": job_count,
            "next_job": next_job,
        }))
    }
}

// ---------------------------------------------------------------------------
// CronAddHandler
// ---------------------------------------------------------------------------

/// Parameters for cron.add method.
#[derive(Debug, Deserialize)]
pub struct CronAddParams {
    /// Cron schedule expression.
    pub schedule: String,
    /// Job description.
    pub description: Option<String>,
    /// Agent ID to run.
    pub agent_id: String,
    /// Prompt to send.
    pub prompt: String,
    /// Whether to enable immediately.
    pub enabled: Option<bool>,
}

/// Cron add handler.
pub struct CronAddHandler {
    context: Arc<HandlerContext>,
}

impl CronAddHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CronAddHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronAddParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Cron add: schedule={}", params.schedule);

        let job = smartassist_cron::JobBuilder::new()
            .schedule(params.schedule.clone())
            .agent_id(params.agent_id.clone())
            .prompt(params.prompt.clone())
            .enabled(params.enabled.unwrap_or(true))
            .build()
            .map_err(|e| GatewayError::InvalidParams(e.to_string()))?;

        let job_id = job.id.clone();

        self.context
            .cron_scheduler
            .store()
            .add(&job)
            .await
            .map_err(|e| GatewayError::Internal(e.to_string()))?;

        Ok(serde_json::json!({
            "id": job_id,
            "schedule": params.schedule,
            "agent_id": params.agent_id,
            "enabled": job.enabled,
            "created": true,
        }))
    }
}

// ---------------------------------------------------------------------------
// CronUpdateHandler
// ---------------------------------------------------------------------------

/// Parameters for cron.update method.
#[derive(Debug, Deserialize)]
pub struct CronUpdateParams {
    /// Job ID.
    pub id: String,
    /// New cron schedule expression.
    pub schedule: Option<String>,
    /// New description.
    pub description: Option<String>,
    /// New prompt.
    pub prompt: Option<String>,
    /// Enable/disable.
    pub enabled: Option<bool>,
}

/// Cron update handler.
pub struct CronUpdateHandler {
    context: Arc<HandlerContext>,
}

impl CronUpdateHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CronUpdateHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronUpdateParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Cron update: id={}", params.id);

        let store = self.context.cron_scheduler.store();
        let mut job = store
            .get(&params.id)
            .await
            .map_err(|e| GatewayError::NotFound(e.to_string()))?
            .ok_or_else(|| GatewayError::NotFound(format!("Job not found: {}", params.id)))?;

        if let Some(s) = params.schedule {
            // Validate cron expression
            ::cron::Schedule::from_str(&s)
                .map_err(|e| GatewayError::InvalidParams(format!("Invalid cron expression: {}", e)))?;
            job.schedule = s;
        }
        if let Some(d) = params.description {
            job.description = Some(d);
        }
        if let Some(p) = params.prompt {
            job.prompt = p;
        }
        if let Some(e) = params.enabled {
            job.enabled = e;
        }

        store
            .update(&job)
            .await
            .map_err(|e| GatewayError::Internal(e.to_string()))?;

        Ok(serde_json::json!({
            "id": params.id,
            "updated": true,
        }))
    }
}

// ---------------------------------------------------------------------------
// CronRemoveHandler
// ---------------------------------------------------------------------------

/// Parameters for cron.remove method.
#[derive(Debug, Deserialize)]
pub struct CronRemoveParams {
    /// Job ID.
    pub id: String,
}

/// Cron remove handler.
pub struct CronRemoveHandler {
    context: Arc<HandlerContext>,
}

impl CronRemoveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CronRemoveHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronRemoveParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Cron remove: id={}", params.id);

        let removed = self
            .context
            .cron_scheduler
            .store()
            .remove(&params.id)
            .await
            .map_err(|e| GatewayError::NotFound(e.to_string()))?;

        if removed.is_none() {
            return Err(GatewayError::NotFound(format!(
                "Job not found: {}",
                params.id
            )));
        }

        Ok(serde_json::json!({
            "id": params.id,
            "removed": true,
        }))
    }
}

// ---------------------------------------------------------------------------
// CronRunHandler
// ---------------------------------------------------------------------------

/// Parameters for cron.run method.
#[derive(Debug, Deserialize)]
pub struct CronRunParams {
    /// Job ID.
    pub id: String,
}

/// Cron run handler (manual trigger).
pub struct CronRunHandler {
    context: Arc<HandlerContext>,
}

impl CronRunHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CronRunHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronRunParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Cron run: id={}", params.id);

        let store = self.context.cron_scheduler.store();
        let mut job = store
            .get(&params.id)
            .await
            .map_err(|e| GatewayError::NotFound(e.to_string()))?
            .ok_or_else(|| GatewayError::NotFound(format!("Job not found: {}", params.id)))?;

        job.last_run = Some(chrono::Utc::now());
        job.run_count += 1;

        store
            .update(&job)
            .await
            .map_err(|e| GatewayError::Internal(e.to_string()))?;

        let run_id = uuid::Uuid::new_v4().to_string();

        Ok(serde_json::json!({
            "job_id": params.id,
            "run_id": run_id,
            "triggered": true,
            "run_count": job.run_count,
            "last_run": job.last_run.map(|t| t.to_rfc3339()),
        }))
    }
}

// ---------------------------------------------------------------------------
// CronRunsHandler
// ---------------------------------------------------------------------------

/// Parameters for cron.runs method.
#[derive(Debug, Deserialize)]
pub struct CronRunsParams {
    /// Job ID (optional, all jobs if not specified).
    pub id: Option<String>,
    /// Maximum runs to return.
    pub limit: Option<usize>,
}

/// Cron runs handler (run history).
pub struct CronRunsHandler {
    _context: Arc<HandlerContext>,
}

impl CronRunsHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for CronRunsHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronRunsParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        debug!("Cron runs: id={:?}", params.id);

        // Run history is not yet persisted; return empty for now.
        Ok(serde_json::json!({
            "runs": [],
            "count": 0,
        }))
    }
}

impl Default for CronRunsParams {
    fn default() -> Self {
        Self {
            id: None,
            limit: Some(20),
        }
    }
}

// ---------------------------------------------------------------------------
// WakeHandler
// ---------------------------------------------------------------------------

/// Wake handler - send wake event.
pub struct WakeHandler {
    _context: Arc<HandlerContext>,
}

impl WakeHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for WakeHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Wake event");

        Ok(serde_json::json!({
            "woke": true,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

// ---------------------------------------------------------------------------
// TryFrom implementations
// ---------------------------------------------------------------------------

impl TryFrom<serde_json::Value> for CronAddParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CronUpdateParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CronRemoveParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CronRunParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> Arc<HandlerContext> {
        Arc::new(HandlerContext::new())
    }

    #[test]
    fn test_cron_job_info() {
        let job = smartassist_cron::JobBuilder::new()
            .schedule("0 * * * * *")
            .agent_id("agent-1")
            .prompt("Check status")
            .build()
            .unwrap();

        let info = job_to_info(&job);
        assert_eq!(info.schedule, "0 * * * * *");
        assert_eq!(info.agent_id, "agent-1");
        assert!(info.next_run.is_some());
    }

    #[tokio::test]
    async fn test_cron_list_handler() {
        let ctx = test_context();
        let handler = CronListHandler::new(ctx.clone());

        // Empty list initially
        let result = handler.call(None).await.unwrap();
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_cron_add_and_list() {
        let ctx = test_context();
        let add_handler = CronAddHandler::new(ctx.clone());
        let list_handler = CronListHandler::new(ctx.clone());

        let params = serde_json::json!({
            "schedule": "0 0 * * * *",
            "agent_id": "agent-1",
            "prompt": "hello",
        });

        let result = add_handler.call(Some(params)).await.unwrap();
        assert!(result["created"].as_bool().unwrap());
        let job_id = result["id"].as_str().unwrap();
        assert!(!job_id.is_empty());

        let list = list_handler.call(None).await.unwrap();
        assert_eq!(list["count"], 1);
    }

    #[tokio::test]
    async fn test_cron_add_invalid_schedule() {
        let ctx = test_context();
        let handler = CronAddHandler::new(ctx);

        let params = serde_json::json!({
            "schedule": "not valid",
            "agent_id": "agent-1",
            "prompt": "hello",
        });

        let result = handler.call(Some(params)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cron_remove() {
        let ctx = test_context();
        let add_handler = CronAddHandler::new(ctx.clone());
        let remove_handler = CronRemoveHandler::new(ctx.clone());

        let params = serde_json::json!({
            "schedule": "0 0 * * * *",
            "agent_id": "agent-1",
            "prompt": "hello",
        });

        let result = add_handler.call(Some(params)).await.unwrap();
        let job_id = result["id"].as_str().unwrap().to_string();

        let remove = remove_handler.call(Some(serde_json::json!({"id": job_id}))).await.unwrap();
        assert!(remove["removed"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_cron_run() {
        let ctx = test_context();
        let add_handler = CronAddHandler::new(ctx.clone());
        let run_handler = CronRunHandler::new(ctx.clone());

        let params = serde_json::json!({
            "schedule": "0 0 * * * *",
            "agent_id": "agent-1",
            "prompt": "hello",
        });

        let result = add_handler.call(Some(params)).await.unwrap();
        let job_id = result["id"].as_str().unwrap().to_string();

        let run = run_handler.call(Some(serde_json::json!({"id": job_id}))).await.unwrap();
        assert!(run["triggered"].as_bool().unwrap());
        assert_eq!(run["run_count"], 1);
        assert!(run["last_run"].is_string());
    }

    #[tokio::test]
    async fn test_cron_update() {
        let ctx = test_context();
        let add_handler = CronAddHandler::new(ctx.clone());
        let update_handler = CronUpdateHandler::new(ctx.clone());
        let list_handler = CronListHandler::new(ctx.clone());

        let params = serde_json::json!({
            "schedule": "0 0 * * * *",
            "agent_id": "agent-1",
            "prompt": "hello",
        });

        let result = add_handler.call(Some(params)).await.unwrap();
        let job_id = result["id"].as_str().unwrap().to_string();

        let update = update_handler
            .call(Some(serde_json::json!({
                "id": job_id,
                "prompt": "new prompt",
                "enabled": false,
            })))
            .await
            .unwrap();
        assert!(update["updated"].as_bool().unwrap());

        let list = list_handler.call(None).await.unwrap();
        let jobs = list["jobs"].as_array().unwrap();
        assert_eq!(jobs[0]["prompt"], "new prompt");
        assert_eq!(jobs[0]["enabled"], false);
    }

    #[tokio::test]
    async fn test_cron_status() {
        let ctx = test_context();
        let handler = CronStatusHandler::new(ctx);

        let result = handler.call(None).await.unwrap();
        assert_eq!(result["enabled"], true);
        assert_eq!(result["job_count"], 0);
    }

    #[tokio::test]
    async fn test_wake_handler() {
        let ctx = test_context();
        let handler = WakeHandler::new(ctx);

        let result = handler.call(None).await.unwrap();
        assert!(result["woke"].as_bool().unwrap());
        assert!(result["timestamp"].is_string());
    }
}
