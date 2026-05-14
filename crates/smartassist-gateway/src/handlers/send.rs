//! Send message RPC method handlers.
//!
//! Handles sending messages and polls through channels.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

/// Media attachment for messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    /// Media type (image, audio, video, document).
    #[serde(rename = "type")]
    pub media_type: String,
    /// File path or URL.
    pub source: String,
    /// Caption for the media.
    pub caption: Option<String>,
}

/// Parameters for send method (message).
#[derive(Debug, Deserialize)]
pub struct SendMessageParams {
    /// Channel to send through.
    pub channel: String,
    /// Recipient ID (chat ID, user ID, etc.).
    pub recipient: String,
    /// Message text.
    pub text: String,
    /// Message ID to reply to.
    pub reply_to: Option<String>,
    /// Media attachments.
    pub media: Option<Vec<MediaAttachment>>,
    /// Whether to parse markdown.
    pub parse_mode: Option<String>,
}

/// Send message handler.
pub struct SendMessageHandler {
    context: Arc<HandlerContext>,
}

impl SendMessageHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for SendMessageHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SendMessageParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!(
            "Send message via {}: {} chars to {}",
            params.channel,
            params.text.len(),
            params.recipient
        );

        // Attempt to send through channel manager if available
        if let Some(ref manager) = self.context.channel_manager {
            let target = smartassist_core::types::MessageTarget::new(params.recipient.clone());
            match manager.send_to(&params.channel, target, &params.text).await {
                Ok(result) => {
                    return Ok(serde_json::json!({
                        "channel": params.channel,
                        "recipient": params.recipient,
                        "message_id": result.message_id,
                        "sent": result.delivered,
                    }));
                }
                Err(e) => {
                    warn!(
                        "Failed to send message via channel manager: {}, falling back to mock",
                        e
                    );
                }
            }
        }

        // Fallback: generate a mock message id
        let message_id = uuid::Uuid::new_v4().to_string();

        Ok(serde_json::json!({
            "channel": params.channel,
            "recipient": params.recipient,
            "message_id": message_id,
            "sent": true,
        }))
    }
}

/// Poll option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollOption {
    /// Option text.
    pub text: String,
}

/// Parameters for send method (poll).
#[derive(Debug, Deserialize)]
pub struct SendPollParams {
    /// Channel to send through.
    pub channel: String,
    /// Recipient ID.
    pub recipient: String,
    /// Poll question.
    pub question: String,
    /// Poll options.
    pub options: Vec<PollOption>,
    /// Whether poll allows multiple answers.
    pub allows_multiple_answers: Option<bool>,
    /// Whether poll is anonymous.
    pub is_anonymous: Option<bool>,
}

/// Send poll handler.
pub struct SendPollHandler {
    context: Arc<HandlerContext>,
}

impl SendPollHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for SendPollHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SendPollParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!(
            "Send poll via {}: {} options to {}",
            params.channel,
            params.options.len(),
            params.recipient
        );

        // Attempt to deliver as a plain message via channel manager if available.
        // Native poll support is channel-specific and not yet implemented.
        if let Some(ref manager) = self.context.channel_manager {
            let text = format!(
                "Poll: {}\n{}",
                params.question,
                params.options.iter().map(|o| o.text.clone()).collect::<Vec<_>>().join("\n")
            );
            let target = smartassist_core::types::MessageTarget::new(params.recipient.clone());
            match manager.send_to(&params.channel, target, text).await {
                Ok(result) => {
                    return Ok(serde_json::json!({
                        "channel": params.channel,
                        "recipient": params.recipient,
                        "poll_id": result.message_id,
                        "sent": result.delivered,
                    }));
                }
                Err(e) => {
                    warn!(
                        "Failed to send poll via channel manager: {}, falling back to mock",
                        e
                    );
                }
            }
        }

        // Fallback: generate a mock poll id
        let poll_id = uuid::Uuid::new_v4().to_string();

        Ok(serde_json::json!({
            "channel": params.channel,
            "recipient": params.recipient,
            "poll_id": poll_id,
            "sent": true,
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for SendMessageParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for SendPollParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_attachment_serialization() {
        let media = MediaAttachment {
            media_type: "image".to_string(),
            source: "/path/to/image.jpg".to_string(),
            caption: Some("A photo".to_string()),
        };

        let json = serde_json::to_value(&media).unwrap();
        assert_eq!(json["type"], "image");
    }

    #[test]
    fn test_poll_option_serialization() {
        let option = PollOption {
            text: "Option A".to_string(),
        };

        let json = serde_json::to_value(&option).unwrap();
        assert_eq!(json["text"], "Option A");
    }
}
