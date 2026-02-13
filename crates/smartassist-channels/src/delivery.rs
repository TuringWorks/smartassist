//! Message delivery queue and status tracking.

use crate::error::ChannelError;
use crate::traits::{Channel, SendResult};
use crate::Result;
use smartassist_core::types::OutboundMessage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, warn};

/// Message delivery queue with retry support.
pub struct DeliveryQueue {
    /// Pending messages.
    queue: Arc<Mutex<VecDeque<QueuedMessage>>>,

    /// Channel registry for sending.
    channels: Arc<RwLock<HashMap<String, Arc<dyn Channel>>>>,

    /// Delivery status tracking.
    status: Arc<RwLock<HashMap<String, DeliveryStatus>>>,

    /// Configuration.
    config: DeliveryConfig,

    /// Shutdown signal sender.
    _shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Configuration for the delivery queue.
#[derive(Debug, Clone)]
pub struct DeliveryConfig {
    /// Maximum retry attempts.
    pub max_retries: u32,

    /// Initial retry delay.
    pub initial_retry_delay: Duration,

    /// Maximum retry delay.
    pub max_retry_delay: Duration,

    /// Retry delay multiplier (exponential backoff).
    pub retry_multiplier: f64,

    /// Maximum queue size.
    pub max_queue_size: usize,

    /// Message TTL (time-to-live).
    pub message_ttl: Duration,

    /// Processing batch size.
    pub batch_size: usize,
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_retry_delay: Duration::from_secs(1),
            max_retry_delay: Duration::from_secs(60),
            retry_multiplier: 2.0,
            max_queue_size: 10000,
            message_ttl: Duration::from_secs(3600), // 1 hour
            batch_size: 100,
        }
    }
}

/// A message in the delivery queue.
#[derive(Debug, Clone)]
struct QueuedMessage {
    /// Unique delivery ID.
    id: String,

    /// The message to deliver.
    message: OutboundMessage,

    /// Target channel instance.
    channel_id: String,

    /// Number of retry attempts.
    attempts: u32,

    /// Time when message was queued.
    queued_at: Instant,

    /// Next retry time (if retrying).
    next_retry: Option<Instant>,

    /// Last error (if any).
    last_error: Option<String>,
}

/// Status of a message delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryStatus {
    /// Delivery ID.
    pub id: String,

    /// Current status.
    pub status: DeliveryState,

    /// Number of attempts.
    pub attempts: u32,

    /// Message ID from channel (if delivered).
    pub message_id: Option<String>,

    /// Error message (if failed).
    pub error: Option<String>,

    /// Timestamp of last status change.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// State of a delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryState {
    /// Message is queued for delivery.
    Pending,

    /// Message is being delivered.
    InProgress,

    /// Message was delivered successfully.
    Delivered,

    /// Message delivery failed (may retry).
    Failed,

    /// Message delivery permanently failed.
    Dropped,

    /// Message was cancelled.
    Cancelled,
}

/// Result of a delivery attempt.
#[derive(Debug)]
pub struct DeliveryResult {
    /// Delivery ID.
    pub id: String,

    /// Whether delivery succeeded.
    pub success: bool,

    /// Message ID from channel (if successful).
    pub message_id: Option<String>,

    /// Error (if failed).
    pub error: Option<ChannelError>,

    /// Number of attempts.
    pub attempts: u32,
}

