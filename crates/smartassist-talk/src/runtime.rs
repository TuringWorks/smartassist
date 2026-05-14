//! Talk runtime - manages voice sessions and the audio pipeline.

use crate::error::TalkError;
use crate::events::{AudioFormat, EndReason, TalkEvent, TalkSource};
use crate::session::VoiceSession;
use crate::Result;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// The talk runtime manages active voice sessions and routes events.
pub struct TalkRuntime {
    /// Active sessions by ID.
    sessions: DashMap<String, Arc<VoiceSession>>,
    /// Global event broadcaster.
    event_tx: mpsc::Sender<TalkEvent>,
    /// Default audio format for new sessions.
    default_format: AudioFormat,
    /// Whether push-to-talk is the default mode (vs. wake-word).
    ptt_default: bool,
}

impl TalkRuntime {
    /// Create a new talk runtime.
    pub fn new(
        event_tx: mpsc::Sender<TalkEvent>,
        default_format: AudioFormat,
    ) -> Self {
        Self {
            sessions: DashMap::new(),
            event_tx,
            default_format,
            ptt_default: true,
        }
    }

    /// Set whether push-to-talk is the default mode.
    pub fn with_ptt_default(mut self, ptt_default: bool) -> Self {
        self.ptt_default = ptt_default;
        self
    }

    /// Spawn a new voice session.
    pub async fn spawn_session(
        &self,
        source: TalkSource,
    ) -> Result<Arc<VoiceSession>> {
        let id = format!("talk-{}", uuid::Uuid::new_v4());
        let (session_event_tx, mut session_event_rx) = mpsc::channel(256);

        let session = Arc::new(VoiceSession::new(
            id.clone(),
            source,
            self.default_format,
            session_event_tx,
        ));

        // Forward session events to the global event channel.
        let global_tx = self.event_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = session_event_rx.recv().await {
                let _ = global_tx.send(event).await;
            }
        });

        self.sessions.insert(id.clone(), session.clone());
        info!("Spawned voice session {}", id);
        Ok(session)
    }

    /// Get an active session by ID.
    pub fn get_session(&self,
        id: &str,
    ) -> Option<Arc<VoiceSession>> {
        self.sessions.get(id).map(|s| s.clone())
    }

    /// End and remove a session.
    pub async fn end_session(
        &self,
        id: &str,
        reason: EndReason,
    ) -> Result<()> {
        if let Some((_, session)) = self.sessions.remove(id) {
            session.end(reason).await?;
            info!("Removed voice session {}", id);
        } else {
            return Err(TalkError::session_not_found(id));
        }
        Ok(())
    }

    /// List all active session IDs.
    pub fn list_sessions(&self) -> Vec<String> {
        self.sessions.iter().map(|e| e.key().clone()).collect()
    }

    /// End all sessions.
    pub async fn end_all_sessions(&self,
        reason: EndReason,
    ) {
        let ids: Vec<String> = self.list_sessions();
        for id in ids {
            if let Err(e) = self.end_session(&id, reason.clone()).await {
                warn!("Failed to end session {}: {}", id, e);
            }
        }
    }

    /// Handle a push-to-talk press for a session.
    pub async fn handle_ptt_press(&self,
        session_id: &str,
    ) -> Result<()> {
        let session = self.get_session(session_id)
            .ok_or_else(|| TalkError::session_not_found(session_id))?;
        session.ptt_press().await
    }

    /// Handle a push-to-talk release for a session.
    pub async fn handle_ptt_release(
        &self,
        session_id: &str,
        duration_ms: u64,
    ) -> Result<()> {
        let session = self.get_session(session_id)
            .ok_or_else(|| TalkError::session_not_found(session_id))?;
        session.ptt_release(duration_ms).await
    }

    /// Handle incoming audio for a session.
    pub async fn handle_audio(
        &self,
        session_id: &str,
        audio: Vec<u8>,
    ) -> Result<()> {
        let session = self.get_session(session_id)
            .ok_or_else(|| TalkError::session_not_found(session_id))?;
        session.send_audio(audio).await
    }

    /// Handle a transcript result for a session.
    pub async fn handle_transcript(
        &self,
        session_id: &str,
        is_final: bool,
        text: String,
        confidence: Option<f32>,
    ) -> Result<()> {
        let session = self.get_session(session_id)
            .ok_or_else(|| TalkError::session_not_found(session_id))?;
        session.send_transcript(is_final, text, confidence).await
    }

    /// Get the number of active sessions.
    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }

    /// Clean up ended sessions.
    pub async fn cleanup(&self,
    ) {
        let to_remove: Vec<String> = self
            .sessions
            .iter()
            .filter(|_e| {
                let rt = tokio::runtime::Handle::try_current();
                match rt {
                    Ok(_) => {
                        // Can't easily check state in a blocking way here.
                        // Instead, sessions should be removed when end() is called.
                        false
                    }
                    Err(_) => false,
                }
            })
            .map(|e| e.key().clone())
            .collect();

        for id in to_remove {
            self.sessions.remove(&id);
        }
    }
}

