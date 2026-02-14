//! WebSocket JSON-RPC client for communicating with the gateway.

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    id: String,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<String>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

struct Inner {
    write: Mutex<Option<futures::stream::SplitSink<WsStream, Message>>>,
    pending: Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>,
    connected: std::sync::atomic::AtomicBool,
}

/// WebSocket JSON-RPC client for the SmartAssist gateway.
#[derive(Clone)]
pub struct WsClient {
    inner: Arc<Inner>,
}

impl WsClient {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                write: Mutex::new(None),
                pending: Mutex::new(HashMap::new()),
                connected: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }

    /// Connect to the gateway WebSocket at the given URL.
    pub async fn connect(&self, url: &str) -> Result<(), String> {
        // Disconnect existing connection if any
        self.disconnect().await;

        let (ws_stream, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| format!("WebSocket connection failed: {}", e))?;

        let (write, read) = ws_stream.split();
        *self.inner.write.lock().await = Some(write);
        self.inner
            .connected
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Spawn read loop
        let inner = self.inner.clone();
        tokio::spawn(async move {
            Self::read_loop(inner, read).await;
        });

        Ok(())
    }

    async fn read_loop(
        inner: Arc<Inner>,
        mut read: futures::stream::SplitStream<WsStream>,
    ) {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&text) {
                        if let Some(id) = resp.id {
                            let mut pending = inner.pending.lock().await;
                            if let Some(sender) = pending.remove(&id) {
                                let result = if let Some(err) = resp.error {
                                    Err(format!("RPC error {}: {}", err.code, err.message))
                                } else {
                                    Ok(resp.result.unwrap_or(Value::Null))
                                };
                                let _ = sender.send(result);
                            }
                        }
                        // Notifications (no id) are silently ignored for now
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {} // Ignore ping/pong/binary
            }
        }

        // Mark as disconnected and fail all pending requests
        inner
            .connected
            .store(false, std::sync::atomic::Ordering::Relaxed);
        *inner.write.lock().await = None;
        let mut pending = inner.pending.lock().await;
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err("WebSocket disconnected".to_string()));
        }
    }

    /// Make a JSON-RPC call and wait for the response.
    pub async fn call(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id: id.clone(),
        };

        let msg = serde_json::to_string(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let (tx, rx) = oneshot::channel();

        // Register pending request
        self.inner.pending.lock().await.insert(id.clone(), tx);

        // Send the message
        {
            let mut write_guard = self.inner.write.lock().await;
            match write_guard.as_mut() {
                Some(write) => {
                    if let Err(e) = write.send(Message::Text(msg)).await {
                        self.inner.pending.lock().await.remove(&id);
                        return Err(format!("Failed to send: {}", e));
                    }
                }
                None => {
                    self.inner.pending.lock().await.remove(&id);
                    return Err("Not connected to gateway".to_string());
                }
            }
        }

        // Wait for response with timeout
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Request cancelled".to_string()),
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                Err("Request timed out".to_string())
            }
        }
    }

    /// Disconnect from the gateway.
    pub async fn disconnect(&self) {
        if let Some(mut write) = self.inner.write.lock().await.take() {
            let _ = write.close().await;
        }
        self.inner
            .connected
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let mut pending = self.inner.pending.lock().await;
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err("Disconnected".to_string()));
        }
    }

    pub fn is_connected(&self) -> bool {
        self.inner
            .connected
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
