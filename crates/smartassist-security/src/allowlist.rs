//! Allowlist resolution for approved tools, channels, and operations.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Resolved allowlist.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Allowlist {
    /// Allowed tool names.
    pub tools: HashSet<String>,
    /// Allowed channel types.
    pub channels: HashSet<String>,
    /// Allowed operations.
    pub operations: HashSet<String>,
    /// Whether the allowlist is enforced (deny-by-default).
    pub enforced: bool,
}

impl Allowlist {
    /// Create an empty allowlist.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tool to the allowlist.
    pub fn allow_tool(&mut self, name: impl Into<String>) {
        self.tools.insert(name.into());
    }

    /// Add a channel to the allowlist.
    pub fn allow_channel(&mut self, name: impl Into<String>) {
        self.channels.insert(name.into());
    }

    /// Add an operation to the allowlist.
    pub fn allow_operation(&mut self, name: impl Into<String>) {
        self.operations.insert(name.into());
    }

    /// Check if a tool is allowed.
    pub fn is_tool_allowed(&self, name: &str) -> bool {
        !self.enforced || self.tools.contains(name)
    }

    /// Check if a channel is allowed.
    pub fn is_channel_allowed(&self, name: &str) -> bool {
        !self.enforced || self.channels.contains(name)
    }

    /// Load an allowlist from a JSON file.
    pub fn from_json(data: &str) -> anyhow::Result<Self> {
        let list: Self = serde_json::from_str(data)?;
        Ok(list)
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

/// Resolver that can load and merge multiple allowlist sources.
pub struct AllowlistResolver;

impl AllowlistResolver {
    /// Resolve the effective allowlist from config.
    pub fn resolve(_config_path: &std::path::Path) -> Allowlist {
        // TODO: read from config files
        let mut list = Allowlist::new();
        list.enforced = false;
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowlist_tool() {
        let mut list = Allowlist::new();
        list.enforced = true;
        list.allow_tool("read_file");
        assert!(list.is_tool_allowed("read_file"));
        assert!(!list.is_tool_allowed("bash"));
    }

    #[test]
    fn test_allowlist_json_roundtrip() {
        let mut list = Allowlist::new();
        list.enforced = true;
        list.allow_tool("read_file");
        list.allow_channel("slack");

        let json = list.to_json().unwrap();
        let parsed = Allowlist::from_json(&json).unwrap();

        assert!(parsed.is_tool_allowed("read_file"));
        assert!(parsed.is_channel_allowed("slack"));
    }
}
