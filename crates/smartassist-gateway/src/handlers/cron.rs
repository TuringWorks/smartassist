//! Cron job RPC method handlers.
//!
//! Handles scheduling and management of cron jobs.
//! Includes an in-memory [`CronScheduler`] that validates cron expressions
//! and tracks job metadata (last run, run count, next fire time).

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
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

// ---------------------------------------------------------------------------
// CronJob (internal scheduler state)
// ---------------------------------------------------------------------------

/// A scheduled cron job stored in the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub description: Option<String>,
    pub agent_id: String,
    pub prompt: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    pub run_count: u64,
}

impl CronJob {
    /// Convert to the wire-format [`CronJobInfo`], computing next_run from the
    /// cron expression.
    fn to_info(&self) -> CronJobInfo {
        CronJobInfo {
            id: self.id.clone(),
            schedule: self.schedule.clone(),
            description: self.description.clone(),
            agent_id: self.agent_id.clone(),
            prompt: self.prompt.clone(),
            enabled: self.enabled,
            next_run: CronScheduler::next_run(&self.schedule).map(|t| t.to_rfc3339()),
            last_run: self.last_run.map(|t| t.to_rfc3339()),
            run_count: self.run_count,
        }
    }
}

// ---------------------------------------------------------------------------
// CronScheduler
// ---------------------------------------------------------------------------

/// In-memory cron job scheduler.
pub struct CronScheduler {
    jobs: RwLock<HashMap<String, CronJob>>,
}

