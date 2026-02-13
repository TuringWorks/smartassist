//! RPC method registry and handlers.

use crate::error::GatewayError;
use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// Type alias for method handler futures.
pub type MethodFuture = Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send>>;

/// Trait for RPC method handlers.
#[async_trait]
pub trait MethodHandler: Send + Sync {
    /// Handle the method call.
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value>;
}

/// Registry for RPC methods.
pub struct MethodRegistry {
    /// Registered methods.
    methods: RwLock<HashMap<String, Arc<dyn MethodHandler>>>,
}

impl Default for MethodRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MethodRegistry {
    /// Create a new method registry.
    pub fn new() -> Self {
        let registry = Self {
            methods: RwLock::new(HashMap::new()),
        };

        // Register built-in methods
        // (Would be done here or via register_builtin)

        registry
    }

    /// Register a method handler.
    pub async fn register(&self, name: impl Into<String>, handler: Arc<dyn MethodHandler>) {
        let mut methods = self.methods.write().await;
        methods.insert(name.into(), handler);
    }

    /// Unregister a method.
    pub async fn unregister(&self, name: &str) {
        let mut methods = self.methods.write().await;
        methods.remove(name);
    }

    /// Call a method.
    pub async fn call(
        &self,
        name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let methods = self.methods.read().await;

        let handler = methods
            .get(name)
            .ok_or_else(|| GatewayError::MethodNotFound(name.to_string()))?;

        debug!("Calling method: {}", name);
        handler.call(params).await
    }

    /// List registered methods.
    pub async fn list(&self) -> Vec<String> {
        let methods = self.methods.read().await;
        methods.keys().cloned().collect()
    }
}

/// Helper macro for creating method handlers from closures.
#[macro_export]
macro_rules! method_handler {
    ($f:expr) => {{
        struct Handler<F>(F);

        #[async_trait::async_trait]
        impl<F, Fut> MethodHandler for Handler<F>
        where
            F: Fn(Option<serde_json::Value>) -> Fut + Send + Sync + 'static,
            Fut: std::future::Future<Output = Result<serde_json::Value>> + Send,
        {
            async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
                (self.0)(params).await
            }
        }

        std::sync::Arc::new(Handler($f)) as std::sync::Arc<dyn MethodHandler>
    }};
}

// Built-in method handlers

/// System info method.
pub struct SystemInfoHandler;

#[async_trait]
impl MethodHandler for SystemInfoHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "name": "smartassist-gateway",
            "version": env!("CARGO_PKG_VERSION"),
            "platform": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        }))
    }
}

/// Ping method.
pub struct PingHandler;

#[async_trait]
impl MethodHandler for PingHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "pong": true,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

/// List methods handler.
pub struct ListMethodsHandler {
    registry: Arc<MethodRegistry>,
}

impl ListMethodsHandler {
    pub fn new(registry: Arc<MethodRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl MethodHandler for ListMethodsHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let methods = self.registry.list().await;
        Ok(serde_json::json!({
            "methods": methods,
        }))
    }
}

/// Register built-in methods.
pub async fn register_builtin(registry: &MethodRegistry) {
    registry
        .register("system.info", Arc::new(SystemInfoHandler))
        .await;
    registry.register("ping", Arc::new(PingHandler)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_method_registry() {
        let registry = MethodRegistry::new();
        register_builtin(&registry).await;

        let result = registry.call("ping", None).await.unwrap();
        assert!(result.get("pong").is_some());
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let registry = MethodRegistry::new();

        let result = registry.call("nonexistent", None).await;
        assert!(matches!(result, Err(GatewayError::MethodNotFound(_))));
    }
}
