//! Session management and persistence.

use crate::Result;
use chrono::{DateTime, Utc};
use smartassist_core::types::{
    AgentId, ContentBlock, Message, MessageContent, Role, SessionKey, TokenUsage,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tracing::debug;

/// A conversation session with an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session key.
    pub key: SessionKey,

    /// Associated agent ID.
    pub agent_id: AgentId,

    /// Conversation messages.
    pub messages: Vec<Message>,

    /// Session metadata.
    pub metadata: SessionMetadata,

    /// Session state.
    pub state: SessionState,

    /// Total token usage.
    pub total_tokens: TokenUsage,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp.
    pub last_activity: DateTime<Utc>,
}

/// Session metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Custom key-value pairs.
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,

    /// System prompt override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// Model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Temperature override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// Session state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Session is active.
    #[default]
    Active,

    /// Session is paused.
    Paused,

    /// Session is processing a request.
    Processing,

    /// Session is waiting for approval.
    WaitingApproval,

    /// Session is archived.
    Archived,
}

impl Session {
    /// Create a new session.
    pub fn new(key: SessionKey, agent_id: AgentId) -> Self {
        let now = Utc::now();
        Self {
            key,
            agent_id,
            messages: Vec::new(),
            metadata: SessionMetadata::default(),
            state: SessionState::Active,
            total_tokens: TokenUsage::default(),
            created_at: now,
            last_activity: now,
        }
    }

    /// Add a user message.
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::user(content));
        self.last_activity = Utc::now();
    }

    /// Add an assistant message.
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::assistant(content));
        self.last_activity = Utc::now();
    }

    /// Add a message with content blocks.
    pub fn add_message(&mut self, role: Role, content: Vec<ContentBlock>) {
        self.messages.push(Message {
            role,
            content: MessageContent::Blocks(content),
            name: None,
            tool_use_id: None,
            timestamp: Utc::now(),
        });
        self.last_activity = Utc::now();
    }

    /// Get the last message.
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Get the last assistant message.
    pub fn last_assistant_message(&self) -> Option<&Message> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
    }

    /// Update token usage.
    pub fn update_tokens(&mut self, usage: &TokenUsage) {
        self.total_tokens.add(usage);
    }

    /// Get the message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Check if the session is active.
    pub fn is_active(&self) -> bool {
        self.state == SessionState::Active
    }

    /// Pause the session.
    pub fn pause(&mut self) {
        self.state = SessionState::Paused;
    }

    /// Resume the session.
    pub fn resume(&mut self) {
        self.state = SessionState::Active;
    }

    /// Archive the session.
    pub fn archive(&mut self) {
        self.state = SessionState::Archived;
    }

    /// Apply compaction to the session's messages.
    ///
    /// Replaces the current message history with the compacted version
    /// and logs the compaction event.
    pub fn apply_compaction(&mut self, new_messages: Vec<Message>, messages_removed: usize) {
        tracing::info!(
            session = %self.key.as_str(),
            removed = messages_removed,
            remaining = new_messages.len(),
            "Applied context compaction"
        );
        self.messages = new_messages;
        self.last_activity = Utc::now();
    }
}

/// Manager for session persistence and lifecycle.
pub struct SessionManager {
    /// Base directory for session storage.
    base_dir: PathBuf,

    /// In-memory session cache.
    cache: RwLock<HashMap<String, Session>>,

