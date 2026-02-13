//! Task management tools for tracking work items.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

/// Task status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Deleted,
}

/// A task item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
    pub owner: Option<String>,
    pub blocked_by: Vec<String>,
    pub blocks: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// In-memory task store.
#[derive(Default)]
pub struct TaskStore {
    tasks: RwLock<HashMap<String, Task>>,
    next_id: RwLock<u64>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create(&self, subject: String, description: String) -> Task {
        let mut next_id = self.next_id.write().await;
        *next_id += 1;
        let id = next_id.to_string();

        let now = chrono::Utc::now();
        let task = Task {
            id: id.clone(),
            subject,
            description,
            status: TaskStatus::Pending,
            owner: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
            created_at: now,
            updated_at: now,
        };

        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), task.clone());
        task
    }

    pub async fn get(&self, id: &str) -> Option<Task> {
        let tasks = self.tasks.read().await;
        tasks.get(id).cloned()
    }

    pub async fn list(&self) -> Vec<Task> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    pub async fn update(&self, id: &str, updates: TaskUpdate) -> Option<Task> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(id) {
            if let Some(status) = updates.status {
                task.status = status;
            }
            if let Some(subject) = updates.subject {
                task.subject = subject;
            }
            if let Some(description) = updates.description {
                task.description = description;
            }
            if let Some(owner) = updates.owner {
                task.owner = Some(owner);
            }
            task.updated_at = chrono::Utc::now();
            Some(task.clone())
        } else {
            None
        }
    }

    pub async fn delete(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        tasks.remove(id).is_some()
    }
}

/// Task update parameters.
#[derive(Default)]
pub struct TaskUpdate {
    pub status: Option<TaskStatus>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
}

/// Tool for creating tasks.
pub struct TaskCreateTool {
    store: Arc<TaskStore>,
}

impl TaskCreateTool {
    pub fn new(store: Arc<TaskStore>) -> Self {
        Self { store }
    }

    pub fn with_new_store() -> Self {
        Self {
            store: Arc::new(TaskStore::new()),
        }
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "task_create"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_create".to_string(),
            description: "Create a new task to track work items".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Brief title for the task"
                    },
                    "description": {
                        "type": "string",
                        "description": "Detailed description of what needs to be done"
                    }
                },
                "required": ["subject", "description"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let subject = args
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("subject is required"))?
            .to_string();

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        debug!("Creating task: {}", subject);

        let task = self.store.create(subject, description).await;
        let duration = start.elapsed();

        Ok(ToolResult::success(tool_use_id, serde_json::to_value(&task).unwrap())
            .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for listing tasks.
pub struct TaskListTool {
    store: Arc<TaskStore>,
}

impl TaskListTool {
    pub fn new(store: Arc<TaskStore>) -> Self {
        Self { store }
    }

    pub fn with_new_store() -> Self {
        Self {
            store: Arc::new(TaskStore::new()),
        }
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "task_list"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_list".to_string(),
            description: "List all tasks".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        _args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let tasks = self.store.list().await;
        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "tasks": tasks,
                "count": tasks.len()
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for updating tasks.
pub struct TaskUpdateTool {
    store: Arc<TaskStore>,
}

impl TaskUpdateTool {
    pub fn new(store: Arc<TaskStore>) -> Self {
        Self { store }
    }

    pub fn with_new_store() -> Self {
        Self {
            store: Arc::new(TaskStore::new()),
        }
    }
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "task_update"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_update".to_string(),
            description: "Update a task's status or details".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "ID of the task to update"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed", "deleted"],
                        "description": "New status for the task"
                    },
                    "subject": {
                        "type": "string",
                        "description": "New subject for the task"
                    },
                    "description": {
                        "type": "string",
                        "description": "New description for the task"
                    },
                    "owner": {
                        "type": "string",
                        "description": "Assign task to an owner"
                    }
                },
                "required": ["task_id"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let task_id = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("task_id is required"))?;

        let status = args.get("status").and_then(|v| v.as_str()).map(|s| match s {
            "pending" => TaskStatus::Pending,
            "in_progress" => TaskStatus::InProgress,
            "completed" => TaskStatus::Completed,
            "deleted" => TaskStatus::Deleted,
            _ => TaskStatus::Pending,
        });

        let updates = TaskUpdate {
            status,
            subject: args.get("subject").and_then(|v| v.as_str()).map(String::from),
            description: args
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner: args.get("owner").and_then(|v| v.as_str()).map(String::from),
        };

        debug!("Updating task: {}", task_id);

        match self.store.update(task_id, updates).await {
            Some(task) => {
                let duration = start.elapsed();
                Ok(ToolResult::success(tool_use_id, serde_json::to_value(&task).unwrap())
                    .with_duration(duration))
            }
            None => Ok(ToolResult::error(
                tool_use_id,
                format!("Task {} not found", task_id),
            )),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for getting a single task.
pub struct TaskGetTool {
    store: Arc<TaskStore>,
}

impl TaskGetTool {
    pub fn new(store: Arc<TaskStore>) -> Self {
        Self { store }
    }

    pub fn with_new_store() -> Self {
        Self {
            store: Arc::new(TaskStore::new()),
        }
    }
}

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "task_get"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_get".to_string(),
            description: "Get details of a specific task".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "ID of the task to retrieve"
                    }
                },
                "required": ["task_id"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let task_id = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("task_id is required"))?;

        debug!("Getting task: {}", task_id);

        match self.store.get(task_id).await {
            Some(task) => {
                let duration = start.elapsed();
                Ok(ToolResult::success(tool_use_id, serde_json::to_value(&task).unwrap())
                    .with_duration(duration))
            }
            None => Ok(ToolResult::error(
                tool_use_id,
                format!("Task {} not found", task_id),
            )),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_store() {
        let store = TaskStore::new();

        // Create task
        let task = store.create("Test task".to_string(), "Description".to_string()).await;
        assert_eq!(task.id, "1");
        assert_eq!(task.subject, "Test task");
        assert_eq!(task.status, TaskStatus::Pending);

        // Get task
        let retrieved = store.get("1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().subject, "Test task");

        // Update task
        let updated = store
            .update(
                "1",
                TaskUpdate {
                    status: Some(TaskStatus::Completed),
                    ..Default::default()
                },
            )
            .await;
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().status, TaskStatus::Completed);

        // List tasks
        let tasks = store.list().await;
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn test_task_create_tool_definition() {
        let tool = TaskCreateTool::with_new_store();
        let def = tool.definition();

        assert_eq!(def.name, "task_create");
        assert!(def.description.contains("task"));
    }
}
