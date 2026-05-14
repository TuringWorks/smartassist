//! VNC bridge for visual browser debugging.
//!
//! Exposes a NoVNC-compatible WebSocket endpoint for remote
//! interaction with sandboxed browser sessions.

use super::BrowserError;
use std::net::SocketAddr;

/// VNC server configuration.
#[derive(Debug, Clone)]
pub struct VncConfig {
    pub bind_addr: SocketAddr,
    pub password: Option<String>,
    pub no_password: bool,
}

impl Default for VncConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:5900".parse().unwrap(),
            password: None,
            no_password: true,
        }
    }
}

/// VNC bridge for a single browser session.
pub struct VncBridge {
    config: VncConfig,
}

impl VncBridge {
    pub fn new(config: VncConfig) -> Self {
        Self { config }
    }

    /// Start the VNC server.
    pub async fn start(&self) -> Result<(), BrowserError> {
        // TODO: Implement VNC server using tokio-tungstenite or similar
        tracing::info!(
            "VNC bridge starting on {} (password: {})",
            self.config.bind_addr,
            if self.config.no_password { "none" } else { "set" }
        );
        Ok(())
    }

    /// Stop the VNC server.
    pub async fn stop(&self) -> Result<(), BrowserError> {
        tracing::info!("VNC bridge stopping");
        Ok(())
    }

    /// Get the NoVNC WebSocket URL.
    pub fn websocket_url(&self) -> String {
        format!("ws://{}/vnc", self.config.bind_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vnc_bridge_start_stop() {
        let bridge = VncBridge::new(VncConfig::default());
        bridge.start().await.unwrap();
        bridge.stop().await.unwrap();
    }

    #[test]
    fn test_websocket_url_format() {
        let bridge = VncBridge::new(VncConfig::default());
        assert_eq!(bridge.websocket_url(), "ws://0.0.0.0:5900/vnc");
    }
}
