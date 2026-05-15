//! Channel lifecycle integration tests.

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
async fn test_channel_list_and_health() {
    let (_gateway, url) = boot_test_gateway().await;
    let ws_url = url.replacen("http://", "ws://", 1) + "/ws";

    let list = ws_rpc_call(&ws_url, "channel.list", None).await;
    assert!(
        list.get("error").is_none(),
        "Expected no error, got: {:?}",
        list.get("error")
    );
    assert!(list["result"]["channels"].is_array());
    assert!(list["result"]["count"].is_number());

    let health = ws_rpc_call(&ws_url, "channel.health", None).await;
    assert!(
        health.get("error").is_none(),
        "Expected no error, got: {:?}",
        health.get("error")
    );
    assert!(health["result"]["running"].is_boolean());
    assert!(health["result"]["channels_total"].is_number());
    assert!(health["result"]["channels_connected"].is_number());
}

#[tokio::test]
async fn test_channel_send_roundtrip() {
    let (_gateway, url) = boot_test_gateway().await;
    let ws_url = url.replacen("http://", "ws://", 1) + "/ws";

    // channel.send requires recipient; without real channels it will fall back to mock
    let response = ws_rpc_call(
        &ws_url,
        "send",
        Some(serde_json::json!({
            "channel": "web",
            "recipient": "test-user",
            "text": "Hello from integration test",
        })),
    )
    .await;

    assert!(
        response.get("error").is_none(),
        "Expected no error, got: {:?}",
        response.get("error")
    );
    assert!(response["result"]["sent"].as_bool().unwrap() || response["result"]["mock"].as_bool().unwrap());
}
