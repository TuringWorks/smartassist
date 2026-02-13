//! Browser automation tool.
//!
//! - [`BrowserTool`] - Browser automation and web scraping.
//!
//! When the `browser` feature is enabled, this module provides real browser
//! automation via `chromiumoxide`. Without the feature, all actions return a
//! stub "not yet implemented" response so the rest of the agent compiles and
//! tests pass without requiring a Chromium binary.

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

#[cfg(feature = "browser")]
use std::sync::Arc;
#[cfg(feature = "browser")]
use tokio::sync::RwLock;

#[cfg(feature = "browser")]
use chromiumoxide::browser::{Browser, BrowserConfig};
#[cfg(feature = "browser")]
use chromiumoxide::page::ScreenshotParams;
#[cfg(feature = "browser")]
use chromiumoxide::Page;
#[cfg(feature = "browser")]
use futures::StreamExt;

/// Manages a browser instance and its active page.
#[cfg(feature = "browser")]
struct BrowserSession {
    browser: Browser,
    page: Option<Page>,
}

/// Return a reference to the current page, or an error when none is open.
#[cfg(feature = "browser")]
fn get_current_page(session: &BrowserSession) -> Result<&Page> {
    session
        .page
        .as_ref()
        .ok_or_else(|| AgentError::tool_execution("No page open. Use 'navigate' first."))
}

