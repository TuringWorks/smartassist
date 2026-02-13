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

pub use error::ChannelError;
pub use traits::{Channel, ChannelConfig, ChannelReceiver, ChannelSender, ChannelLifecycle, MessageHandler, MessageRef, SendResult};
pub use routing::{Router, RouteMatch, RouteRule};
pub use delivery::{DeliveryQueue, DeliveryStatus, DeliveryResult};
pub use attachment::{Attachment, AttachmentType};
pub use registry::{ChannelRegistry, RegisteredChannel};
pub use manager::{ChannelManager, ChannelManagerBuilder, ManagerStatus, ManagerMessageHandler};

/// Result type for channel operations.
pub type Result<T> = std::result::Result<T, ChannelError>;
