//! Language Server Protocol (LSP) tools for code intelligence.
//!
//! Provides tools for interacting with LSP servers to get:
//! - Go to definition
//! - Find references
//! - Hover information
//! - Document symbols
//! - Workspace symbols

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

/// LSP operation types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LspOperation {
    /// Go to definition of a symbol.
    GoToDefinition,
    /// Find all references to a symbol.
    FindReferences,
    /// Get hover information for a symbol.
    Hover,
    /// Get all symbols in a document.
    DocumentSymbol,
    /// Search for symbols in the workspace.
    WorkspaceSymbol,
    /// Go to implementation of an interface/trait.
    GoToImplementation,
    /// Prepare call hierarchy at a position.
    PrepareCallHierarchy,
    /// Find incoming calls to a function.
    IncomingCalls,
    /// Find outgoing calls from a function.
    OutgoingCalls,
}

impl std::fmt::Display for LspOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LspOperation::GoToDefinition => write!(f, "goToDefinition"),
            LspOperation::FindReferences => write!(f, "findReferences"),
            LspOperation::Hover => write!(f, "hover"),
            LspOperation::DocumentSymbol => write!(f, "documentSymbol"),
            LspOperation::WorkspaceSymbol => write!(f, "workspaceSymbol"),
            LspOperation::GoToImplementation => write!(f, "goToImplementation"),
            LspOperation::PrepareCallHierarchy => write!(f, "prepareCallHierarchy"),
            LspOperation::IncomingCalls => write!(f, "incomingCalls"),
            LspOperation::OutgoingCalls => write!(f, "outgoingCalls"),
        }
    }
}

/// A location in source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Location {
    /// File path.
    pub path: String,
    /// Line number (1-indexed).
    pub line: u32,
    /// Character offset (1-indexed).
    pub character: u32,
    /// Optional end line for ranges.
    pub end_line: Option<u32>,
    /// Optional end character for ranges.
    pub end_character: Option<u32>,
}

/// A symbol in the code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Symbol {
    /// Symbol name.
    pub name: String,
    /// Symbol kind (function, class, variable, etc.).
    pub kind: String,
    /// Location of the symbol.
    pub location: Location,
    /// Container name (parent scope).
    pub container_name: Option<String>,
}

/// Hover information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct HoverInfo {
    /// Hover content (usually markdown).
    pub contents: String,
    /// Range of the hovered symbol.
    pub range: Option<Location>,
}

/// LSP client configuration.
#[derive(Debug, Clone)]
pub struct LspClientConfig {
    /// Language to server command mapping.
    pub servers: HashMap<String, Vec<String>>,
    /// Root directories for language detection.
    pub root_markers: HashMap<String, Vec<String>>,
}

impl Default for LspClientConfig {
    fn default() -> Self {
        let mut servers = HashMap::new();
        // Common LSP servers
        servers.insert(
            "rust".to_string(),
            vec!["rust-analyzer".to_string()],
        );
        servers.insert(
            "typescript".to_string(),
            vec!["typescript-language-server".to_string(), "--stdio".to_string()],
        );
        servers.insert(
            "javascript".to_string(),
            vec!["typescript-language-server".to_string(), "--stdio".to_string()],
        );
        servers.insert(
            "python".to_string(),
            vec!["pylsp".to_string()],
        );
        servers.insert(
            "go".to_string(),
            vec!["gopls".to_string()],
        );

        let mut root_markers = HashMap::new();
        root_markers.insert(
            "rust".to_string(),
            vec!["Cargo.toml".to_string(), "Cargo.lock".to_string()],
        );
        root_markers.insert(
            "typescript".to_string(),
            vec!["tsconfig.json".to_string(), "package.json".to_string()],
        );
        root_markers.insert(
            "javascript".to_string(),
            vec!["package.json".to_string(), "jsconfig.json".to_string()],
        );
        root_markers.insert(
            "python".to_string(),
            vec!["pyproject.toml".to_string(), "setup.py".to_string(), "requirements.txt".to_string()],
        );
        root_markers.insert(
            "go".to_string(),
            vec!["go.mod".to_string(), "go.sum".to_string()],
        );

        Self {
            servers,
            root_markers,
        }
    }
}

/// LSP tool for code intelligence operations.
pub struct LspTool {
    /// Client configuration.
    config: LspClientConfig,
    /// Cache for language detection.
    #[allow(dead_code)]
    language_cache: Arc<RwLock<HashMap<PathBuf, String>>>,
}

