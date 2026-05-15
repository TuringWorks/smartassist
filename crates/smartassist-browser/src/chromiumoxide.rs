//! Chromiumoxide-based browser backend.
//!
//! Provides real browser automation using `chromiumoxide` with Chrome/Chromium.
//! Each session owns a `Browser` instance and an optional current `Page`.

use crate::{
    BrowserAction, BrowserActionResult, BrowserBackend, BrowserError, BrowserSession, LaunchOptions,
};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Internal session state holding the live browser, page, and metadata.
struct SessionState {
    browser: Browser,
    page: Option<chromiumoxide::Page>,
    session: BrowserSession,
    #[allow(dead_code)]
    _handler_task: tokio::task::JoinHandle<()>,
}

/// Browser backend powered by `chromiumoxide`.
pub struct ChromiumoxideBackend {
    sessions: RwLock<HashMap<String, SessionState>>,
}

impl ChromiumoxideBackend {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl BrowserBackend for ChromiumoxideBackend {
    async fn launch(&self,
        options: LaunchOptions,
    ) -> Result<String, BrowserError> {
        let mut builder = BrowserConfig::builder();

        if options.headless {
            builder = builder.arg("--headless=new");
        }
        if !options.sandboxed {
            builder = builder.arg("--no-sandbox");
        }

        builder = builder
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-background-networking")
            .arg("--disable-background-timer-throttling")
            .arg("--disable-client-side-phishing-detection")
            .arg("--disable-default-apps")
            .arg("--disable-hang-monitor")
            .arg("--disable-popup-blocking")
            .arg("--disable-prompt-on-repost")
            .arg("--disable-sync")
            .arg("--metrics-recording-only")
            .arg("--no-first-run")
            .arg("--safebrowsing-disable-auto-update");

        if let Some(ref proxy) = options.proxy {
            builder = builder.arg(format!("--proxy-server={}", proxy));
        }
        if let Some(ref ua) = options.user_agent {
            builder = builder.arg(format!("--user-agent={}", ua));
        }
        for arg in &options.extra_args {
            builder = builder.arg(arg.clone());
        }

        let config = builder
            .build()
            .map_err(|e| BrowserError::Other(anyhow::anyhow!(e)))?;

        let (browser, mut handler) = Browser::launch(config).await.map_err(|e| {
            BrowserError::NavigationFailed(format!("Failed to launch browser: {}", e))
        })?;

        // Drive the CDP event loop in a background task.
        let handler_task = tokio::spawn(async move {
            while handler.next().await.is_some() {}
        });

        let id = uuid::Uuid::new_v4().to_string();
        let session = BrowserSession {
            id: id.clone(),
            url: None,
            viewport: options.viewport.clone(),
            headless: options.headless,
            sandboxed: options.sandboxed,
            created_at: chrono::Utc::now(),
        };

        self.sessions.write().await.insert(
            id.clone(),
            SessionState {
                browser,
                page: None,
                session,
                _handler_task: handler_task,
            },
        );

        Ok(id)
    }

    async fn close(&self, session_id: &str) -> Result<(), BrowserError> {
        self.sessions.write().await.remove(session_id);
        Ok(())
    }

    async fn execute(
        &self,
        session_id: &str,
        action: BrowserAction,
    ) -> Result<BrowserActionResult, BrowserError> {
        let mut sessions = self.sessions.write().await;
        let state = sessions
            .get_mut(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_string()))?;

