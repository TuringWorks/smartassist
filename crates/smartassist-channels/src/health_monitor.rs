//! Health monitoring for messaging channels.
//!
//! Provides periodic health polling, history tracking, configurable
//! reaction policies, and event broadcasting for status transitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use smartassist_core::types::{ChannelHealth, HealthStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

/// A single health check snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthRecord {
    /// Timestamp of the check.
    pub checked_at: DateTime<Utc>,
    /// Health status at this check.
    pub status: HealthStatus,
    /// Latency in milliseconds, if available.
    pub latency_ms: Option<u64>,
    /// Error message, if any.
    pub error: Option<String>,
}

impl HealthRecord {
    /// Create a new health record from a ChannelHealth snapshot.
    pub fn from_health(health: &ChannelHealth) -> Self {
        Self {
            checked_at: Utc::now(),
            status: health.status.clone(),
            latency_ms: health.latency_ms,
            error: health.error.clone(),
        }
    }
}

/// In-memory ring-buffer history of health checks per channel.
pub struct HealthHistory {
    max_records: usize,
    records: HashMap<String, Vec<HealthRecord>>,
}

impl HealthHistory {
    /// Create a new history buffer.
    pub fn new(max_records: usize) -> Self {
        Self {
            max_records,
            records: HashMap::new(),
        }
    }

    /// Record a health check for a channel.
    pub fn record(&mut self, channel_id: &str, record: HealthRecord) {
        let entry = self.records.entry(channel_id.to_string()).or_default();
        entry.push(record);
        if entry.len() > self.max_records {
            entry.remove(0);
        }
    }

    /// Get the history for a channel.
    pub fn get(&self, channel_id: &str) -> Vec<HealthRecord> {
        self.records.get(channel_id).cloned().unwrap_or_default()
    }

    /// Count consecutive checks matching a predicate.
    pub fn consecutive<F>(&self, channel_id: &str, predicate: F) -> usize
    where
        F: Fn(&HealthStatus) -> bool,
    {
        self.records
            .get(channel_id)
            .map(|records| {
                records
                    .iter()
                    .rev()
                    .take_while(|r| predicate(&r.status))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Get the most recent record for a channel.
    pub fn last(&self, channel_id: &str) -> Option<HealthRecord> {
        self.records
            .get(channel_id)
            .and_then(|r| r.last().cloned())
    }
}

/// Configurable policy for health monitoring reactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthPolicy {
    /// Seconds between health polls.
    pub poll_interval_secs: u64,
    /// Consecutive unhealthy checks before triggering reconnect.
    pub consecutive_unhealthy_before_reconnect: usize,
    /// Maximum reconnect attempts before giving up.
    pub max_reconnect_attempts: usize,
    /// Seconds to wait between reconnect attempts (linear backoff).
    pub reconnect_backoff_secs: u64,
    /// Consecutive failed reconnects before disabling the channel.
    pub disable_after_consecutive_failures: usize,
    /// Whether to emit events for degraded status.
    pub alert_on_degraded: bool,
    /// Maximum health history records per channel.
    pub history_capacity: usize,
}

impl Default for HealthPolicy {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            consecutive_unhealthy_before_reconnect: 2,
            max_reconnect_attempts: 5,
            reconnect_backoff_secs: 10,
            disable_after_consecutive_failures: 3,
            alert_on_degraded: true,
            history_capacity: 60,
        }
    }
}

/// Events emitted when a channel's health status changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthEvent {
    /// Channel became healthy.
    BecameHealthy {
        channel_id: String,
        latency_ms: Option<u64>,
    },
    /// Channel became degraded.
    BecameDegraded {
        channel_id: String,
        error: Option<String>,
    },
    /// Channel became unhealthy.
    BecameUnhealthy {
        channel_id: String,
        error: Option<String>,
    },
    /// Reconnect was attempted.
    ReconnectAttempted {
        channel_id: String,
        attempt: usize,
        success: bool,
    },
    /// Channel was disabled after repeated failures.
    Disabled {
        channel_id: String,
        reason: String,
    },
}

/// Per-channel reconnect tracking.
struct ReconnectState {
    attempts: usize,
    last_attempt: Option<DateTime<Utc>>,
}