impl LspTool {
    /// Create a new LSP tool with default configuration.
    pub fn new() -> Self {
        Self {
            config: LspClientConfig::default(),
            language_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: LspClientConfig) -> Self {
        Self {
            config,
            language_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Detect language from file extension.
    fn detect_language(&self, path: &str) -> Option<String> {
        let extension = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())?;

        match extension {
            "rs" => Some("rust".to_string()),
            "ts" | "tsx" => Some("typescript".to_string()),
            "js" | "jsx" | "mjs" | "cjs" => Some("javascript".to_string()),
            "py" | "pyi" => Some("python".to_string()),
            "go" => Some("go".to_string()),
            "java" => Some("java".to_string()),
            "c" | "h" => Some("c".to_string()),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp".to_string()),
            "rb" => Some("ruby".to_string()),
            "php" => Some("php".to_string()),
            "swift" => Some("swift".to_string()),
            "kt" | "kts" => Some("kotlin".to_string()),
            "scala" => Some("scala".to_string()),
            "cs" => Some("csharp".to_string()),
            "fs" | "fsx" => Some("fsharp".to_string()),
            "lua" => Some("lua".to_string()),
            "zig" => Some("zig".to_string()),
            "nim" => Some("nim".to_string()),
            "v" => Some("vlang".to_string()),
            "elm" => Some("elm".to_string()),
            "ex" | "exs" => Some("elixir".to_string()),
            "erl" | "hrl" => Some("erlang".to_string()),
            "hs" | "lhs" => Some("haskell".to_string()),
            "ml" | "mli" => Some("ocaml".to_string()),
            "clj" | "cljs" | "cljc" => Some("clojure".to_string()),
            "vue" => Some("vue".to_string()),
            "svelte" => Some("svelte".to_string()),
            _ => None,
        }
    }

    /// Check if an LSP server is available for a language.
    fn has_server(&self, language: &str) -> bool {
        self.config.servers.contains_key(language)
    }

    /// Execute an LSP operation (simulated for now).
    /// In a full implementation, this would communicate with actual LSP servers.
    async fn execute_lsp_operation(
        &self,
        operation: LspOperation,
        file_path: &str,
        line: u32,
        character: u32,
        _query: Option<&str>,
    ) -> Result<serde_json::Value> {
        let language = self.detect_language(file_path)
            .ok_or_else(|| crate::error::AgentError::tool_execution(
                format!("Cannot detect language for file: {}", file_path)
            ))?;

        if !self.has_server(&language) {
            return Err(crate::error::AgentError::tool_execution(
                format!("No LSP server configured for language: {}", language)
            ));
        }

        debug!(
            "LSP operation {:?} on {} at {}:{} (language: {})",
            operation, file_path, line, character, language
        );

        // For now, return a simulated response indicating LSP is not yet fully implemented
        // In a production implementation, this would:
        // 1. Start or connect to the appropriate LSP server
        // 2. Send the appropriate LSP request
        // 3. Parse and return the response

        match operation {
            LspOperation::GoToDefinition | LspOperation::GoToImplementation => {
                Ok(serde_json::json!({
                    "status": "not_implemented",
                    "message": "LSP server integration pending. Use grep/glob tools for now.",
                    "operation": operation.to_string(),
                    "file": file_path,
                    "line": line,
                    "character": character,
                    "language": language,
                }))
            }
            LspOperation::FindReferences => {
                Ok(serde_json::json!({
                    "status": "not_implemented",
                    "message": "LSP server integration pending. Use grep tool for now.",
                    "operation": operation.to_string(),
                    "file": file_path,
                    "line": line,
                    "character": character,
                    "language": language,
                }))
            }
            LspOperation::Hover => {
                Ok(serde_json::json!({
                    "status": "not_implemented",
                    "message": "LSP server integration pending.",
                    "operation": operation.to_string(),
                    "file": file_path,
                    "line": line,
                    "character": character,
                    "language": language,
                }))
            }
            LspOperation::DocumentSymbol => {
                Ok(serde_json::json!({
                    "status": "not_implemented",
                    "message": "LSP server integration pending. Use grep for pattern matching.",
                    "operation": operation.to_string(),
                    "file": file_path,
                    "language": language,
                }))
            }
            LspOperation::WorkspaceSymbol => {
                Ok(serde_json::json!({
                    "status": "not_implemented",
                    "message": "LSP server integration pending. Use grep/glob tools.",
                    "operation": operation.to_string(),
                    "language": language,
                }))
            }
            LspOperation::PrepareCallHierarchy | LspOperation::IncomingCalls | LspOperation::OutgoingCalls => {
                Ok(serde_json::json!({
                    "status": "not_implemented",
                    "message": "Call hierarchy requires LSP server integration.",
                    "operation": operation.to_string(),
                    "file": file_path,
                    "line": line,
                    "character": character,
                    "language": language,
                }))
            }
        }
    }
}

impl Default for LspTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "lsp"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "lsp".to_string(),
            description: "Interact with Language Server Protocol servers for code intelligence. \
                         Supports go-to-definition, find-references, hover, and symbol search."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": [
                            "goToDefinition",
                            "findReferences",
                            "hover",
                            "documentSymbol",
                            "workspaceSymbol",
                            "goToImplementation",
                            "prepareCallHierarchy",
                            "incomingCalls",
                            "outgoingCalls"
                        ],
                        "description": "The LSP operation to perform"
                    },
                    "filePath": {
                        "type": "string",
                        "description": "Path to the file"
                    },
                    "line": {
                        "type": "integer",
                        "description": "Line number (1-indexed)"
                    },
                    "character": {
                        "type": "integer",
                        "description": "Character offset (1-indexed)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query for workspaceSymbol operation"
                    }
                },
                "required": ["operation", "filePath", "line", "character"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let operation_str = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("operation is required"))?;

        let operation = match operation_str {
            "goToDefinition" => LspOperation::GoToDefinition,
            "findReferences" => LspOperation::FindReferences,
            "hover" => LspOperation::Hover,
            "documentSymbol" => LspOperation::DocumentSymbol,
            "workspaceSymbol" => LspOperation::WorkspaceSymbol,
            "goToImplementation" => LspOperation::GoToImplementation,
            "prepareCallHierarchy" => LspOperation::PrepareCallHierarchy,
            "incomingCalls" => LspOperation::IncomingCalls,
            "outgoingCalls" => LspOperation::OutgoingCalls,
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown operation: {}", operation_str),
                ));
            }
        };

        let file_path = args
            .get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("filePath is required"))?;

        let line = args
            .get("line")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        let character = args
            .get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        let query = args.get("query").and_then(|v| v.as_str());

        // Resolve path
        let path = if std::path::Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            ctx.cwd.join(file_path).to_string_lossy().to_string()
        };

        let result = self
            .execute_lsp_operation(operation, &path, line, character, query)
            .await?;

        let duration = start.elapsed();

        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_tool_creation() {
        let tool = LspTool::new();
        assert_eq!(tool.name(), "lsp");
    }

    #[test]
    fn test_lsp_tool_definition() {
        let tool = LspTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "lsp");
        assert!(def.description.contains("Language Server Protocol"));
    }

    #[test]
    fn test_language_detection() {
        let tool = LspTool::new();

        assert_eq!(tool.detect_language("foo.rs"), Some("rust".to_string()));
        assert_eq!(tool.detect_language("bar.ts"), Some("typescript".to_string()));
        assert_eq!(tool.detect_language("baz.py"), Some("python".to_string()));
        assert_eq!(tool.detect_language("qux.go"), Some("go".to_string()));
        assert_eq!(tool.detect_language("test.js"), Some("javascript".to_string()));
        assert_eq!(tool.detect_language("unknown.xyz"), None);
    }

    #[test]
    fn test_has_server() {
        let tool = LspTool::new();

        assert!(tool.has_server("rust"));
        assert!(tool.has_server("typescript"));
        assert!(tool.has_server("python"));
        assert!(tool.has_server("go"));
        assert!(!tool.has_server("unknown_language"));
    }

    #[test]
    fn test_lsp_operation_display() {
        assert_eq!(LspOperation::GoToDefinition.to_string(), "goToDefinition");
        assert_eq!(LspOperation::FindReferences.to_string(), "findReferences");
        assert_eq!(LspOperation::Hover.to_string(), "hover");
    }

    #[tokio::test]
    async fn test_lsp_execute() {
        let tool = LspTool::new();
        let ctx = ToolContext::default();

        let args = serde_json::json!({
            "operation": "goToDefinition",
            "filePath": "test.rs",
            "line": 10,
            "character": 5
        });

        let result = tool.execute("test_id", args, &ctx).await.unwrap();
        assert!(!result.is_error);
    }
}
