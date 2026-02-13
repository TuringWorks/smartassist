//! Skills RPC method handlers.
//!
//! Handles skill/plugin installation and management.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Skill info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Skill ID.
    pub id: String,
    /// Skill name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Description.
    pub description: Option<String>,
    /// Whether skill is enabled.
    pub enabled: bool,
    /// Whether skill is built-in.
    pub builtin: bool,
    /// Installation path.
    pub path: Option<String>,
}

/// Skills status handler.
pub struct SkillsStatusHandler {
    _context: Arc<HandlerContext>,
}

impl SkillsStatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for SkillsStatusHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Skills status request");

        // TODO: Get actual skills from plugin manager
        let skills: Vec<SkillInfo> = vec![
            SkillInfo {
                id: "commit".to_string(),
                name: "Git Commit".to_string(),
                version: "1.0.0".to_string(),
                description: Some("Create git commits with AI-generated messages".to_string()),
                enabled: true,
                builtin: true,
                path: None,
            },
            SkillInfo {
                id: "review-pr".to_string(),
                name: "PR Review".to_string(),
                version: "1.0.0".to_string(),
                description: Some("Review pull requests".to_string()),
                enabled: true,
                builtin: true,
                path: None,
            },
        ];

        Ok(serde_json::json!({
            "skills": skills,
            "count": skills.len(),
        }))
    }
}

/// Skills bins handler - get skill binaries/executables.
pub struct SkillsBinsHandler {
    _context: Arc<HandlerContext>,
}

impl SkillsBinsHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for SkillsBinsHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Skills bins request");

        Ok(serde_json::json!({
            "bins": [],
            "count": 0,
        }))
    }
}

/// Parameters for skills.install method.
#[derive(Debug, Deserialize)]
pub struct SkillsInstallParams {
    /// Skill package name or URL.
    pub package: String,
    /// Version constraint.
    pub version: Option<String>,
}

/// Skills install handler.
pub struct SkillsInstallHandler {
    _context: Arc<HandlerContext>,
}

impl SkillsInstallHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for SkillsInstallHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SkillsInstallParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Skills install: {}", params.package);

        // TODO: Actually install the skill

        Ok(serde_json::json!({
            "package": params.package,
            "version": params.version,
            "installed": true,
        }))
    }
}

/// Parameters for skills.update method.
#[derive(Debug, Deserialize)]
pub struct SkillsUpdateParams {
    /// Skill ID to update.
    pub id: String,
    /// Target version (optional, latest if not specified).
    pub version: Option<String>,
}

/// Skills update handler.
pub struct SkillsUpdateHandler {
    _context: Arc<HandlerContext>,
}

impl SkillsUpdateHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for SkillsUpdateHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SkillsUpdateParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Skills update: {}", params.id);

        // TODO: Actually update the skill

        Ok(serde_json::json!({
            "id": params.id,
            "version": params.version,
            "updated": true,
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for SkillsInstallParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for SkillsUpdateParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_info_serialization() {
        let skill = SkillInfo {
            id: "test-skill".to_string(),
            name: "Test Skill".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test skill".to_string()),
            enabled: true,
            builtin: false,
            path: Some("/path/to/skill".to_string()),
        };

        let json = serde_json::to_value(&skill).unwrap();
        assert_eq!(json["id"], "test-skill");
        assert_eq!(json["enabled"], true);
    }
}