impl DeliveryQueue {
    /// Create a new delivery queue.
    pub fn new(config: DeliveryConfig) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            channels: Arc::new(RwLock::new(HashMap::new())),
            status: Arc::new(RwLock::new(HashMap::new())),
            config,
            _shutdown_tx: None,
        }
    }

    /// Register a channel for delivery.
    pub async fn register_channel(&self, id: String, channel: Arc<dyn Channel>) {
        let mut channels = self.channels.write().await;
        channels.insert(id, channel);
    }

    /// Unregister a channel.
    pub async fn unregister_channel(&self, id: &str) {
        let mut channels = self.channels.write().await;
        channels.remove(id);
    }

    /// Queue a message for delivery.
    pub async fn enqueue(
        &self,
        channel_id: String,
        message: OutboundMessage,
    ) -> Result<String> {
        let mut queue = self.queue.lock().await;

        if queue.len() >= self.config.max_queue_size {
            return Err(ChannelError::Delivery("Queue is full".to_string()));
        }

        let delivery_id = smartassist_core::id::uuid();

        let queued = QueuedMessage {
            id: delivery_id.clone(),
            message,
            channel_id,
            attempts: 0,
            queued_at: Instant::now(),
            next_retry: None,
            last_error: None,
        };

        queue.push_back(queued);

        // Track status
        let mut status_map = self.status.write().await;
        status_map.insert(
            delivery_id.clone(),
            DeliveryStatus {
                id: delivery_id.clone(),
                status: DeliveryState::Pending,
                attempts: 0,
                message_id: None,
                error: None,
                updated_at: chrono::Utc::now(),
            },
        );

        debug!("Queued message {} for delivery", delivery_id);
        Ok(delivery_id)
    }

    /// Get the status of a delivery.
    pub async fn get_status(&self, delivery_id: &str) -> Option<DeliveryStatus> {
        let status_map = self.status.read().await;
        status_map.get(delivery_id).cloned()
    }

    /// Cancel a pending delivery.
    pub async fn cancel(&self, delivery_id: &str) -> Result<()> {
        let mut queue = self.queue.lock().await;
        let mut status_map = self.status.write().await;

        // Remove from queue
        queue.retain(|m| m.id != delivery_id);

        // Update status
        if let Some(status) = status_map.get_mut(delivery_id) {
            status.status = DeliveryState::Cancelled;
            status.updated_at = chrono::Utc::now();
            Ok(())
        } else {
            Err(ChannelError::not_found(delivery_id))
        }
    }

    /// Process pending deliveries.
    pub async fn process(&self) -> Vec<DeliveryResult> {
        let mut results = Vec::new();
        let now = Instant::now();

        // Get messages ready for delivery
        let messages: Vec<QueuedMessage> = {
            let mut queue = self.queue.lock().await;
            let mut ready = Vec::new();

            while let Some(msg) = queue.pop_front() {
                // Check TTL
                if msg.queued_at.elapsed() > self.config.message_ttl {
                    results.push(DeliveryResult {
                        id: msg.id.clone(),
                        success: false,
                        message_id: None,
                        error: Some(ChannelError::Delivery("Message TTL expired".to_string())),
                        attempts: msg.attempts,
                    });
                    continue;
                }

                // Check if ready for retry
                if let Some(next_retry) = msg.next_retry {
                    if now < next_retry {
                        queue.push_back(msg);
                        continue;
                    }
                }

                ready.push(msg);

                if ready.len() >= self.config.batch_size {
                    break;
                }
            }

            ready
        };

        // Process each message
        for mut msg in messages {
            let result = self.deliver(&mut msg).await;
            results.push(result);
        }

        results
    }

    /// Deliver a single message.
    async fn deliver(&self, msg: &mut QueuedMessage) -> DeliveryResult {
        msg.attempts += 1;

        // Update status to in progress
        {
            let mut status_map = self.status.write().await;
            if let Some(status) = status_map.get_mut(&msg.id) {
                status.status = DeliveryState::InProgress;
                status.attempts = msg.attempts;
                status.updated_at = chrono::Utc::now();
            }
        }

        // Get the channel
        let channel = {
            let channels = self.channels.read().await;
            channels.get(&msg.channel_id).cloned()
        };

        let channel = match channel {
            Some(c) => c,
            None => {
                return self
                    .handle_failure(
                        msg,
                        ChannelError::not_found(&msg.channel_id),
                        false,
                    )
                    .await;
            }
        };

        // Attempt delivery
        match channel.send(msg.message.clone()).await {
            Ok(send_result) => {
                self.handle_success(msg, send_result).await
            }
            Err(e) => {
                let retriable = e.is_retriable();
                self.handle_failure(msg, e, retriable).await
            }
        }
    }

    /// Handle successful delivery.
    async fn handle_success(&self, msg: &QueuedMessage, send_result: SendResult) -> DeliveryResult {
        let mut status_map = self.status.write().await;
        if let Some(status) = status_map.get_mut(&msg.id) {
            status.status = DeliveryState::Delivered;
            status.message_id = Some(send_result.message_id.clone());
            status.updated_at = chrono::Utc::now();
        }

        debug!("Successfully delivered message {}", msg.id);

        DeliveryResult {
            id: msg.id.clone(),
            success: true,
            message_id: Some(send_result.message_id),
            error: None,
            attempts: msg.attempts,
        }
    }

    /// Handle failed delivery.
    async fn handle_failure(
        &self,
        msg: &mut QueuedMessage,
        error: ChannelError,
        retriable: bool,
    ) -> DeliveryResult {
        let should_retry = retriable && msg.attempts < self.config.max_retries;

        if should_retry {
            // Calculate next retry time with exponential backoff
            let delay = self.config.initial_retry_delay.mul_f64(
                self.config.retry_multiplier.powi(msg.attempts as i32 - 1),
            );
            let delay = delay.min(self.config.max_retry_delay);

            msg.next_retry = Some(Instant::now() + delay);
            msg.last_error = Some(error.to_string());

            // Put back in queue
            let mut queue = self.queue.lock().await;
            queue.push_back(msg.clone());

            // Update status
            let mut status_map = self.status.write().await;
            if let Some(status) = status_map.get_mut(&msg.id) {
                status.status = DeliveryState::Failed;
                status.error = Some(error.to_string());
                status.updated_at = chrono::Utc::now();
            }

            warn!(
                "Delivery {} failed (attempt {}), will retry in {:?}: {}",
                msg.id, msg.attempts, delay, error
            );

            DeliveryResult {
                id: msg.id.clone(),
                success: false,
                message_id: None,
                error: Some(error),
                attempts: msg.attempts,
            }
        } else {
            // Permanent failure
            let mut status_map = self.status.write().await;
            if let Some(status) = status_map.get_mut(&msg.id) {
                status.status = DeliveryState::Dropped;
                status.error = Some(error.to_string());
                status.updated_at = chrono::Utc::now();
            }

            error!(
                "Delivery {} permanently failed after {} attempts: {}",
                msg.id, msg.attempts, error
            );

            DeliveryResult {
                id: msg.id.clone(),
                success: false,
                message_id: None,
                error: Some(error),
                attempts: msg.attempts,
            }
        }
    }

    /// Get queue statistics.
    pub async fn stats(&self) -> QueueStats {
        let queue = self.queue.lock().await;
        let status_map = self.status.read().await;

        let mut stats = QueueStats::default();
        stats.pending = queue.len();

        for status in status_map.values() {
            match status.status {
                DeliveryState::Pending => stats.pending += 0, // Already counted
                DeliveryState::InProgress => stats.in_progress += 1,
                DeliveryState::Delivered => stats.delivered += 1,
                DeliveryState::Failed => stats.failed += 1,
                DeliveryState::Dropped => stats.dropped += 1,
                DeliveryState::Cancelled => stats.cancelled += 1,
            }
        }

        stats
    }
}

