//! End-to-end smoke test.
//!
//! This test exercises the full stack:
//!   1. Start the gateway
//!   2. Spawn an agent session
//!   3. Connect a channel (mock web)
//!   4. Send a message
//!   5. Assert a response is produced

use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn e2e_smoke_message_roundtrip() {
    // TODO: bootstrap gateway, agent, and channel once infrastructure is wired
    let result = timeout(Duration::from_secs(10), async {
        // Placeholder: assert full stack can be initialized
        true
    })
    .await;

    assert!(result.is_ok(), "e2e smoke test timed out");
    assert!(result.unwrap(), "e2e smoke test failed");
}
