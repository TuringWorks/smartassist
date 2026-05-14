//! Voice session management.

use crate::events::{AudioFormat, EndReason, TalkEvent, TalkSource};
use crate::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};

/// State of a voice session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is idle, waiting for activation.
    Idle,
    /// User has pressed push-to-talk.
    Listening,
    /// Audio is being transcribed.
    Processing,
    /// TTS response is playing.
    Speaking,
    /// Session has ended.
    Ended,
}

/// A voice talk session.
pub struct VoiceSession {
    /// Unique session ID.
    id: String,
    /// Session state.
    state: RwLock<SessionState>,
    /// Who/what started this session.
    source: TalkSource,
    /// Whether push-to-talk is currently pressed.
    ptt_pressed: AtomicBool,
    /// Event channel sender.
    event_tx: mpsc::Sender<TalkEvent>,
    /// Audio format for this session.
    audio_format: AudioFormat,
    /// Whether wake-word detection is enabled.
    wake_word_enabled: AtomicBool,
    /// The wake word to listen for.
    wake_word: RwLock<Option<String>>,
    /// Sequence counter for audio chunks.
    sequence: RwLock<u32>,
    /// Session creation timestamp.
    created_at: std::time::Instant,
}

impl VoiceSession {
    /// Create a new voice session.
    pub fn new(
        id: impl Into<String>,
        source: TalkSource,
        audio_format: AudioFormat,
        event_tx: mpsc::Sender<TalkEvent>,
    ) -> Self {
        let id = id.into();
        debug!("Created voice session {}", id);
        Self {
            id,
            state: RwLock::new(SessionState::Idle),
            source,
            ptt_pressed: AtomicBool::new(false),
            event_tx,
            audio_format,
            wake_word_enabled: AtomicBool::new(false),
            wake_word: RwLock::new(None),
            sequence: RwLock::new(0),
            created_at: std::time::Instant::now(),
        }
    }

    /// Get the session ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the current session state.
    pub async fn state(&self) -> SessionState {
        *self.state.read().await
    }

    /// Get the session source.
    pub fn source(&self) -> &TalkSource {
        &self.source
    }

    /// Get the audio format.
    pub fn audio_format(&self) -> AudioFormat {
        self.audio_format
    }

    /// Start the session (transition from Idle to Listening or just activate).
    pub async fn start(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if *state == SessionState::Ended {
            return Err(crate::TalkError::audio_pipeline("Session already ended"));
        }
        *state = SessionState::Listening;
        info!("Voice session {} started", self.id);
        let _ = self.event_tx.send(TalkEvent::SessionStarted {
            session_id: self.id.clone(),
            source: self.source.clone(),
        }).await;
        Ok(())
    }

    /// End the session.
    pub async fn end(&self, reason: EndReason) -> Result<()> {
        let mut state = self.state.write().await;
        *state = SessionState::Ended;
        info!("Voice session {} ended: {:?}", self.id, reason);
        let _ = self.event_tx.send(TalkEvent::SessionEnded {
            session_id: self.id.clone(),
            reason,
        }).await;
        Ok(())
    }

    /// Press push-to-talk.
    pub async fn ptt_press(&self) -> Result<()> {
        self.ptt_pressed.store(true, Ordering::Relaxed);
        let mut state = self.state.write().await;
        *state = SessionState::Listening;
        let _ = self.event_tx.send(TalkEvent::PushToTalkPressed {
            session_id: self.id.clone(),
            timestamp: now_millis(),
        }).await;
        Ok(())
    }

    /// Release push-to-talk.
    pub async fn ptt_release(&self, duration_ms: u64) -> Result<()> {
        self.ptt_pressed.store(false, Ordering::Relaxed);
        let mut state = self.state.write().await;
        *state = SessionState::Processing;
        let _ = self.event_tx.send(TalkEvent::PushToTalkReleased {
            session_id: self.id.clone(),
            timestamp: now_millis(),
            duration_ms,
        }).await;
        Ok(())
    }

    /// Send an audio chunk through the session.
    pub async fn send_audio(&self,
        audio: Vec<u8>,
    ) -> Result<()> {
        let seq = {
            let mut s = self.sequence.write().await;
            *s += 1;
            *s
        };
        let _ = self.event_tx.send(TalkEvent::VoiceChunk {
            session_id: self.id.clone(),
            sequence: seq,
            audio,
            format: self.audio_format,
        }).await;
        Ok(())
    }

