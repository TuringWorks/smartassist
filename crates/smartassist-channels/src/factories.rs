//! Channel factory registration helpers.
//!
//! Provides a single function to register all available channel factories
//! with a `ChannelManager`, gated by Cargo features.

#![allow(unused_imports)]

use crate::manager::ChannelManager;
use crate::traits::ChannelFactory;
use std::sync::Arc;

/// Register all available channel factories with a `ChannelManager`.
///
/// Each factory is only registered if its corresponding Cargo feature is enabled.
/// This allows the gateway (or any binary) to dynamically create channels from
/// configuration without hard-coding every channel type.
pub async fn register_default_factories(manager: &ChannelManager) {
    #[cfg(feature = "telegram")]
    manager
        .register_factory(Arc::new(crate::telegram::TelegramChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "discord")]
    manager
        .register_factory(Arc::new(crate::discord::DiscordChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "slack")]
    manager
        .register_factory(Arc::new(crate::slack::SlackChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "web")]
    manager
        .register_factory(Arc::new(crate::web::WebChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "signal")]
    manager
        .register_factory(Arc::new(crate::signal::SignalChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "imessage")]
    manager
        .register_factory(Arc::new(crate::imessage::IMessageChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "whatsapp")]
    manager
        .register_factory(Arc::new(crate::whatsapp::WhatsAppChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "line")]
    manager
        .register_factory(Arc::new(crate::line::LineChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "irc")]
    manager
        .register_factory(Arc::new(crate::irc::IrcChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "matrix")]
    manager
        .register_factory(Arc::new(crate::matrix::MatrixChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "googlechat")]
    manager
        .register_factory(Arc::new(crate::googlechat::GoogleChatChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "msteams")]
    manager
        .register_factory(Arc::new(crate::msteams::MsTeamsChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "feishu")]
    manager
        .register_factory(Arc::new(crate::feishu::FeishuChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "mattermost")]
    manager
        .register_factory(Arc::new(crate::mattermost::MattermostChannelFactory) as Arc<dyn ChannelFactory>)
        .await;

    #[cfg(feature = "zalo")]
    manager
        .register_factory(Arc::new(crate::zalo::ZaloChannelFactory) as Arc<dyn ChannelFactory>)
        .await;
}