    /// Maximum messages to keep in memory.
    max_messages: usize,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            cache: RwLock::new(HashMap::new()),
            max_messages: 100,
        }
    }

    /// Set the maximum messages per session.
    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = max;
        self
    }

    /// Get or create a session.
    pub async fn get_or_create(&self, key: &SessionKey, agent_id: &AgentId) -> Result<Session> {
        let cache_key = self.cache_key(key);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(session) = cache.get(&cache_key) {
                return Ok(session.clone());
            }
        }

        // Try to load from disk
        if let Ok(session) = self.load(key).await {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, session.clone());
            return Ok(session);
        }

        // Create new session
        let session = Session::new(key.clone(), agent_id.clone());
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, session.clone());
        }

        Ok(session)
    }

    /// Save a session.
    pub async fn save(&self, session: &Session) -> Result<()> {
        let cache_key = self.cache_key(&session.key);

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, session.clone());
        }

        // Save to disk
        let path = self.session_path(&session.key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(session)?;
        fs::write(&path, json).await?;

        debug!("Saved session to {:?}", path);
        Ok(())
    }

    /// Load a session from disk.
    pub async fn load(&self, key: &SessionKey) -> Result<Session> {
        let path = self.session_path(key);
        let content = fs::read_to_string(&path).await?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// Delete a session.
    pub async fn delete(&self, key: &SessionKey) -> Result<()> {
        let cache_key = self.cache_key(key);

        // Remove from cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&cache_key);
        }

        // Remove from disk
        let path = self.session_path(key);
        if path.exists() {
            fs::remove_file(&path).await?;
        }

        Ok(())
    }

    /// List all sessions for an agent.
    pub async fn list_for_agent(&self, agent_id: &AgentId) -> Result<Vec<SessionKey>> {
        let agent_dir = self.base_dir.join(agent_id.as_str());

        if !agent_dir.exists() {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        let mut entries = fs::read_dir(&agent_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Some(stem) = path.file_stem() {
                    // Create session key from agent:session format
                    let key_str = format!("{}:{}", agent_id.as_str(), stem.to_string_lossy());
                    keys.push(SessionKey::new(key_str));
                }
            }
        }

        Ok(keys)
    }

    /// Get the path for a session file.
    fn session_path(&self, key: &SessionKey) -> PathBuf {
        // Extract agent ID and session from the key
        let key_str = key.as_str();
        let parts: Vec<&str> = key_str.splitn(2, ':').collect();
        let (agent, session) = if parts.len() >= 2 {
            (parts[0], parts[1])
        } else {
            ("default", key_str)
        };

        self.base_dir
            .join(agent)
            .join(format!("{}.json", session.replace(':', "_")))
    }

    /// Generate a cache key for a session key.
    fn cache_key(&self, key: &SessionKey) -> String {
        key.as_str().to_string()
    }
}

/// Session log entry for JSONL logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLogEntry {
    /// Timestamp.
    pub timestamp: DateTime<Utc>,

    /// Entry type.
    pub entry_type: LogEntryType,

    /// Entry data.
    pub data: serde_json::Value,
}

/// Type of session log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogEntryType {
    /// Message added.
    Message,

    /// Tool use.
    ToolUse,

    /// Tool result.
    ToolResult,

    /// Token usage.
    TokenUsage,

    /// State change.
    StateChange,

    /// Error.
    Error,

    /// Custom event.
    Custom(String),
}

/// Session log writer for JSONL format.
pub struct SessionLogger {
    /// Path to the log file.
    path: PathBuf,
}

impl SessionLogger {
    /// Create a new session logger.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Append an entry to the log.
    pub async fn append(&self, entry: SessionLogEntry) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let line = serde_json::to_string(&entry)? + "\n";

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        file.write_all(line.as_bytes()).await?;

        Ok(())
    }

    /// Read all entries from the log.
    pub async fn read_all(&self) -> Result<Vec<SessionLogEntry>> {
        let file = fs::File::open(&self.path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut entries = Vec::new();

        while let Some(line) = lines.next_line().await? {
            if let Ok(entry) = serde_json::from_str(&line) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Read entries after a certain timestamp.
    pub async fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<SessionLogEntry>> {
        let all = self.read_all().await?;
        Ok(all.into_iter().filter(|e| e.timestamp > since).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let key = SessionKey::new("agent1:session1");
        let agent_id = AgentId::new("agent1");
        let session = Session::new(key.clone(), agent_id);

        assert_eq!(session.key.as_str(), "agent1:session1");
        assert!(session.messages.is_empty());
        assert!(session.is_active());
    }

    #[test]
    fn test_session_messages() {
        let key = SessionKey::new("agent1:session1");
        let mut session = Session::new(key, AgentId::new("agent1"));

        session.add_user_message("Hello");
        session.add_assistant_message("Hi there!");

        assert_eq!(session.message_count(), 2);
    }

    #[test]
    fn test_session_state() {
        let key = SessionKey::new("agent1:session1");
        let mut session = Session::new(key, AgentId::new("agent1"));

        assert!(session.is_active());

        session.pause();
        assert_eq!(session.state, SessionState::Paused);

        session.resume();
        assert!(session.is_active());
    }
}