        match action {
            BrowserAction::Navigate { url } => {
                let page = state
                    .browser
                    .new_page(&url)
                    .await
                    .map_err(|e| BrowserError::NavigationFailed(format!("{}", e)))?;

                let title = page.get_title().await.ok().flatten().unwrap_or_default();
                state.session.url = Some(url.clone());
                state.page = Some(page);

                Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "url": url, "title": title })),
                    screenshot: None,
                    message: None,
                })
            }

            BrowserAction::Click { selector } => {
                let page = state
                    .page
                    .as_ref()
                    .ok_or(BrowserError::NotInitialized)?;

                page.find_element(selector.as_str())
                    .await
                    .map_err(|e| {
                        BrowserError::ElementNotFound(format!(
                            "Failed to find element '{}': {}",
                            selector, e
                        ))
                    })?
                    .click()
                    .await
                    .map_err(|e| BrowserError::Other(anyhow::anyhow!(e)))?;

                Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "selector": selector })),
                    screenshot: None,
                    message: None,
                })
            }

            BrowserAction::Type { selector, text } => {
                let page = state
                    .page
                    .as_ref()
                    .ok_or(BrowserError::NotInitialized)?;

                page.find_element(&selector)
                    .await
                    .map_err(|e| {
                        BrowserError::ElementNotFound(format!(
                            "Failed to find element '{}': {}",
                            selector, e
                        ))
                    })?
                    .type_str(text.as_str())
                    .await
                    .map_err(|e| BrowserError::Other(anyhow::anyhow!(e)))?;

                Ok(BrowserActionResult {
                    success: true,
                    data: Some(
                        serde_json::json!({ "selector": selector, "text_length": text.len() }),
                    ),
                    screenshot: None,
                    message: None,
                })
            }

            BrowserAction::Screenshot { full_page } => {
                let page = state
                    .page
                    .as_ref()
                    .ok_or(BrowserError::NotInitialized)?;

                let screenshot = page
                    .screenshot(ScreenshotParams::builder().full_page(full_page).build())
                    .await
                    .map_err(|e| {
                        BrowserError::ScreenshotFailed(format!("Screenshot failed: {}", e))
                    })?;

                Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "size": screenshot.len() })),
                    screenshot: Some(screenshot),
                    message: None,
                })
            }

            BrowserAction::Content => {
                let page = state
                    .page
                    .as_ref()
                    .ok_or(BrowserError::NotInitialized)?;

                let html = page.content().await.map_err(|e| BrowserError::Other(anyhow::anyhow!(e)))?;
                let text = page
                    .evaluate("document.body.innerText")
                    .await
                    .map_err(|e| BrowserError::Other(anyhow::anyhow!(e)))?
                    .into_value::<String>()
                    .unwrap_or_default();

                Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "html_length": html.len(), "text": text })),
                    screenshot: None,
                    message: None,
                })
            }

            BrowserAction::Wait { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "waited_ms": ms })),
                    screenshot: None,
                    message: None,
                })
            }

            BrowserAction::Evaluate { script } => {
                let page = state
                    .page
                    .as_ref()
                    .ok_or(BrowserError::NotInitialized)?;

                let result = page
                    .evaluate(script.as_str())
                    .await
                    .map_err(|e| BrowserError::Other(anyhow::anyhow!(e)))?;
                let value: serde_json::Value =
                    result.into_value().unwrap_or(serde_json::Value::Null);

                Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "result": value })),
                    screenshot: None,
                    message: None,
                })
            }

            BrowserAction::Scroll { x, y } => {
                let page = state
                    .page
                    .as_ref()
                    .ok_or(BrowserError::NotInitialized)?;

                let script = format!("window.scrollBy({}, {})", x, y);
                page.evaluate(script.as_str())
                    .await
                    .map_err(|e| BrowserError::Other(anyhow::anyhow!(e)))?;

                Ok(BrowserActionResult {
                    success: true,
                    data: Some(serde_json::json!({ "x": x, "y": y })),
                    screenshot: None,
                    message: None,
                })
            }
        }
    }

    async fn list_sessions(&self,
    ) -> Result<Vec<BrowserSession>, BrowserError> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values().map(|s| s.session.clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chromiumoxide_backend_new() {
        let backend = ChromiumoxideBackend::new();
        // Just verify construction succeeds.
        assert!(true);
    }
}
