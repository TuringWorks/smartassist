//! Tool step definitions.

use cucumber::{when, then};
use crate::SmartAssistWorld;

#[when(regex = r"^I invoke the '(\w+)' tool with args '(.*)'$")]
async fn invoke_tool(world: &mut SmartAssistWorld, tool_name: String, args: String) {
    let content = match tool_name.as_str() {
        "bash" => {
            // Extract command from args JSON for realistic mock output
            let parsed: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
            parsed.get("command").and_then(|v| v.as_str()).unwrap_or("mock").to_string()
        }
        _ => "mock content".to_string(),
    };
    world.tool_result = Some(serde_json::json!({"status": "ok", "content": content}));
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
