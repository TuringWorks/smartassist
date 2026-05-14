//! Canvas workspace and A2UI protocol for SmartAssist.
//!
//! Provides an agent-driven visual surface with a gateway subscription
//! protocol, matching OpenClaw's canvas extension capabilities.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod protocol;

use smartassist_core::types::{AgentId, SessionKey};
use thiserror::Error;

/// Errors returned by the canvas subsystem.
#[derive(Error, Debug)]
pub enum CanvasError {
    #[error("surface not found: {0}")]
    SurfaceNotFound(String),
    #[error("surface already exists: {0}")]
    SurfaceExists(String),
    #[error("invalid action: {0}")]
    InvalidAction(String),
    #[error("agent not subscribed: {0}")]
    NotSubscribed(String),
    #[error("protocol error: {0}")]
    ProtocolError(String),
    #[error("render error: {0}")]
    RenderError(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A visual element on the canvas surface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CanvasElement {
    pub id: String,
    pub element_type: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub properties: serde_json::Value,
    pub children: Vec<CanvasElement>,
}

/// Canvas surface state.
#[derive(Debug, Clone)]
pub struct CanvasSurface {
    pub id: String,
    pub agent_id: Option<AgentId>,
    pub session_key: Option<SessionKey>,
    pub elements: Vec<CanvasElement>,
    pub subscribers: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Action request for the canvas.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum CanvasAction {
    Present,
    Hide,
    Navigate { target: String },
    Eval { script: String },
    Snapshot,
    A2uiPush { elements: Vec<CanvasElement> },
    A2uiReset,
    AddElement { element: CanvasElement },
    RemoveElement { id: String },
    UpdateElement { id: String, properties: serde_json::Value },
}

/// Action result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CanvasActionResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub message: Option<String>,
}

/// Canvas manager owns all surfaces.
pub struct CanvasManager {
    surfaces: RwLock<HashMap<String, CanvasSurface>>,
}

impl CanvasManager {
    pub fn new() -> Self {
        Self {
            surfaces: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new canvas surface.
    pub async fn create_surface(
        &self,
        id: impl Into<String>,
        agent_id: Option<AgentId>,
        session_key: Option<SessionKey>,
    ) -> Result<CanvasSurface, CanvasError> {
        let id = id.into();
        let mut surfaces = self.surfaces.write().await;
        if surfaces.contains_key(&id) {
            return Err(CanvasError::SurfaceExists(id));
        }
        let surface = CanvasSurface {
            id: id.clone(),
            agent_id,
            session_key,
            elements: Vec::new(),
            subscribers: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        surfaces.insert(id, surface.clone());
        Ok(surface)
    }

    /// Get a surface by ID.
    pub async fn get_surface(&self,
        id: &str,
    ) -> Result<CanvasSurface, CanvasError> {
        let surfaces = self.surfaces.read().await;
        surfaces
            .get(id)
            .cloned()
            .ok_or_else(|| CanvasError::SurfaceNotFound(id.to_string()))
    }

    /// Delete a surface.
    pub async fn delete_surface(&self,
        id: &str,
    ) -> Result<(), CanvasError> {
        let mut surfaces = self.surfaces.write().await;
        surfaces
            .remove(id)
            .ok_or_else(|| CanvasError::SurfaceNotFound(id.to_string()))?;
        Ok(())
    }

    /// Execute an action on a surface.
    pub async fn execute(
        &self,
        surface_id: &str,
        action: CanvasAction,
    ) -> Result<CanvasActionResult, CanvasError> {
        let mut surfaces = self.surfaces.write().await;
        let surface = surfaces
            .get_mut(surface_id)
            .ok_or_else(|| CanvasError::SurfaceNotFound(surface_id.to_string()))?;

        let result = match action {
            CanvasAction::Present => CanvasActionResult {
                success: true,
                data: Some(serde_json::json!({ "visible": true })),
                message: None,
            },
            CanvasAction::Hide => CanvasActionResult {
                success: true,
                data: Some(serde_json::json!({ "visible": false })),
                message: None,
            },
            CanvasAction::Navigate { ref target } => {
                surface.elements.clear();
                CanvasActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "target": target })),
                    message: None,
                }
            }
            CanvasAction::Eval { ref script } => CanvasActionResult {
                success: true,
                data: Some(serde_json::json!({ "result": format!("evaluated: {}", script.len()) })),
                message: None,
            },
            CanvasAction::Snapshot => CanvasActionResult {
                success: true,
                data: Some(serde_json::json!({ "element_count": surface.elements.len() })),
                message: None,
            },
            CanvasAction::A2uiPush { ref elements } => {
                surface.elements.extend(elements.clone());
                CanvasActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "added": elements.len() })),
                    message: None,
                }
            }
            CanvasAction::A2uiReset => {
                surface.elements.clear();
                CanvasActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "reset": true })),
                    message: None,
                }
            }
            CanvasAction::AddElement { ref element } => {
                surface.elements.push(element.clone());
                CanvasActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "id": element.id })),
                    message: None,
                }
            }
            CanvasAction::RemoveElement { ref id } => {
                let before = surface.elements.len();
                surface.elements.retain(|e| &e.id != id);
                let removed = before - surface.elements.len();
                CanvasActionResult {
                    success: removed > 0,
                    data: Some(serde_json::json!({ "removed": removed })),
                    message: None,
                }
            }
            CanvasAction::UpdateElement { ref id, ref properties } => {
                if let Some(el) = surface.elements.iter_mut().find(|e| &e.id == id) {
                    el.properties = properties.clone();
                    CanvasActionResult {
                        success: true,
                        data: Some(serde_json::json!({ "updated": true })),
                        message: None,
                    }
                } else {
                    CanvasActionResult {
                        success: false,
                        data: None,
                        message: Some(format!("element {} not found", id)),
                    }
                }
            }
        };

        surface.updated_at = chrono::Utc::now();
        Ok(result)
    }

    /// List all surfaces.
    pub async fn list_surfaces(&self,
    ) -> Result<Vec<CanvasSurface>, CanvasError> {
        let surfaces = self.surfaces.read().await;
        Ok(surfaces.values().cloned().collect())
    }

    /// Subscribe a client to a surface.
    pub async fn subscribe(
        &self,
        surface_id: &str,
        client_id: impl Into<String>,
    ) -> Result<(), CanvasError> {
        let mut surfaces = self.surfaces.write().await;
        let surface = surfaces
            .get_mut(surface_id)
            .ok_or_else(|| CanvasError::SurfaceNotFound(surface_id.to_string()))?;
        let client_id = client_id.into();
        if !surface.subscribers.contains(&client_id) {
            surface.subscribers.push(client_id);
        }
        Ok(())
    }

    /// Unsubscribe a client from a surface.
    pub async fn unsubscribe(
        &self,
        surface_id: &str,
        client_id: &str,
    ) -> Result<(), CanvasError> {
        let mut surfaces = self.surfaces.write().await;
        let surface = surfaces
            .get_mut(surface_id)
            .ok_or_else(|| CanvasError::SurfaceNotFound(surface_id.to_string()))?;
        surface.subscribers.retain(|s| s != client_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_surface() {
        let mgr = CanvasManager::new();
        let surface = mgr
            .create_surface("surf-1", None, None)
            .await
            .unwrap();
        assert_eq!(surface.id, "surf-1");

        let fetched = mgr.get_surface("surf-1").await.unwrap();
        assert_eq!(fetched.id, "surf-1");
    }

    #[tokio::test]
    async fn test_duplicate_surface_fails() {
        let mgr = CanvasManager::new();
        mgr.create_surface("surf-1", None, None).await.unwrap();
        let result = mgr.create_surface("surf-1", None, None).await;
        assert!(matches!(result, Err(CanvasError::SurfaceExists(_))));
    }

    #[tokio::test]
    async fn test_add_element() {
        let mgr = CanvasManager::new();
        mgr.create_surface("surf-1", None, None).await.unwrap();
        let element = CanvasElement {
            id: "el-1".to_string(),
            element_type: "text".to_string(),
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
            properties: serde_json::json!({ "text": "hello" }),
            children: Vec::new(),
        };
        let result = mgr
            .execute("surf-1", CanvasAction::AddElement { element })
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data.unwrap()["id"], "el-1");

        let surface = mgr.get_surface("surf-1").await.unwrap();
        assert_eq!(surface.elements.len(), 1);
    }

    #[tokio::test]
    async fn test_a2ui_push_and_reset() {
        let mgr = CanvasManager::new();
        mgr.create_surface("surf-1", None, None).await.unwrap();

        let elements = vec![
            CanvasElement {
                id: "el-1".to_string(),
                element_type: "box".to_string(),
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
                properties: serde_json::Value::Null,
                children: Vec::new(),
            },
        ];
        let result = mgr
            .execute("surf-1", CanvasAction::A2uiPush { elements })
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data.unwrap()["added"], 1);

        let surface = mgr.get_surface("surf-1").await.unwrap();
        assert_eq!(surface.elements.len(), 1);

        let result = mgr.execute("surf-1", CanvasAction::A2uiReset).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data.unwrap()["reset"], true);

        let surface = mgr.get_surface("surf-1").await.unwrap();
        assert!(surface.elements.is_empty());
    }

    #[tokio::test]
    async fn test_subscribe_unsubscribe() {
        let mgr = CanvasManager::new();
        mgr.create_surface("surf-1", None, None).await.unwrap();
        mgr.subscribe("surf-1", "client-1").await.unwrap();

        let surface = mgr.get_surface("surf-1").await.unwrap();
        assert_eq!(surface.subscribers, vec!["client-1"]);

        mgr.unsubscribe("surf-1", "client-1").await.unwrap();
        let surface = mgr.get_surface("surf-1").await.unwrap();
        assert!(surface.subscribers.is_empty());
    }

    #[tokio::test]
    async fn test_execute_on_missing_surface_fails() {
        let mgr = CanvasManager::new();
        let result = mgr.execute("missing", CanvasAction::Present).await;
        assert!(matches!(result, Err(CanvasError::SurfaceNotFound(_))));
    }
}
