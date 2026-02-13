//! Web tools.
//!
//! - [`WebFetchTool`] - Fetch and extract web content
//! - [`WebSearchTool`] - Search the web

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use reqwest::Client;
use std::time::{Duration, Instant};
use tracing::debug;

/// Web fetch tool - Fetch and extract content from a URL.
pub struct WebFetchTool {
    /// HTTP client.
    client: Client,
    /// Maximum content length (bytes).
    max_content_length: usize,
    /// Request timeout.
    timeout: Duration,
    /// User agent string.
    user_agent: String,
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    /// Create a new web fetch tool.
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            max_content_length: 1024 * 1024, // 1 MB
            timeout: Duration::from_secs(30),
            user_agent: "SmartAssist/1.0".to_string(),
        }
    }

    /// Set maximum content length.
    pub fn with_max_content_length(mut self, max: usize) -> Self {
        self.max_content_length = max;
        self
    }

    /// Set request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set user agent.
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    /// Extract text content from HTML.
    fn extract_text(&self, html: &str) -> String {
        // Simple HTML to text conversion
        // Remove script and style tags
        let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
        let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
        let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
        let re_whitespace = regex::Regex::new(r"\s+").unwrap();

        let text = re_script.replace_all(html, "");
        let text = re_style.replace_all(&text, "");
        let text = re_tags.replace_all(&text, " ");
        let text = re_whitespace.replace_all(&text, " ");

        // Decode common HTML entities
        text.replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .trim()
            .to_string()
    }

    /// Extract title from HTML.
    fn extract_title(&self, html: &str) -> Option<String> {
        let re = regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>").ok()?;
        re.captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
    }

    /// Extract meta description from HTML.
    fn extract_description(&self, html: &str) -> Option<String> {
        let re = regex::Regex::new(
            r#"(?is)<meta[^>]*name\s*=\s*["']description["'][^>]*content\s*=\s*["']([^"']+)["']"#,
        )
        .ok()?;
        re.captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_fetch".to_string(),
            description: "Fetch content from a URL and extract text".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    },
                    "extract": {
                        "type": "string",
                        "enum": ["text", "html", "json"],
                        "description": "Content type to extract (default: text)"
                    },
                    "selector": {
                        "type": "string",
                        "description": "CSS selector to extract specific content (optional)"
                    }
                },
                "required": ["url"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'url' argument"))?;

        let extract = args
            .get("extract")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        debug!("Fetching URL: {}", url);

        // Validate URL
        let parsed_url = url::Url::parse(url)
            .map_err(|e| AgentError::tool_execution(format!("Invalid URL: {}", e)))?;

        // Only allow HTTP/HTTPS
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Ok(ToolResult::error(
                tool_use_id,
                "Only HTTP and HTTPS URLs are supported",
            ));
        }

        // Make request
        let response = self
            .client
            .get(url)
            .header("User-Agent", &self.user_agent)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| AgentError::tool_execution(format!("Request failed: {}", e)))?;

        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if !status.is_success() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("HTTP error: {} {}", status.as_u16(), status.canonical_reason().unwrap_or("")),
            ));
        }

        // Get body with size limit
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AgentError::tool_execution(format!("Failed to read body: {}", e)))?;

        if bytes.len() > self.max_content_length {
            return Ok(ToolResult::error(
                tool_use_id,
                format!(
                    "Content too large: {} bytes (max {})",
                    bytes.len(),
                    self.max_content_length
                ),
            ));
        }

        let body = String::from_utf8_lossy(&bytes).to_string();
        let duration = start.elapsed();

        match extract {
            "json" => {
                // Try to parse as JSON
                match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(json) => Ok(
                        ToolResult::success(tool_use_id, serde_json::json!({
                            "url": url,
                            "status": status.as_u16(),
                            "content_type": content_type,
                            "content": json,
                        }))
                        .with_duration(duration),
                    ),
                    Err(e) => Ok(ToolResult::error(
                        tool_use_id,
                        format!("Failed to parse JSON: {}", e),
                    )),
                }
            }
            "html" => Ok(
                ToolResult::success(tool_use_id, serde_json::json!({
                    "url": url,
                    "status": status.as_u16(),
                    "content_type": content_type,
                    "content": body,
                    "length": body.len(),
                }))
                .with_duration(duration),
            ),
            _ => {
                // Extract text from HTML
                let title = self.extract_title(&body);
                let description = self.extract_description(&body);
                let text = self.extract_text(&body);

                // Truncate text if too long
                let text = if text.len() > 50000 {
                    format!("{}...", &text[..50000])
                } else {
                    text
                };

                Ok(
                    ToolResult::success(tool_use_id, serde_json::json!({
                        "url": url,
                        "status": status.as_u16(),
                        "title": title,
                        "description": description,
                        "content": text,
                        "length": text.len(),
                    }))
                    .with_duration(duration),
                )
            }
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Web
    }
}

