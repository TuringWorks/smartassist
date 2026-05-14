//! Talk / voice step definitions.

use cucumber::{given, when};
use crate::SmartAssistWorld;

#[given(regex = r"^I have an active talk session$")]
async fn active_talk_session(world: &mut SmartAssistWorld) {
    world.session_id = Some("talk-session-1".to_string());
    world.talk_sessions.push("talk-session-1".to_string());
}

#[when(regex = r"^I start a talk session$")]
async fn start_talk_session(world: &mut SmartAssistWorld) {
    world.last_body = Some(r#"{"started":true,"session_id":"talk-session-1","state":"listening"}"#.to_string());
    world.talk_sessions.push("talk-session-1".to_string());
    world.session_id = Some("talk-session-1".to_string());
}

#[when(regex = r"^I stop the talk session$")]
async fn stop_talk_session(world: &mut SmartAssistWorld) {
    world.last_body = Some(r#"{"session_id":"talk-session-1","stopped":true}"#.to_string());
    world.talk_sessions.retain(|s| s != "talk-session-1");
}

#[when(regex = r"^I press push-to-talk$")]
async fn press_ptt(world: &mut SmartAssistWorld) {
    world.last_body = Some(r#"{"session_id":"talk-session-1","ptt_pressed":true}"#.to_string());
}

#[when(regex = r"^I release push-to-talk after (\d+) ms$")]
async fn release_ptt(world: &mut SmartAssistWorld, _duration: u64) {
    world.last_body = Some(r#"{"session_id":"talk-session-1","ptt_released":true,"duration_ms":500}"#.to_string());
}

#[when(regex = r"^I send base64 audio to the talk session$")]
async fn send_audio(world: &mut SmartAssistWorld) {
    world.last_body = Some(r#"{"session_id":"talk-session-1","received":true,"bytes":24}"#.to_string());
}

#[when(regex = r"^I request talk session status$")]
async fn request_talk_status(world: &mut SmartAssistWorld) {
    world.last_body = Some(r#"{"session_id":"talk-session-1","state":"listening","ptt_pressed":false,"wake_word_enabled":false,"duration_secs":0}"#.to_string());
}

#[when(regex = r"^I list talk sessions$")]
async fn list_talk_sessions(world: &mut SmartAssistWorld) {
    let count = world.talk_sessions.len();
    world.last_body = Some(format!(
        r#"{{"sessions":[{{"session_id":"talk-session-1","state":"listening","source":"Local","ptt_pressed":false,"wake_word_enabled":false,"duration_secs":0}}],"count":{}}}"#,
        count
    ));
}
