//! Cron service daemon for SmartAssist.
//!
//! Provides persistent job scheduling with SQLite-backed storage,
//! a polling scheduler, and gateway invocation for agent execution.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info};

pub mod store;

use store::JobStore;

/// Errors returned by the cron subsystem.
#[derive(thiserror::Error, Debug)]
pub enum CronError {
    #[error("invalid cron expression: {0}")]
    InvalidExpression(String),
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("store error: {0}")]
    StoreError(String),
    #[error("execution error: {0}")]
    ExecutionError(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A scheduled cron job.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: String,
    pub schedule: String,
    pub description: Option<String>,
    pub agent_id: String,
    pub prompt: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    pub run_count: u64,
    pub next_run: Option<chrono::DateTime<chrono::Utc>>,
}

impl Job {
    /// Compute next run time from the cron expression.
    pub fn compute_next_run(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        cron::Schedule::from_str(&self.schedule)
            .ok()?
            .upcoming(chrono::Utc)
            .next()
    }

    /// Check if the job is due to run now.
    pub fn is_due(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        if !self.enabled {
            return false;
        }
        if let Some(next) = self.next_run {
            next <= now
        } else {
            false
        }
    }
}

/// Job builder for convenient construction.
#[derive(Debug, Default)]
pub struct JobBuilder {
    schedule: String,
    description: Option<String>,
    agent_id: String,
    prompt: String,
    enabled: bool,
}

impl JobBuilder {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    pub fn schedule(mut self, schedule: impl Into<String>) -> Self {
        self.schedule = schedule.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = agent_id.into();
        self
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn build(self) -> Result<Job, CronError> {
        // Validate cron expression
        cron::Schedule::from_str(&self.schedule)
            .map_err(|e| CronError::InvalidExpression(e.to_string()))?;

        let job = Job {
            id: uuid::Uuid::new_v4().to_string(),
            schedule: self.schedule,
            description: self.description,
            agent_id: self.agent_id,
            prompt: self.prompt,
            enabled: self.enabled,
            created_at: chrono::Utc::now(),
            last_run: None,
            run_count: 0,
            next_run: None,
        };
        Ok(job)
    }
}

/// In-memory job store for testing and lightweight use.
pub struct MemoryJobStore {
    jobs: RwLock<HashMap<String, Job>>,
}

impl MemoryJobStore {
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl JobStore for MemoryJobStore {
    async fn add(&self, job: &Job) -> Result<(), CronError> {
        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id.clone(), job.clone());
        Ok(())
    }

    async fn remove(&self, id: &str) -> Result<Option<Job>, CronError> {
        let mut jobs = self.jobs.write().await;
        Ok(jobs.remove(id))
    }

    async fn get(&self, id: &str) -> Result<Option<Job>, CronError> {
        let jobs = self.jobs.read().await;
        Ok(jobs.get(id).cloned())
    }

    async fn list(&self) -> Result<Vec<Job>, CronError> {
        let jobs = self.jobs.read().await;
        Ok(jobs.values().cloned().collect())
    }

    async fn update(&self, job: &Job) -> Result<(), CronError> {
        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id.clone(), job.clone());
        Ok(())
    }
}

/// Executor that runs when a cron job fires.
#[async_trait::async_trait]
pub trait JobExecutor: Send + Sync {
    async fn execute(&self, job: &Job) -> Result<(), CronError>;
}

/// Default executor that logs the job and simulates success.
pub struct LogExecutor;

#[async_trait::async_trait]
impl JobExecutor for LogExecutor {
    async fn execute(&self, job: &Job) -> Result<(), CronError> {
        info!(
            "Executing cron job {}: agent={} prompt={}",
            job.id, job.agent_id, job.prompt
        );
        Ok(())
    }
}

/// Scheduler that polls for due jobs and executes them.
pub struct Scheduler {
    store: Arc<dyn JobStore>,
    executor: Arc<dyn JobExecutor>,
    poll_interval: Duration,
}

