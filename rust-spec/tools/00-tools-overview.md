# Tools Specification Overview

## Tool Categories

101 built-in tools organized into 37 categories:

### 1. File System Tools
- `read` - Read file contents
- `write` - Write file contents
- `edit` - Edit file with search/replace
- `glob` - Find files by pattern
- `grep` - Search file contents

### 2. Execution Tools
- `bash` - Execute shell commands

### 3. Web Tools
- `web_fetch` - Fetch and extract web content
- `web_search` - Search the web

### 4. Media Tools
- `image` - Analyze images with vision models
- `tts` - Text to speech conversion

### 5. Messaging Tools
- `message` - Multi-channel messaging
- `telegram_actions` - Telegram-specific actions
- `discord_actions` - Discord-specific actions
- `slack_actions` - Slack-specific actions

### 6. Session Tools
- `sessions_spawn` - Create sub-agent sessions
- `sessions_send` - Send messages to sessions
- `sessions_list` - List active sessions
- `sessions_history` - Get session history
- `session_status` - Get current session status

### 7. Memory Tools
- `memory_search` - Semantic search of memory
- `memory_get` - Read memory files

### 8. Automation Tools
- `cron` - Manage scheduled jobs
- `gateway` - Gateway management

### 9. Node/Device Tools
- `nodes` - Control paired devices

### 10. Browser Tools
- `browser` - Browser automation

### 11. File Operations Tools
- `file_copy` - Copy files to a new location
- `file_move` - Move or rename files
- `file_stat` - Get file/directory information
- `file_delete` - Delete files or directories

### 12. Archive Tools
- `zip` - Create/extract zip archives
- `tar` - Create/extract tar archives

### 13. Checksum Tools
- `file_checksum` - Compute file hash (MD5, SHA1, SHA256, SHA512)
- `file_verify` - Verify file matches expected hash

### 14. Template Tools
- `template` - Substitute variables in template strings
- `format` - Format values (JSON, numbers, bytes, durations)

### 15. Process Tools
- `process_list` - List running processes
- `process_info` - Get current process information

### 16. Utility Tools
- `sleep` - Wait for a specified duration
- `temp_file` - Create a temporary file
- `temp_dir` - Create a temporary directory
- `echo` - Echo a value back

### 17. Environment Tools
- `env_get` - Get environment variable value
- `env_list` - List environment variables
- `env_check` - Check if environment variables exist

### 18. HTTP Tools
- `http_request` - Make HTTP requests to APIs
- `url_parse` - Parse URL into components
- `url_build` - Build URL with query parameters

### 19. Network Tools
- `dns_lookup` - DNS record lookup
- `port_check` - Check TCP port connectivity
- `http_ping` - Check HTTP/HTTPS endpoint reachability
- `net_info` - Get network interface information

### 20. Notebook Tools
- `notebook_edit` - Edit Jupyter notebook cells

### 21. Code Intelligence Tools
- `lsp` - Language Server Protocol (go-to-definition, find-references, hover)

### 22. Task Management Tools
- `task_create` - Create tasks to track work
- `task_list` - List all tasks
- `task_update` - Update task status
- `task_get` - Get task details

### 23. Interactive Tools
- `ask_user` - Ask user questions with multiple choice options
- `confirm` - Request user confirmation for actions

### 24. Planning Tools
- `enter_plan_mode` - Enter planning mode for implementation design
- `exit_plan_mode` - Exit planning mode and submit plan

### 25. Skills Tools
- `skill` - Invoke a registered skill
- `skill_list` - List available skills

### 26. Diagnostics Tools
- `system_info` - Get system information
- `health_check` - Check agent health and status
- `diagnostic` - Run diagnostics to troubleshoot issues

### 27. Context Management Tools
- `context_add` - Add information to working context
- `context_get` - Retrieve entries from working context
- `context_clear` - Clear the working context

### 28. Diff & Patch Tools
- `diff` - Generate diffs between text or files
- `patch` - Preview and apply search/replace changes

### 29. Git Tools
- `git_status` - Get repository status
- `git_log` - View commit history
- `git_diff` - View changes
- `git_branch` - List and manage branches

### 30. JSON/YAML Tools
- `json_query` - Query JSON data using path expressions
- `json_transform` - Transform JSON (pick, omit, rename, flatten)
- `yaml` - Parse and convert between YAML and JSON

### 31. Encoding & Hashing Tools
- `base64` - Base64 encode/decode
- `hex` - Hexadecimal encode/decode
- `hash` - Compute hashes (MD5, SHA1, SHA256, SHA512)
- `url_encode` - URL encode/decode

### 32. Time & Date Tools
- `now` - Get current date and time
- `date_parse` - Parse and format date strings
- `date_calc` - Date calculations (add, subtract, diff)

