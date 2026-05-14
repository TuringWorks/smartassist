//! Heartbeat filter step definitions.

use cucumber::{given, then, when};
use crate::SmartAssistWorld;
use smartassist_channels::heartbeat::HeartbeatFilter;
use smartassist_core::types::{ChatInfo, InboundMessage, MessageId};

fn make_inbound_message(text: &str, channel: &str) -> InboundMessage {
    InboundMessage {
        id: MessageId::new("msg-1"),
        text: text.to_string(),
        channel: channel.to_string(),
        chat: ChatInfo {
            id: "chat-1".to_string(),
            ..ChatInfo::default()
        },
        ..InboundMessage::default()
    }
}

#[given(regex = r#"^a heartbeat filter with default patterns$"#)]
async fn heartbeat_filter_default(world: &mut SmartAssistWorld) {
    world.heartbeat_filter = Some(HeartbeatFilter::new());
}

#[when(regex = r#"^a message "(.*)" arrives from channel "(\w+)"$"#)]
async fn message_arrives(world: &mut SmartAssistWorld, text: String, channel: String) {
    let filter = world.heartbeat_filter.as_ref().expect("no heartbeat filter");
    let msg = make_inbound_message(&text, &channel);
    world.last_heartbeat_result = Some(filter.is_heartbeat(&msg));
}

#[then(regex = r#"^the message should be classified as heartbeat$"#)]
async fn classified_as_heartbeat(world: &mut SmartAssistWorld) {
    assert_eq!(
        world.last_heartbeat_result,
        Some(true),
        "expected heartbeat, got {:?}",
        world.last_heartbeat_result
    );
}

#[then(regex = r#"^the message should not be classified as heartbeat$"#)]
async fn not_classified_as_heartbeat(world: &mut SmartAssistWorld) {
    assert_eq!(
        world.last_heartbeat_result,
        Some(false),
        "expected not heartbeat, got {:?}",
        world.last_heartbeat_result
    );
}
