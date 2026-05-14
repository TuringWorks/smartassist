//! Mobile app step definitions.

use cucumber::{given, then};
use crate::SmartAssistWorld;

// Android steps

#[given(regex = r#"^an Android device with id "(.*)", name "(.*)", type "(.*)"$"#)]
async fn android_device(
    world: &mut SmartAssistWorld,
    id: String,
    name: String,
    _device_type: String,
) {
    world.android_device_id = Some(id);
    world.android_device_name = Some(name);
    world.android_device_connected = Some(false);
}

#[given(regex = r#"^an Android chat message with role "(.*)" and content "(.*)"$"#)]
async fn android_chat_message(world: &mut SmartAssistWorld, role: String, content: String) {
    world.android_chat_role = Some(role);
    world.android_chat_content = Some(content);
}

#[given(regex = r#"^a new Android session$"#)]
async fn new_android_session(world: &mut SmartAssistWorld) {
    world.android_session_status = Some("active".to_string());
    world.android_session_messages.clear();
}

#[given(regex = r#"^a new GatewayClient$"#)]
async fn new_gateway_client(world: &mut SmartAssistWorld) {
    world.gateway_url = Some("http://localhost:18789".to_string());
}

// iOS steps

#[given(regex = r#"^the iOS SmartAssist app$"#)]
async fn ios_app(world: &mut SmartAssistWorld) {
    world.ios_app_version = Some("0.1.0".to_string());
}

#[given(regex = r#"^a device with id "(.*)", name "(.*)", type "(.*)"$"#)]
async fn ios_device(
    world: &mut SmartAssistWorld,
    id: String,
    name: String,
    _device_type: String,
) {
    world.ios_device_id = Some(id);
    world.ios_device_name = Some(name);
    world.ios_device_connected = Some(false);
}

#[given(regex = r#"^a chat message with role "(.*)" and content "(.*)"$"#)]
async fn ios_chat_message(world: &mut SmartAssistWorld, role: String, content: String) {
    world.ios_chat_role = Some(role);
    world.ios_chat_content = Some(content);
}

#[given(regex = r#"^a new session$"#)]
async fn new_ios_session(world: &mut SmartAssistWorld) {
    world.ios_session_status = Some("active".to_string());
    world.ios_session_messages.clear();
}

// Unified Then steps (shared between Android and iOS)

#[then(regex = r#"^the device id should be "(.*)"$"#)]
async fn device_id_should_be(world: &mut SmartAssistWorld, expected: String) {
    let android = world.android_device_id.as_deref() == Some(&expected);
    let ios = world.ios_device_id.as_deref() == Some(&expected);
    assert!(
        android || ios,
        "expected device id {:?} in android ({:?}) or ios ({:?}) state",
        expected,
        world.android_device_id,
        world.ios_device_id
    );
}

#[then(regex = r#"^the device should not be connected$"#)]
async fn device_not_connected(world: &mut SmartAssistWorld) {
    let android = world.android_device_connected == Some(false);
    let ios = world.ios_device_connected == Some(false);
    assert!(
        android || ios,
        "expected device not connected in android ({:?}) or ios ({:?}) state",
        world.android_device_connected,
        world.ios_device_connected
    );
}

#[then(regex = r#"^the message role should be "(.*)"$"#)]
async fn message_role_should_be(world: &mut SmartAssistWorld, expected: String) {
    let android = world.android_chat_role.as_deref() == Some(&expected);
    let ios = world.ios_chat_role.as_deref() == Some(&expected);
    assert!(
        android || ios,
        "expected message role {:?} in android ({:?}) or ios ({:?}) state",
        expected,
        world.android_chat_role,
        world.ios_chat_role
    );
}

#[then(regex = r#"^the message content should be "(.*)"$"#)]
async fn message_content_should_be(world: &mut SmartAssistWorld, expected: String) {
    let android = world.android_chat_content.as_deref() == Some(&expected);
    let ios = world.ios_chat_content.as_deref() == Some(&expected);
    assert!(
        android || ios,
        "expected message content {:?} in android ({:?}) or ios ({:?}) state",
        expected,
        world.android_chat_content,
        world.ios_chat_content
    );
}

#[then(regex = r#"^the session status should be "(.*)"$"#)]
async fn session_status_should_be(world: &mut SmartAssistWorld, expected: String) {
    let android = world.android_session_status.as_deref() == Some(&expected);
    let ios = world.ios_session_status.as_deref() == Some(&expected);
    assert!(
        android || ios,
        "expected session status {:?} in android ({:?}) or ios ({:?}) state",
        expected,
        world.android_session_status,
        world.ios_session_status
    );
}

#[then(regex = r#"^the session should have no messages$"#)]
async fn session_no_messages(world: &mut SmartAssistWorld) {
    let android = world.android_session_messages.is_empty();
    let ios = world.ios_session_messages.is_empty();
    assert!(
        android || ios,
        "expected empty session messages in android ({:?}) or ios ({:?}) state",
        world.android_session_messages,
        world.ios_session_messages
    );
}

#[then(regex = r#"^the app version should be "(.*)"$"#)]
async fn app_version_should_be(world: &mut SmartAssistWorld, expected: String) {
    assert_eq!(world.ios_app_version, Some(expected.clone()));
}

#[then(regex = r#"^the base URL should be "(.*)"$"#)]
async fn base_url_should_be(world: &mut SmartAssistWorld, expected: String) {
    assert_eq!(world.gateway_url, Some(expected.clone()));
}
