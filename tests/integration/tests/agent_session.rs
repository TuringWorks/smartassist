//! Agent session integration tests.

use futures::{sink::SinkExt, stream::StreamExt};
use smartassist_gateway::server::{Gateway, GatewayConfig};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;

async fn boot_test_gateway() -> (Gateway, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let gateway = Gateway::with_default_handlers(GatewayConfig::default()).await;
    let gateway_clone = gateway.clone();

    tokio::spawn(async move {
        let _ = gateway_clone.run_with_listener(listener).await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    let url = format!("http://{}", addr);
    (gateway, url)
}

async fn ws_rpc_call(ws_url: &str, method: &str, params: Option<serde_json::Value>) -> serde_json::Value {
    let (mut ws, _) = connect_async(ws_url).await.expect("WebSocket connect failed");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    ws.send(tokio_tungstenite::tungstenite::protocol::Message::Text(request.to_string()))
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

#[tokio::test]
async fn test_agent_session_create() {
    let (_gateway, url) = boot_test_gateway().await;
    let ws_url = url.replacen("http://", "ws://", 1) + "/ws";

    let response = ws_rpc_call(
        &ws_url,
        "sessions.create",
        Some(serde_json::json!({
            "session_key": "test-session-1",
            "agent_id": "agent-1",
            "system": "You are a helpful assistant.",
        })),
    )
    .await;

    assert!(
        response.get("error").is_none(),
        "Expected no error, got: {:?}",
        response.get("error")
    );
    assert_eq!(response["result"]["session_key"], "test-session-1");
    assert_eq!(response["result"]["status"], "active");
    assert!(response["result"]["created"].as_bool().unwrap());
}

#[tokio::test]
async fn test_agent_session_list() {
    let (_gateway, url) = boot_test_gateway().await;
    let ws_url = url.replacen("http://", "ws://", 1) + "/ws";

    // Create a session first
    ws_rpc_call(
        &ws_url,
        "sessions.create",
        Some(serde_json::json!({
            "session_key": "list-test",
            "agent_id": "agent-1",
        })),
    )
    .await;

    let response = ws_rpc_call(&ws_url, "sessions.list", None).await;
    assert!(response.get("error").is_none());
    assert!(response["result"]["sessions"].is_array());
    assert_eq!(response["result"]["total"], 1);
}

#[tokio::test]
async fn test_agent_session_history() {
    let (_gateway, url) = boot_test_gateway().await;
    let ws_url = url.replacen("http://", "ws://", 1) + "/ws";

    // Create a session with a system message
    ws_rpc_call(
        &ws_url,
        "sessions.create",
        Some(serde_json::json!({
            "session_key": "history-test",
            "agent_id": "agent-1",
            "system": "System prompt here",
        })),
    )
    .await;

    let response = ws_rpc_call(
        &ws_url,
        "sessions.history",
        Some(serde_json::json!({
            "session_key": "history-test",
        })),
    )
    .await;

    assert!(response.get("error").is_none());
    assert_eq!(response["result"]["session_key"], "history-test");
    assert_eq!(response["result"]["message_count"], 1);
}

#[tokio::test]
async fn test_agent_session_resolve() {
    let (_gateway, url) = boot_test_gateway().await;
    let ws_url = url.replacen("http://", "ws://", 1) + "/ws";

    ws_rpc_call(
        &ws_url,
        "sessions.create",
        Some(serde_json::json!({
            "session_key": "resolve-test",
            "agent_id": "agent-1",
        })),
    )
    .await;

    let response = ws_rpc_call(
        &ws_url,
        "sessions.resolve",
        Some(serde_json::json!({
            "label": "resolve-test",
        })),
    )
    .await;

    assert!(response.get("error").is_none());
    assert_eq!(response["result"]["found"], true);
    assert_eq!(response["result"]["session_key"], "resolve-test");
}
