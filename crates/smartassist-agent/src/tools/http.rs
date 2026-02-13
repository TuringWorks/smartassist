//! HTTP client tools for API interactions.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tool for making HTTP requests.
pub struct HttpRequestTool {
    client: reqwest::Client,
}

impl HttpRequestTool {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl Default for HttpRequestTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct HttpRequestArgs {
    /// HTTP method
    method: String,
    /// URL to request
    url: String,
    /// Request headers
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
    /// Request body (for POST/PUT/PATCH)
    #[serde(default)]
    body: Option<serde_json::Value>,
    /// Timeout in seconds
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
struct HttpResponse {
    status: u16,
    status_text: String,
    headers: HashMap<String, String>,
    body: serde_json::Value,
    response_time_ms: u64,
}

#[async_trait]
impl Tool for HttpRequestTool {
    fn name(&self) -> &str {
        "http_request"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "http_request".to_string(),
            description: "Make HTTP requests to APIs".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "method": {
                        "type": "string",
                        "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"],
                        "description": "HTTP method"
                    },
                    "url": {
                        "type": "string",
                        "description": "URL to request"
                    },
                    "headers": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Request headers"
                    },
                    "body": {
                        "description": "Request body (JSON)"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Request timeout in seconds (default: 30)"
                    }
                },
                "required": ["method", "url"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Web
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: HttpRequestArgs = serde_json::from_value(args)?;

        let method = args.method.to_uppercase();
        let timeout = Duration::from_secs(args.timeout_secs.unwrap_or(30));

        // Build the request
        let mut request = match method.as_str() {
            "GET" => self.client.get(&args.url),
            "POST" => self.client.post(&args.url),
            "PUT" => self.client.put(&args.url),
            "PATCH" => self.client.patch(&args.url),
            "DELETE" => self.client.delete(&args.url),
            "HEAD" => self.client.head(&args.url),
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unsupported HTTP method: {}", method),
                ));
            }
        };

        // Set timeout
        request = request.timeout(timeout);

        // Add headers
        if let Some(headers) = args.headers {
            for (key, value) in headers {
                request = request.header(&key, &value);
            }
        }

        // Add body
        if let Some(body) = args.body {
            request = request.json(&body);
        }

        // Execute request
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                let status_code = status.as_u16();
                let status_text = status.canonical_reason().unwrap_or("Unknown").to_string();

                // Collect headers
                let headers: HashMap<String, String> = response
                    .headers()
                    .iter()
                    .map(|(k, v)| {
                        (k.to_string(), v.to_str().unwrap_or("").to_string())
                    })
                    .collect();

                // Get body
                let body_text = response.text().await.unwrap_or_default();
                let body: serde_json::Value = serde_json::from_str(&body_text)
                    .unwrap_or_else(|_| json!(body_text));

                let elapsed = start.elapsed().as_millis() as u64;

                let result = HttpResponse {
                    status: status_code,
                    status_text,
                    headers,
                    body,
                    response_time_ms: elapsed,
                };

                Ok(ToolResult::success(
                    tool_use_id,
                    json!(result),
                ).with_duration(start.elapsed()))
            }
            Err(e) => {
                let elapsed = start.elapsed().as_millis() as u64;
                Ok(ToolResult::error(
                    tool_use_id,
                    format!("Request failed after {}ms: {}", elapsed, e),
                ))
            }
        }
    }
}

/// Tool for URL parsing and manipulation.
pub struct UrlParseTool;

impl UrlParseTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UrlParseTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct UrlParseArgs {
    /// URL to parse
    url: String,
}

#[async_trait]
impl Tool for UrlParseTool {
    fn name(&self) -> &str {
        "url_parse"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "url_parse".to_string(),
            description: "Parse a URL into its components".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to parse"
                    }
                },
                "required": ["url"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Web
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: UrlParseArgs = serde_json::from_value(args)?;

        match url::Url::parse(&args.url) {
            Ok(parsed) => {
                let query_params: HashMap<String, String> = parsed
                    .query_pairs()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                Ok(ToolResult::success(
                    tool_use_id,
                    json!({
                        "scheme": parsed.scheme(),
                        "host": parsed.host_str(),
                        "port": parsed.port(),
                        "path": parsed.path(),
                        "query": parsed.query(),
                        "query_params": query_params,
                        "fragment": parsed.fragment(),
                        "username": if parsed.username().is_empty() { None } else { Some(parsed.username()) },
                        "origin": parsed.origin().unicode_serialization()
                    }),
                ).with_duration(start.elapsed()))
            }
            Err(e) => Ok(ToolResult::error(
                tool_use_id,
                format!("Failed to parse URL: {}", e),
            )),
        }
    }
}

/// Tool for building URLs with query parameters.
pub struct UrlBuildTool;

impl UrlBuildTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UrlBuildTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct UrlBuildArgs {
    /// Base URL
    base: String,
    /// Path to append
    #[serde(default)]
    path: Option<String>,
    /// Query parameters
    #[serde(default)]
    params: Option<HashMap<String, String>>,
    /// Fragment
    #[serde(default)]
    fragment: Option<String>,
}

#[async_trait]
impl Tool for UrlBuildTool {
    fn name(&self) -> &str {
        "url_build"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "url_build".to_string(),
            description: "Build a URL with path and query parameters".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "base": {
                        "type": "string",
                        "description": "Base URL"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to append"
                    },
                    "params": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Query parameters"
                    },
                    "fragment": {
                        "type": "string",
                        "description": "URL fragment (hash)"
                    }
                },
                "required": ["base"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Web
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: UrlBuildArgs = serde_json::from_value(args)?;

        match url::Url::parse(&args.base) {
            Ok(mut url) => {
                // Append path if provided
                if let Some(path) = args.path {
                    let base_path = url.path().trim_end_matches('/');
                    let new_path = path.trim_start_matches('/');
                    url.set_path(&format!("{}/{}", base_path, new_path));
                }

                // Add query parameters
                if let Some(params) = args.params {
                    let mut query = url.query_pairs_mut();
                    for (key, value) in params {
                        query.append_pair(&key, &value);
                    }
                }

                // Set fragment
                if let Some(fragment) = args.fragment {
                    url.set_fragment(Some(&fragment));
                }

                Ok(ToolResult::success(
                    tool_use_id,
                    json!({
                        "url": url.to_string()
                    }),
                ).with_duration(start.elapsed()))
            }
            Err(e) => Ok(ToolResult::error(
                tool_use_id,
                format!("Failed to parse base URL: {}", e),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_url_parse() {
        let tool = UrlParseTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "url": "https://example.com:8080/path?foo=bar&baz=qux#section"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert_eq!(output["scheme"], "https");
        assert_eq!(output["host"], "example.com");
        assert_eq!(output["port"], 8080);
        assert_eq!(output["path"], "/path");
        assert_eq!(output["fragment"], "section");
    }

    #[tokio::test]
    async fn test_url_build() {
        let tool = UrlBuildTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "base": "https://api.example.com",
                "path": "/v1/users",
                "params": {
                    "page": "1",
                    "limit": "10"
                }
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        let url = output["url"].as_str().unwrap();
        assert!(url.starts_with("https://api.example.com/v1/users"));
        assert!(url.contains("page=1"));
        assert!(url.contains("limit=10"));
    }

    #[tokio::test]
    async fn test_url_parse_invalid() {
        let tool = UrlParseTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "url": "not a valid url"
            }),
            &context,
        ).await.unwrap();

        assert!(result.is_error);
    }
}
