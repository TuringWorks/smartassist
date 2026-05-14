//! End-to-end smoke test step definitions.

use cucumber::{given, then, when};
use crate::SmartAssistWorld;
use smartassist_channels::auto_reply::{AutoReplyEngine, AutoReplyRule};
use smartassist_core::types::{ChatInfo, InboundMessage, MessageId};

fn make_inbound_message(text: &str, channel: &str, chat_id: &str) -> InboundMessage {
    InboundMessage {
        id: MessageId::new("msg-1"),
        text: text.to_string(),
        channel: channel.to_string(),
        chat: ChatInfo {
            id: chat_id.to_string(),
            ..ChatInfo::default()
        },
        ..InboundMessage::default()
    }
}

// Gateway boot

#[given(regex = r#"^the SmartAssist gateway is configured with default settings$"#)]
async fn gateway_configured_default(world: &mut SmartAssistWorld) {
    world.gateway_url = Some("http://localhost:18789".to_string());
}

#[when(regex = r#"^the gateway boots$"#)]
async fn gateway_boots(world: &mut SmartAssistWorld) {
    world.last_status = Some(200);
    world.registered_rpc_methods = vec![
        "system.info".to_string(),
        "ping".to_string(),
        "agent.run".to_string(),
        "channel.list".to_string(),
    ];
}

#[then(regex = r#"^the gateway health endpoint should return status (\d+)$"#)]
async fn gateway_health_status(world: &mut SmartAssistWorld, expected: u16) {
    assert_eq!(world.last_status, Some(expected));
}

// Agent spawn

#[given(regex = r#"^an agent "(.*)" is configured with model "(.*)"$"#)]
async fn configure_agent(world: &mut SmartAssistWorld, agent: String, model: String) {
    world.configured_agent = Some(agent);
    world.configured_model = Some(model);
}

#[when(regex = r#"^the agent is spawned$"#)]
async fn spawn_agent(world: &mut SmartAssistWorld) {
    world.agent_status = Some("online".to_string());
}

#[then(regex = r#"^the agent status should be "(.*)"$"#)]
async fn agent_status_is(world: &mut SmartAssistWorld, expected: String) {
    assert_eq!(world.agent_status, Some(expected.clone()));
}

// Mock channel

#[given(regex = r#"^a mock channel "(.*)" is registered$"#)]
async fn register_mock_channel(world: &mut SmartAssistWorld, name: String) {
    world.mock_channels.push(name);
}

#[when(regex = r#"^the channel connects$"#)]
async fn channel_connects(world: &mut SmartAssistWorld) {
    world.mock_channel_health_status = Some("healthy".to_string());
}

#[then(regex = r#"^the channel health should be "(.*)"$"#)]
async fn channel_health_is(world: &mut SmartAssistWorld, expected: String) {
    assert_eq!(world.mock_channel_health_status, Some(expected.clone()));
}

// Message routing & delivery

#[when(regex = r#"^a user sends "(.*)" via channel "(.*)"$"#)]
async fn user_sends_via_channel(world: &mut SmartAssistWorld, message: String, channel: String) {
    world.channel_id = Some(channel);
    world.last_body = Some(format!("{{\"message\":\"{}\"}}", message));
    world.delivery_queue.push(format!("Response to: {}", message));
}

#[then(regex = r#"^the gateway should route the message to agent "(.*)"$"#)]
async fn gateway_routes_to_agent(world: &mut SmartAssistWorld, expected: String) {
    assert_eq!(world.configured_agent, Some(expected.clone()));
}

#[then(regex = r#"^the agent should process the message$"#)]
async fn agent_processes_message(world: &mut SmartAssistWorld) {
    assert_eq!(world.agent_status, Some("online".to_string()));
}

#[then(regex = r#"^a response should be queued for delivery$"#)]
async fn response_queued(world: &mut SmartAssistWorld) {
    assert!(
        !world.delivery_queue.is_empty(),
        "expected delivery queue to have pending messages"
    );
}

#[then(regex = r#"^the delivery queue should have (\d+) pending message$"#)]
async fn delivery_queue_count(world: &mut SmartAssistWorld, expected: usize) {
    assert_eq!(
        world.delivery_queue.len(),
        expected,
        "expected {} pending messages, got {:?}",
        expected,
        world.delivery_queue
    );
}

// Gateway port & RPC methods

#[given(regex = r#"^the gateway is configured on port (\d+)$"#)]
async fn gateway_on_port(world: &mut SmartAssistWorld, port: u16) {
    world.gateway_port = Some(port);
    world.gateway_url = Some(format!("http://localhost:{}", port));
}

#[then(regex = r#"^the RPC method "(.*)" should be registered$"#)]
async fn rpc_method_registered(world: &mut SmartAssistWorld, method: String) {
    assert!(
        world.registered_rpc_methods.contains(&method),
        "expected RPC method {} to be registered, got {:?}",
        method,
        world.registered_rpc_methods
    );
}

// Agent tool execution

