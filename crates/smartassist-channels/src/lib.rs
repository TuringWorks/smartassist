//! Messaging channel abstractions for SmartAssist.
//!
//! This crate provides the core traits and types for messaging channels,
//! along with routing and delivery mechanisms.

pub mod error;
pub mod traits;
pub mod routing;
pub mod delivery;
pub mod attachment;
pub mod registry;
pub mod manager;
pub mod auto_reply;
pub mod heartbeat;
pub mod factories;

#[cfg(feature = "telegram")]
pub mod telegram;

#[cfg(feature = "discord")]
pub mod discord;

#[cfg(feature = "slack")]
pub mod slack;

#[cfg(feature = "web")]
pub mod web;

#[cfg(feature = "signal")]
pub mod signal;

#[cfg(feature = "imessage")]
pub mod imessage;

#[cfg(feature = "whatsapp")]
pub mod whatsapp;

#[cfg(feature = "line")]
pub mod line;

#[cfg(feature = "irc")]
pub mod irc;

#[cfg(feature = "matrix")]
pub mod matrix;

#[cfg(feature = "googlechat")]
pub mod googlechat;

#[cfg(feature = "msteams")]
pub mod msteams;

#[cfg(feature = "feishu")]
pub mod feishu;

#[cfg(feature = "mattermost")]
pub mod mattermost;

#[cfg(feature = "zalo")]
pub mod zalo;

pub use error::ChannelError;
pub use traits::{Channel, ChannelConfig, ChannelReceiver, ChannelSender, ChannelLifecycle, MessageHandler, MessageRef, SendResult, ChannelFactory};
pub use routing::{Router, RouteMatch, RouteRule};
pub use delivery::{DeliveryQueue, DeliveryStatus, DeliveryResult};
pub use attachment::{Attachment, AttachmentType};
pub use registry::{ChannelRegistry, RegisteredChannel};
pub use manager::{ChannelManager, ChannelManagerBuilder, ManagerStatus, ManagerMessageHandler};
pub use auto_reply::{AutoReplyEngine, AutoReplyRule, MatchMode};
pub use heartbeat::{HeartbeatFilter, HeartbeatPattern, default_patterns};
pub use factories::register_default_factories;

#[cfg(feature = "telegram")]
pub use telegram::TelegramChannelFactory;

#[cfg(feature = "discord")]
pub use discord::DiscordChannelFactory;

#[cfg(feature = "slack")]
pub use slack::SlackChannelFactory;

#[cfg(feature = "web")]
pub use web::WebChannelFactory;

#[cfg(feature = "signal")]
pub use signal::SignalChannelFactory;

#[cfg(feature = "imessage")]
pub use imessage::IMessageChannelFactory;

#[cfg(feature = "whatsapp")]
pub use whatsapp::WhatsAppChannelFactory;

#[cfg(feature = "line")]
pub use line::LineChannelFactory;

#[cfg(feature = "irc")]
pub use irc::IrcChannelFactory;

#[cfg(feature = "matrix")]
pub use matrix::MatrixChannelFactory;

#[cfg(feature = "googlechat")]
pub use googlechat::GoogleChatChannelFactory;

#[cfg(feature = "msteams")]
pub use msteams::MsTeamsChannelFactory;

#[cfg(feature = "feishu")]
pub use feishu::FeishuChannelFactory;

#[cfg(feature = "mattermost")]
pub use mattermost::MattermostChannelFactory;

#[cfg(feature = "zalo")]
pub use zalo::ZaloChannelFactory;

/// Result type for channel operations.
pub type Result<T> = std::result::Result<T, ChannelError>;
