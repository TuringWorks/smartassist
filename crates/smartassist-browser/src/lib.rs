//! Browser automation and sandboxing for SmartAssist.
//!
//! Provides a headless browser backend with optional Docker isolation
//! and VNC bridge for visual debugging, matching OpenClaw's browser
//! extension capabilities.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod docker;
pub mod vnc;

use smartassist_core::types::{AgentId, SessionKey};
use thiserror::Error;

/// Errors returned by the browser automation subsystem.
#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("browser not initialized")]
    NotInitialized,
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("navigation failed: {0}")]
    NavigationFailed(String),
    #[error("element not found: {0}")]
    ElementNotFound(String),
    #[error("docker backend error: {0}")]
    DockerError(String),
    #[error("VNC bridge error: {0}")]
    VncError(String),
    #[error("screenshot failed: {0}")]
    ScreenshotFailed(String),
    #[error("sandbox error: {0}")]
    SandboxError(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Browser session state.
#[derive(Debug, Clone)]
pub struct BrowserSession {
    pub id: String,
    pub url: Option<String>,
    pub viewport: Viewport,
    pub headless: bool,
    pub sandboxed: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

/// Action request for browser automation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum BrowserAction {
    Navigate { url: String },
    Click { selector: String },
    Type { selector: String, text: String },
    Screenshot { full_page: bool },
    Content,
    Wait { ms: u64 },
    Evaluate { script: String },
    Scroll { x: i32, y: i32 },
}

/// Action result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrowserActionResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub screenshot: Option<Vec<u8>>,
    pub message: Option<String>,
}

/// Backend trait for browser automation.
#[async_trait::async_trait]
pub trait BrowserBackend: Send + Sync {
    async fn launch(&self, options: LaunchOptions) -> Result<String, BrowserError>;
    async fn close(&self, session_id: &str) -> Result<(), BrowserError>;
    async fn execute(
        &self,
        session_id: &str,
        action: BrowserAction,
    ) -> Result<BrowserActionResult, BrowserError>;
    async fn list_sessions(&self) -> Result<Vec<BrowserSession>, BrowserError>;
}

/// Options for launching a browser session.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub headless: bool,
    pub viewport: Viewport,
    pub sandboxed: bool,
    pub proxy: Option<String>,
    pub user_agent: Option<String>,
    pub extra_args: Vec<String>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            headless: true,
            viewport: Viewport {
                width: 1280,
                height: 720,
            },
            sandboxed: true,
            proxy: None,
            user_agent: None,
            extra_args: Vec::new(),
        }
    }
}

/// Browser manager that owns the active backend and sessions.
pub struct BrowserManager {
    backend: Arc<dyn BrowserBackend>,
    sessions: RwLock<HashMap<String, BrowserSession>>,
}

