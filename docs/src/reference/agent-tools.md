# Agent Tools

The agent includes 101 built-in tools:

### File System

- `read` - Read file contents
- `write` - Write file contents
- `edit` - Edit files with diff-based changes
- `glob` - Find files by pattern
- `grep` - Search file contents

### File Operations

- `file_copy` - Copy files to a new location
- `file_move` - Move or rename files
- `file_stat` - Get file/directory information (size, timestamps, permissions)
- `file_delete` - Delete files or directories (requires approval)

### Archive

- `zip` - Create/extract zip archives
- `tar` - Create/extract tar archives (with gzip support)

### Checksum

- `file_checksum` - Compute file hash (MD5, SHA1, SHA256, SHA512)
- `file_verify` - Verify file matches expected hash

### Template

- `template` - Substitute variables in template strings
- `format` - Format values (JSON, numbers, bytes, durations)

### System

- `bash` - Execute shell commands

### Process

- `process_list` - List running processes
- `process_info` - Get current process information

### Utility

- `sleep` - Wait for a specified duration
- `temp_file` - Create a temporary file
- `temp_dir` - Create a temporary directory
- `echo` - Echo a value back (useful for testing)

### Environment

- `env_get` - Get environment variable value
- `env_list` - List environment variables
- `env_check` - Check if environment variables exist

### HTTP

- `http_request` - Make HTTP requests to APIs
- `url_parse` - Parse URL into components
- `url_build` - Build URL with query parameters

### Network

- `dns_lookup` - DNS record lookup
- `port_check` - Check TCP port connectivity
- `http_ping` - Check HTTP/HTTPS endpoint reachability
- `net_info` - Get network interface information

### Web

- `web_fetch` - Fetch and process web pages
- `web_search` - Search the web

### Messaging

- `message` - Send messages
- `sessions_spawn` - Create new agent sessions
- `sessions_send` - Send to existing sessions
- `sessions_list` - List active sessions
- `sessions_history` - Get session history
- `session_status` - Get session status

### Memory

- `memory_search` - Search stored memories
- `memory_get` - Retrieve specific memories
- `memory_store` - Store information in memory
- `memory_index` - Index and organize memory entries

### Automation

- `cron` - Schedule recurring tasks
- `gateway` - Control the gateway
- `nodes` - Manage distributed nodes

### Media

- `image` - Analyze images
- `tts` - Text-to-speech

### Browser

- `browser` - Browser automation

### Channel Actions

- `telegram_actions` - Telegram-specific actions
- `discord_actions` - Discord-specific actions
- `slack_actions` - Slack-specific actions

### Notebook

- `notebook_edit` - Edit Jupyter notebook cells

### Code Intelligence

- `lsp` - Language Server Protocol for go-to-definition, find-references, hover

### Task Management

- `task_create` - Create tasks to track work
- `task_list` - List all tasks
- `task_update` - Update task status
- `task_get` - Get task details

### Interactive

- `ask_user` - Ask user questions with multiple choice options
- `confirm` - Request user confirmation for actions

### Planning

- `enter_plan_mode` - Enter planning mode for implementation design
- `exit_plan_mode` - Exit planning mode and submit plan for approval

### Skills

- `skill` - Invoke a registered skill (slash command)
- `skill_list` - List available skills

### Diagnostics

- `system_info` - Get system information (OS, architecture, environment)
- `health_check` - Check agent health and status
- `diagnostic` - Run diagnostics to troubleshoot issues

### Context Management

- `context_add` - Add information to working context for later reference
- `context_get` - Retrieve entries from working context
- `context_clear` - Clear the working context

### Diff & Patch

- `diff` - Generate diffs between text or files
- `patch` - Preview and apply search/replace changes

### Git

- `git_status` - Get repository status (staged, modified, untracked)
- `git_log` - View commit history
- `git_diff` - View changes
- `git_branch` - List and manage branches

### JSON/YAML

- `json_query` - Query JSON data using path expressions
- `json_transform` - Transform JSON (pick, omit, rename, flatten)
- `yaml` - Parse and convert between YAML and JSON

### Encoding & Hashing

- `base64` - Base64 encode/decode
- `hex` - Hexadecimal encode/decode
- `hash` - Compute hashes (MD5, SHA1, SHA256, SHA512)
- `url_encode` - URL encode/decode

### Time & Date

- `now` - Get current date and time
- `date_parse` - Parse and format date strings
- `date_calc` - Date calculations (add, subtract, diff)

### String Manipulation

- `case` - Convert case (upper, lower, camel, snake, kebab)
- `split_join` - Split and join strings
- `replace` - Text replacement with regex support
- `trim_pad` - Trim whitespace or pad strings

### Math & Random

- `calc` - Mathematical calculations (add, sqrt, power, etc.)
- `random` - Generate random numbers, strings, or pick items
- `uuid` - Generate UUIDs

### Validation

- `validate` - Validate formats (email, URL, JSON, UUID, IP, etc.)
- `is_empty` - Check if value is empty, null, or blank

### Comparison & Assertions

- `compare` - Compare two values (eq, ne, lt, gt, contains, starts_with, ends_with)
- `assert` - Assert a condition is true (returns error if false)
- `match` - Match text against regex patterns with capture groups
- `version_compare` - Compare semantic version strings