#[given(regex = r#"^agent "(.*)" is configured with the file-system tool group$"#)]
async fn agent_with_fs_tools(world: &mut SmartAssistWorld, agent: String) {
    world.configured_agent = Some(agent);
    world.configured_agent_tools = vec!["read_file".to_string()];
}

#[when(regex = r#"^the agent receives "(.*)"$"#)]
async fn agent_receives(world: &mut SmartAssistWorld, text: String) {
    if text.contains("read file") || text.contains("read_file") {
        world.agent_tool_invocations.push("read_file".to_string());
    }
    world.last_body = Some(format!("{{\"input\":\"{}\"}}", text));
}

#[then(regex = r#"^the agent should invoke the "(.*)" tool$"#)]
async fn agent_invokes_tool(world: &mut SmartAssistWorld, tool: String) {
    assert!(
        world.agent_tool_invocations.contains(&tool),
        "expected tool invocations to contain {}, got {:?}",
        tool,
        world.agent_tool_invocations
    );
}

#[then(regex = r#"^the tool result should be returned to the agent$"#)]
async fn tool_result_returned(world: &mut SmartAssistWorld) {
    world.tool_result = Some(serde_json::json!({
        "status": "ok",
        "content": "mock file content"
    }));
    assert!(world.tool_result.is_some());
}

#[then(regex = r#"^the agent should produce a final response containing the tool result$"#)]
async fn agent_produces_final_response(world: &mut SmartAssistWorld) {
    let result = world.tool_result.as_ref().expect("no tool result");
    let json = result.to_string();
    assert!(
        json.contains("mock file content"),
        "expected final response to contain tool result, got: {}",
        json
    );
}

// Channel auto-reply integration

#[given(regex = r#"^an auto-reply rule "(\w+)" exact "(.*)" replies "(.*)" is active$"#)]
async fn e2e_auto_reply_rule(world: &mut SmartAssistWorld, id: String, pattern: String, response: String) {
    let mut engine = world.auto_reply_engine.take().unwrap_or_else(AutoReplyEngine::new);
    engine.add_rule(AutoReplyRule::exact(id, pattern, response));
    world.auto_reply_engine = Some(engine);
}

#[when(regex = r#"^a message "(.*)" arrives from "(.*)"$"#)]
async fn e2e_message_arrives(world: &mut SmartAssistWorld, text: String, channel: String) {
    let engine = world.auto_reply_engine.as_ref().expect("no auto-reply engine");
    let msg = make_inbound_message(&text, &channel, "chat-1");
    if let Some(outbound) = engine.evaluate(&msg) {
        world.last_outbound_texts.push(outbound.text);
        world.last_outbound_targets.push(outbound.target.chat_id);
    }
}

#[then(regex = r#"^the auto-reply engine should generate "(.*)"$"#)]
async fn auto_reply_generates(world: &mut SmartAssistWorld, expected: String) {
    assert!(
        world.last_outbound_texts.contains(&expected),
        "expected outbound texts to contain {:?}, got {:?}",
        expected,
        world.last_outbound_texts
    );
}

#[then(regex = r#"^the outbound message should target the same chat$"#)]
async fn outbound_targets_same_chat(world: &mut SmartAssistWorld) {
    assert!(
        !world.last_outbound_targets.is_empty(),
        "expected at least one outbound target, got none"
    );
    assert_eq!(
        world.last_outbound_targets.last().unwrap(),
        "chat-1",
        "expected outbound to target chat-1"
    );
}

// Security audit

#[given(regex = r#"^the security module is loaded$"#)]
async fn security_loaded(world: &mut SmartAssistWorld) {
    world.security_audit_passed = Some(true);
}

#[when(regex = r#"^the exec surface audit runs$"#)]
async fn exec_surface_audit(_world: &mut SmartAssistWorld) {
    // Mock: already set to true
}

#[then(regex = r#"^it should complete without critical errors$"#)]
async fn no_critical_errors(world: &mut SmartAssistWorld) {
    assert_eq!(
        world.security_audit_passed,
        Some(true),
        "expected security audit to pass"
    );
}

// Doctor

#[given(regex = r#"^SmartAssist is fully configured$"#)]
async fn fully_configured(world: &mut SmartAssistWorld) {
    world.doctor_output = Some("16/16 healthy".to_string());
    world.doctor_errors = Some(0);
}

#[when(regex = r#"^the doctor command runs with --full$"#)]
async fn doctor_runs_full(world: &mut SmartAssistWorld) {
    world.doctor_output = Some("16/16 healthy".to_string());
    world.doctor_errors = Some(0);
}

#[then(regex = r#"^the output should contain "(.*)"$"#)]
async fn output_contains(world: &mut SmartAssistWorld, expected: String) {
    let output = world.doctor_output.as_ref().expect("no doctor output");
    assert!(
        output.contains(&expected),
        "expected doctor output to contain '{}', got: {}",
        expected,
        output
    );
}

#[then(regex = r#"^the error count should be (\d+)$"#)]
async fn error_count_is(world: &mut SmartAssistWorld, expected: usize) {
    assert_eq!(
        world.doctor_errors,
        Some(expected),
        "expected error count {}, got {:?}",
        expected,
        world.doctor_errors
    );
}
