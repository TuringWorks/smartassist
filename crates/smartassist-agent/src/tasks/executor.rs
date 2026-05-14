//! Task executor with policies for detached task runtime.

use super::registry::{TaskEntry, TaskRegistry, TaskStatus};
use crate::tools::{ToolContext, ToolExecutor};
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Execution policy for a task.
#[derive(Debug, Clone)]
pub struct TaskPolicy {
    /// Maximum duration before timeout.
    pub timeout: Duration,
    /// Maximum retry attempts.
    pub max_retries: u32,
    /// Delay between retries.
    pub retry_delay: Duration,
    /// Whether to run immediately or queue.
    pub immediate: bool,
}

impl Default for TaskPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            max_retries: 3,
            retry_delay: Duration::from_secs(5),
            immediate: false,
        }
    }
}

impl TaskPolicy {
    /// Set timeout duration.
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    /// Set max retries.
    pub fn max_retries(mut self, count: u32) -> Self {
        self.max_retries = count;
        self
    }

    /// Set immediate execution.
    pub fn immediate(mut self) -> Self {
        self.immediate = true;
        self
    }
}

/// Task executor that runs tasks from the registry.
pub struct TaskExecutor {
    registry: Arc<TaskRegistry>,
    tool_executor: Arc<ToolExecutor>,
}

impl TaskExecutor {
    /// Create a new task executor.
    pub fn new(registry: Arc<TaskRegistry>, tool_executor: Arc<ToolExecutor>) -> Self {
        Self {
            registry,
            tool_executor,
        }
    }

    /// Submit a new task to the registry.
    pub async fn submit(
        &self,
        id: impl Into<String>,
        name: impl Into<String>,
        task_type: impl Into<String>,
        params: serde_json::Value,
        session_id: Option<String>,
        tool_name: Option<String>,
        policy: TaskPolicy,
    ) -> crate::Result<TaskEntry> {
        let entry = self
            .registry
            .create(id, name, task_type, params, session_id, tool_name)
            .await?;

        if policy.immediate {
            let _ = self.execute_task(&entry.id, policy.clone()).await;
        }

        Ok(entry)
    }

    /// Execute a single task by ID.
    pub async fn execute_task(
        &self,
        task_id: &str,
        policy: TaskPolicy,
    ) -> crate::Result<TaskEntry> {
        let task = self
            .registry
            .get(task_id)
            .await?
            .ok_or_else(|| crate::AgentError::Internal(format!("Task {} not found", task_id)))?;

        if task.status != TaskStatus::Pending && task.status != TaskStatus::Failed {
            return Ok(task);
        }

        info!("Executing task {} ({})", task_id, task.name);

        self.registry
            .update_status(task_id, TaskStatus::Running, None, None)
            .await?;

        let deadline = Instant::now() + policy.timeout;
        let tool_name = task.tool_name.clone();
        let params = task.params.clone();

        let result = if let Some(name) = tool_name {
            tokio::time::timeout_at(deadline, self.run_tool(&name, params)).await
        } else {
            tokio::time::timeout_at(deadline, self.run_noop()).await
        };

        let updated = match result {
            Ok(Ok(output)) => {
                self.registry
                    .update_status(task_id, TaskStatus::Completed, Some(output), None)
                    .await?;
                self.registry.get(task_id).await?.unwrap()
            }
            Ok(Err(e)) => {
                let should_retry = task.retry_count < policy.max_retries as i32;
                if should_retry {
                    self.registry.increment_retry(task_id).await?;
                    warn!(
                        "Task {} failed (attempt {}/{}), will retry: {}",
                        task_id,
                        task.retry_count + 1,
                        policy.max_retries,
                        e
                    );
                    tokio::time::sleep(policy.retry_delay).await;
                    return Box::pin(self.execute_task(task_id, policy)).await;
                } else {
                    self.registry
                        .update_status(task_id, TaskStatus::Failed, None, Some(e.to_string()))
                        .await?;
                    self.registry.get(task_id).await?.unwrap()
                }
            }
            Err(_) => {
                self.registry
                    .update_status(task_id, TaskStatus::TimedOut, None, Some("Execution timed out".to_string()))
                    .await?;
                self.registry.get(task_id).await?.unwrap()
            }
        };

        Ok(updated)
    }

