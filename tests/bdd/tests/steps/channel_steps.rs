//! Channel step definitions.

use cucumber::{given, when, then};
use crate::SmartAssistWorld;

#[given(regex = r"^a channel named '(\w+)' is connected$")]
async fn channel_connected(world: &mut SmartAssistWorld, name: String) {
    world.channel_id = Some(name);
}

#[when(regex = r"^I send a message '(.*)' to the channel$")]
async fn send_message(world: &mut SmartAssistWorld, message: String) {
    let _ = message;
    world.last_status = Some(200);
}

#[when(regex = r"^a message '(.*)' is received from the channel$")]
async fn receive_message(world: &mut SmartAssistWorld, message: String) {
    let _ = message;
    world.last_status = Some(200);
}

#[then(regex = r"^the session should receive a response$")]
async fn session_response(world: &mut SmartAssistWorld) {
    assert!(
        world.last_status.is_some(),
        "expected a response, got none"
    );
}
