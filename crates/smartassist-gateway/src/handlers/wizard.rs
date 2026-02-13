//! Wizard RPC method handlers.
//!
//! Handles setup wizard flow for initial configuration.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Wizard step info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardStep {
    /// Step ID.
    pub id: String,
    /// Step title.
    pub title: String,
    /// Step description.
    pub description: Option<String>,
    /// Whether step is completed.
    pub completed: bool,
    /// Whether step is current.
    pub current: bool,
    /// Step type (info, input, select, confirm).
    pub step_type: String,
    /// Step data/options.
    pub data: Option<serde_json::Value>,
}

/// Wizard status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardStatus {
    /// Whether wizard is active.
    pub active: bool,
    /// Current step index.
    pub current_step: usize,
    /// Total steps.
    pub total_steps: usize,
    /// Steps info.
    pub steps: Vec<WizardStep>,
    /// Whether wizard is complete.
    pub complete: bool,
}

/// Wizard start handler.
pub struct WizardStartHandler {
    _context: Arc<HandlerContext>,
}

impl WizardStartHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for WizardStartHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Wizard start request");

        let steps = vec![
            WizardStep {
                id: "welcome".to_string(),
                title: "Welcome".to_string(),
                description: Some("Welcome to SmartAssist setup".to_string()),
                completed: false,
                current: true,
                step_type: "info".to_string(),
                data: None,
            },
            WizardStep {
                id: "provider".to_string(),
                title: "AI Provider".to_string(),
                description: Some("Configure your AI provider".to_string()),
                completed: false,
                current: false,
                step_type: "select".to_string(),
                data: Some(serde_json::json!({
                    "options": ["anthropic", "openai", "ollama"]
                })),
            },
            WizardStep {
                id: "api_key".to_string(),
                title: "API Key".to_string(),
                description: Some("Enter your API key".to_string()),
                completed: false,
                current: false,
                step_type: "input".to_string(),
                data: Some(serde_json::json!({
                    "input_type": "password"
                })),
            },
            WizardStep {
                id: "channels".to_string(),
                title: "Channels".to_string(),
                description: Some("Configure messaging channels".to_string()),
                completed: false,
                current: false,
                step_type: "select".to_string(),
                data: Some(serde_json::json!({
                    "options": ["telegram", "discord", "slack", "signal"],
                    "multiple": true
                })),
            },
            WizardStep {
                id: "complete".to_string(),
                title: "Complete".to_string(),
                description: Some("Setup complete!".to_string()),
                completed: false,
                current: false,
                step_type: "confirm".to_string(),
                data: None,
            },
        ];

        let status = WizardStatus {
            active: true,
            current_step: 0,
            total_steps: steps.len(),
            steps,
            complete: false,
        };

        Ok(serde_json::to_value(status).unwrap())
    }
}

/// Parameters for wizard.next method.
#[derive(Debug, Deserialize)]
pub struct WizardNextParams {
    /// Current step ID.
    pub step_id: String,
    /// Step response/input.
    pub response: Option<serde_json::Value>,
}

/// Wizard next handler.
pub struct WizardNextHandler {
    _context: Arc<HandlerContext>,
}

impl WizardNextHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for WizardNextHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: WizardNextParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Wizard next: step={}", params.step_id);

        // TODO: Actually process the step and advance

        Ok(serde_json::json!({
            "previous_step": params.step_id,
            "next_step": "provider",
            "complete": false,
        }))
    }
}

/// Wizard cancel handler.
pub struct WizardCancelHandler {
    _context: Arc<HandlerContext>,
}

impl WizardCancelHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for WizardCancelHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Wizard cancel request");

        Ok(serde_json::json!({
            "cancelled": true,
        }))
    }
}

/// Wizard status handler.
pub struct WizardStatusHandler {
    _context: Arc<HandlerContext>,
}

impl WizardStatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for WizardStatusHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Wizard status request");

        // Return inactive wizard status
        let status = WizardStatus {
            active: false,
            current_step: 0,
            total_steps: 0,
            steps: vec![],
            complete: true,
        };

        Ok(serde_json::to_value(status).unwrap())
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for WizardNextParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_step_serialization() {
        let step = WizardStep {
            id: "test".to_string(),
            title: "Test Step".to_string(),
            description: Some("A test step".to_string()),
            completed: false,
            current: true,
            step_type: "info".to_string(),
            data: None,
        };

        let json = serde_json::to_value(&step).unwrap();
        assert_eq!(json["id"], "test");
        assert_eq!(json["current"], true);
    }

    #[test]
    fn test_wizard_status_serialization() {
        let status = WizardStatus {
            active: true,
            current_step: 0,
            total_steps: 3,
            steps: vec![],
            complete: false,
        };

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["active"], true);
        assert_eq!(json["total_steps"], 3);
    }
}
