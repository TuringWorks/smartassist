//! Slack channel implementation with Socket Mode support.
//!
//! This module provides a Slack channel adapter that supports:
//! - Socket Mode for real-time event reception (requires app token)
//! - Web API for sending messages
//! - File uploads and downloads
//! - Thread replies
//! - Reactions

#![cfg(feature = "slack")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    MessageRef, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use smartassist_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatInfo, ChatType,
    HealthStatus, InboundMessage, MediaAttachment, MediaCapabilities, MediaType, MessageId,
    MessageTarget, OutboundMessage, QuotedMessage, SenderInfo,
};
use slack_morphism::prelude::*;
use slack_morphism::api::{
    SlackApiFilesGetUploadUrlExternalRequest, SlackApiFilesUploadViaUrlRequest,
    SlackApiFilesCompleteUploadExternalRequest, SlackApiFilesComplete,
};
use slack_morphism::hyper_tokio::{SlackClientHyperHttpsConnector, SlackHyperClient};
use slack_morphism::listener::SlackClientEventsUserState;
use slack_morphism::socket_mode::{
    SlackClientSocketModeConfig, SlackClientSocketModeListener, SlackSocketModeListenerCallbacks,
};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Slack channel implementation.
pub struct SlackChannel {
    /// Bot token.
    bot_token: String,

    /// App token for Socket Mode.
    app_token: Option<String>,

    /// Channel instance ID.
    instance_id: String,

    /// Workspace ID.
    workspace_id: Option<String>,

    /// Slack API client.
    client: Arc<RwLock<Option<Arc<SlackHyperClient>>>>,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Incoming message channel.
    #[allow(dead_code)]
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl std::fmt::Debug for SlackChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackChannel")
            .field("instance_id", &self.instance_id)
            .field("workspace_id", &self.workspace_id)
            .finish()
    }
}