impl CronScheduler {
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
        }
    }

    /// Add a new job. Validates the cron expression before inserting.
    pub async fn add(&self, job: CronJob) -> std::result::Result<(), String> {
        Schedule::from_str(&job.schedule)
            .map_err(|e| format!("Invalid cron expression: {}", e))?;
        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id.clone(), job);
        Ok(())
    }

    /// Remove a job by ID.
    pub async fn remove(&self, id: &str) -> Option<CronJob> {
        let mut jobs = self.jobs.write().await;
        jobs.remove(id)
    }

    /// Update fields on an existing job. Only non-None fields are applied.
    pub async fn update(
        &self,
        id: &str,
        schedule: Option<String>,
        description: Option<String>,
        prompt: Option<String>,
        enabled: Option<bool>,
    ) -> std::result::Result<(), String> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| format!("Job not found: {}", id))?;

        if let Some(s) = schedule {
            Schedule::from_str(&s).map_err(|e| format!("Invalid cron expression: {}", e))?;
            job.schedule = s;
        }
        if let Some(d) = description {
            job.description = Some(d);
        }
        if let Some(p) = prompt {
            job.prompt = p;
        }
        if let Some(e) = enabled {
            job.enabled = e;
        }
        Ok(())
    }

    /// List all jobs.
    pub async fn list(&self) -> Vec<CronJob> {
        let jobs = self.jobs.read().await;
        jobs.values().cloned().collect()
    }

    /// Get a single job by ID.
    pub async fn get(&self, id: &str) -> Option<CronJob> {
        let jobs = self.jobs.read().await;
        jobs.get(id).cloned()
    }

    /// Record a manual trigger: update `last_run` and increment `run_count`.
    pub async fn record_run(&self, id: &str) -> std::result::Result<CronJob, String> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| format!("Job not found: {}", id))?;
        job.last_run = Some(chrono::Utc::now());
        job.run_count += 1;
        Ok(job.clone())
    }

    /// Compute the next run time for a given cron expression.
    pub fn next_run(schedule: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        Schedule::from_str(schedule)
            .ok()?
            .upcoming(chrono::Utc)
            .next()
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
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

        let jobs = self.context.cron_scheduler.list().await;
        let infos: Vec<CronJobInfo> = jobs.iter().map(|j| j.to_info()).collect();
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

        let jobs = self.context.cron_scheduler.list().await;
        let job_count = jobs.len();

        // Find the earliest upcoming fire time across all enabled jobs.
        let next_job: Option<serde_json::Value> = jobs
            .iter()
            .filter(|j| j.enabled)
            .filter_map(|j| {
                CronScheduler::next_run(&j.schedule).map(|t| {
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

        let job_id = uuid::Uuid::new_v4().to_string();
        let enabled = params.enabled.unwrap_or(true);

        let job = CronJob {
            id: job_id.clone(),
            schedule: params.schedule.clone(),
            description: params.description,
            agent_id: params.agent_id.clone(),
            prompt: params.prompt,
            enabled,
            created_at: chrono::Utc::now(),
            last_run: None,
            run_count: 0,
        };

        self.context
            .cron_scheduler
            .add(job)
            .await
            .map_err(|e| GatewayError::InvalidParams(e))?;

        Ok(serde_json::json!({
            "id": job_id,
            "schedule": params.schedule,
            "agent_id": params.agent_id,
            "enabled": enabled,
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

        self.context
            .cron_scheduler
            .update(
                &params.id,
                params.schedule,
                params.description,
                params.prompt,
                params.enabled,
            )
            .await
            .map_err(|e| GatewayError::NotFound(e))?;

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

        let removed = self.context.cron_scheduler.remove(&params.id).await;

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

        let job = self
            .context
            .cron_scheduler
            .record_run(&params.id)
            .await
            .map_err(|e| GatewayError::NotFound(e))?;

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

    #[test]
    fn test_cron_job_info() {
        let job = CronJobInfo {
            id: "job-1".to_string(),
            schedule: "0 * * * *".to_string(),
            description: Some("Hourly job".to_string()),
            agent_id: "agent-1".to_string(),
            prompt: "Check status".to_string(),
            enabled: true,
            next_run: None,
            last_run: None,
            run_count: 0,
        };

        let json = serde_json::to_value(&job).unwrap();
        assert_eq!(json["schedule"], "0 * * * *");
    }

    #[test]
    fn test_cron_next_run_valid() {
        // Standard 7-field cron: sec min hour day month weekday year
        let next = CronScheduler::next_run("0 0 * * * * *");
        assert!(next.is_some(), "Expected a next run time for a valid cron expression");
    }

    #[test]
    fn test_cron_next_run_invalid() {
        let next = CronScheduler::next_run("not-a-cron");
        assert!(next.is_none());
    }

    #[tokio::test]
    async fn test_scheduler_add_list_remove() {
        let scheduler = CronScheduler::new();

        let job = CronJob {
            id: "j1".to_string(),
            schedule: "0 0 * * * * *".to_string(),
            description: None,
            agent_id: "agent".to_string(),
            prompt: "hello".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
            last_run: None,
            run_count: 0,
        };

        scheduler.add(job).await.unwrap();

        let jobs = scheduler.list().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "j1");

        let removed = scheduler.remove("j1").await;
        assert!(removed.is_some());
        assert!(scheduler.list().await.is_empty());
    }

    #[tokio::test]
    async fn test_scheduler_add_invalid_cron() {
        let scheduler = CronScheduler::new();

        let job = CronJob {
            id: "bad".to_string(),
            schedule: "not valid".to_string(),
            description: None,
            agent_id: "agent".to_string(),
            prompt: "hello".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
            last_run: None,
            run_count: 0,
        };

        let result = scheduler.add(job).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_scheduler_record_run() {
        let scheduler = CronScheduler::new();

        let job = CronJob {
            id: "j1".to_string(),
            schedule: "0 0 * * * * *".to_string(),
            description: None,
            agent_id: "agent".to_string(),
            prompt: "hello".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
            last_run: None,
            run_count: 0,
        };

        scheduler.add(job).await.unwrap();

        let updated = scheduler.record_run("j1").await.unwrap();
        assert_eq!(updated.run_count, 1);
        assert!(updated.last_run.is_some());

        let updated2 = scheduler.record_run("j1").await.unwrap();
        assert_eq!(updated2.run_count, 2);
    }

    #[tokio::test]
    async fn test_scheduler_update() {
        let scheduler = CronScheduler::new();

        let job = CronJob {
            id: "j1".to_string(),
            schedule: "0 0 * * * * *".to_string(),
            description: None,
            agent_id: "agent".to_string(),
            prompt: "hello".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
            last_run: None,
            run_count: 0,
        };

        scheduler.add(job).await.unwrap();

        scheduler
            .update("j1", None, None, Some("new prompt".to_string()), Some(false))
            .await
            .unwrap();

        let j = scheduler.get("j1").await.unwrap();
        assert_eq!(j.prompt, "new prompt");
        assert!(!j.enabled);
    }
}
