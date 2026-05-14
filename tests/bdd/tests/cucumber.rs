//! BDD test runner using cucumber-rust.
//!
//! Run with: cargo test --test bdd -p smartassist-bdd-tests

use cucumber::World;

mod steps;

/// The test world holds shared state across steps.
#[derive(Default, World)]
pub struct SmartAssistWorld {
    /// Last HTTP / RPC response status.
    pub last_status: Option<u16>,
    /// Last response body as JSON string.
    pub last_body: Option<String>,
    /// Gateway base URL (set during boot scenarios).
    pub gateway_url: Option<String>,
    /// Active session ID.
    pub session_id: Option<String>,
    /// Active channel ID.
    pub channel_id: Option<String>,
    /// Last tool result.
    pub tool_result: Option<serde_json::Value>,
    /// Active browser sessions.
    pub browser_sessions: Vec<String>,
    /// Active canvas surfaces.
    pub canvas_surfaces: Vec<String>,
    /// Active nodes.
    pub nodes: Vec<String>,
    /// Active voice talk sessions.
    pub talk_sessions: Vec<String>,

    // TUI state
    /// Parsed TUI agent argument.
    pub parsed_tui_agent: Option<String>,
    /// Mock TUI agent name.
    pub tui_agent: Option<String>,
    /// Mock TUI messages.
    pub tui_messages: Vec<String>,
    /// Mock TUI input field.
    pub tui_input: Option<String>,
    /// Mock TUI status text.
    pub tui_status: Option<String>,

    // i18n state
    /// Current locale for translations.
    pub current_locale: Option<String>,
    /// Last translated string.
    pub last_translation: Option<String>,

    // Auto-reply state
    /// Active auto-reply engine.
    pub auto_reply_engine: Option<smartassist_channels::auto_reply::AutoReplyEngine>,
    /// Texts of last outbound messages generated.
    pub last_outbound_texts: Vec<String>,
    /// Chat IDs of last outbound message targets.
    pub last_outbound_targets: Vec<String>,

    // Heartbeat state
    /// Active heartbeat filter.
    pub heartbeat_filter: Option<smartassist_channels::heartbeat::HeartbeatFilter>,
    /// Last heartbeat classification result.
    pub last_heartbeat_result: Option<bool>,

    // Mobile state
    /// Android device id.
    pub android_device_id: Option<String>,
    /// Android device name.
    pub android_device_name: Option<String>,
    /// Android device connected flag.
    pub android_device_connected: Option<bool>,
    /// Android chat message role.
    pub android_chat_role: Option<String>,
    /// Android chat message content.
    pub android_chat_content: Option<String>,
    /// Android session status.
    pub android_session_status: Option<String>,
    /// Android session messages.
    pub android_session_messages: Vec<String>,
    /// iOS app version.
    pub ios_app_version: Option<String>,
    /// iOS device id.
    pub ios_device_id: Option<String>,
    /// iOS device name.
    pub ios_device_name: Option<String>,
    /// iOS device connected flag.
    pub ios_device_connected: Option<bool>,
    /// iOS chat message role.
    pub ios_chat_role: Option<String>,
    /// iOS chat message content.
    pub ios_chat_content: Option<String>,
    /// iOS session status.
    pub ios_session_status: Option<String>,
    /// iOS session messages.
    pub ios_session_messages: Vec<String>,

    // E2E state
    /// Configured agent name.
    pub configured_agent: Option<String>,
    /// Configured model name.
    pub configured_model: Option<String>,
    /// Configured agent tools.
    pub configured_agent_tools: Vec<String>,
    /// Registered mock channels.
    pub mock_channels: Vec<String>,
    /// Agent online status.
    pub agent_status: Option<String>,
    /// Mock channel health status.
    pub mock_channel_health_status: Option<String>,
    /// Pending delivery queue.
    pub delivery_queue: Vec<String>,
    /// Registered RPC method names.
    pub registered_rpc_methods: Vec<String>,
    /// Tool invocations recorded.
    pub agent_tool_invocations: Vec<String>,
    /// Doctor command output.
    pub doctor_output: Option<String>,
    /// Doctor error count.
    pub doctor_errors: Option<usize>,
    /// Security audit passed flag.
    pub security_audit_passed: Option<bool>,
    /// Gateway configured port.
    pub gateway_port: Option<u16>,
}

impl std::fmt::Debug for SmartAssistWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmartAssistWorld")
            .field("last_status", &self.last_status)
            .field("last_body", &self.last_body)
            .field("gateway_url", &self.gateway_url)
            .field("session_id", &self.session_id)
            .field("channel_id", &self.channel_id)
            .field("tool_result", &self.tool_result)
            .field("browser_sessions", &self.browser_sessions)
            .field("canvas_surfaces", &self.canvas_surfaces)
            .field("nodes", &self.nodes)
            .field("talk_sessions", &self.talk_sessions)
            .field("parsed_tui_agent", &self.parsed_tui_agent)
            .field("tui_agent", &self.tui_agent)
            .field("tui_messages", &self.tui_messages)
            .field("tui_input", &self.tui_input)
            .field("tui_status", &self.tui_status)
            .field("current_locale", &self.current_locale)
            .field("last_translation", &self.last_translation)
            .field("auto_reply_engine", &self.auto_reply_engine.is_some())
            .field("last_outbound_texts", &self.last_outbound_texts)
            .field("last_outbound_targets", &self.last_outbound_targets)
            .field("heartbeat_filter", &self.heartbeat_filter.is_some())
            .field("last_heartbeat_result", &self.last_heartbeat_result)
            .field("android_device_id", &self.android_device_id)
            .field("android_device_name", &self.android_device_name)
            .field("android_device_connected", &self.android_device_connected)
            .field("android_chat_role", &self.android_chat_role)
            .field("android_chat_content", &self.android_chat_content)
            .field("android_session_status", &self.android_session_status)
            .field("android_session_messages", &self.android_session_messages)
            .field("ios_app_version", &self.ios_app_version)
            .field("ios_device_id", &self.ios_device_id)
            .field("ios_device_name", &self.ios_device_name)
            .field("ios_device_connected", &self.ios_device_connected)
            .field("ios_chat_role", &self.ios_chat_role)
            .field("ios_chat_content", &self.ios_chat_content)
            .field("ios_session_status", &self.ios_session_status)
            .field("ios_session_messages", &self.ios_session_messages)
            .field("configured_agent", &self.configured_agent)
            .field("configured_model", &self.configured_model)
            .field("configured_agent_tools", &self.configured_agent_tools)
            .field("mock_channels", &self.mock_channels)
            .field("agent_status", &self.agent_status)
            .field("mock_channel_health_status", &self.mock_channel_health_status)
            .field("delivery_queue", &self.delivery_queue)
            .field("registered_rpc_methods", &self.registered_rpc_methods)
            .field("agent_tool_invocations", &self.agent_tool_invocations)
            .field("doctor_output", &self.doctor_output)
            .field("doctor_errors", &self.doctor_errors)
            .field("security_audit_passed", &self.security_audit_passed)
            .field("gateway_port", &self.gateway_port)
            .finish()
    }
}

#[tokio::test]
async fn test_bdd_features() {
    SmartAssistWorld::run("features").await;
}
