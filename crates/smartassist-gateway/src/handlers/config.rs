//! Config RPC method handlers.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::handlers::exec::persist_config;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use tracing::debug;

/// Parameters for config.get method.
#[derive(Debug, Default, Deserialize)]
pub struct ConfigGetParams {
    /// Specific key to get (optional, returns all if not specified).
    pub key: Option<String>,
}

/// Config get method handler.
pub struct ConfigGetHandler {
    context: Arc<HandlerContext>,
}

impl ConfigGetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ConfigGetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ConfigGetParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        debug!("Config get request: {:?}", params.key);

        let config = self
            .context
            .config
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Config not available".to_string()))?;

        let config_value = config.read().await;

        match params.key {
            Some(key) => {
                // Get specific key using dot notation
                let value = get_nested_value(&config_value, &key);
                Ok(serde_json::json!({
                    "key": key,
                    "value": value,
                }))
            }
            None => {
                // Return all config
                Ok(config_value.clone())
            }
        }
    }
}

/// Parameters for config.set method.
#[derive(Debug, Deserialize)]
pub struct ConfigSetParams {
    /// Key to set.
    pub key: String,

    /// Value to set.
    pub value: serde_json::Value,
}

/// Config set method handler.
pub struct ConfigSetHandler {
    context: Arc<HandlerContext>,
}

impl ConfigSetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ConfigSetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ConfigSetParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Config set request: {} = {:?}", params.key, params.value);

        let config = self
            .context
            .config
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Config not available".to_string()))?;

        let mut config_value = config.write().await;
        set_nested_value(&mut config_value, &params.key, params.value.clone());

        // Persist to disk if a config path is configured.
        if let Some(ref path) = self.context.config_path {
            persist_config(&config_value, path).await?;
        }

        Ok(serde_json::json!({
            "key": params.key,
            "value": params.value,
            "updated": true,
        }))
    }
}

/// Parameters for config.patch method.
#[derive(Debug, Deserialize)]
pub struct ConfigPatchParams {
    /// Patch to apply (JSON merge patch).
    pub patch: serde_json::Value,
}

/// Config patch method handler.
pub struct ConfigPatchHandler {
    context: Arc<HandlerContext>,
}

impl ConfigPatchHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ConfigPatchHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ConfigPatchParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Config patch request");

        let config = self
            .context
            .config
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Config not available".to_string()))?;

        let mut config_value = config.write().await;
        json_merge_patch(&mut config_value, &params.patch);

        // Persist to disk if a config path is configured.
        if let Some(ref path) = self.context.config_path {
            persist_config(&config_value, path).await?;
        }

        Ok(serde_json::json!({
            "patched": true,
        }))
    }
}

/// Config schema method handler.
pub struct ConfigSchemaHandler {
    _context: Arc<HandlerContext>,
}

impl ConfigSchemaHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for ConfigSchemaHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Config schema request");

        // Return JSON Schema for configuration
        Ok(serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "gateway": {
                    "type": "object",
                    "properties": {
                        "bind": {
                            "type": "string",
                            "enum": ["loopback", "lan", "tailnet", "auto"],
                            "description": "Network bind mode"
                        },
                        "port": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 65535,
                            "description": "Gateway port"
                        }
                    }
                },
                "agent": {
                    "type": "object",
                    "properties": {
                        "model": {
                            "type": "string",
                            "description": "Default model to use"
                        },
                        "max_tokens": {
                            "type": "integer",
                            "description": "Maximum tokens for responses"
                        }
                    }
                },
                "channels": {
                    "type": "object",
                    "description": "Channel configurations"
                },
                "tools": {
                    "type": "object",
                    "description": "Tool configurations"
                }
            }
        }))
    }
}

// Helper functions

/// Get a nested value from JSON using dot notation.
fn get_nested_value(value: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = value;

    for part in parts {
        current = current.get(part)?;
    }

    Some(current.clone())
}

/// Set a nested value in JSON using dot notation.
fn set_nested_value(value: &mut serde_json::Value, key: &str, new_value: serde_json::Value) {
    let parts: Vec<&str> = key.split('.').collect();

    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        if let serde_json::Value::Object(map) = value {
            map.insert(parts[0].to_string(), new_value);
        }
        return;
    }

    let mut current = value;
    for part in &parts[..parts.len() - 1] {
        if !current.is_object() {
            *current = serde_json::json!({});
        }
        current = current
            .as_object_mut()
            .unwrap()
            .entry(part.to_string())
            .or_insert(serde_json::json!({}));
    }

    if let serde_json::Value::Object(map) = current {
        map.insert(parts[parts.len() - 1].to_string(), new_value);
    }
}

/// Apply JSON merge patch (RFC 7386).
fn json_merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if patch.is_object() {
        if !target.is_object() {
            *target = serde_json::json!({});
        }

        let target_map = target.as_object_mut().unwrap();
        let patch_map = patch.as_object().unwrap();

        for (key, value) in patch_map {
            if value.is_null() {
                target_map.remove(key);
            } else {
                let entry = target_map.entry(key.clone()).or_insert(serde_json::json!(null));
                json_merge_patch(entry, value);
            }
        }
    } else {
        *target = patch.clone();
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for ConfigSetParams {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ConfigPatchParams {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_nested_value() {
        let value = serde_json::json!({
            "a": {
                "b": {
                    "c": 123
                }
            }
        });

        assert_eq!(
            get_nested_value(&value, "a.b.c"),
            Some(serde_json::json!(123))
        );
        assert_eq!(get_nested_value(&value, "a.b.d"), None);
    }

    #[test]
    fn test_set_nested_value() {
        let mut value = serde_json::json!({});

        set_nested_value(&mut value, "a.b.c", serde_json::json!(123));
        assert_eq!(value, serde_json::json!({
            "a": {
                "b": {
                    "c": 123
                }
            }
        }));
    }

    #[test]
    fn test_json_merge_patch() {
        let mut target = serde_json::json!({
            "a": 1,
            "b": 2
        });

        let patch = serde_json::json!({
            "b": 3,
            "c": 4
        });

        json_merge_patch(&mut target, &patch);

        assert_eq!(target, serde_json::json!({
            "a": 1,
            "b": 3,
            "c": 4
        }));
    }

    #[test]
    fn test_json_merge_patch_delete() {
        let mut target = serde_json::json!({
            "a": 1,
            "b": 2
        });

        let patch = serde_json::json!({
            "b": null
        });

        json_merge_patch(&mut target, &patch);

        assert_eq!(target, serde_json::json!({
            "a": 1
        }));
    }
}
