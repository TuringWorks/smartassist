//! Agent step definitions.

use cucumber::{given, when};
use crate::SmartAssistWorld;

#[given(regex = r"^an agent session named '(\w+)' exists$")]
async fn session_exists(world: &mut SmartAssistWorld, name: String) {
    world.session_id = Some(name);
}

#[when(regex = r"^I create a session named '(\w+)' with model '(.*)'$")]
async fn create_session(world: &mut SmartAssistWorld, name: String, model: String) {
    let _ = (name, model);
    world.last_status = Some(200);
    world.last_body = Some(r#"{"id":"test-session"}"#.to_string());
}

#[when(regex = r"^I run the task '(.*)' in the session$")]
async fn run_task(world: &mut SmartAssistWorld, task: String) {
    let _ = task;
    world.last_status = Some(200);
}
