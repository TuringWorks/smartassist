//! SmartAssist Talk / Voice System.
//!
//! Provides a voice interaction runtime with:
//! - [`VoiceSession`] — Individual voice session state machine
//! - [`TalkRuntime`] — Manages multiple sessions and routes events
//! - [`TalkEvent`] — Events: PTT press/release, audio chunks, transcripts, TTS
//! - [`TalkSource`] — Where the session originated (local, mobile, channel, automation)
//!
//! # Example
//!
//! ```rust,no_run
//! use smartassist_talk::{TalkRuntimeBuilder, TalkSource, AudioFormat};
//!
//! #[tokio::main]
//! async fn main() {
//!     let (runtime, mut events) = TalkRuntimeBuilder::default()
//!         .default_format(AudioFormat::PcmS16Le)
//!         .build();
//!
//!     let session = runtime.spawn_session(TalkSource::Local).await.unwrap();
//!     session.start().await.unwrap();
//!
//!     // In a real app, audio would come from a microphone capture thread
//!     // and events would be consumed by the UI or agent loop.
//! }
//! ```

pub mod error;
pub mod events;
pub mod runtime;
pub mod session;

pub use error::{TalkError, Result};
pub use events::{AudioFormat, EndReason, TalkEvent, TalkSource};
pub use runtime::{TalkRuntime, TalkRuntimeBuilder};
pub use session::{SessionState, VoiceSession};
