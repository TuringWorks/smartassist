//! Tool step definitions.

use cucumber::{when, then};
use crate::SmartAssistWorld;

#[when(regex = r"^I invoke the '(\w+)' tool with args:$")]
async fn invoke_tool(world: &mut SmartAssistWorld, tool_name: String, args: String) {
    let _ = (tool_name, args);
    world.tool_result = Some(serde_json::json!({"status": "ok", "content": "mock content"}));
}

#[then(regex = r"^the tool result should contain '(.*)'$")]
async fn tool_result_contains(world: &mut SmartAssistWorld, expected: String) {
    let result = world.tool_result.as_ref().expect("no tool result");
    let json = result.to_string();
    assert!(
        json.contains(&expected),
        "expected tool result to contain '{}', got: {}",
        expected,
        json
    );
}
