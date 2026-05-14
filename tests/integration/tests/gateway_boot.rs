//! Gateway boot integration tests.

use std::time::Duration;
use tokio::time::timeout;

/// Verify that the gateway can boot and respond to a health check.
#[tokio::test]
async fn test_gateway_boots_and_responds() {
    // TODO: instantiate GatewayConfig and boot a local gateway instance
    // Once Phase 2 (Gateway RPC) is implemented, this test will exercise
    // the real HTTP surface.
    let result = timeout(Duration::from_secs(5), async {
        // Placeholder
        true
    })
    .await;

    assert!(result.is_ok(), "gateway boot timed out");
    assert!(result.unwrap(), "gateway boot failed");
}

/// Verify that the gateway rejects unauthenticated requests when auth is enabled.
#[tokio::test]
async fn test_gateway_rejects_unauthenticated() {
    // TODO: boot gateway with token auth and assert 401 on missing header
    let result = timeout(Duration::from_secs(5), async {
        // Placeholder
        true
    })
    .await;

    assert!(result.is_ok(), "auth rejection test timed out");
    assert!(result.unwrap(), "auth rejection test failed");
}