/// Health monitor that polls channels and applies reaction policies.
#[derive(Clone)]
pub struct HealthMonitor {
    policy: HealthPolicy,
    history: Arc<RwLock<HealthHistory>>,
    event_tx: broadcast::Sender<HealthEvent>,
    reconnect_states: Arc<RwLock<HashMap<String, ReconnectState>>>,
}

impl HealthMonitor {
    /// Create a new health monitor with the given policy.
    pub fn new(policy: HealthPolicy) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        let history_capacity = policy.history_capacity;
        Self {
            policy,
            history: Arc::new(RwLock::new(HealthHistory::new(history_capacity))),
            event_tx,
            reconnect_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default policy.
    pub fn default_policy() -> Self {
        Self::new(HealthPolicy::default())
    }

    /// Subscribe to health events.
    pub fn subscribe(&self) -> broadcast::Receiver<HealthEvent> {
        self.event_tx.subscribe()
    }

    /// Get a snapshot of the latest health record for each channel.
    pub async fn latest(&self, channel_ids: &[String]) -> HashMap<String, Option<HealthRecord>> {
        let history = self.history.read().await;
        channel_ids
            .iter()
            .map(|id| (id.clone(), history.last(id)))
            .collect()
    }

    /// Get the full history for a channel.
    pub async fn history(&self, channel_id: &str) -> Vec<HealthRecord> {
        let history = self.history.read().await;
        history.get(channel_id)
    }

    /// Run a single health check cycle against the provided channels.
    ///
    /// `channels` is a map of instance_id → channel (Arc<dyn Channel>) that
    /// the caller must provide under a read guard of its own.
    pub async fn check_cycle(&self,
        channels: &HashMap<String, Arc<dyn crate::traits::Channel>>,
    ) {
        for (id, channel) in channels {
            let health = match channel.health().await {
                Ok(h) => h,
                Err(e) => ChannelHealth {
                    status: HealthStatus::Unhealthy,
                    latency_ms: None,
                    last_message_at: None,
                    error: Some(e.to_string()),
                },
            };

            let record = HealthRecord::from_health(&health);
            let previous = {
                let mut history = self.history.write().await;
                let prev = history.last(id);
                history.record(id, record);
                prev
            };

            // Emit events on transitions
            self.emit_transition(id, &previous, &health).await;

            // Apply reaction policy
            self.apply_policy(id, channel, &health, previous.as_ref())
                .await;
        }
    }

    /// Start the background polling loop using the provided registry.
    pub fn start(
        self: Arc<Self>,
        running: Arc<RwLock<bool>>,
        registry: Arc<crate::registry::ChannelRegistry>,
    ) {
        let interval = tokio::time::Duration::from_secs(self.policy.poll_interval_secs);
        let this = self.clone();

        tokio::spawn(async move {
            info!("Starting health monitor loop (interval: {:?})", interval);

            loop {
                tokio::time::sleep(interval).await;

                if !*running.read().await {
                    break;
                }

                let ids = registry.list().await;
                let mut channels = HashMap::new();
                for id in ids {
                    if let Some(ch) = registry.get(&id).await {
                        channels.insert(id, ch);
                    }
                }

                if !channels.is_empty() {
                    this.check_cycle(&channels).await;
                }
            }

            info!("Health monitor loop stopped");
        });
    }

    // --- internal helpers ---

    async fn emit_transition(
        &self,
        channel_id: &str,
        previous: &Option<HealthRecord>,
        current: &ChannelHealth,
    ) {
        let event = match (&previous, current.status.clone()) {
            (Some(prev), HealthStatus::Healthy) if prev.status != HealthStatus::Healthy => {
                Some(HealthEvent::BecameHealthy {
                    channel_id: channel_id.to_string(),
                    latency_ms: current.latency_ms,
                })
            }
            (Some(prev), HealthStatus::Degraded) if prev.status != HealthStatus::Degraded => {
                if self.policy.alert_on_degraded {
                    Some(HealthEvent::BecameDegraded {
                        channel_id: channel_id.to_string(),
                        error: current.error.clone(),
                    })
                } else {
                    None
                }
            }
            (Some(prev), HealthStatus::Unhealthy) if prev.status != HealthStatus::Unhealthy => {
                Some(HealthEvent::BecameUnhealthy {
                    channel_id: channel_id.to_string(),
                    error: current.error.clone(),
                })
            }
            _ => None,
        };

        if let Some(ev) = event {
            let _ = self.event_tx.send(ev);
        }
    }

    async fn apply_policy(
        &self,
        channel_id: &str,
        channel: &Arc<dyn crate::traits::Channel>,
        health: &ChannelHealth,
        _previous: Option<&HealthRecord>,
    ) {
        match health.status {
            HealthStatus::Healthy => {
                // Reset reconnect state on healthy
                let mut states = self.reconnect_states.write().await;
                if states.remove(channel_id).is_some() {
                    debug!("Reset reconnect state for {}", channel_id);
                }
            }
            HealthStatus::Degraded => {
                warn!(
                    "Channel {} is degraded: {:?}",
                    channel_id, health.error
                );
            }
            HealthStatus::Unhealthy | HealthStatus::Unknown => {
                let consec = {
                    let history = self.history.read().await;
                    history.consecutive(channel_id, |s| {
                        matches!(s, HealthStatus::Unhealthy | HealthStatus::Unknown)
                    })
                };

                if consec >= self.policy.consecutive_unhealthy_before_reconnect {
                    self.attempt_reconnect(channel_id, channel).await;
                }
            }
        }
    }

    async fn attempt_reconnect(
        &self,
        channel_id: &str,
        channel: &Arc<dyn crate::traits::Channel>,
    ) {
        let mut states = self.reconnect_states.write().await;
        let state = states.entry(channel_id.to_string()).or_insert(ReconnectState {
            attempts: 0,
            last_attempt: None,
        });

        if state.attempts >= self.policy.max_reconnect_attempts {
            if state.attempts == self.policy.max_reconnect_attempts {
                // Only log / disable once
                warn!(
                    "Channel {} exceeded max reconnect attempts ({}); disabling",
                    channel_id, self.policy.max_reconnect_attempts
                );
                let _ = self.event_tx.send(HealthEvent::Disabled {
                    channel_id: channel_id.to_string(),
                    reason: format!(
                        "Exceeded {} reconnect attempts",
                        self.policy.max_reconnect_attempts
                    ),
                });
            }
            state.attempts += 1; // prevent re-logging
            return;
        }

        state.attempts += 1;
        let attempt = state.attempts;
        drop(states);

        // Linear backoff
        if attempt > 1 {
            let backoff = tokio::time::Duration::from_secs(
                self.policy.reconnect_backoff_secs * (attempt - 1) as u64,
            );
            tokio::time::sleep(backoff).await;
        }

        info!("Attempting reconnect for {} (attempt {})", channel_id, attempt);
        let success = match channel.reconnect().await {
            Ok(()) => {
                info!("Channel {} reconnected successfully", channel_id);
                true
            }
            Err(e) => {
                error!("Channel {} reconnect failed: {}", channel_id, e);
                false
            }
        };

        let _ = self.event_tx.send(HealthEvent::ReconnectAttempted {
            channel_id: channel_id.to_string(),
            attempt,
            success,
        });

        if !success {
            let mut states = self.reconnect_states.write().await;
            if let Some(state) = states.get_mut(channel_id) {
                state.last_attempt = Some(Utc::now());

                if state.attempts >= self.policy.disable_after_consecutive_failures {
                    warn!(
                        "Channel {} failed {} consecutive reconnects; disabling",
                        channel_id, state.attempts
                    );
                    let _ = self.event_tx.send(HealthEvent::Disabled {
                        channel_id: channel_id.to_string(),
                        reason: format!(
                            "{} consecutive reconnect failures",
                            state.attempts
                        ),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{Channel, ChannelLifecycle, ChannelReceiver, ChannelSender};
    use crate::{Result, SendResult};
    use async_trait::async_trait;
    use smartassist_core::types::{ChannelCapabilities, InboundMessage, MessageTarget, OutboundMessage};
    use crate::traits::MessageRef;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[derive(Debug)]
    struct MockChannel {
        health_status: Arc<RwLock<HealthStatus>>,
        connected: Arc<AtomicBool>,
        reconnect_count: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn channel_type(&self) -> &str {
            "mock"
        }
        fn instance_id(&self) -> &str {
            "mock-1"
        }
        fn capabilities(&self) -> ChannelCapabilities {
            ChannelCapabilities::default()
        }
    }

    #[async_trait]
    impl ChannelSender for MockChannel {
        async fn send(&self, _msg: OutboundMessage) -> Result<SendResult> {
            Ok(SendResult::new(""))
        }
        async fn send_with_attachments(
            &self,
            _msg: OutboundMessage,
            _attachments: Vec<crate::attachment::Attachment>,
        ) -> Result<SendResult> {
            Ok(SendResult::new(""))
        }
        async fn edit(&self, _msg: &MessageRef, _new: &str) -> Result<()> {
            Ok(())
        }
        async fn delete(&self, _msg: &MessageRef) -> Result<()> {
            Ok(())
        }
        async fn react(&self, _msg: &MessageRef, _emoji: &str) -> Result<()> {
            Ok(())
        }
        async fn unreact(&self, _msg: &MessageRef, _emoji: &str) -> Result<()> {
            Ok(())
        }
        async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
            Ok(())
        }
        fn max_message_length(&self) -> usize {
            1000
        }
    }

    #[async_trait]
    impl ChannelReceiver for MockChannel {
        async fn start_receiving(&self) -> Result<()> {
            Ok(())
        }
        async fn stop_receiving(&self) -> Result<()> {
            Ok(())
        }
        async fn receive(&self) -> Result<InboundMessage> {
            loop { tokio::task::yield_now().await; }
        }
        async fn try_receive(&self) -> Result<Option<InboundMessage>> {
            Ok(None)
        }
        fn set_handler(&self, _handler: Box<dyn crate::traits::MessageHandler>) {}
    }

    #[async_trait]
    impl ChannelLifecycle for MockChannel {
        async fn connect(&self) -> Result<()> {
            self.connected.store(true, Ordering::Relaxed);
            Ok(())
        }
        async fn disconnect(&self) -> Result<()> {
            self.connected.store(false, Ordering::Relaxed);
            Ok(())
        }
        fn is_connected(&self) -> bool {
            self.connected.load(Ordering::Relaxed)
        }
        async fn health(&self) -> Result<ChannelHealth> {
            let status = self.health_status.read().await.clone();
            Ok(ChannelHealth {
                status,
                latency_ms: Some(10),
                last_message_at: None,
                error: None,
            })
        }
        async fn reconnect(&self) -> Result<()> {
            self.reconnect_count.fetch_add(1, Ordering::Relaxed);
            self.connect().await
        }
    }

    fn mock_channels(
        channel: MockChannel,
    ) -> HashMap<String, Arc<dyn crate::traits::Channel>> {
        let mut map = HashMap::new();
        map.insert("mock-1".to_string(), Arc::new(channel) as Arc<dyn crate::traits::Channel>);
        map
    }

    #[tokio::test]
    async fn test_health_history_records_and_consecutive() {
        let mut history = HealthHistory::new(5);

        history.record("ch1", HealthRecord {
            checked_at: Utc::now(),
            status: HealthStatus::Healthy,
            latency_ms: Some(10),
            error: None,
        });
        history.record("ch1", HealthRecord {
            checked_at: Utc::now(),
            status: HealthStatus::Unhealthy,
            latency_ms: None,
            error: Some("fail".to_string()),
        });
        history.record("ch1", HealthRecord {
            checked_at: Utc::now(),
            status: HealthStatus::Unhealthy,
            latency_ms: None,
            error: Some("fail2".to_string()),
        });

        assert_eq!(history.consecutive("ch1", |s| *s == HealthStatus::Unhealthy), 2);
        assert_eq!(history.consecutive("ch1", |s| *s == HealthStatus::Healthy), 0);
    }

    #[tokio::test]
    async fn test_health_monitor_emits_healthy_transition() {
        let policy = HealthPolicy {
            poll_interval_secs: 1,
            consecutive_unhealthy_before_reconnect: 2,
            max_reconnect_attempts: 3,
            reconnect_backoff_secs: 0,
            disable_after_consecutive_failures: 3,
            alert_on_degraded: true,
            history_capacity: 10,
        };
        let monitor = HealthMonitor::new(policy);
        let mut rx = monitor.subscribe();

        let health_status = Arc::new(RwLock::new(HealthStatus::Unhealthy));
        let connected = Arc::new(AtomicBool::new(false));
        let reconnect_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let channel = MockChannel {
            health_status: health_status.clone(),
            connected,
            reconnect_count,
        };

        let channels = mock_channels(channel);

        // First check: unhealthy (no transition event because no prior state)
        monitor.check_cycle(&channels).await;

        // Second check: healthy → should emit BecameHealthy
        *health_status.write().await = HealthStatus::Healthy;
        monitor.check_cycle(&channels).await;

        let ev = rx.try_recv().expect("Expected health event");
        match ev {
            HealthEvent::BecameHealthy { channel_id, .. } => {
                assert_eq!(channel_id, "mock-1");
            }
            other => panic!("Expected BecameHealthy, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_health_monitor_reconnects_after_consecutive_unhealthy() {
        let policy = HealthPolicy {
            poll_interval_secs: 1,
            consecutive_unhealthy_before_reconnect: 2,
            max_reconnect_attempts: 3,
            reconnect_backoff_secs: 0,
            disable_after_consecutive_failures: 3,
            alert_on_degraded: true,
            history_capacity: 10,
        };
        let monitor = HealthMonitor::new(policy);
        let mut rx = monitor.subscribe();

        let health_status = Arc::new(RwLock::new(HealthStatus::Healthy));
        let connected = Arc::new(AtomicBool::new(true));
        let reconnect_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let channel = MockChannel {
            health_status: health_status.clone(),
            connected: connected.clone(),
            reconnect_count: reconnect_count.clone(),
        };

        let channels = mock_channels(channel);

        // First check: healthy
        monitor.check_cycle(&channels).await;

        // Switch to unhealthy
        *health_status.write().await = HealthStatus::Unhealthy;

        // Second check: first unhealthy (not enough consecutive yet)
        monitor.check_cycle(&channels).await;

        // Third check: second unhealthy → triggers reconnect
        monitor.check_cycle(&channels).await;

        let ev = rx.try_recv();
        assert!(ev.is_ok(), "Expected at least one event");

        assert_eq!(reconnect_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_health_monitor_resets_reconnect_state_on_healthy() {
        let policy = HealthPolicy {
            poll_interval_secs: 1,
            consecutive_unhealthy_before_reconnect: 2,
            max_reconnect_attempts: 3,
            reconnect_backoff_secs: 0,
            disable_after_consecutive_failures: 3,
            alert_on_degraded: true,
            history_capacity: 10,
        };
        let monitor = HealthMonitor::new(policy);

        let health_status = Arc::new(RwLock::new(HealthStatus::Unhealthy));
        let connected = Arc::new(AtomicBool::new(false));
        let reconnect_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let channel = MockChannel {
            health_status: health_status.clone(),
            connected: connected.clone(),
            reconnect_count: reconnect_count.clone(),
        };

        let channels = mock_channels(channel);

        // Two unhealthy checks → reconnect attempt 1
        monitor.check_cycle(&channels).await;
        monitor.check_cycle(&channels).await;
        assert_eq!(reconnect_count.load(Ordering::Relaxed), 1);

        // Become healthy → resets state
        *health_status.write().await = HealthStatus::Healthy;
        monitor.check_cycle(&channels).await;

        // Go unhealthy again
        *health_status.write().await = HealthStatus::Unhealthy;
        monitor.check_cycle(&channels).await;
        monitor.check_cycle(&channels).await;

        // Should reconnect again because state was reset
        assert_eq!(reconnect_count.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn test_health_policy_default() {
        let policy = HealthPolicy::default();
        assert_eq!(policy.poll_interval_secs, 30);
        assert_eq!(policy.consecutive_unhealthy_before_reconnect, 2);
        assert_eq!(policy.max_reconnect_attempts, 5);
    }
}