/// Queue statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStats {
    /// Pending deliveries.
    pub pending: usize,

    /// In-progress deliveries.
    pub in_progress: usize,

    /// Successful deliveries.
    pub delivered: usize,

    /// Failed deliveries (may retry).
    pub failed: usize,

    /// Permanently failed deliveries.
    pub dropped: usize,

    /// Cancelled deliveries.
    pub cancelled: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue_enqueue() {
        let queue = DeliveryQueue::new(DeliveryConfig::default());

        let message = OutboundMessage {
            text: "Hello".to_string(),
            ..Default::default()
        };

        let id = queue.enqueue("channel1".to_string(), message).await.unwrap();
        assert!(!id.is_empty());

        let status = queue.get_status(&id).await.unwrap();
        assert_eq!(status.status, DeliveryState::Pending);
    }

    #[tokio::test]
    async fn test_queue_cancel() {
        let queue = DeliveryQueue::new(DeliveryConfig::default());

        let message = OutboundMessage {
            text: "Hello".to_string(),
            ..Default::default()
        };

        let id = queue.enqueue("channel1".to_string(), message).await.unwrap();
        queue.cancel(&id).await.unwrap();

        let status = queue.get_status(&id).await.unwrap();
        assert_eq!(status.status, DeliveryState::Cancelled);
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = DeliveryQueue::new(DeliveryConfig::default());

        let message = OutboundMessage {
            text: "Hello".to_string(),
            ..Default::default()
        };

        queue.enqueue("channel1".to_string(), message.clone()).await.unwrap();
        queue.enqueue("channel1".to_string(), message).await.unwrap();

        let stats = queue.stats().await;
        assert_eq!(stats.pending, 2);
    }
}