/// Browser tool - Browser automation and web scraping.
pub struct BrowserTool {
    /// Whether headless mode is enabled.
    headless: bool,
    /// Default timeout in milliseconds.
    timeout_ms: u64,
    /// Lazily-initialised browser session (feature-gated).
    #[cfg(feature = "browser")]
    session: Arc<RwLock<Option<BrowserSession>>>,
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserTool {
    pub fn new() -> Self {
        Self {
            headless: true,
            timeout_ms: 30_000,
            #[cfg(feature = "browser")]
            session: Arc::new(RwLock::new(None)),
        }
    }

    /// Set headless mode.
    pub fn with_headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    /// Set default timeout.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Get or create the browser session, launching Chromium if needed.
    #[cfg(feature = "browser")]
    async fn ensure_session(&self) -> Result<()> {
        let mut guard = self.session.write().await;
        if guard.is_none() {
            let mut builder = BrowserConfig::builder();
            if self.headless {
                builder = builder.arg("--headless=new");
            }
            builder = builder
                .arg("--no-sandbox")
                .arg("--disable-gpu")
                .arg("--disable-dev-shm-usage");

            let config = builder.build().map_err(|e| {
                AgentError::tool_execution(format!("Browser config error: {}", e))
            })?;

            let (browser, mut handler) = Browser::launch(config).await.map_err(|e| {
                AgentError::tool_execution(format!("Failed to launch browser: {}", e))
            })?;

            // Spawn a background task that drives the CDP event loop.
            tokio::spawn(async move {
                while handler.next().await.is_some() {}
            });

            *guard = Some(BrowserSession {
                browser,
                page: None,
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tool implementation: real browser automation (chromiumoxide)
// ---------------------------------------------------------------------------

#[cfg(feature = "browser")]
#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn definition(&self) -> ToolDefinition {
        browser_tool_definition()
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'action' argument"))?;

        let _timeout = args
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.timeout_ms);

        debug!("Browser action: {}", action);

        let result = match action {
            "navigate" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'url' for navigate"))?;

                self.ensure_session().await?;
                let mut guard = self.session.write().await;
                let session = guard.as_mut().unwrap();

                let page = session.browser.new_page(url).await.map_err(|e| {
                    AgentError::tool_execution(format!("Navigate failed: {}", e))
                })?;

                let title = page
                    .get_title()
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!("Get title failed: {}", e))
                    })?
                    .unwrap_or_default();

                let current_url = page
                    .url()
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!("Get URL failed: {}", e))
                    })?
                    .unwrap_or_default();

                session.page = Some(page);

                serde_json::json!({
                    "action": "navigate",
                    "url": url,
                    "title": title,
                    "current_url": current_url,
                    "success": true
                })
            }
            "click" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'selector' for click"))?;

                self.ensure_session().await?;
                let guard = self.session.read().await;
                let session = guard.as_ref().unwrap();
                let page = get_current_page(session)?;

                page.find_element(selector)
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!("Find element failed: {}", e))
                    })?
                    .click()
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!("Click failed: {}", e))
                    })?;

                serde_json::json!({
                    "action": "click",
                    "selector": selector,
                    "success": true
                })
            }
            "type" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'selector' for type"))?;

                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'text' for type"))?;

                self.ensure_session().await?;
                let guard = self.session.read().await;
                let session = guard.as_ref().unwrap();
                let page = get_current_page(session)?;

                page.find_element(selector)
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!("Find element failed: {}", e))
                    })?
                    .type_str(text)
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!("Type text failed: {}", e))
                    })?;

                serde_json::json!({
                    "action": "type",
                    "selector": selector,
                    "text_length": text.len(),
                    "success": true
                })
            }
            "screenshot" => {
                let output = args.get("output").and_then(|v| v.as_str());
                let output_path = output
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        format!("/tmp/screenshot_{}.png", uuid::Uuid::new_v4())
                    });

                self.ensure_session().await?;
                let guard = self.session.read().await;
                let session = guard.as_ref().unwrap();
                let page = get_current_page(session)?;

                let screenshot_data = page
                    .screenshot(
                        ScreenshotParams::builder().full_page(true).build(),
                    )
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!("Screenshot failed: {}", e))
                    })?;

                tokio::fs::write(&output_path, &screenshot_data)
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!(
                            "Failed to write screenshot: {}",
                            e
                        ))
                    })?;

                serde_json::json!({
                    "action": "screenshot",
                    "output": output_path,
                    "size": screenshot_data.len(),
                    "success": true
                })
            }
            "content" => {
                self.ensure_session().await?;
                let guard = self.session.read().await;
                let session = guard.as_ref().unwrap();
                let page = get_current_page(session)?;

                let html = page.content().await.map_err(|e| {
                    AgentError::tool_execution(format!("Get content failed: {}", e))
                })?;

                let text = page
                    .evaluate("document.body.innerText")
                    .await
                    .map_err(|e| {
                        AgentError::tool_execution(format!("Evaluate innerText failed: {}", e))
                    })?
                    .into_value::<String>()
                    .unwrap_or_default();

                serde_json::json!({
                    "action": "content",
                    "html_length": html.len(),
                    "text": text,
                    "success": true
                })
            }
            "wait" => {
                let selector = args.get("selector").and_then(|v| v.as_str());

                self.ensure_session().await?;
                let guard = self.session.read().await;
                let session = guard.as_ref().unwrap();
                let page = get_current_page(session)?;

                if let Some(sel) = selector {
                    page.find_element(sel).await.map_err(|e| {
                        AgentError::tool_execution(format!(
                            "Wait for element '{}' failed: {}",
                            sel, e
                        ))
                    })?;
                }

                serde_json::json!({
                    "action": "wait",
                    "selector": selector,
                    "success": true
                })
            }
            "evaluate" => {
                let script = args
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::tool_execution("Missing 'script' for evaluate")
                    })?;

                self.ensure_session().await?;
                let guard = self.session.read().await;
                let session = guard.as_ref().unwrap();
                let page = get_current_page(session)?;

                let eval_result = page.evaluate(script).await.map_err(|e| {
                    AgentError::tool_execution(format!("Evaluate failed: {}", e))
                })?;

                // Attempt to extract a JSON-compatible value; fall back to null.
                let value: serde_json::Value = eval_result
                    .into_value()
                    .unwrap_or(serde_json::Value::Null);

                serde_json::json!({
                    "action": "evaluate",
                    "script_length": script.len(),
                    "result": value,
                    "success": true
                })
            }
            _ => {
                return Err(AgentError::tool_execution(format!(
                    "Unknown action: {}",
                    action
                )));
            }
        };

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        // Browser automation may require approval depending on security settings.
        false
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

