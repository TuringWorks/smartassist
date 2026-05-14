//! TUI step definitions.

use clap::Parser;
use cucumber::{given, then, when};
use crate::SmartAssistWorld;

#[when(regex = r#"^I run "smartassist tui --agent ([^"]+)"$"#)]
async fn run_tui_with_agent(world: &mut SmartAssistWorld, agent: String) {
    let cli = smartassist_cli::Cli::parse_from(["smartassist", "tui", "--agent", &agent]);
    match cli.command {
        smartassist_cli::Commands::Tui(args) => {
            world.parsed_tui_agent = args.agent;
        }
        _ => panic!("expected Tui command"),
    }
}

#[when(regex = r#"^I run "smartassist tui"$"#)]
async fn run_tui_without_agent(world: &mut SmartAssistWorld) {
    let cli = smartassist_cli::Cli::parse_from(["smartassist", "tui"]);
    match cli.command {
        smartassist_cli::Commands::Tui(args) => {
            world.parsed_tui_agent = args.agent;
        }
        _ => panic!("expected Tui command"),
    }
}

#[then(regex = r#"^the CLI should parse the Tui command with agent "([^"]+)"$"#)]
async fn tui_parsed_agent(world: &mut SmartAssistWorld, expected: String) {
    assert_eq!(
        world.parsed_tui_agent,
        Some(expected.clone()),
        "expected agent {:?}, got {:?}",
        expected,
        world.parsed_tui_agent
    );
}

#[then(regex = r#"^the CLI should parse the Tui command with no agent specified$"#)]
async fn tui_parsed_no_agent(world: &mut SmartAssistWorld) {
    assert!(
        world.parsed_tui_agent.is_none(),
        "expected no agent, got {:?}",
        world.parsed_tui_agent
    );
}

#[given(regex = r#"^a TUI app with agent "([^"]+)"$"#)]
async fn tui_app_with_agent(world: &mut SmartAssistWorld, agent: String) {
    world.tui_agent = Some(agent);
    world.tui_messages.clear();
    world.tui_input = Some(String::new());
    world.tui_status = Some("Press Enter to send, Esc to quit".to_string());
}

#[then(regex = r#"^the messages list should be empty$"#)]
async fn messages_list_empty(world: &mut SmartAssistWorld) {
    assert!(
        world.tui_messages.is_empty(),
        "expected empty messages, got {:?}",
        world.tui_messages
    );
}

#[then(regex = r#"^the input field should be empty$"#)]
async fn input_field_empty(world: &mut SmartAssistWorld) {
    let input = world.tui_input.as_deref().unwrap_or("");
    assert!(
        input.is_empty(),
        "expected empty input, got {:?}",
        world.tui_input
    );
}

#[then(regex = r#"^the status should show "(.*)"$"#)]
async fn status_should_show(world: &mut SmartAssistWorld, expected: String) {
    assert_eq!(
        world.tui_status,
        Some(expected.clone()),
        "expected status {:?}, got {:?}",
        expected,
        world.tui_status
    );
}

#[when(regex = r#"^the user types "(.*)"$"#)]
async fn user_types(world: &mut SmartAssistWorld, text: String) {
    let input = world.tui_input.get_or_insert_default();
    input.push_str(&text);
}

#[when(regex = r#"^the user presses Enter$"#)]
async fn user_presses_enter(world: &mut SmartAssistWorld) {
    let input = world.tui_input.as_deref().unwrap_or("").to_string();
    if input == "/clear" {
        world.tui_messages.clear();
    } else if !input.is_empty() {
        world.tui_messages.push(format!("User: {}", input));
    }
    world.tui_input = Some(String::new());
}

#[then(regex = r#"^a User message "(.*)" should appear in chat$"#)]
async fn user_message_appears(world: &mut SmartAssistWorld, expected: String) {
    let expected_formatted = format!("User: {}", expected);
    assert!(
        world.tui_messages.contains(&expected_formatted),
        "expected messages to contain {:?}, got {:?}",
        expected_formatted,
        world.tui_messages
    );
}

#[then(regex = r#"^the input field should contain "(.*)"$"#)]
async fn input_field_contains(world: &mut SmartAssistWorld, expected: String) {
    let input = world.tui_input.as_deref().unwrap_or("");
    assert_eq!(input, &expected, "expected input {:?}, got {:?}", expected, input);
}

#[when(regex = r#"^the user sends "(.*)"$"#)]
async fn user_sends(world: &mut SmartAssistWorld, text: String) {
    world.tui_input = Some(text.clone());
    if text == "/clear" {
        world.tui_messages.clear();
    } else if !text.is_empty() {
        world.tui_messages.push(format!("User: {}", text));
    }
    world.tui_input = Some(String::new());
}
