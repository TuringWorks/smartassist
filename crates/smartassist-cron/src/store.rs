//! Persistent job store implementations.

use super::{CronError, Job};

/// Trait for storing and retrieving cron jobs.
#[async_trait::async_trait]
pub trait JobStore: Send + Sync {
    async fn add(&self, job: &Job) -> Result<(), CronError>;
    async fn remove(&self, id: &str) -> Result<Option<Job>, CronError>;
    async fn get(&self, id: &str) -> Result<Option<Job>, CronError>;
    async fn list(&self) -> Result<Vec<Job>, CronError>;
    async fn update(&self, job: &Job) -> Result<(), CronError>;
}
