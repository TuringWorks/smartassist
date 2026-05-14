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
    context: Arc<HandlerContext>,
}

impl SkillsStatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for SkillsStatusHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Skills status request");

        let skills = self.context.skills.read().await;
        let result: Vec<SkillInfo> = skills
            .values()
            .map(|s| SkillInfo {
                id: s.id.clone(),
                name: s.name.clone(),
                version: s.version.clone(),
                description: s.description.clone(),
                enabled: s.enabled,
                builtin: s.builtin,
                path: s.path.clone(),
            })
            .collect();

        Ok(serde_json::json!({
            "skills": result,
            "count": result.len(),
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
    context: Arc<HandlerContext>,
}

impl SkillsInstallHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
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

        let mut skills = self.context.skills.write().await;
        skills.insert(
            params.package.clone(),
            super::SkillData {
                id: params.package.clone(),
                name: params.package.clone(),
                version: params.version.unwrap_or_else(|| "1.0.0".to_string()),
                description: None,
                enabled: true,
                builtin: false,
                path: None,
            },
        );

        Ok(serde_json::json!({
            "package": params.package,
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
    context: Arc<HandlerContext>,
}

impl SkillsUpdateHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
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

        let mut skills = self.context.skills.write().await;
        let updated = if let Some(skill) = skills.get_mut(&params.id) {
            if let Some(version) = params.version {
                skill.version = version;
            }
            true
        } else {
            false
        };

        Ok(serde_json::json!({
            "id": params.id,
            "updated": updated,
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

    #[tokio::test]
    async fn test_skills_status_empty() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = SkillsStatusHandler::new(ctx);
        let result = handler.call(None).await.unwrap();
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_skills_install_and_status() {
        let ctx = Arc::new(HandlerContext::new());

        let install = SkillsInstallHandler::new(ctx.clone());
        let params = serde_json::json!({
            "package": "test-skill",
            "version": "2.0.0"
        });
        let result = install.call(Some(params)).await.unwrap();
        assert_eq!(result["installed"], true);

        let status = SkillsStatusHandler::new(ctx);
        let result = status.call(None).await.unwrap();
        assert_eq!(result["count"], 1);
        let skills = result["skills"].as_array().unwrap();
        assert_eq!(skills[0]["id"], "test-skill");
        assert_eq!(skills[0]["version"], "2.0.0");
    }

    #[tokio::test]
    async fn test_skills_update_existing() {
        let ctx = Arc::new(HandlerContext::new());

        // Install first
        let install = SkillsInstallHandler::new(ctx.clone());
        let params = serde_json::json!({"package": "skill-a"});
        install.call(Some(params)).await.unwrap();

        // Update
        let update = SkillsUpdateHandler::new(ctx);
        let params = serde_json::json!({
            "id": "skill-a",
            "version": "3.0.0"
        });
        let result = update.call(Some(params)).await.unwrap();
        assert_eq!(result["updated"], true);
    }

    #[tokio::test]
    async fn test_skills_update_missing() {
        let ctx = Arc::new(HandlerContext::new());
        let update = SkillsUpdateHandler::new(ctx);
        let params = serde_json::json!({
            "id": "missing",
            "version": "1.0.0"
        });
        let result = update.call(Some(params)).await.unwrap();
        assert_eq!(result["updated"], false);
    }
}