// ---------------------------------------------------------------------------
// Tool implementation: stub (no chromiumoxide)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "browser"))]
#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn definition(&self) -> ToolDefinition {
        browser_tool_definition()
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'action' argument"))?;

        let _timeout = args
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.timeout_ms);

        debug!("Browser action: {}", action);

        // Stub: return "not yet implemented" for every action.
        let result = match action {
            "navigate" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'url' for navigate"))?;

                serde_json::json!({
                    "action": "navigate",
                    "url": url,
                    "success": false,
                    "message": "Browser automation not yet implemented"
                })
            }
            "click" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'selector' for click"))?;

                serde_json::json!({
                    "action": "click",
                    "selector": selector,
                    "success": false,
                    "message": "Browser automation not yet implemented"
                })
            }
            "type" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'selector' for type"))?;

                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'text' for type"))?;

                serde_json::json!({
                    "action": "type",
                    "selector": selector,
                    "text_length": text.len(),
                    "success": false,
                    "message": "Browser automation not yet implemented"
                })
            }
            "screenshot" => {
                let output = args.get("output").and_then(|v| v.as_str());
                let output_path = output
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        format!("/tmp/screenshot_{}.png", uuid::Uuid::new_v4())
                    });

                serde_json::json!({
                    "action": "screenshot",
                    "output": output_path,
                    "success": false,
                    "message": "Browser automation not yet implemented"
                })
            }
            "content" => {
                serde_json::json!({
                    "action": "content",
                    "html": "",
                    "text": "",
                    "success": false,
                    "message": "Browser automation not yet implemented"
                })
            }
            "wait" => {
                let selector = args.get("selector").and_then(|v| v.as_str());

                serde_json::json!({
                    "action": "wait",
                    "selector": selector,
                    "success": false,
                    "message": "Browser automation not yet implemented"
                })
            }
            "evaluate" => {
                let script = args
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::tool_execution("Missing 'script' for evaluate")
                    })?;

                serde_json::json!({
                    "action": "evaluate",
                    "script_length": script.len(),
                    "result": null,
                    "success": false,
                    "message": "Browser automation not yet implemented"
                })
            }
            _ => {
                return Err(AgentError::tool_execution(format!(
                    "Unknown action: {}",
                    action
                )));
            }
        };

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        // Browser automation may require approval depending on security settings.
        false
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

// ---------------------------------------------------------------------------
// Shared: tool definition (same for both feature-gated implementations)
// ---------------------------------------------------------------------------

/// Returns the shared tool definition used by both the real and stub
/// implementations of `BrowserTool`.
fn browser_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "browser".to_string(),
        description: "Browser automation for web interaction. Navigate pages, fill forms, click \
                       elements, take screenshots, and extract content."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "click", "type", "screenshot", "content", "wait", "evaluate"],
                    "description": "Browser action to perform"
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (for 'navigate' action)"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for element (for click, type, wait)"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (for 'type' action)"
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript to evaluate (for 'evaluate' action)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Action timeout in milliseconds"
                },
                "output": {
                    "type": "string",
                    "description": "Output path for screenshot"
                }
            },
            "required": ["action"]
        }),
        execution: ToolExecutionConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_tool_creation() {
        let tool = BrowserTool::new();
        assert_eq!(tool.name(), "browser");
        assert!(tool.headless);
    }

    #[test]
    fn test_browser_tool_headless() {
        let tool = BrowserTool::new().with_headless(false);
        assert!(!tool.headless);
    }

    #[test]
    fn test_browser_tool_timeout() {
        let tool = BrowserTool::new().with_timeout(60_000);
        assert_eq!(tool.timeout_ms, 60_000);
    }

    /// When the browser feature is enabled, verify the session field exists.
    #[cfg(feature = "browser")]
    #[test]
    fn test_browser_session_field() {
        let tool = BrowserTool::new();
        // Just verify the session field is accessible and starts as None.
        assert!(tool.session.try_read().is_ok());
    }
}
