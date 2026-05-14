//! Canvas A2UI protocol for agent-driven rendering.
//!
//! Defines the WebSocket message format used between the gateway
//! and canvas clients (desktop, mobile, web).

use serde::{Deserialize, Serialize};

/// Protocol version.
pub const PROTOCOL_VERSION: &str = "1.0";

/// Messages sent from the gateway to the canvas client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ServerMessage {
    Init {
        protocol_version: String,
        surface_id: String,
        capabilities: Vec<String>,
    },
    Update {
        surface_id: String,
        action: crate::CanvasAction,
        result: crate::CanvasActionResult,
    },
    Sync {
        surface_id: String,
        elements: Vec<crate::CanvasElement>,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Messages sent from the canvas client to the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ClientMessage {
    Subscribe {
        surface_id: String,
    },
    Unsubscribe {
        surface_id: String,
    },
    Action {
        surface_id: String,
        action: crate::CanvasAction,
    },
    Ack {
        message_id: String,
    },
}

/// Parse a raw WebSocket text message into a client message.
pub fn parse_client_message(text: &str) -> Result<ClientMessage, ProtocolError> {
    serde_json::from_str(text).map_err(|e| ProtocolError::ParseError(e.to_string()))
}

/// Serialize a server message to JSON.
pub fn serialize_server_message(msg: &ServerMessage) -> Result<String, ProtocolError> {
    serde_json::to_string(msg).map_err(|e| ProtocolError::SerializeError(e.to_string()))
}

/// Protocol errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProtocolError {
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("serialize error: {0}")]
    SerializeError(String),
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasAction, CanvasActionResult};

    #[test]
    fn test_parse_subscribe_message() {
        let json = r#"{"type":"subscribe","surface_id":"surf-1"}"#;
        let msg = parse_client_message(json).unwrap();
        match msg {
            ClientMessage::Subscribe { surface_id } => assert_eq!(surface_id, "surf-1"),
            _ => panic!("expected Subscribe"),
        }
    }

    #[test]
    fn test_parse_action_message() {
        let json = r#"{"type":"action","surface_id":"surf-1","action":{"action":"present"}}"#;
        let msg = parse_client_message(json).unwrap();
        match msg {
            ClientMessage::Action { surface_id, action } => {
                assert_eq!(surface_id, "surf-1");
                assert!(matches!(action, CanvasAction::Present));
            }
            _ => panic!("expected Action"),
        }
    }

    #[test]
    fn test_serialize_init_message() {
        let msg = ServerMessage::Init {
            protocol_version: PROTOCOL_VERSION.to_string(),
            surface_id: "surf-1".to_string(),
            capabilities: vec!["add".to_string(), "remove".to_string()],
        };
        let json = serialize_server_message(&msg).unwrap();
        assert!(json.contains("init"));
        assert!(json.contains("1.0"));
    }

    #[test]
    fn test_serialize_error_message() {
        let msg = ServerMessage::Error {
            code: "SURFACE_NOT_FOUND".to_string(),
            message: "Surface does not exist".to_string(),
        };
        let json = serialize_server_message(&msg).unwrap();
        assert!(json.contains("SURFACE_NOT_FOUND"));
    }
}