    /// Send a transcript event.
    pub async fn send_transcript(
        &self,
        is_final: bool,
        text: String,
        confidence: Option<f32>,
    ) -> Result<()> {
        if is_final {
            let mut state = self.state.write().await;
            if *state == SessionState::Processing {
                *state = SessionState::Idle;
            }
        }
        let _ = self.event_tx.send(TalkEvent::Transcript {
            session_id: self.id.clone(),
            is_final,
            text,
            confidence,
        }).await;
        Ok(())
    }

    /// Enable wake-word detection.
    pub async fn enable_wake_word(&self,
        wake_word: impl Into<String>,
    ) -> Result<()> {
        let mut w = self.wake_word.write().await;
        *w = Some(wake_word.into());
        self.wake_word_enabled.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Disable wake-word detection.
    pub fn disable_wake_word(&self,
    ) {
        self.wake_word_enabled.store(false, Ordering::Relaxed);
    }

    /// Check if wake-word detection is enabled.
    pub fn is_wake_word_enabled(&self) -> bool {
        self.wake_word_enabled.load(Ordering::Relaxed)
    }

    /// Get the current wake word.
    pub async fn wake_word(&self) -> Option<String> {
        self.wake_word.read().await.clone()
    }

    /// Check if push-to-talk is currently pressed.
    pub fn is_ptt_pressed(&self) -> bool {
        self.ptt_pressed.load(Ordering::Relaxed)
    }

    /// Get session duration.
    pub fn duration(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session() -> (VoiceSession, mpsc::Receiver<TalkEvent>) {
        let (tx, rx) = mpsc::channel(10);
        let session = VoiceSession::new(
            "test-sess",
            TalkSource::Local,
            AudioFormat::PcmS16Le,
            tx,
        );
        (session, rx)
    }

    #[tokio::test]
    async fn test_session_creation() {
        let (session, _rx) = test_session();
        assert_eq!(session.id(), "test-sess");
        assert_eq!(session.state().await, SessionState::Idle);
        assert!(!session.is_ptt_pressed());
    }

    #[tokio::test]
    async fn test_session_start_end() {
        let (session, mut rx) = test_session();
        session.start().await.unwrap();
        assert_eq!(session.state().await, SessionState::Listening);

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::SessionStarted { .. }));

        session.end(EndReason::UserStopped).await.unwrap();
        assert_eq!(session.state().await, SessionState::Ended);

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::SessionEnded { .. }));
    }

    #[tokio::test]
    async fn test_ptt_press_release() {
        let (session, mut rx) = test_session();
        session.start().await.unwrap();
        let _ = rx.recv().await; // SessionStarted

        session.ptt_press().await.unwrap();
        assert!(session.is_ptt_pressed());
        assert_eq!(session.state().await, SessionState::Listening);
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::PushToTalkPressed { .. }));

        session.ptt_release(1500).await.unwrap();
        assert!(!session.is_ptt_pressed());
        assert_eq!(session.state().await, SessionState::Processing);
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, TalkEvent::PushToTalkReleased { .. }));
    }

    #[tokio::test]
    async fn test_wake_word() {
        let (session, _rx) = test_session();
        assert!(!session.is_wake_word_enabled());

        session.enable_wake_word("Hey SmartAssist").await.unwrap();
        assert!(session.is_wake_word_enabled());
        assert_eq!(session.wake_word().await, Some("Hey SmartAssist".to_string()));

        session.disable_wake_word();
        assert!(!session.is_wake_word_enabled());
    }

    #[tokio::test]
    async fn test_send_audio() {
        let (session, mut rx) = test_session();
        session.send_audio(vec![1, 2, 3]).await.unwrap();

        let event = rx.recv().await.unwrap();
        match event {
            TalkEvent::VoiceChunk { session_id, sequence, audio, .. } => {
                assert_eq!(session_id, "test-sess");
                assert_eq!(sequence, 1);
                assert_eq!(audio, vec![1, 2, 3]);
            }
            _ => panic!("Expected VoiceChunk event"),
        }
    }

    #[tokio::test]
    async fn test_send_transcript() {
        let (session, mut rx) = test_session();
        session.start().await.unwrap();
        let _ = rx.recv().await;
        session.ptt_release(1000).await.unwrap();
        let _ = rx.recv().await;

        session.send_transcript(true, "Hello world".to_string(), Some(0.95)).await.unwrap();

        let event = rx.recv().await.unwrap();
        match event {
            TalkEvent::Transcript { is_final, text, confidence, .. } => {
                assert!(is_final);
                assert_eq!(text, "Hello world");
                assert_eq!(confidence, Some(0.95));
            }
            _ => panic!("Expected Transcript event"),
        }
    }
}