    async fn run_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> crate::Result<serde_json::Value> {
        let context = ToolContext::default();
        self.tool_executor
            .execute(name, name, params, Some(&context))
            .await
            .map(|r| serde_json::json!({ "output": r }))
    }

    async fn run_noop(&self) -> crate::Result<serde_json::Value> {
        Ok(serde_json::json!({ "output": "noop" }))
    }

    /// Process all pending tasks.
    pub async fn process_pending(&self,
        policy: TaskPolicy,
        batch_size: usize,
    ) -> crate::Result<Vec<TaskEntry>> {
        let pending = self
            .registry
            .list(Some(TaskStatus::Pending), Some(batch_size as i64))
            .await?;

        let mut results = Vec::new();
        for task in pending {
            match self.execute_task(&task.id, policy.clone()).await {
                Ok(entry) => results.push(entry),
                Err(e) => {
                    error!("Failed to execute task {}: {}", task.id, e);
                }
            }
        }

        Ok(results)
    }

    /// Cancel a running or pending task.
    pub async fn cancel(&self, task_id: &str) -> crate::Result<bool> {
        let task = self.registry.get(task_id).await?;
        if let Some(t) = task {
            if t.status == TaskStatus::Running || t.status == TaskStatus::Pending {
                self.registry
                    .update_status(task_id, TaskStatus::Cancelled, None, None)
                    .await?;
                info!("Cancelled task {}", task_id);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Maintenance: mark stale running tasks as timed out.
    pub async fn maintenance_stale_tasks(&self,
        threshold_minutes: i64,
    ) -> crate::Result<usize> {
        let stale = self
            .registry
            .stale_running_tasks(threshold_minutes)
            .await?;

        let mut count = 0;
        for task in stale {
            self.registry
                .update_status(
                    &task.id,
                    TaskStatus::TimedOut,
                    None,
                    Some("Marked stale by maintenance".to_string()),
                )
                .await?;
            count += 1;
        }

        if count > 0 {
            warn!("Maintenance marked {} stale tasks as timed out", count);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolRegistry;

    async fn test_executor() -> (TaskExecutor, Arc<TaskRegistry>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.db");
        let registry = Arc::new(TaskRegistry::open(&path).unwrap());
        let tool_registry = Arc::new(ToolRegistry::new());
        let tool_executor = Arc::new(ToolExecutor::new(tool_registry));
        let executor = TaskExecutor::new(registry.clone(), tool_executor);
        (executor, registry, dir)
    }

    #[tokio::test]
    async fn test_submit_and_execute_noop() {
        let (executor, registry, _dir) = test_executor().await;

        let policy = TaskPolicy::default().immediate();
        let task = executor
            .submit("exec-1", "Noop Task", "noop", serde_json::json!({}), None, None, policy)
            .await
            .unwrap();

        assert_eq!(task.id, "exec-1");

        let completed = registry.get("exec-1").await.unwrap().unwrap();
        assert_eq!(completed.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let (executor, registry, _dir) = test_executor().await;

        let task = executor
            .submit("cancel-1", "Cancel Me", "test", serde_json::json!({}), None, None, TaskPolicy::default())
            .await
            .unwrap();

        assert_eq!(task.status, TaskStatus::Pending);

        let cancelled = executor.cancel("cancel-1").await.unwrap();
        assert!(cancelled);

        let task = registry.get("cancel-1").await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_process_pending() {
        let (executor, _registry, _dir) = test_executor().await;

        executor
            .submit("p1", "P1", "test", serde_json::json!({}), None, None, TaskPolicy::default())
            .await
            .unwrap();
        executor
            .submit("p2", "P2", "test", serde_json::json!({}), None, None, TaskPolicy::default())
            .await
            .unwrap();

        let results = executor.process_pending(TaskPolicy::default(), 10).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.status == TaskStatus::Completed));
    }
}