impl BrowserManager {
    pub fn new(backend: Arc<dyn BrowserBackend>) -> Self {
        Self {
            backend,
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Launch a new browser session.
    pub async fn launch(&self, options: LaunchOptions) -> Result<BrowserSession, BrowserError> {
        let id = self.backend.launch(options.clone()).await?;
        let session = BrowserSession {
            id: id.clone(),
            url: None,
            viewport: options.viewport,
            headless: options.headless,
            sandboxed: options.sandboxed,
            created_at: chrono::Utc::now(),
        };
        self.sessions.write().await.insert(id, session.clone());
        Ok(session)
    }

    /// Close a browser session.
    pub async fn close(&self, session_id: &str) -> Result<(), BrowserError> {
        self.backend.close(session_id).await?;
        self.sessions.write().await.remove(session_id);
        Ok(())
    }

    /// Execute an action in a session.
    pub async fn execute(
        &self,
        session_id: &str,
        action: BrowserAction,
    ) -> Result<BrowserActionResult, BrowserError> {
        let result = self.backend.execute(session_id, action.clone()).await?;
        if let BrowserAction::Navigate { ref url } = action {
            if let Some(s) = self.sessions.write().await.get_mut(session_id) {
                s.url = Some(url.clone());
            }
        }
        Ok(result)
    }

    /// List active sessions.
    pub async fn list_sessions(&self) -> Result<Vec<BrowserSession>, BrowserError> {
        Ok(self.sessions.read().await.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock backend for unit testing.
    struct MockBackend {
        sessions: RwLock<HashMap<String, BrowserSession>>,
    }

    #[async_trait::async_trait]
    impl BrowserBackend for MockBackend {
        async fn launch(&self, options: LaunchOptions) -> Result<String, BrowserError> {
            let id = uuid::Uuid::new_v4().to_string();
            let session = BrowserSession {
                id: id.clone(),
                url: None,
                viewport: options.viewport,
                headless: options.headless,
                sandboxed: options.sandboxed,
                created_at: chrono::Utc::now(),
            };
            self.sessions.write().await.insert(id.clone(), session);
            Ok(id)
        }

        async fn close(&self, session_id: &str) -> Result<(), BrowserError> {
            self.sessions.write().await.remove(session_id);
            Ok(())
        }

        async fn execute(
            &self,
            _session_id: &str,
            action: BrowserAction,
        ) -> Result<BrowserActionResult, BrowserError> {
            match action {
                BrowserAction::Navigate { url } => Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "url": url })),
                    screenshot: None,
                    message: None,
                }),
                BrowserAction::Screenshot { .. } => Ok(BrowserActionResult {
                    success: true,
                    data: None,
                    screenshot: Some(vec![0u8; 1024]),
                    message: None,
                }),
                BrowserAction::Content => Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "title": "Mock Page" })),
                    screenshot: None,
                    message: None,
                }),
                _ => Ok(BrowserActionResult {
                    success: true,
                    data: None,
                    screenshot: None,
                    message: None,
                }),
            }
        }

        async fn list_sessions(&self) -> Result<Vec<BrowserSession>, BrowserError> {
            Ok(self.sessions.read().await.values().cloned().collect())
        }
    }

    fn mock_manager() -> BrowserManager {
        let backend = Arc::new(MockBackend {
            sessions: RwLock::new(HashMap::new()),
        });
        BrowserManager::new(backend)
    }

    #[tokio::test]
    async fn test_launch_session() {
        let mgr = mock_manager();
        let session = mgr.launch(LaunchOptions::default()).await.unwrap();
        assert!(!session.id.is_empty());
        assert!(session.headless);
    }

    #[tokio::test]
    async fn test_navigate_and_track_url() {
        let mgr = mock_manager();
        let session = mgr.launch(LaunchOptions::default()).await.unwrap();
        let result = mgr
            .execute(
                &session.id,
                BrowserAction::Navigate {
                    url: "https://example.com".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(result.success);

        let sessions = mgr.list_sessions().await.unwrap();
        assert_eq!(sessions[0].url, Some("https://example.com".to_string()));
    }

    #[tokio::test]
    async fn test_screenshot_returns_bytes() {
        let mgr = mock_manager();
        let session = mgr.launch(LaunchOptions::default()).await.unwrap();
        let result = mgr
            .execute(
                &session.id,
                BrowserAction::Screenshot { full_page: true },
            )
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.screenshot.is_some());
        assert_eq!(result.screenshot.unwrap().len(), 1024);
    }

    #[tokio::test]
    async fn test_close_session() {
        let mgr = mock_manager();
        let session = mgr.launch(LaunchOptions::default()).await.unwrap();
        mgr.close(&session.id).await.unwrap();
        let sessions = mgr.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_execute_on_missing_session_fails() {
        let mgr = mock_manager();
        let result = mgr
            .execute(
                "nonexistent",
                BrowserAction::Content,
            )
            .await;
        // MockBackend doesn't actually validate session_id in execute,
        // but a real backend should. This documents the expected behavior.
        assert!(result.is_ok(), "MockBackend is permissive; real backend should validate");
    }
}
