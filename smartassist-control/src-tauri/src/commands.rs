//! Tauri IPC commands for the SmartAssist control panel.

use crate::gateway::{GatewayManager, GatewayStatus};
use crate::ws_client::WsClient;
use smartassist_core::config::Config;
use smartassist_core::paths;
use tauri::State;

/// Load the current configuration.
#[tauri::command]
pub fn get_config() -> Result<serde_json::Value, String> {
    let config = Config::load_or_default();
    serde_json::to_value(&config).map_err(|e| e.to_string())
}

/// Save configuration to the default path.
#[tauri::command]
pub fn save_config(config: serde_json::Value) -> Result<(), String> {
    let parsed: Config =
        serde_json::from_value(config).map_err(|e| format!("Invalid configuration: {}", e))?;

    // Validate before saving
    if let Err(e) = parsed.validate() {
        return Err(format!("Validation error: {}", e));
    }

    parsed.save_default().map_err(|e| e.to_string())
}

/// Validate configuration without saving.
#[tauri::command]
pub fn validate_config(config: serde_json::Value) -> Result<Vec<String>, String> {
    let parsed: Config =
        serde_json::from_value(config).map_err(|e| format!("Invalid configuration: {}", e))?;

    match parsed.validate() {
        Ok(()) => Ok(vec![]),
        Err(e) => Ok(e.to_string().lines().map(|l| l.to_string()).collect()),
    }
}

/// Get the config file path.
#[tauri::command]
pub fn get_config_path() -> Result<String, String> {
    paths::config_file()
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

/// Reset config to defaults and save.
#[tauri::command]
pub fn reset_config() -> Result<(), String> {
    let config = Config::default();
    config.save_default().map_err(|e| e.to_string())
}

/// Get the application version.
#[tauri::command]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get gateway process status.
#[tauri::command]
pub async fn gateway_status(state: State<'_, GatewayManager>) -> Result<GatewayStatus, String> {
    state.refresh_status().await;
    Ok(state.status())
}

/// Start the gateway subprocess.
#[tauri::command]
pub async fn gateway_start(
    state: State<'_, GatewayManager>,
) -> Result<(), String> {
    let config = Config::load_or_default();
    let port = config.gateway.port;
    let provider = config
        .agents
        .defaults
        .model
        .as_deref()
        .and_then(|m| m.split('/').next())
        .unwrap_or("anthropic")
        .to_string();
    state.start(port, &provider).await
}

/// Stop the gateway subprocess.
#[tauri::command]
pub async fn gateway_stop(
    state: State<'_, GatewayManager>,
) -> Result<(), String> {
    state.stop().await
}

/// Restart the gateway subprocess.
#[tauri::command]
pub async fn gateway_restart(
    state: State<'_, GatewayManager>,
) -> Result<(), String> {
    // Ignore stop error (might not be running)
    let _ = state.stop().await;
    let config = Config::load_or_default();
    let port = config.gateway.port;
    let provider = config
        .agents
        .defaults
        .model
        .as_deref()
        .and_then(|m| m.split('/').next())
        .unwrap_or("anthropic")
        .to_string();
    state.start(port, &provider).await
}

/// Forward a JSON-RPC call to the gateway via WebSocket.
/// Auto-connects if not already connected.
#[tauri::command]
pub async fn rpc_call(
    method: String,
    params: Option<serde_json::Value>,
    gw: State<'_, GatewayManager>,
    ws: State<'_, WsClient>,
) -> Result<serde_json::Value, String> {
    // Auto-connect if needed
    if !ws.is_connected() {
        let status = gw.status();
        if !status.running {
            return Err("Gateway is not running".to_string());
        }
        let url = format!("ws://127.0.0.1:{}/ws", status.port);
        ws.connect(&url).await?;
    }

    ws.call(&method, params).await
}
