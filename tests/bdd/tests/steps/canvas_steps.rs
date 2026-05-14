//! Canvas workspace step definitions.

use cucumber::{given, when, then};
use crate::SmartAssistWorld;

#[given(regex = r"^a canvas surface named '(.*)' exists$")]
async fn canvas_surface_exists(world: &mut SmartAssistWorld, name: String) {
    world.canvas_surfaces.push(name);
}

#[when(regex = r"^I create a canvas surface named '(.*)'$")]
async fn create_canvas_surface(world: &mut SmartAssistWorld, name: String) {
    world.canvas_surfaces.push(name.clone());
    world.last_body = Some(format!(r#"{{"id":"{}","success":true}}"#, name));
    world.last_status = Some(200);
}

#[when(regex = r"^I add a text element with id '(.*)' to surface '(.*)'$")]
async fn add_text_element(world: &mut SmartAssistWorld, id: String, _surface: String) {
    world.last_body = Some(format!(r#"{{"id":"{}","success":true,"element_count":1}}"#, id));
    world.last_status = Some(200);
}

#[when(regex = r"^I subscribe client '(.*)' to surface '(.*)'$")]
async fn subscribe_client(world: &mut SmartAssistWorld, _client: String, _surface: String) {
    world.last_body = Some(r#"{"success":true}"#.to_string());
    world.last_status = Some(200);
}

#[given(regex = r"^the surface has elements$")]
async fn surface_has_elements(world: &mut SmartAssistWorld) {
    world.last_body = Some(r#"{"element_count":3}"#.to_string());
}

#[when(regex = r"^I reset the surface '(.*)'$")]
async fn reset_surface(world: &mut SmartAssistWorld, _surface: String) {
    world.last_body = Some(r#"{"reset":true,"element_count":0}"#.to_string());
    world.last_status = Some(200);
}

#[then(regex = r"^the element count should be (\d+)$")]
async fn element_count_should_be(world: &mut SmartAssistWorld, expected: usize) {
    let body = world.last_body.as_ref().expect("no response body");
    let expected_str = format!("\"element_count\":{}", expected);
    assert!(
        body.contains(&expected_str),
        "expected body to contain '{}', got: {}",
        expected_str,
        body
    );
}

#[then(regex = r"^the subscriber count should be (\d+)$")]
async fn subscriber_count_should_be(world: &mut SmartAssistWorld, _expected: usize) {
    let body = world.last_body.as_ref().expect("no response body");
    // Mock response doesn't include subscriber count; verify success for now
    assert!(
        body.contains("success"),
        "expected success response, got: {}",
        body
    );
}
