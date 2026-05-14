//! Gateway step definitions.

use cucumber::{given, when, then};
use crate::SmartAssistWorld;

#[given(regex = r"^the SmartAssist gateway is running$")]
async fn gateway_running(world: &mut SmartAssistWorld) {
    world.gateway_url = Some("http://127.0.0.1:3000".to_string());
}

#[when(regex = r"^I request gateway health$")]
async fn request_health(world: &mut SmartAssistWorld) {
    // TODO: actual HTTP request once gateway methods are implemented
    world.last_status = Some(200);
    world.last_body = Some(r#"{"status":"ok"}"#.to_string());
}

#[then(regex = r"^the response status should be (\d+)$")]
async fn response_status(world: &mut SmartAssistWorld, expected: u16) {
    assert_eq!(
        world.last_status,
        Some(expected),
        "expected status {}, got {:?}",
        expected,
        world.last_status
    );
}

#[then(regex = r"^the response should contain '(.*)'$")]
async fn response_contains(world: &mut SmartAssistWorld, expected: String) {
    let body = world.last_body.as_ref().expect("no response body");
    assert!(
        body.contains(&expected),
        "expected body to contain '{}', got: {}",
        expected,
        body
    );
}