/// Builder for configuring the talk runtime.
pub struct TalkRuntimeBuilder {
    default_format: AudioFormat,
    ptt_default: bool,
    buffer_capacity: usize,
}

impl Default for TalkRuntimeBuilder {
    fn default() -> Self {
        Self {
            default_format: AudioFormat::PcmS16Le,
            ptt_default: true,
            buffer_capacity: 256,
        }
    }
}

impl TalkRuntimeBuilder {
    /// Set the default audio format.
    pub fn default_format(mut self, format: AudioFormat) -> Self {
        self.default_format = format;
        self
    }

    /// Set whether PTT is the default mode.
    pub fn ptt_default(mut self, ptt_default: bool) -> Self {
        self.ptt_default = ptt_default;
        self
    }

    /// Set the event buffer capacity.
    pub fn buffer_capacity(mut self, capacity: usize) -> Self {
        self.buffer_capacity = capacity;
        self
    }

    /// Build the talk runtime.
    pub fn build(self) -> (TalkRuntime, mpsc::Receiver<TalkEvent>) {
        let (event_tx, event_rx) = mpsc::channel(self.buffer_capacity);
        let runtime = TalkRuntime::new(event_tx, self.default_format)
            .with_ptt_default(self.ptt_default);
        (runtime, event_rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_runtime() -> (TalkRuntime, mpsc::Receiver<TalkEvent>) {
        TalkRuntimeBuilder::default().build()
    }

    #[tokio::test]
    async fn test_spawn_session() {
        let (runtime, _rx) = test_runtime();
        let session = runtime.spawn_session(TalkSource::Local).await.unwrap();
        assert!(!session.id().is_empty());
        assert_eq!(runtime.active_count(), 1);
    }

    #[tokio::test]
    async fn test_get_session() {
        let (runtime, _rx) = test_runtime();
        let session = runtime.spawn_session(TalkSource::Local).await.unwrap();
        let id = session.id().to_string();

        let retrieved = runtime.get_session(&id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), id);

        assert!(runtime.get_session("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_end_session() {
        let (runtime, mut rx) = test_runtime();
        let session = runtime.spawn_session(TalkSource::Local).await.unwrap();
        let id = session.id().to_string();

        // Consume the SessionStarted event
        let _ = rx.recv().await;

        runtime.end_session(&id, EndReason::UserStopped).await.unwrap();
        assert_eq!(runtime.active_count(), 0);
        assert!(runtime.get_session(&id).is_none());

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::SessionEnded { .. }));
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let (runtime, _rx) = test_runtime();
        let s1 = runtime.spawn_session(TalkSource::Local).await.unwrap();
        let s2 = runtime.spawn_session(TalkSource::Mobile { device_id: "d1".to_string() }).await.unwrap();

        let ids = runtime.list_sessions();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&s1.id().to_string()));
        assert!(ids.contains(&s2.id().to_string()));
    }

    #[tokio::test]
    async fn test_handle_ptt() {
        let (runtime, mut rx) = test_runtime();
        let session = runtime.spawn_session(TalkSource::Local).await.unwrap();
        let id = session.id().to_string();

        let _ = rx.recv().await; // SessionStarted

        runtime.handle_ptt_press(&id).await.unwrap();
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::PushToTalkPressed { .. }));
        assert!(session.is_ptt_pressed());

        runtime.handle_ptt_release(&id, 500).await.unwrap();
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::PushToTalkReleased { .. }));
        assert!(!session.is_ptt_pressed());
    }

    #[tokio::test]
    async fn test_handle_audio() {
        let (runtime, mut rx) = test_runtime();
        let session = runtime.spawn_session(TalkSource::Local).await.unwrap();
        let id = session.id().to_string();

        let _ = rx.recv().await; // SessionStarted

        runtime.handle_audio(&id, vec![1, 2, 3, 4]).await.unwrap();
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::VoiceChunk { .. }));
    }

    #[tokio::test]
    async fn test_handle_transcript() {
        let (runtime, mut rx) = test_runtime();
        let session = runtime.spawn_session(TalkSource::Local).await.unwrap();
        let id = session.id().to_string();

        let _ = rx.recv().await; // SessionStarted
        session.start().await.unwrap();
        session.ptt_release(100).await.unwrap();
        let _ = rx.recv().await; // PushToTalkReleased

        runtime.handle_transcript(&id, true, "Hello".to_string(), Some(0.9)
        ).await.unwrap();

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::Transcript { text, .. } if text == "Hello"));
    }

    #[tokio::test]
    async fn test_end_all_sessions() {
        let (runtime, _rx) = test_runtime();
        runtime.spawn_session(TalkSource::Local).await.unwrap();
        runtime.spawn_session(TalkSource::Local).await.unwrap();
        assert_eq!(runtime.active_count(), 2);

        runtime.end_all_sessions(EndReason::Timeout).await;
        assert_eq!(runtime.active_count(), 0);
    }

    #[tokio::test]
    async fn test_session_not_found() {
        let (runtime, _rx) = test_runtime();
        let result = runtime.handle_ptt_press("nonexistent").await;
        assert!(result.is_err());
    }
}
