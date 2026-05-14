//! Talk system events.

use serde::{Deserialize, Serialize};

/// An event from the talk system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TalkEvent {
    /// Voice session has started.
    SessionStarted {
        /// Session ID.
        session_id: String,
        /// User or device that started the session.
        source: TalkSource,
    },

    /// Voice session has ended.
    SessionEnded {
        /// Session ID.
        session_id: String,
        /// Reason the session ended.
        reason: EndReason,
    },

    /// Push-to-talk button pressed.
    PushToTalkPressed {
        /// Session ID.
        session_id: String,
        /// Timestamp (Unix millis).
        timestamp: u64,
    },

    /// Push-to-talk button released.
    PushToTalkReleased {
        /// Session ID.
        session_id: String,
        /// Timestamp (Unix millis).
        timestamp: u64,
        /// Duration the button was held (ms).
        duration_ms: u64,
    },

    /// Voice audio chunk received.
    VoiceChunk {
        /// Session ID.
        session_id: String,
        /// Sequence number for ordering.
        sequence: u32,
        /// Audio data (base64-encoded in JSON).
        #[serde(with = "serde_bytes")]
        audio: Vec<u8>,
        /// Audio format.
        format: AudioFormat,
    },

    /// Wake word detected.
    WakeWordDetected {
        /// Session ID.
        session_id: String,
        /// The wake word that was detected.
        wake_word: String,
        /// Confidence score (0.0 - 1.0).
        confidence: f32,
    },

    /// A transcript (partial or final) was received.
    Transcript {
        /// Session ID.
        session_id: String,
        /// Whether this is a final result.
        is_final: bool,
        /// The transcript text.
        text: String,
        /// Confidence score.
        #[serde(skip_serializing_if = "Option::is_none")]
        confidence: Option<f32>,
    },

    /// TTS audio is ready for playback.
    TtsReady {
        /// Session ID.
        session_id: String,
        /// TTS request ID.
        tts_id: String,
        /// Audio data.
        #[serde(with = "serde_bytes")]
        audio: Vec<u8>,
        /// Audio format.
        format: AudioFormat,
    },

    /// An error occurred in the talk system.
    Error {
        /// Session ID (if applicable).
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        /// Error message.
        message: String,
    },
}

/// Source of a talk session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TalkSource {
    /// Local user initiated the session.
    Local,
    /// A paired mobile device initiated the session.
    Mobile { device_id: String },
    /// A channel (e.g. voice message) initiated the session.
    Channel { channel_type: String, channel_id: String },
    /// An automation/cron job initiated the session.
    Automation { job_id: String },
}

/// Reason a session ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// User explicitly stopped the session.
    UserStopped,
    /// Session timed out due to inactivity.
    Timeout,
    /// An error caused the session to end.
    Error { message: String },
    /// The device disconnected.
    DeviceDisconnected,
}

/// Audio format for talk audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    /// PCM 16-bit signed little-endian.
    PcmS16Le,
    /// Opus encoded.
    Opus,
    /// MP3 encoded.
    Mp3,
    /// WAV container.
    Wav,
}

impl AudioFormat {
    /// Get the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::PcmS16Le => "audio/pcm;rate=16000;format=S16LE",
            Self::Opus => "audio/opus",
            Self::Mp3 => "audio/mpeg",
            Self::Wav => "audio/wav",
        }
    }
}

/// Module for serde_bytes compatibility.
mod serde_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'a, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'a>,
    {
        let s = <&[u8]>::deserialize(deserializer)?;
        Ok(s.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_talk_event_serialization() {
        let event = TalkEvent::PushToTalkPressed {
            session_id: "sess-1".to_string(),
            timestamp: 1234567890,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("push_to_talk_pressed"));
        assert!(json.contains("sess-1"));
    }

    #[test]
    fn test_audio_format_mime_type() {
        assert_eq!(AudioFormat::PcmS16Le.mime_type(), "audio/pcm;rate=16000;format=S16LE");
        assert_eq!(AudioFormat::Opus.mime_type(), "audio/opus");
    }

    #[test]
    fn test_end_reason_serialization() {
        let reason = EndReason::Error {
            message: "Device lost".to_string(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("Device lost"));
    }

    #[test]
    fn test_talk_source_serialization() {
        let source = TalkSource::Mobile {
            device_id: "iphone-123".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("mobile"));
        assert!(json.contains("iphone-123"));
    }
}
