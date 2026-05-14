//! Node management step definitions.

use cucumber::{given, when, then};
use crate::SmartAssistWorld;

#[given(regex = r"^a node named '(.*)' is approved$")]
async fn node_approved(world: &mut SmartAssistWorld, name: String) {
    world.nodes.push(name);
}

#[when(regex = r"^I list all nodes$")]
async fn list_all_nodes(world: &mut SmartAssistWorld) {
    world.last_body = Some(format!(
        r#"{{"nodes":{},"count":{}}}"#,
        serde_json::to_string(&world.nodes).unwrap(),
        world.nodes.len()
    ));
    world.last_status = Some(200);
}

#[when(regex = r"^I approve node pairing for '(.*)' with code '(.*)'$")]
async fn approve_node(world: &mut SmartAssistWorld, name: String, _code: String) {
    world.nodes.push(name.clone());
    world.last_body = Some(format!(r#"{{"node_id":"{}","paired":true}}"#, name));
    world.last_status = Some(200);
}

#[when(regex = r"^I describe node '(.*)'$")]
async fn describe_node(world: &mut SmartAssistWorld, name: String) {
    if world.nodes.contains(&name) {
        world.last_body = Some(format!(
            r#"{{"id":"{}","name":"{}","paired":true}}"#,
            name, name
        ));
        world.last_status = Some(200);
    } else {
        world.last_status = Some(404);
        world.last_body = Some(r#"{"error":"not found"}"#.to_string());
    }
}

#[when(regex = r"^I rename node '(.*)' to '(.*)'$")]
async fn rename_node(world: &mut SmartAssistWorld, old_name: String, new_name: String) {
    if let Some(pos) = world.nodes.iter().position(|n| n == &old_name) {
        world.nodes[pos] = new_name;
        world.last_body = Some(r#"{"renamed":true}"#.to_string());
    } else {
        world.last_body = Some(r#"{"renamed":false}"#.to_string());
    }
    world.last_status = Some(200);
}

#[when(regex = r"^I unpair node '(.*)'$")]
async fn unpair_node(world: &mut SmartAssistWorld, name: String) {
    world.nodes.retain(|n| n != &name);
    world.last_body = Some(r#"{"unpaired":true}"#.to_string());
    world.last_status = Some(200);
}

#[then(regex = r"^the node count should be (\d+)$")]
async fn node_count_should_be(world: &mut SmartAssistWorld, expected: usize) {
    let body = world.last_body.as_ref().expect("no response body");
    let expected_str = format!("\"count\":{}", expected);
    assert!(
        body.contains(&expected_str),
        "expected body to contain '{}', got: {}",
        expected_str,
        body
    );
}