### 33. String Manipulation Tools
- `case` - Convert case (upper, lower, camel, snake, kebab)
- `split_join` - Split and join strings
- `replace` - Text replacement with regex support
- `trim_pad` - Trim whitespace or pad strings

### 34. Math & Random Tools
- `calc` - Mathematical calculations
- `random` - Generate random numbers, strings, or pick items
- `uuid` - Generate UUIDs

### 35. Validation Tools
- `validate` - Validate formats (email, URL, JSON, UUID, IP, etc.)
- `is_empty` - Check if value is empty, null, or blank

### 36. Comparison & Assertion Tools
- `compare` - Compare two values
- `assert` - Assert a condition is true
- `match` - Match text against regex patterns
- `version_compare` - Compare semantic version strings

### 37. Canvas Tools
- `canvas` - Visual canvas operations

## Core Tool Trait

```rust
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

/// Result of a tool execution.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Whether the tool execution succeeded.
    pub ok: bool,

    /// Result content (for success).
    pub content: Option<Vec<ToolContent>>,

    /// Error reason (for failure).
    pub error: Option<String>,

    /// Structured details.
    pub details: Option<Value>,
}

/// Content types that tools can return.
#[derive(Debug, Clone)]
pub enum ToolContent {
    /// Text content.
    Text(String),

    /// Image content (base64 or path).
    Image {
        data: String,
        mime_type: String,
    },

    /// File content.
    File {
        path: String,
        mime_type: Option<String>,
    },
}

impl ToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            ok: true,
            content: Some(vec![ToolContent::Text(content.into())]),
            error: None,
            details: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            content: None,
            error: Some(error.into()),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Tool definition trait.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (unique identifier).
    fn name(&self) -> &str;

    /// Tool description for the LLM.
    fn description(&self) -> &str;

    /// JSON Schema for input parameters.
    fn input_schema(&self) -> Value;

    /// Execute the tool.
    async fn execute(&self, params: Value, ctx: &ToolContext) -> ToolResult;

    /// Whether this tool requires approval for given params.
    fn requires_approval(&self, params: &Value) -> bool {
        false
    }

    /// Tool group for categorization.
    fn group(&self) -> ToolGroup {
        ToolGroup::General
    }
}

/// Tool execution context.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Current working directory.
    pub cwd: PathBuf,

    /// Sandbox root directory.
    pub sandbox_root: PathBuf,

    /// Agent ID.
    pub agent_id: String,

    /// Session key.
    pub session_key: String,

    /// Channel name.
    pub channel: Option<String>,

    /// Account ID.
    pub account_id: Option<String>,

    /// Whether sandboxing is enabled.
    pub sandboxed: bool,

    /// Configuration reference.
    pub config: Arc<SmartAssistConfig>,
}

/// Tool group categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGroup {
    FileSystem,
    Execution,
    Web,
    Media,
    Messaging,
    Session,
    Memory,
    Automation,
    Device,
    Browser,
    General,
}
```

## Tool Registry

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    groups: HashMap<ToolGroup, Vec<String>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            groups: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        let group = tool.group();

        self.tools.insert(name.clone(), tool);
        self.groups.entry(group).or_default().push(name);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn list(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    pub fn list_by_group(&self, group: ToolGroup) -> Vec<&str> {
        self.groups.get(&group)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn to_openai_tools(&self) -> Vec<Value> {
        self.tools.values().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name(),
                    "description": t.description(),
                    "parameters": t.input_schema(),
                }
            })
        }).collect()
    }

    pub fn to_anthropic_tools(&self) -> Vec<Value> {
        self.tools.values().map(|t| {
            serde_json::json!({
                "name": t.name(),
                "description": t.description(),
                "input_schema": t.input_schema(),
            })
        }).collect()
    }
}

