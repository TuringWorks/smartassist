//! Gateway boot integration tests.

use futures::{sink::SinkExt, stream::StreamExt};
use smartassist_core::config::BindMode;
use smartassist_gateway::server::{Gateway, GatewayConfig};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;

/// Helper: boot a gateway on a random port and return the gateway + HTTP URL.
async fn boot_test_gateway() -> (Gateway, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let gateway = Gateway::with_default_handlers(GatewayConfig::default()).await;
    let gateway_clone = gateway.clone();

    tokio::spawn(async move {
        let _ = gateway_clone.run_with_listener(listener).await;
    });

    // Give the server a moment to start accepting connections
    tokio::time::sleep(Duration::from_millis(100)).await;

    let url = format!("http://{}", addr);
    (gateway, url)
}

/// Helper: connect WebSocket and send a JSON-RPC request, returning the response.
async fn ws_rpc_call(ws_url: &str, method: &str, params: Option<serde_json::Value>) -> serde_json::Value {
    let (mut ws, _) = connect_async(ws_url).await.expect("WebSocket connect failed");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    ws.send(tokio_tungstenite::tungstenite::protocol::Message::Text(
        request.to_string(),
    ))
    .await
    .unwrap();

    let response = timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("WebSocket response timed out")
        .expect("WebSocket stream ended")
        .expect("WebSocket error");

    match response {
        tokio_tungstenite::tungstenite::protocol::Message::Text(text) => {
            serde_json::from_str(&text).expect("Invalid JSON response")
        }
        _ => panic!("Expected text message"),
    }
}

/// Verify that the gateway can boot and respond to a health check.
#[tokio::test]
async fn test_gateway_boots_and_responds() {
    let (_gateway, url) = boot_test_gateway().await;

    let client = reqwest::Client::new();
    let resp = timeout(
        Duration::from_secs(5),
        client.get(format!("{}/health", url)).send(),
    )
    .await
    .expect("HTTP request timed out")
    .expect("HTTP request failed");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.expect("Invalid JSON body");
    assert_eq!(body["status"], "ok");
    assert!(body["clients"].is_number());
}

/// Verify that the gateway rejects unauthenticated requests when auth is enabled.
#[tokio::test]
async fn test_gateway_rejects_unauthenticated() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = GatewayConfig {
        bind: BindMode::Lan,
        require_auth: true,
        auth_token: Some("secret-token".to_string()),
        ..Default::default()
    };

    let gateway = Gateway::with_default_handlers(config).await;
    let gateway_clone = gateway.clone();

    tokio::spawn(async move {
        let _ = gateway_clone.run_with_listener(listener).await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let ws_url = format!("ws://{}/ws", addr);
    let connect_result = timeout(Duration::from_secs(5), connect_async(&ws_url)).await;
    assert!(connect_result.is_ok(), "Connection timed out");
    assert!(
        connect_result.unwrap().is_err(),
        "Expected 401 rejection without auth token"
    );
}

/// Verify that WebSocket ping works.
#[tokio::test]
async fn test_gateway_websocket_ping() {
    let (_gateway, url) = boot_test_gateway().await;
    let ws_url = url.replacen("http://", "ws://", 1) + "/ws";

    let response = ws_rpc_call(&ws_url, "ping", None).await;
    assert_eq!(response["jsonrpc"], "2.0");
    assert!(
        response.get("error").is_none(),
        "Expected no error, got: {:?}",
        response.get("error")
    );
}

/// Verify that WebSocket system.info returns metadata.
#[tokio::test]
async fn test_gateway_websocket_system_info() {
    let (_gateway, url) = boot_test_gateway().await;
    let ws_url = url.replacen("http://", "ws://", 1) + "/ws";

    let response = ws_rpc_call(&ws_url, "system.info", None).await;
    assert_eq!(response["jsonrpc"], "2.0");
    assert!(
        response.get("error").is_none(),
        "Expected no error, got: {:?}",
        response.get("error")
    );
    assert!(response["result"]["name"].is_string());
    assert!(response["result"]["version"].is_string());
}
