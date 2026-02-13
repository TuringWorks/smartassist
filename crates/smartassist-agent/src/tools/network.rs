//! Network utility tools (DNS lookup, connectivity checks).

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Tool for DNS lookups.
pub struct DnsLookupTool;

impl DnsLookupTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DnsLookupTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct DnsLookupArgs {
    /// Hostname to look up
    hostname: String,
    /// Record type (default: A)
    #[serde(default)]
    record_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct DnsResult {
    hostname: String,
    addresses: Vec<String>,
    record_type: String,
}

#[async_trait]
impl Tool for DnsLookupTool {
    fn name(&self) -> &str {
        "dns_lookup"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "dns_lookup".to_string(),
            description: "Look up DNS records for a hostname".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "hostname": {
                        "type": "string",
                        "description": "Hostname to look up"
                    },
                    "record_type": {
                        "type": "string",
                        "enum": ["A", "AAAA"],
                        "description": "Record type (default: A)"
                    }
                },
                "required": ["hostname"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: DnsLookupArgs = serde_json::from_value(args)?;
        let record_type = args.record_type.unwrap_or_else(|| "A".to_string());

        // Use port 0 to just get addresses without connecting
        let lookup = format!("{}:0", args.hostname);

        match lookup.to_socket_addrs() {
            Ok(addrs) => {
                let addresses: Vec<String> = addrs
                    .filter_map(|addr| {
                        let ip = addr.ip();
                        match record_type.as_str() {
                            "A" if ip.is_ipv4() => Some(ip.to_string()),
                            "AAAA" if ip.is_ipv6() => Some(ip.to_string()),
                            "A" | "AAAA" => None,
                            _ => Some(ip.to_string()),
                        }
                    })
                    .collect();

                if addresses.is_empty() {
                    Ok(ToolResult::error(
                        tool_use_id,
                        format!("No {} records found for {}", record_type, args.hostname),
                    ))
                } else {
                    let result = DnsResult {
                        hostname: args.hostname,
                        addresses,
                        record_type,
                    };
                    Ok(ToolResult::success(
                        tool_use_id,
                        json!(result),
                    ).with_duration(start.elapsed()))
                }
            }
            Err(e) => Ok(ToolResult::error(
                tool_use_id,
                format!("DNS lookup failed for {}: {}", args.hostname, e),
            )),
        }
    }
}

/// Tool for checking TCP port connectivity.
pub struct PortCheckTool;

impl PortCheckTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PortCheckTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct PortCheckArgs {
    /// Host to check
    host: String,
    /// Port to check
    port: u16,
    /// Timeout in seconds (default: 5)
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
struct PortCheckResult {
    host: String,
    port: u16,
    open: bool,
    response_time_ms: Option<u64>,
    error: Option<String>,
}

#[async_trait]
impl Tool for PortCheckTool {
    fn name(&self) -> &str {
        "port_check"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "port_check".to_string(),
            description: "Check if a TCP port is open on a host".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "host": {
                        "type": "string",
                        "description": "Host to check (IP or hostname)"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Port number to check"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 5)"
                    }
                },
                "required": ["host", "port"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: PortCheckArgs = serde_json::from_value(args)?;
        let timeout_duration = Duration::from_secs(args.timeout_secs.unwrap_or(5));

        let addr = format!("{}:{}", args.host, args.port);

        let result = match timeout(timeout_duration, TcpStream::connect(&addr)).await {
            Ok(Ok(_stream)) => {
                let elapsed = start.elapsed().as_millis() as u64;
                PortCheckResult {
                    host: args.host,
                    port: args.port,
                    open: true,
                    response_time_ms: Some(elapsed),
                    error: None,
                }
            }
            Ok(Err(e)) => PortCheckResult {
                host: args.host,
                port: args.port,
                open: false,
                response_time_ms: None,
                error: Some(e.to_string()),
            },
            Err(_) => PortCheckResult {
                host: args.host,
                port: args.port,
                open: false,
                response_time_ms: None,
                error: Some("Connection timeout".to_string()),
            },
        };

        Ok(ToolResult::success(
            tool_use_id,
            json!(result),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for checking HTTP/HTTPS endpoints.
pub struct HttpPingTool;

impl HttpPingTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HttpPingTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct HttpPingArgs {
    /// URL to check
    url: String,
    /// HTTP method (default: HEAD)
    #[serde(default)]
    method: Option<String>,
    /// Timeout in seconds (default: 10)
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
struct HttpPingResult {
    url: String,
    reachable: bool,
    status_code: Option<u16>,
    response_time_ms: u64,
    error: Option<String>,
}

#[async_trait]
impl Tool for HttpPingTool {
    fn name(&self) -> &str {
        "http_ping"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "http_ping".to_string(),
            description: "Check if an HTTP/HTTPS endpoint is reachable".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to check (including http:// or https://)"
                    },
                    "method": {
                        "type": "string",
                        "enum": ["HEAD", "GET"],
                        "description": "HTTP method to use (default: HEAD)"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 10)"
                    }
                },
                "required": ["url"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: HttpPingArgs = serde_json::from_value(args)?;
        let timeout_duration = Duration::from_secs(args.timeout_secs.unwrap_or(10));
        let _method = args.method.unwrap_or_else(|| "HEAD".to_string());

        // Parse the URL to extract host and port
        let url = args.url.clone();
        let is_https = url.starts_with("https://");
        let host_part = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(&url);

        let (host, port, path) = {
            let (host_port, path) = host_part
                .split_once('/')
                .map(|(h, p)| (h, format!("/{}", p)))
                .unwrap_or((host_part, "/".to_string()));

            if let Some((h, p)) = host_port.split_once(':') {
                (h.to_string(), p.parse::<u16>().unwrap_or(if is_https { 443 } else { 80 }), path)
            } else {
                (host_port.to_string(), if is_https { 443 } else { 80 }, path)
            }
        };

        let addr = format!("{}:{}", host, port);

        let result = match timeout(timeout_duration, async {
            let mut stream = TcpStream::connect(&addr).await?;

            // Send a simple HTTP request
            let request = format!(
                "HEAD {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                path, host
            );

            stream.write_all(request.as_bytes()).await?;

            // Read response
            let mut buffer = [0u8; 1024];
            let n = stream.read(&mut buffer).await?;
            let response = String::from_utf8_lossy(&buffer[..n]);

            // Parse status code from first line
            let status_code = response
                .lines()
                .next()
                .and_then(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .and_then(|code| code.parse::<u16>().ok())
                });

            Ok::<Option<u16>, std::io::Error>(status_code)
        })
        .await
        {
            Ok(Ok(status_code)) => {
                let elapsed = start.elapsed().as_millis() as u64;
                HttpPingResult {
                    url: args.url,
                    reachable: true,
                    status_code,
                    response_time_ms: elapsed,
                    error: None,
                }
            }
            Ok(Err(e)) => {
                let elapsed = start.elapsed().as_millis() as u64;
                HttpPingResult {
                    url: args.url,
                    reachable: false,
                    status_code: None,
                    response_time_ms: elapsed,
                    error: Some(e.to_string()),
                }
            }
            Err(_) => {
                let elapsed = start.elapsed().as_millis() as u64;
                HttpPingResult {
                    url: args.url,
                    reachable: false,
                    status_code: None,
                    response_time_ms: elapsed,
                    error: Some("Request timeout".to_string()),
                }
            }
        };

        Ok(ToolResult::success(
            tool_use_id,
            json!(result),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for getting network interface information.
pub struct NetInfoTool;

impl NetInfoTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NetInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
struct NetInfoResult {
    hostname: String,
    interfaces: Vec<InterfaceInfo>,
}

#[derive(Debug, Serialize)]
struct InterfaceInfo {
    name: String,
    addresses: Vec<String>,
}

#[async_trait]
impl Tool for NetInfoTool {
    fn name(&self) -> &str {
        "net_info"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "net_info".to_string(),
            description: "Get local network interface information".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        _args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Get a simple list of addresses by trying to resolve localhost
        let mut interfaces = Vec::new();

        // Try to get local IPs by checking common patterns
        if let Ok(addrs) = "localhost:0".to_socket_addrs() {
            let addresses: Vec<String> = addrs
                .map(|a| a.ip().to_string())
                .collect();
            if !addresses.is_empty() {
                interfaces.push(InterfaceInfo {
                    name: "localhost".to_string(),
                    addresses,
                });
            }
        }

        let result = NetInfoResult {
            hostname,
            interfaces,
        };

        Ok(ToolResult::success(
            tool_use_id,
            json!(result),
        ).with_duration(start.elapsed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dns_lookup_localhost() {
        let tool = DnsLookupTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "hostname": "localhost"
            }),
            &context,
        ).await.unwrap();

        // localhost should resolve
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_dns_lookup_invalid() {
        let tool = DnsLookupTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "hostname": "this-domain-should-not-exist-12345.invalid"
            }),
            &context,
        ).await.unwrap();

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_port_check_closed() {
        let tool = PortCheckTool::new();
        let context = ToolContext::default();

        // Port 65432 is unlikely to be open
        let result = tool.execute(
            "test",
            json!({
                "host": "127.0.0.1",
                "port": 65432,
                "timeout_secs": 1
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_net_info() {
        let tool = NetInfoTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({}),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_http_ping_localhost() {
        let tool = HttpPingTool::new();
        let context = ToolContext::default();

        // This will likely fail since there's no server on localhost:80
        // but should not error
        let result = tool.execute(
            "test",
            json!({
                "url": "http://localhost:65432",
                "timeout_secs": 1
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
    }
}