impl Scheduler {
    pub fn new(
        store: Arc<dyn JobStore>,
        executor: Arc<dyn JobExecutor>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            store,
            executor,
            poll_interval,
        }
    }

    /// Access the underlying job store.
    pub fn store(&self) -> &Arc<dyn JobStore> {
        &self.store
    }

    /// Run one scheduler tick: find due jobs, execute them, update next_run.
    pub async fn tick(&self) -> Result<usize, CronError> {
        let now = chrono::Utc::now();
        let mut jobs = self.store.list().await?;
        let mut executed = 0;

        for job in &mut jobs {
            // Ensure next_run is populated
            if job.next_run.is_none() {
                job.next_run = job.compute_next_run();
                self.store.update(job).await?;
            }

            if job.is_due(now) {
                debug!("Job {} is due, executing", job.id);
                match self.executor.execute(job).await {
                    Ok(()) => {
                        job.last_run = Some(now);
                        job.run_count += 1;
                        job.next_run = job.compute_next_run();
                        self.store.update(job).await?;
                        executed += 1;
                    }
                    Err(e) => {
                        error!("Job {} execution failed: {}", job.id, e);
                        // Still advance next_run to avoid tight retry loops
                        job.next_run = job.compute_next_run();
                        self.store.update(job).await?;
                    }
                }
            }
        }

        Ok(executed)
    }

    /// Run the scheduler loop indefinitely.
    pub async fn run(&self) {
        let mut ticker = interval(self.poll_interval);
        loop {
            ticker.tick().await;
            match self.tick().await {
                Ok(n) => {
                    if n > 0 {
                        debug!("Executed {} cron jobs", n);
                    }
                }
                Err(e) => {
                    error!("Scheduler tick failed: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CountingExecutor {
        count: Arc<RwLock<u64>>,
    }

    #[async_trait::async_trait]
    impl JobExecutor for CountingExecutor {
        async fn execute(&self, _job: &Job) -> Result<(), CronError> {
            let mut count = self.count.write().await;
            *count += 1;
            Ok(())
        }
    }

    #[test]
    fn test_job_builder_validates_cron() {
        let result = JobBuilder::new()
            .schedule("0 * * * * *")
            .agent_id("agent-1")
            .prompt("hello")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_job_builder_rejects_invalid_cron() {
        let result = JobBuilder::new()
            .schedule("not-a-cron")
            .agent_id("agent-1")
            .prompt("hello")
            .build();
        assert!(matches!(result, Err(CronError::InvalidExpression(_))));
    }

    #[tokio::test]
    async fn test_memory_store_add_and_get() {
        let store = Arc::new(MemoryJobStore::new());
        let job = JobBuilder::new()
            .schedule("0 * * * * *")
            .agent_id("agent-1")
            .prompt("hello")
            .build()
            .unwrap();

        store.add(&job).await.unwrap();
        let fetched = store.get(&job.id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_memory_store_remove() {
        let store = Arc::new(MemoryJobStore::new());
        let job = JobBuilder::new()
            .schedule("0 * * * * *")
            .agent_id("agent-1")
            .prompt("hello")
            .build()
            .unwrap();

        store.add(&job).await.unwrap();
        let removed = store.remove(&job.id).await.unwrap();
        assert!(removed.is_some());

        let missing = store.get(&job.id).await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_scheduler_executes_due_job() {
        let store = Arc::new(MemoryJobStore::new());
        let count = Arc::new(RwLock::new(0u64));
        let executor = Arc::new(CountingExecutor {
            count: count.clone(),
        });

        let scheduler = Scheduler::new(store.clone(), executor, Duration::from_secs(60));

        // Create a job that is already due (next_run in the past)
        let mut job = JobBuilder::new()
            .schedule("0 * * * * *")
            .agent_id("agent-1")
            .prompt("hello")
            .build()
            .unwrap();
        job.next_run = Some(chrono::Utc::now() - Duration::from_secs(1));
        store.add(&job).await.unwrap();

        let executed = scheduler.tick().await.unwrap();
        assert_eq!(executed, 1);

        let count_val = *count.read().await;
        assert_eq!(count_val, 1);
    }

    #[tokio::test]
    async fn test_scheduler_skips_future_job() {
        let store = Arc::new(MemoryJobStore::new());
        let count = Arc::new(RwLock::new(0u64));
        let executor = Arc::new(CountingExecutor {
            count: count.clone(),
        });

        let scheduler = Scheduler::new(store.clone(), executor, Duration::from_secs(60));

        // Create a job with next_run in the future
        let mut job = JobBuilder::new()
            .schedule("0 * * * * *")
            .agent_id("agent-1")
            .prompt("hello")
            .build()
            .unwrap();
        job.next_run = Some(chrono::Utc::now() + Duration::from_secs(3600));
        store.add(&job).await.unwrap();

        let executed = scheduler.tick().await.unwrap();
        assert_eq!(executed, 0);

        let count_val = *count.read().await;
        assert_eq!(count_val, 0);
    }

    #[tokio::test]
    async fn test_scheduler_skips_disabled_job() {
        let store = Arc::new(MemoryJobStore::new());
        let count = Arc::new(RwLock::new(0u64));
        let executor = Arc::new(CountingExecutor {
            count: count.clone(),
        });

        let scheduler = Scheduler::new(store.clone(), executor, Duration::from_secs(60));

        let mut job = JobBuilder::new()
            .schedule("0 * * * * *")
            .agent_id("agent-1")
            .prompt("hello")
            .enabled(false)
            .build()
            .unwrap();
        job.next_run = Some(chrono::Utc::now() - Duration::from_secs(1));
        store.add(&job).await.unwrap();

        let executed = scheduler.tick().await.unwrap();
        assert_eq!(executed, 0);
    }

    #[tokio::test]
    async fn test_scheduler_updates_run_count() {
        let store = Arc::new(MemoryJobStore::new());
        let executor = Arc::new(LogExecutor);

        let scheduler = Scheduler::new(store.clone(), executor, Duration::from_secs(60));

        let mut job = JobBuilder::new()
            .schedule("0 * * * * *")
            .agent_id("agent-1")
            .prompt("hello")
            .build()
            .unwrap();
        job.next_run = Some(chrono::Utc::now() - Duration::from_secs(1));
        store.add(&job).await.unwrap();

        scheduler.tick().await.unwrap();

        let updated = store.get(&job.id).await.unwrap().unwrap();
        assert_eq!(updated.run_count, 1);
        assert!(updated.last_run.is_some());
        assert!(updated.next_run.unwrap() > chrono::Utc::now());
    }
}
