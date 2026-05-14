//! Auto-reply step definitions.

use cucumber::{given, then, when};
use crate::SmartAssistWorld;
use smartassist_channels::auto_reply::{AutoReplyEngine, AutoReplyRule};
use smartassist_core::types::{ChatInfo, InboundMessage, MessageId};

fn make_inbound_message(text: &str, chat_id: &str) -> InboundMessage {
    InboundMessage {
        id: MessageId::new("msg-1"),
        text: text.to_string(),
        chat: ChatInfo {
            id: chat_id.to_string(),
            ..ChatInfo::default()
        },
        ..InboundMessage::default()
    }
}

#[given(regex = r#"^an auto-reply engine with rule "(\w+)" exact "(.*)" replies "(.*)"$"#)]
async fn auto_reply_exact(world: &mut SmartAssistWorld, id: String, pattern: String, response: String) {
    let mut engine = world.auto_reply_engine.take().unwrap_or_else(AutoReplyEngine::new);
    engine.add_rule(AutoReplyRule::exact(id, pattern, response));
    world.auto_reply_engine = Some(engine);
}

#[given(regex = r#"^an auto-reply engine with rule "(\w+)" contains "(.*)" replies "(.*)"$"#)]
async fn auto_reply_contains(world: &mut SmartAssistWorld, id: String, pattern: String, response: String) {
    let mut engine = world.auto_reply_engine.take().unwrap_or_else(AutoReplyEngine::new);
    engine.add_rule(AutoReplyRule::contains(id, pattern, response));
    world.auto_reply_engine = Some(engine);
}

#[given(regex = r#"^an auto-reply engine with rule "(\w+)" regex "(.*)" replies "(.*)"$"#)]
async fn auto_reply_regex(world: &mut SmartAssistWorld, id: String, pattern: String, response: String) {
    let mut engine = world.auto_reply_engine.take().unwrap_or_else(AutoReplyEngine::new);
    engine.add_rule(AutoReplyRule::regex(id, pattern, response));
    world.auto_reply_engine = Some(engine);
}

#[given(regex = r#"^an auto-reply engine with disabled rule "(\w+)" exact "(.*)"$"#)]
async fn auto_reply_disabled(world: &mut SmartAssistWorld, id: String, pattern: String) {
    let mut engine = world.auto_reply_engine.take().unwrap_or_else(AutoReplyEngine::new);
    let mut rule = AutoReplyRule::exact(id, pattern, "");
    rule.enabled = false;
    engine.add_rule(rule);
    world.auto_reply_engine = Some(engine);
}

#[given(regex = r#"^an auto-reply engine with rule "(\w+)" exact "(.*)" replies "(.*)" and cooldown (\d+) seconds$"#)]
async fn auto_reply_with_cooldown(
    world: &mut SmartAssistWorld,
    id: String,
    pattern: String,
    response: String,
    cooldown: u64,
) {
    let mut engine = world.auto_reply_engine.take().unwrap_or_else(AutoReplyEngine::new);
    engine.add_rule(AutoReplyRule::exact(id, pattern, response).with_cooldown(cooldown));
    world.auto_reply_engine = Some(engine);
}

#[when(regex = r#"^a message "(.*)" is received in chat "(\w+)"(?: again immediately)?$"#)]
async fn message_received(world: &mut SmartAssistWorld, text: String, chat_id: String) {
    let engine = world.auto_reply_engine.as_ref().expect("no auto-reply engine");
    let msg = make_inbound_message(&text, &chat_id);
    if let Some(outbound) = engine.evaluate(&msg) {
        world.last_outbound_texts.push(outbound.text);
        world.last_outbound_targets.push(outbound.target.chat_id);
    }
}

#[then(regex = r#"^an outbound message "(.*)" should be generated$"#)]
async fn outbound_message_generated(world: &mut SmartAssistWorld, expected: String) {
    assert!(
        world.last_outbound_texts.contains(&expected),
        "expected outbound texts to contain {:?}, got {:?}",
        expected,
        world.last_outbound_texts
    );
}

#[then(regex = r#"^no outbound message should be generated$"#)]
async fn no_outbound_message(world: &mut SmartAssistWorld) {
    assert!(
        world.last_outbound_texts.is_empty(),
        "expected no outbound messages, got {:?}",
        world.last_outbound_texts
    );
}

#[then(regex = r#"^only 1 outbound message should be generated$"#)]
async fn only_one_outbound_message(world: &mut SmartAssistWorld) {
    assert_eq!(
        world.last_outbound_texts.len(),
        1,
        "expected exactly 1 outbound message, got {:?}",
        world.last_outbound_texts
    );
}
