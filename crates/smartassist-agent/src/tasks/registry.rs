//! SQLite-backed task registry for detached task persistence.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;
use tracing::{debug, info};

/// Status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is pending execution.
    Pending,
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
    /// Task was cancelled.
    Cancelled,
    /// Task timed out.
    TimedOut,
}

/// A task entry in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEntry {
    /// Unique task ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Task type / kind.
    pub task_type: String,
    /// Current status.
    pub status: TaskStatus,
    /// Task parameters / payload.
    pub params: Value,
    /// Result payload (if completed).
    pub result: Option<Value>,
    /// Error message (if failed).
    pub error: Option<String>,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task started running.
    pub started_at: Option<DateTime<Utc>>,
    /// When the task finished.
    pub completed_at: Option<DateTime<Utc>>,
    /// Number of retry attempts.
    pub retry_count: i32,
    /// Maximum retries allowed.
    pub max_retries: i32,
    /// Agent session that owns this task.
    pub session_id: Option<String>,
    /// Tool name to execute.
    pub tool_name: Option<String>,
}

/// SQLite-backed task registry (synchronous DB wrapped in Mutex for thread safety).
pub struct TaskRegistry {
    conn: Mutex<Connection>,
}

impl TaskRegistry {
    /// Create or open a task registry at the given database path.
    pub fn open(path: impl AsRef<Path>) -> crate::Result<Self> {
        let path = path.as_ref();
        info!("Opening task registry at {:?}", path);

        let conn = Connection::open(path)
            .map_err(|e| crate::AgentError::Internal(format!("Failed to open task db: {}", e)))?;

        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                params TEXT NOT NULL DEFAULT '{}',
                result TEXT,
                error TEXT,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 3,
                session_id TEXT,
                tool_name TEXT
            )
            "#,
            [],
        )
        .map_err(|e| crate::AgentError::Internal(format!("Failed to create tasks table: {}", e)))?;

        conn.execute(
            r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_session ON tasks(session_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at);
            "#,
            [],
        )
        .map_err(|e| crate::AgentError::Internal(format!("Failed to create indices: {}", e)))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create a new task entry.
    pub async fn create(
        &self,
        id: impl Into<String>,
        name: impl Into<String>,
        task_type: impl Into<String>,
        params: Value,
        session_id: Option<String>,
        tool_name: Option<String>,
    ) -> crate::Result<TaskEntry> {
        let id = id.into();
        let name = name.into();
        let task_type = task_type.into();
        let now = Utc::now();

        let params_json = serde_json::to_string(&params)
            .map_err(|e| crate::AgentError::Internal(format!("JSON error: {}", e)))?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO tasks (id, name, task_type, status, params, created_at, session_id, tool_name)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            [
                &id,
                &name,
                &task_type,
                "pending",
                &params_json,
                &now.to_rfc3339(),
                &session_id.as_deref().unwrap_or(""),
                &tool_name.as_deref().unwrap_or(""),
            ],
        )
        .map_err(|e| crate::AgentError::Internal(format!("Failed to insert task: {}", e)))?;

        debug!("Created task {} ({})", id, name);

        Ok(TaskEntry {
            id,
            name,
            task_type,
            status: TaskStatus::Pending,
            params,
            result: None,
            error: None,
            created_at: now,
            started_at: None,
            completed_at: None,
            retry_count: 0,
            max_retries: 3,
            session_id,
            tool_name,
        })
    }

    /// Get a task by ID.
    pub async fn get(&self, id: &str) -> crate::Result<Option<TaskEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                r#"
            SELECT id, name, task_type, status, params, result, error,
                   created_at, started_at, completed_at,
                   retry_count, max_retries, session_id, tool_name
            FROM tasks WHERE id = ?1
            "#,
            )
            .map_err(|e| crate::AgentError::Internal(format!("Failed to prepare: {}", e)))?;

        let row = stmt
            .query_row([id], |row| {
                let session_id: String = row.get(12)?;
                let tool_name: String = row.get(13)?;
                Ok(TaskRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    task_type: row.get(2)?,
                    status: row.get(3)?,
                    params: row.get(4)?,
                    result: row.get(5)?,
                    error: row.get(6)?,
                    created_at: row.get(7)?,
                    started_at: row.get(8)?,
                    completed_at: row.get(9)?,
                    retry_count: row.get(10)?,
                    max_retries: row.get(11)?,
                    session_id: if session_id.is_empty() {
                        None
                    } else {
                        Some(session_id)
                    },
                    tool_name: if tool_name.is_empty() {
                        None
                    } else {
                        Some(tool_name)
                    },
                })
            })
            .optional()
            .map_err(|e| crate::AgentError::Internal(format!("Failed to fetch task: {}", e)))?;

        Ok(row.map(|r| r.into()))
    }

    /// Update task status.
    pub async fn update_status(
        &self,
        id: &str,
        status: TaskStatus,
        result: Option<Value>,
        error: Option<String>,
    ) -> crate::Result<bool> {
        let now = Utc::now();
        let result_json = result.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default());
        let status_str = status.to_string();

        let conn = self.conn.lock().unwrap();
        let result_str = result_json.as_deref().unwrap_or("");
        let error_str = error.as_deref().unwrap_or("");
        let now_str = now.to_rfc3339();

        let rows = conn
            .execute(
                r#"
                UPDATE tasks
                SET status = ?1,
                    result = ?2,
                    error = ?3,
                    started_at = CASE WHEN ?1 = 'running' AND started_at IS NULL THEN ?4 ELSE started_at END,
                    completed_at = CASE WHEN ?1 IN ('completed', 'failed', 'cancelled', 'timed_out') THEN ?4 ELSE completed_at END
                WHERE id = ?5
                "#,
                rusqlite::params![
                    &status_str,
                    result_str,
                    error_str,
                    &now_str,
                    id,
                ],
            )
            .map_err(|e| crate::AgentError::Internal(format!("Failed to update task: {}", e)))?;

        debug!("Updated task {} to {:?}", id, status);
        Ok(rows > 0)
    }

    /// Increment retry count.
    pub async fn increment_retry(&self, id: &str) -> crate::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn
            .execute(
                "UPDATE tasks SET retry_count = retry_count + 1 WHERE id = ?1",
                [id],
            )
            .map_err(|e| crate::AgentError::Internal(format!("Failed to increment retry: {}", e)))?;

        Ok(rows > 0)
    }

    /// List tasks with optional status filter.
    pub async fn list(
        &self,
        status: Option<TaskStatus>,
        limit: Option<i64>,
    ) -> crate::Result<Vec<TaskEntry>> {
        let limit = limit.unwrap_or(100);
        let conn = self.conn.lock().unwrap();

        let query = if status.is_some() {
            r#"
                SELECT id, name, task_type, status, params, result, error,
                       created_at, started_at, completed_at,
                       retry_count, max_retries, session_id, tool_name
                FROM tasks WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2
                "#
        } else {
            r#"
                SELECT id, name, task_type, status, params, result, error,
                       created_at, started_at, completed_at,
                       retry_count, max_retries, session_id, tool_name
                FROM tasks ORDER BY created_at DESC LIMIT ?1
                "#
        };

        let mut stmt = conn
            .prepare(query)
            .map_err(|e| crate::AgentError::Internal(format!("Failed to prepare list: {}", e)))?;

        let rows: Vec<TaskRow> = if let Some(s) = status {
            let status_str = s.to_string();
            stmt.query_map([status_str.as_str(), &limit.to_string()], |row| {
                let session_id: String = row.get(12)?;
                let tool_name: String = row.get(13)?;
                Ok(TaskRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    task_type: row.get(2)?,
                    status: row.get(3)?,
                    params: row.get(4)?,
                    result: row.get(5)?,
                    error: row.get(6)?,
                    created_at: row.get(7)?,
                    started_at: row.get(8)?,
                    completed_at: row.get(9)?,
                    retry_count: row.get(10)?,
                    max_retries: row.get(11)?,
                    session_id: if session_id.is_empty() {
                        None
                    } else {
                        Some(session_id)
                    },
                    tool_name: if tool_name.is_empty() {
                        None
                    } else {
                        Some(tool_name)
                    },
                })
            })
            .map_err(|e| crate::AgentError::Internal(format!("Failed to list tasks: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::AgentError::Internal(format!("Row error: {}", e)))?
        } else {
            stmt.query_map([&limit.to_string()], |row| {
                let session_id: String = row.get(12)?;
                let tool_name: String = row.get(13)?;
                Ok(TaskRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    task_type: row.get(2)?,
                    status: row.get(3)?,
                    params: row.get(4)?,
                    result: row.get(5)?,
                    error: row.get(6)?,
                    created_at: row.get(7)?,
                    started_at: row.get(8)?,
                    completed_at: row.get(9)?,
                    retry_count: row.get(10)?,
                    max_retries: row.get(11)?,
                    session_id: if session_id.is_empty() {
                        None
                    } else {
                        Some(session_id)
                    },
                    tool_name: if tool_name.is_empty() {
                        None
                    } else {
                        Some(tool_name)
                    },
                })
            })
            .map_err(|e| crate::AgentError::Internal(format!("Failed to list tasks: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::AgentError::Internal(format!("Row error: {}", e)))?
        };

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Delete a task by ID.
    pub async fn delete(&self, id: &str) -> crate::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn
            .execute("DELETE FROM tasks WHERE id = ?1", [id])
            .map_err(|e| crate::AgentError::Internal(format!("Failed to delete task: {}", e)))?;

        Ok(rows > 0)
    }

    /// Audit: count tasks by status.
    pub async fn audit_summary(&self) -> crate::Result<Vec<(TaskStatus, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT status, COUNT(*) as cnt FROM tasks GROUP BY status")
            .map_err(|e| crate::AgentError::Internal(format!("Audit failed: {}", e)))?;

        let rows: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| crate::AgentError::Internal(format!("Audit failed: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::AgentError::Internal(format!("Audit row error: {}", e)))?;

        let mut result = Vec::new();
        for (status_str, count) in rows {
            let status = parse_status(&status_str);
            result.push((status, count));
        }

        Ok(result)
    }

    /// Get stale running tasks (running longer than threshold).
    pub async fn stale_running_tasks(
        &self,
        threshold_minutes: i64,
    ) -> crate::Result<Vec<TaskEntry>> {
        let threshold = Utc::now() - chrono::Duration::minutes(threshold_minutes);
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, name, task_type, status, params, result, error,
                       created_at, started_at, completed_at,
                       retry_count, max_retries, session_id, tool_name
                FROM tasks
                WHERE status = 'running' AND started_at < ?1
                ORDER BY started_at ASC
                "#,
            )
            .map_err(|e| crate::AgentError::Internal(format!("Failed to find stale tasks: {}", e)))?;

        let rows: Vec<TaskRow> = stmt
            .query_map([threshold.to_rfc3339()], |row| {
                let session_id: String = row.get(12)?;
                let tool_name: String = row.get(13)?;
                Ok(TaskRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    task_type: row.get(2)?,
                    status: row.get(3)?,
                    params: row.get(4)?,
                    result: row.get(5)?,
                    error: row.get(6)?,
                    created_at: row.get(7)?,
                    started_at: row.get(8)?,
                    completed_at: row.get(9)?,
                    retry_count: row.get(10)?,
                    max_retries: row.get(11)?,
                    session_id: if session_id.is_empty() {
                        None
                    } else {
                        Some(session_id)
                    },
                    tool_name: if tool_name.is_empty() {
                        None
                    } else {
                        Some(tool_name)
                    },
                })
            })
            .map_err(|e| crate::AgentError::Internal(format!("Failed to find stale tasks: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::AgentError::Internal(format!("Row error: {}", e)))?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::TimedOut => "timed_out",
        };
        write!(f, "{}", s)
    }
}

fn parse_status(s: &str) -> TaskStatus {
    match s {
        "pending" => TaskStatus::Pending,
        "running" => TaskStatus::Running,
        "completed" => TaskStatus::Completed,
        "failed" => TaskStatus::Failed,
        "cancelled" => TaskStatus::Cancelled,
        "timed_out" => TaskStatus::TimedOut,
        _ => TaskStatus::Pending,
    }
}

/// Internal row type for rusqlite mapping.
struct TaskRow {
    id: String,
    name: String,
    task_type: String,
    status: String,
    params: String,
    result: Option<String>,
    error: Option<String>,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    retry_count: i32,
    max_retries: i32,
    session_id: Option<String>,
    tool_name: Option<String>,
}

impl From<TaskRow> for TaskEntry {
    fn from(row: TaskRow) -> Self {
        let status = parse_status(&row.status);
        let params = serde_json::from_str(&row.params).unwrap_or_else(|_| serde_json::json!({}));
        let result = row.result.and_then(|r| serde_json::from_str(&r).ok());

        TaskEntry {
            id: row.id,
            name: row.name,
            task_type: row.task_type,
            status,
            params,
            result,
            error: row.error,
            created_at: DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            started_at: row
                .started_at
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
            completed_at: row
                .completed_at
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
            retry_count: row.retry_count,
            max_retries: row.max_retries,
            session_id: row.session_id,
            tool_name: row.tool_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_registry() -> (TaskRegistry, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.db");
        (TaskRegistry::open(&path).unwrap(), dir)
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let (registry, _dir) = test_registry();
        let task = registry
            .create("task-1", "Test Task", "filesystem", serde_json::json!({"path": "/tmp"}), None, Some("read_file".to_string()))
            .await
            .unwrap();

        assert_eq!(task.id, "task-1");
        assert_eq!(task.status, TaskStatus::Pending);

        let fetched = registry.get("task-1").await.unwrap().unwrap();
        assert_eq!(fetched.name, "Test Task");
        assert_eq!(fetched.params["path"], "/tmp");
    }

    #[tokio::test]
    async fn test_update_status() {
        let (registry, _dir) = test_registry();
        registry
            .create("task-2", "Task 2", "bash", serde_json::json!({}), None, None)
            .await
            .unwrap();

        let updated = registry
            .update_status("task-2", TaskStatus::Running, None, None)
            .await
            .unwrap();
        assert!(updated);

        let task = registry.get("task-2").await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let (registry, _dir) = test_registry();
        registry
            .create("task-a", "A", "test", serde_json::json!({}), None, None)
            .await
            .unwrap();
        registry
            .create("task-b", "B", "test", serde_json::json!({}), None, None)
            .await
            .unwrap();

        registry
            .update_status("task-b", TaskStatus::Completed, Some(serde_json::json!("done")), None)
            .await
            .unwrap();

        let pending = registry.list(Some(TaskStatus::Pending), None).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "task-a");

        let completed = registry.list(Some(TaskStatus::Completed), None).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "task-b");
    }

    #[tokio::test]
    async fn test_audit_summary() {
        let (registry, _dir) = test_registry();
        registry.create("t1", "T1", "test", serde_json::json!({}), None, None).await.unwrap();
        registry.create("t2", "T2", "test", serde_json::json!({}), None, None).await.unwrap();
        registry.update_status("t2", TaskStatus::Completed, None, None).await.unwrap();

        let summary = registry.audit_summary().await.unwrap();
        assert_eq!(summary.len(), 2);
    }

    #[tokio::test]
    async fn test_delete() {
        let (registry, _dir) = test_registry();
        registry.create("del-1", "Delete Me", "test", serde_json::json!({}), None, None).await.unwrap();

        let deleted = registry.delete("del-1").await.unwrap();
        assert!(deleted);

        let missing = registry.get("del-1").await.unwrap();
        assert!(missing.is_none());
    }
}