impl SlackChannel {
    /// Create a new Slack channel.
    pub fn new(
        bot_token: impl Into<String>,
        app_token: Option<String>,
        instance_id: impl Into<String>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        Self {
            bot_token: bot_token.into(),
            app_token,
            instance_id: instance_id.into(),
            workspace_id: None,
            client: Arc::new(RwLock::new(None)),
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig, bot_token: String, app_token: Option<String>) -> Self {
        let mut channel = Self::new(bot_token, app_token, config.instance_id);
        channel.workspace_id = config.options.get("workspace_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        channel
    }

    /// Convert Slack message event to InboundMessage.
    #[allow(dead_code)]
    fn convert_message(
        &self,
        event: &SlackMessageEvent,
        channel_id: &SlackChannelId,
    ) -> Option<InboundMessage> {
        let user_id = event.sender.user.as_ref()?.to_string();

        let sender = SenderInfo {
            id: user_id.clone(),
            username: None, // Would need additional API call to get username
            display_name: None,
            phone_number: None,
            is_bot: event.sender.bot_id.is_some(),
        };

        // Determine chat type based on channel prefix
        let channel_id_str = channel_id.to_string();
        let chat_type = if channel_id_str.starts_with('D') {
            ChatType::Direct
        } else if channel_id_str.starts_with('G') {
            ChatType::Group
        } else {
            ChatType::Channel
        };

        let chat = ChatInfo {
            id: channel_id_str.clone(),
            chat_type,
            title: None,
            guild_id: self.workspace_id.clone(),
        };

        let text = event.content.as_ref()
            .and_then(|c| c.text.clone())
            .unwrap_or_default();

        // Extract media from files
        let media = event.content.as_ref()
            .and_then(|c| c.files.as_ref())
            .map(|files| {
                files.iter().map(|f| {
                    let mime_str = f.mimetype.as_ref().map(|m| m.to_string());
                    MediaAttachment {
                        id: f.id.to_string(),
                        media_type: self.guess_media_type(&mime_str),
                        url: f.url_private.as_ref().map(|u| u.to_string()),
                        data: None,
                        filename: f.name.clone(),
                        size_bytes: None, // Size not directly available
                        mime_type: mime_str,
                    }
                }).collect()
            })
            .unwrap_or_default();

        // Handle thread replies
        let quote = event.origin.thread_ts.as_ref().map(|ts| QuotedMessage {
            id: ts.to_string(),
            text: None,
            sender_id: None,
        });

        let timestamp = self.parse_slack_timestamp(&event.origin.ts);

        Some(InboundMessage {
            id: MessageId::new(event.origin.ts.to_string()),
            timestamp,
            channel: "slack".to_string(),
            account_id: self.instance_id.clone(),
            sender,
            chat,
            text,
            media,
            quote,
            thread: event.origin.thread_ts.as_ref().map(|ts| smartassist_core::types::ThreadInfo {
                id: ts.to_string(),
                parent_id: None,
            }),
            metadata: serde_json::json!({
                "workspace_id": self.workspace_id,
                "channel_type": format!("{:?}", chat_type),
            }),
        })
    }

    /// Parse Slack timestamp to DateTime<Utc>.
    #[allow(dead_code)]
    fn parse_slack_timestamp(&self, ts: &SlackTs) -> DateTime<Utc> {
        // Slack timestamps are in format "1234567890.123456"
        let ts_str = ts.to_string();
        if let Some((secs, _micros)) = ts_str.split_once('.') {
            if let Ok(secs) = secs.parse::<i64>() {
                return DateTime::<Utc>::from_timestamp(secs, 0).unwrap_or_else(Utc::now);
            }
        }
        Utc::now()
    }

    /// Guess media type from MIME type.
    #[allow(dead_code)]
    fn guess_media_type(&self, mime_type: &Option<String>) -> MediaType {
        match mime_type.as_deref() {
            Some(mt) if mt.starts_with("image/") => MediaType::Image,
            Some(mt) if mt.starts_with("video/") => MediaType::Video,
            Some(mt) if mt.starts_with("audio/") => MediaType::Audio,
            _ => MediaType::Document,
        }
    }

    /// Get a token for API calls.
    fn get_token(&self) -> SlackApiToken {
        SlackApiToken::new(SlackApiTokenValue(self.bot_token.clone()))
    }
}

/// Convert a Slack push message event to an InboundMessage.
/// This is a standalone function used by the Socket Mode listener.
fn convert_push_message(
    event: &SlackMessageEvent,
    instance_id: &str,
    workspace_id: &Option<String>,
) -> Option<InboundMessage> {
    // Get user ID from sender
    let user_id = event.sender.user.as_ref()?.to_string();

    let sender = SenderInfo {
        id: user_id.clone(),
        username: None, // Would need additional API call to get username
        display_name: None,
        phone_number: None,
        is_bot: event.sender.bot_id.is_some(),
    };

    // Get channel ID from origin
    let channel_id_str = event.origin.channel.as_ref()?.to_string();

    // Determine chat type based on channel prefix
    let chat_type = if channel_id_str.starts_with('D') {
        ChatType::Direct
    } else if channel_id_str.starts_with('G') {
        ChatType::Group
    } else {
        ChatType::Channel
    };

    let chat = ChatInfo {
        id: channel_id_str.clone(),
        chat_type,
        title: None,
        guild_id: workspace_id.clone(),
    };

    let text = event
        .content
        .as_ref()
        .and_then(|c| c.text.clone())
        .unwrap_or_default();

    // Extract media from files
    let media = event
        .content
        .as_ref()
        .and_then(|c| c.files.as_ref())
        .map(|files| {
            files
                .iter()
                .map(|f| {
                    let mime_str = f.mimetype.as_ref().map(|m| m.to_string());
                    MediaAttachment {
                        id: f.id.to_string(),
                        media_type: guess_media_type_from_mime(&mime_str),
                        url: f.url_private.as_ref().map(|u| u.to_string()),
                        data: None,
                        filename: f.name.clone(),
                        size_bytes: None,
                        mime_type: mime_str,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // Handle thread replies
    let quote = event.origin.thread_ts.as_ref().map(|ts| QuotedMessage {
        id: ts.to_string(),
        text: None,
        sender_id: None,
    });

    let timestamp = parse_slack_ts(&event.origin.ts);

    Some(InboundMessage {
        id: MessageId::new(event.origin.ts.to_string()),
        timestamp,
        channel: "slack".to_string(),
        account_id: instance_id.to_string(),
        sender,
        chat,
        text,
        media,
        quote,
        thread: event.origin.thread_ts.as_ref().map(|ts| {
            smartassist_core::types::ThreadInfo {
                id: ts.to_string(),
                parent_id: None,
            }
        }),
        metadata: serde_json::json!({
            "workspace_id": workspace_id,
            "channel_type": format!("{:?}", chat_type),
        }),
    })
}

/// Parse Slack timestamp to DateTime<Utc>.
fn parse_slack_ts(ts: &SlackTs) -> DateTime<Utc> {
    // Slack timestamps are in format "1234567890.123456"
    let ts_str = ts.to_string();
    if let Some((secs, _micros)) = ts_str.split_once('.') {
        if let Ok(secs) = secs.parse::<i64>() {
            return DateTime::<Utc>::from_timestamp(secs, 0).unwrap_or_else(Utc::now);
        }
    }
    Utc::now()
}

/// Guess media type from MIME type string.
fn guess_media_type_from_mime(mime_type: &Option<String>) -> MediaType {
    match mime_type.as_deref() {
        Some(mt) if mt.starts_with("image/") => MediaType::Image,
        Some(mt) if mt.starts_with("video/") => MediaType::Video,
        Some(mt) if mt.starts_with("audio/") => MediaType::Audio,
        _ => MediaType::Document,
    }
}

// --- Socket Mode Implementation ---

/// State passed to Socket Mode callbacks via SlackClientEventsUserState.
struct SocketModeState {
    instance_id: String,
    workspace_id: Option<String>,
    message_tx: mpsc::Sender<InboundMessage>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

/// Run the Socket Mode connection with graceful shutdown support.
async fn run_socket_mode(
    app_token: String,
    instance_id: String,
    workspace_id: Option<String>,
    message_tx: mpsc::Sender<InboundMessage>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let client = SlackClient::new(
        SlackClientHyperConnector::new().map_err(|e| ChannelError::channel("slack", e.to_string()))?,
    );
    let client = Arc::new(client);

    // Create listener environment with our state
    let state = SocketModeState {
        instance_id: instance_id.clone(),
        workspace_id: workspace_id.clone(),
        message_tx,
        handler,
    };

    let listener_environment = SlackClientEventsListenerEnvironment::new(client.clone())
        .with_user_state(state);
    let listener_environment = Arc::new(listener_environment);

    // Create callbacks with static functions that read state from user_state
    let callbacks = SlackSocketModeListenerCallbacks::new()
        .with_push_events(handle_push_event);

    // Create socket mode config
    let socket_mode_config = SlackClientSocketModeConfig::new();

    // Create the listener
    let socket_mode_listener = SlackClientSocketModeListener::new(
        &socket_mode_config,
        listener_environment.clone(),
        callbacks,
    );

    // Create app token for socket mode
    let app_api_token = SlackApiToken::new(SlackApiTokenValue(app_token));

    // Register the token and start listening
    socket_mode_listener
        .listen_for(&app_api_token)
        .await
        .map_err(|e| ChannelError::channel("slack", e.to_string()))?;

    // Start the listener and wait for shutdown
    tokio::select! {
        _ = socket_mode_listener.serve() => {
            info!("Socket Mode serve completed");
        }
        _ = &mut shutdown_rx => {
            info!("Socket Mode shutdown signal received for {}", instance_id);
            socket_mode_listener.shutdown().await;
        }
    }

    Ok(())
}

/// Handle incoming push events from Slack.
/// This is a static function that reads state from SlackClientEventsUserState.
async fn handle_push_event(
    event: SlackPushEventCallback,
    _client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    states: SlackClientEventsUserState,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Read state from user state storage
    let state_guard = states.read().await;
    let state = match state_guard.get_user_state::<SocketModeState>() {
        Some(s) => s,
        None => {
            warn!("No SocketModeState found in user state");
            return Ok(());
        }
    };

    // Handle message events
    if let SlackEventCallbackBody::Message(ref msg_event) = event.event {
        if let Some(inbound) = convert_push_message(msg_event, &state.instance_id, &state.workspace_id) {
            debug!("Received Slack message: {:?}", inbound.id);

            // Call handler if set
            {
                let handler_guard = state.handler.read().await;
                if let Some(ref h) = *handler_guard {
                    if let Err(e) = h.handle(inbound.clone()).await {
                        warn!("Message handler error: {}", e);
                    }
                }
            }

            // Send through channel
            if let Err(e) = state.message_tx.send(inbound).await {
                warn!("Failed to send message to channel: {}", e);
            }
        }
    }

    Ok(())
}

#[async_trait]
impl Channel for SlackChannel {
    fn channel_type(&self) -> &str {
        "slack"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![
                ChatType::Direct,
                ChatType::Group,
                ChatType::Channel,
                ChatType::Thread,
            ],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: false,
                voice_notes: false,
                max_file_size_mb: 1000, // 1GB for paid workspaces
            },
            features: ChannelFeatures {
                reactions: true,
                threads: true,
                edits: true,
                deletes: true,
                typing_indicators: false,
                read_receipts: false,
                mentions: true,
                polls: false,
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 40000,
                caption_max_length: 40000,
                messages_per_second: 1.0, // Tier 2 rate limit
                messages_per_minute: 50,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for SlackChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = SlackChannelId::new(message.target.chat_id.clone());
        let token = self.get_token();
        let session = client.open_session(&token);

        let content = SlackMessageContent::new().with_text(message.text.clone());

        // Add thread_ts if replying to a thread
        let _thread_ts = message.reply_to.as_ref().map(|ts| SlackTs::new(ts.clone()));

        let request = SlackApiChatPostMessageRequest::new(channel_id, content);

        let response = session
            .chat_post_message(&request)
            .await
            .map_err(|e| ChannelError::channel("slack", e.to_string()))?;

        Ok(SendResult::new(response.ts.to_string()))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = SlackChannelId::new(message.target.chat_id.clone());
        let token = self.get_token();
        let session = client.open_session(&token);

        // Track uploaded file IDs for the complete upload request
        let mut uploaded_files: Vec<SlackApiFilesComplete> = Vec::new();

        // Upload files using the new Slack file upload flow
        for attachment in &attachments {
            let (content, filename, content_type) = match &attachment.source {
                crate::attachment::AttachmentSource::Bytes(bytes) => {
                    let filename = attachment.filename.clone();
                    let content_type = attachment.mime_type.clone();
                    (bytes.to_vec(), filename, content_type)
                }
                crate::attachment::AttachmentSource::Path(path) => {
                    let content = tokio::fs::read(path)
                        .await
                        .map_err(|e| ChannelError::channel("slack", format!("Failed to read file: {}", e)))?;
                    let filename = attachment.filename.clone();
                    let content_type = attachment.mime_type.clone();
                    (content, filename, content_type)
                }
                crate::attachment::AttachmentSource::Url(url) => {
                    // Download URL content first
                    let response = reqwest::get(url.as_str())
                        .await
                        .map_err(|e| ChannelError::channel("slack", format!("Failed to fetch URL: {}", e)))?;
                    let content = response.bytes()
                        .await
                        .map_err(|e| ChannelError::channel("slack", format!("Failed to read URL content: {}", e)))?
                        .to_vec();
                    let filename = attachment.filename.clone();
                    let content_type = attachment.mime_type.clone();
                    (content, filename, content_type)
                }
                crate::attachment::AttachmentSource::FileId(file_id) => {
                    // Cannot upload a file ID - skip with warning
                    warn!("Cannot re-upload file from file ID '{}' - skipping", file_id);
                    continue;
                }
            };

            // Step 1: Get upload URL
            let upload_url_req = SlackApiFilesGetUploadUrlExternalRequest::new(
                filename.clone(),
                content.len(),
            );
            let upload_url_resp = session
                .get_upload_url_external(&upload_url_req)
                .await
                .map_err(|e| ChannelError::channel("slack", format!("Failed to get upload URL: {}", e)))?;

            debug!("Got upload URL for file '{}': file_id={}", filename, upload_url_resp.file_id);

            // Step 2: Upload file content to the URL
            let upload_req = SlackApiFilesUploadViaUrlRequest::new(
                upload_url_resp.upload_url,
                content,
                content_type,
            );
            session
                .files_upload_via_url(&upload_req)
                .await
                .map_err(|e| ChannelError::channel("slack", format!("Failed to upload file content: {}", e)))?;

            debug!("Uploaded file content for '{}'", filename);

            // Track file for completion
            uploaded_files.push(
                SlackApiFilesComplete::new(upload_url_resp.file_id)
                    .with_title(filename)
            );
        }

        // Step 3: Complete upload and share to channel (if we have files)
        if !uploaded_files.is_empty() {
            let complete_req = SlackApiFilesCompleteUploadExternalRequest::new(uploaded_files)
                .with_channel_id(channel_id.clone())
                .opt_initial_comment(if message.text.is_empty() { None } else { Some(message.text.clone()) })
                .opt_thread_ts(message.reply_to.as_ref().map(|ts| SlackTs::new(ts.clone())));

            let complete_resp = session
                .files_complete_upload_external(&complete_req)
                .await
                .map_err(|e| ChannelError::channel("slack", format!("Failed to complete upload: {}", e)))?;

            debug!("Completed file upload, {} files shared", complete_resp.files.len());

            // Return the file ID of the first file as the message ID
            if let Some(first_file) = complete_resp.files.first() {
                return Ok(SendResult::new(first_file.id.to_string()));
            }
        }

        // If no files but we have text, send as regular message
        if !message.text.is_empty() {
            return self.send(message).await;
        }

        Ok(SendResult::new(String::new()))
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let token = self.get_token();
        let session = client.open_session(&token);

        let channel_id = SlackChannelId::new(message.chat_id.clone());
        let ts = SlackTs::new(message.message_id.clone());

        let request = SlackApiChatUpdateRequest::new(
            channel_id,
            SlackMessageContent::new().with_text(new_content.to_string()),
            ts,
        );

        session
            .chat_update(&request)
            .await
            .map_err(|e| ChannelError::channel("slack", e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, message: &MessageRef) -> Result<()> {
        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let token = self.get_token();
        let session = client.open_session(&token);

        let channel_id = SlackChannelId::new(message.chat_id.clone());
        let ts = SlackTs::new(message.message_id.clone());

        let request = SlackApiChatDeleteRequest::new(channel_id, ts);

        session
            .chat_delete(&request)
            .await
            .map_err(|e| ChannelError::channel("slack", e.to_string()))?;

        Ok(())
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let token = self.get_token();
        let session = client.open_session(&token);

        let channel_id = SlackChannelId::new(message.chat_id.clone());
        let ts = SlackTs::new(message.message_id.clone());

        // Remove surrounding colons from emoji name if present (e.g., ":thumbsup:" -> "thumbsup")
        let emoji_name = emoji.trim_matches(':');

        let request = SlackApiReactionsAddRequest::new(
            channel_id,
            SlackReactionName::new(emoji_name.to_string()),
            ts,
        );

        session
            .reactions_add(&request)
            .await
            .map_err(|e| ChannelError::channel("slack", e.to_string()))?;

        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let token = self.get_token();
        let session = client.open_session(&token);

        let channel_id = SlackChannelId::new(message.chat_id.clone());
        let ts = SlackTs::new(message.message_id.clone());

        let emoji_name = emoji.trim_matches(':');

        // SlackApiReactionsRemoveRequest uses builder pattern with optional channel/timestamp
        let request = SlackApiReactionsRemoveRequest::new(SlackReactionName::new(emoji_name.to_string()))
            .with_channel(channel_id)
            .with_timestamp(ts);

        session
            .reactions_remove(&request)
            .await
            .map_err(|e| ChannelError::channel("slack", e.to_string()))?;

        Ok(())
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        // Slack doesn't have a typing indicator API for bots
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        40000
    }
}

#[async_trait]
impl ChannelReceiver for SlackChannel {
    async fn start_receiving(&self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        // Check if we have an app token for Socket Mode
        let app_token = match &self.app_token {
            Some(token) => token.clone(),
            None => {
                warn!(
                    "No app token configured for Slack channel {} - Socket Mode disabled. \
                     Only sending will work. To receive messages, configure an app token.",
                    self.instance_id
                );
                return Ok(());
            }
        };

        // Spawn Socket Mode listener task
        let message_tx = self.message_tx.clone();
        let instance_id = self.instance_id.clone();
        let workspace_id = self.workspace_id.clone();
        let handler = self.handler.clone();

        tokio::spawn(async move {
            info!("Starting Slack Socket Mode for channel: {}", instance_id);

            // Run socket mode with graceful shutdown
            let result = run_socket_mode(
                app_token,
                instance_id.clone(),
                workspace_id,
                message_tx,
                handler,
                shutdown_rx,
            )
            .await;

            match result {
                Ok(()) => info!("Socket Mode stopped for channel: {}", instance_id),
                Err(e) => error!("Socket Mode error for channel {}: {}", instance_id, e),
            }
        });

        info!(
            "Started Slack channel (Socket Mode): {}",
            self.instance_id
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        let mut shutdown = self.shutdown.write().await;
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(());
        }

        let mut connected = self.connected.write().await;
        *connected = false;

        Ok(())
    }

    async fn receive(&self) -> Result<InboundMessage> {
        let mut rx = self.message_rx.write().await;
        rx.recv()
            .await
            .ok_or_else(|| ChannelError::Internal("Channel closed".to_string()))
    }

    async fn try_receive(&self) -> Result<Option<InboundMessage>> {
        let mut rx = self.message_rx.write().await;
        match rx.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(ChannelError::Internal("Channel closed".to_string()))
            }
        }
    }

    fn set_handler(&self, handler: Box<dyn MessageHandler>) {
        let handler_arc = self.handler.clone();
        tokio::spawn(async move {
            let mut h = handler_arc.write().await;
            *h = Some(handler);
        });
    }
}

#[async_trait]
impl ChannelLifecycle for SlackChannel {
    async fn connect(&self) -> Result<()> {
        let client = SlackClient::new(SlackClientHyperConnector::new()?);
        let token = self.get_token();
        let session = client.open_session(&token);

        // Test connection by getting auth info
        let auth_test = session
            .auth_test()
            .await
            .map_err(|e| ChannelError::Auth(e.to_string()))?;

        info!("Connected to Slack workspace: {:?} as {:?}", auth_test.team, auth_test.user);

        {
            let mut client_guard = self.client.write().await;
            *client_guard = Some(Arc::new(client));
        }

        let mut connected = self.connected.write().await;
        *connected = true;

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut client_guard = self.client.write().await;
        *client_guard = None;

        let mut connected = self.connected.write().await;
        *connected = false;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let start = std::time::Instant::now();

        let client_guard = self.client.read().await;
        match client_guard.as_ref() {
            Some(client) => {
                let token = self.get_token();
                let session = client.open_session(&token);

                match session.auth_test().await {
                    Ok(_) => Ok(ChannelHealth {
                        status: HealthStatus::Healthy,
                        latency_ms: Some(start.elapsed().as_millis() as u64),
                        last_message_at: None,
                        error: None,
                    }),
                    Err(e) => Ok(ChannelHealth {
                        status: HealthStatus::Unhealthy,
                        latency_ms: None,
                        last_message_at: None,
                        error: Some(e.to_string()),
                    }),
                }
            }
            None => Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: None,
                last_message_at: None,
                error: Some("Not connected".to_string()),
            }),
        }
    }
}

impl Clone for SlackChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            bot_token: self.bot_token.clone(),
            app_token: self.app_token.clone(),
            instance_id: self.instance_id.clone(),
            workspace_id: self.workspace_id.clone(),
            client: self.client.clone(),
            connected: self.connected.clone(),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: self.handler.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_channel_creation() {
        let channel = SlackChannel::new("xoxb-test", None, "test_workspace");
        assert_eq!(channel.channel_type(), "slack");
        assert_eq!(channel.instance_id(), "test_workspace");
    }

    #[test]
    fn test_capabilities() {
        let channel = SlackChannel::new("xoxb-test", None, "test_workspace");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.features.threads);
        assert!(caps.features.reactions);
        assert_eq!(caps.limits.text_max_length, 40000);
        assert!(caps.chat_types.contains(&ChatType::Direct));
        assert!(caps.chat_types.contains(&ChatType::Channel));
    }
}
