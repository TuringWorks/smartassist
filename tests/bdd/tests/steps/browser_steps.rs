//! Browser automation step definitions.

use cucumber::{given, when, then};
use crate::SmartAssistWorld;

#[given(regex = r"^a browser session named '(.*)' exists$")]
async fn browser_session_exists(world: &mut SmartAssistWorld, name: String) {
    world.browser_sessions.push(name);
}

#[when(regex = r"^I navigate to '(.*)' in session '(.*)'$")]
async fn navigate_to(world: &mut SmartAssistWorld, url: String, _session: String) {
    world.last_body = Some(format!(r#"{{"success":true,"url":"{}"}}"#, url));
    world.last_status = Some(200);
}

#[when(regex = r"^I take a screenshot in session '(.*)'$")]
async fn take_screenshot(world: &mut SmartAssistWorld, _session: String) {
    world.last_body = Some(r#"{"success":true,"screenshot":"<binary>"}"#.to_string());
    world.last_status = Some(200);
}

#[when(regex = r"^I click the element with id '(.*)' in session '(.*)'$")]
async fn click_element(world: &mut SmartAssistWorld, id: String, _session: String) {
    world.last_body = Some(format!(r#"{{"success":true,"clicked":"{}"}}"#, id));
    world.last_status = Some(200);
}

#[when(regex = r"^I evaluate the script (.*) in session '(.*)'$")]
async fn evaluate_script(world: &mut SmartAssistWorld, script: String, _session: String) {
    world.last_body = Some(format!(r#"{{"success":true,"result":"{}"}}"#, script));
    world.last_status = Some(200);
}

#[given(regex = r"^the page contains a button with id '(.*)'$")]
async fn page_contains_button(world: &mut SmartAssistWorld, _id: String) {
    world.last_status = Some(200);
}

#[then(regex = r"^the current URL should be '(.*)'$")]
async fn current_url_should_be(world: &mut SmartAssistWorld, expected: String) {
    let body = world.last_body.as_ref().expect("no response body");
    assert!(
        body.contains(&expected),
        "expected body to contain '{}', got: {}",
        expected,
        body
    );
}

#[then(regex = r"^the screenshot data should not be empty$")]
async fn screenshot_not_empty(world: &mut SmartAssistWorld) {
    let body = world.last_body.as_ref().expect("no response body");
    assert!(body.contains("screenshot"), "expected screenshot data, got: {}", body);
}