/// Web search tool - Search the web using a search API.
pub struct WebSearchTool {
    /// HTTP client.
    client: Client,
    /// Search API endpoint.
    api_endpoint: Option<String>,
    /// API key for search service.
    api_key: Option<String>,
    /// Maximum results to return.
    max_results: usize,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    /// Create a new web search tool.
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            api_endpoint: None,
            api_key: None,
            max_results: 10,
        }
    }

    /// Set the search API endpoint.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.api_endpoint = Some(endpoint.into());
        self
    }

    /// Set the API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set maximum results.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_search".to_string(),
            description: "Search the web for information".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "num_results": {
                        "type": "integer",
                        "description": "Number of results to return (default: 10)"
                    }
                },
                "required": ["query"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'query' argument"))?;

        let num_results = args
            .get("num_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(self.max_results)
            .min(self.max_results);

        debug!("Web search: {} (max {} results)", query, num_results);

        // Check if search API is configured
        let (endpoint, api_key) = match (&self.api_endpoint, &self.api_key) {
            (Some(e), Some(k)) => (e.clone(), k.clone()),
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    "Web search is not configured. Set SMARTASSIST_SEARCH_API_KEY and SMARTASSIST_SEARCH_ENDPOINT environment variables.",
                ));
            }
        };

        // Make search request (assuming a generic search API format)
        let response = self
            .client
            .get(&endpoint)
            .query(&[
                ("q", query),
                ("num", &num_results.to_string()),
            ])
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| AgentError::tool_execution(format!("Search request failed: {}", e)))?;

        if !response.status().is_success() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("Search API error: {}", response.status()),
            ));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AgentError::tool_execution(format!("Failed to parse search response: {}", e)))?;

        // Parse results (format depends on the search API)
        let results: Vec<SearchResult> = body
            .get("results")
            .or_else(|| body.get("items"))
            .or_else(|| body.get("organic"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(SearchResult {
                            title: item.get("title")?.as_str()?.to_string(),
                            url: item.get("url")
                                .or_else(|| item.get("link"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())?,
                            snippet: item.get("snippet")
                                .or_else(|| item.get("description"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                        })
                    })
                    .take(num_results)
                    .collect()
            })
            .unwrap_or_default();

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "query": query,
                "results": results,
                "count": results.len(),
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Web
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_fetch_tool_creation() {
        let tool = WebFetchTool::new();
        assert_eq!(tool.name(), "web_fetch");
    }

    #[test]
    fn test_web_search_tool_creation() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
    }

    #[test]
    fn test_extract_text() {
        let tool = WebFetchTool::new();
        let html = r#"
            <html>
            <head><title>Test</title></head>
            <body>
                <script>var x = 1;</script>
                <style>.foo { color: red; }</style>
                <p>Hello World</p>
                <p>Another paragraph</p>
            </body>
            </html>
        "#;

        let text = tool.extract_text(html);
        assert!(text.contains("Hello World"));
        assert!(text.contains("Another paragraph"));
        assert!(!text.contains("var x"));
        assert!(!text.contains("color: red"));
    }

    #[test]
    fn test_extract_title() {
        let tool = WebFetchTool::new();
        let html = "<html><head><title>My Page Title</title></head></html>";
        assert_eq!(tool.extract_title(html), Some("My Page Title".to_string()));
    }

    #[test]
    fn test_extract_description() {
        let tool = WebFetchTool::new();
        let html = r#"<html><head><meta name="description" content="Page description here"></head></html>"#;
        assert_eq!(
            tool.extract_description(html),
            Some("Page description here".to_string())
        );
    }
}