/// Create the default tool registry with all built-in tools.
pub fn create_default_registry(config: &SmartAssistConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // File system tools
    registry.register(Arc::new(ReadTool::new()));
    registry.register(Arc::new(WriteTool::new()));
    registry.register(Arc::new(EditTool::new()));
    registry.register(Arc::new(GlobTool::new()));
    registry.register(Arc::new(GrepTool::new()));

    // Execution tools
    registry.register(Arc::new(BashTool::new(config)));

    // Web tools
    if config.tools.web.fetch.enabled.unwrap_or(true) {
        registry.register(Arc::new(WebFetchTool::new(config)));
    }
    if config.tools.web.search.enabled.unwrap_or(true) {
        registry.register(Arc::new(WebSearchTool::new(config)));
    }

    // Media tools
    registry.register(Arc::new(ImageTool::new(config)));
    if config.tools.tts.enabled.unwrap_or(false) {
        registry.register(Arc::new(TtsTool::new(config)));
    }

    // Session tools
    registry.register(Arc::new(SessionsSpawnTool::new()));
    registry.register(Arc::new(SessionsSendTool::new()));
    registry.register(Arc::new(SessionsListTool::new()));
    registry.register(Arc::new(SessionsHistoryTool::new()));
    registry.register(Arc::new(SessionStatusTool::new()));

    // Memory tools
    if config.memory.enabled.unwrap_or(true) {
        registry.register(Arc::new(MemorySearchTool::new(config)));
        registry.register(Arc::new(MemoryGetTool::new(config)));
    }

    // Automation tools
    registry.register(Arc::new(CronTool::new()));
    registry.register(Arc::new(GatewayTool::new()));

    // Device tools
    registry.register(Arc::new(NodesTool::new()));

    // Browser tools
    if config.tools.browser.enabled.unwrap_or(false) {
        registry.register(Arc::new(BrowserTool::new(config)));
    }

    // Channel-specific tools based on enabled channels
    for channel_id in config.channels.enabled_channels() {
        match channel_id.as_str() {
            "telegram" => registry.register(Arc::new(TelegramActionsTool::new())),
            "discord" => registry.register(Arc::new(DiscordActionsTool::new())),
            "slack" => registry.register(Arc::new(SlackActionsTool::new())),
            _ => {}
        }
    }

    // Universal message tool
    registry.register(Arc::new(MessageTool::new(config)));

    registry
}
```

## Approval System

```rust
/// Approval request for tool execution.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool_name: String,
    pub params: Value,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub agent_id: String,
    pub session_key: String,
    pub timeout_ms: u64,
}

/// Approval decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Denied,
    TimedOut,
}

/// Approval handler trait.
#[async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn request_approval(&self, request: ApprovalRequest) -> ApprovalDecision;
}

/// Check if a command requires approval.
pub fn command_requires_approval(command: &str, config: &ApprovalConfig) -> bool {
    // Check allowlist
    for pattern in &config.allowlist {
        if pattern_matches(command, pattern) {
            return false;
        }
    }

    // Check dangerous commands
    let dangerous_patterns = [
        r"rm\s+-rf",
        r"sudo\s+",
        r"chmod\s+",
        r"chown\s+",
        r"mkfs",
        r"dd\s+",
        r">\s*/dev/",
        r"curl.*\|\s*sh",
        r"wget.*\|\s*sh",
    ];

    for pattern in &dangerous_patterns {
        if regex::Regex::new(pattern).unwrap().is_match(command) {
            return true;
        }
    }

    // Default based on security level
    match config.security_level {
        SecurityLevel::Strict => true,
        SecurityLevel::Normal => false,
        SecurityLevel::Permissive => false,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SecurityLevel {
    Strict,
    Normal,
    Permissive,
}
```

## Parameter Utilities

```rust
/// Read a required string parameter.
pub fn read_string_param(params: &Value, key: &str) -> Result<String, ToolError> {
    params.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ToolError::MissingParam(key.to_string()))
}

/// Read an optional string parameter.
pub fn read_optional_string(params: &Value, key: &str) -> Option<String> {
    params.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Read an optional number parameter.
pub fn read_optional_number(params: &Value, key: &str) -> Option<f64> {
    params.get(key).and_then(|v| v.as_f64())
}

/// Read an optional boolean parameter.
pub fn read_optional_bool(params: &Value, key: &str) -> Option<bool> {
    params.get(key).and_then(|v| v.as_bool())
}

/// Read an optional array parameter.
pub fn read_optional_array<T: DeserializeOwned>(params: &Value, key: &str) -> Option<Vec<T>> {
    params.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Missing required parameter: {0}")]
    MissingParam(String),

    #[error("Invalid parameter type for {0}: expected {1}")]
    InvalidType(String, String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),

    #[error("Timeout")]
    Timeout,

    #[error("Approval denied")]
    ApprovalDenied,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

## Timeout Handling

```rust
/// Default timeouts by tool category.
pub const DEFAULT_TIMEOUTS: &[(ToolGroup, u64)] = &[
    (ToolGroup::FileSystem, 30_000),
    (ToolGroup::Execution, 120_000),
    (ToolGroup::Web, 60_000),
    (ToolGroup::Media, 60_000),
    (ToolGroup::Messaging, 30_000),
    (ToolGroup::Session, 30_000),
    (ToolGroup::Memory, 30_000),
    (ToolGroup::Automation, 30_000),
    (ToolGroup::Device, 30_000),
    (ToolGroup::Browser, 120_000),
    (ToolGroup::General, 30_000),
];

/// Execute a tool with timeout.
pub async fn execute_with_timeout<F, T>(
    future: F,
    timeout_ms: u64,
) -> Result<T, ToolError>
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        future,
    )
    .await
    .map_err(|_| ToolError::Timeout)
}
```
